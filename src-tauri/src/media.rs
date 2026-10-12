//! Local media operations. Processes are invoked directly, never through a shell.
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    /// Zero means that the file did not report a usable frame rate.
    pub fps: f64,
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn binary_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

fn from_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .find_map(|directory| existing_file(directory.join(binary_name(name))))
    })
}

/// Small, deterministic search; never traverses disks or the user's video folders.
fn runtime_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.to_path_buf());
            roots.push(parent.join("resources"));
        }
    }
    if let Ok(directory) = env::current_dir() {
        // Covers `npm run tauri dev` launched from the project or src-tauri.
        roots.extend(directory.ancestors().take(3).map(Path::to_path_buf));
    }
    roots
}

pub fn find_ffmpeg(setting: &str) -> Option<PathBuf> {
    let configured = setting.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        return existing_file(if path.is_dir() {
            path.join(binary_name("ffmpeg"))
        } else {
            path
        });
    }
    if let Some(path) = env::var_os("FFMPEG_PATH").and_then(|path| existing_file(path.into())) {
        return Some(path);
    }
    if let Some(path) = from_path("ffmpeg") {
        return Some(path);
    }
    for root in runtime_roots() {
        for candidate in [
            root.join(binary_name("ffmpeg")),
            root.join("bin").join(binary_name("ffmpeg")),
            root.join("tools").join(binary_name("ffmpeg")),
            root.join("node_modules/ffmpeg-static")
                .join(binary_name("ffmpeg")),
        ] {
            if let Some(path) = existing_file(candidate) {
                return Some(path);
            }
        }
    }
    if cfg!(windows) {
        for directory in ["C:/ffmpeg/bin", "C:/Program Files/ffmpeg/bin"] {
            if let Some(path) = existing_file(Path::new(directory).join("ffmpeg.exe")) {
                return Some(path);
            }
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            if let Some(path) = existing_file(
                PathBuf::from(profile).join("scoop/apps/ffmpeg/current/bin/ffmpeg.exe"),
            ) {
                return Some(path);
            }
        }
    }
    None
}

fn find_ffprobe(ffmpeg: &Path) -> Option<PathBuf> {
    if let Some(parent) = ffmpeg.parent() {
        if let Some(path) = existing_file(parent.join(binary_name("ffprobe"))) {
            return Some(path);
        }
    }
    if let Some(path) = env::var_os("FFPROBE_PATH").and_then(|path| existing_file(path.into())) {
        return Some(path);
    }
    if let Some(path) = from_path("ffprobe") {
        return Some(path);
    }
    let platform = if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let architecture = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "x86") {
        "ia32"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        env::consts::ARCH
    };
    let mut roots = runtime_roots();
    if let Some(parent) = ffmpeg.parent() {
        roots.extend(parent.ancestors().take(3).map(Path::to_path_buf));
    }
    for root in roots {
        for candidate in [
            root.join(binary_name("ffprobe")),
            root.join("bin").join(binary_name("ffprobe")),
            root.join("tools").join(binary_name("ffprobe")),
            root.join("node_modules/ffprobe-static/bin")
                .join(platform)
                .join(architecture)
                .join(binary_name("ffprobe")),
            root.join("ffprobe-static/bin")
                .join(platform)
                .join(architecture)
                .join(binary_name("ffprobe")),
        ] {
            if let Some(path) = existing_file(candidate) {
                return Some(path);
            }
        }
    }
    None
}

fn require_ffmpeg(setting: &str) -> Result<PathBuf, String> {
    find_ffmpeg(setting).ok_or_else(|| {
        if setting.trim().is_empty() {
            "未找到 FFmpeg。请安装包含 ffmpeg 和 ffprobe 的工具包，或在设置中指定 ffmpeg.exe。"
                .to_owned()
        } else {
            "设置中的 FFmpeg 路径不存在；请选择有效的 ffmpeg.exe 或其所在目录。".to_owned()
        }
    })
}

fn validate_input(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("原始视频不存在或无法访问：{}", path.display()));
    }
    Ok(())
}

fn hidden_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

fn run_command(command: &mut Command, purpose: &str) -> Result<Output, String> {
    let output = command
        .output()
        .map_err(|error| format!("{purpose}无法启动：{error}"))?;
    if output.status.success() {
        return Ok(output);
    }
    let raw = String::from_utf8_lossy(&output.stderr);
    let detail = raw.trim();
    // Keep useful diagnostics without flooding a dialog with process output.
    let detail: String = detail.chars().rev().take(1200).collect::<String>().chars().rev().collect();
    let explanation = if detail.contains("Permission denied") || detail.contains("Access is denied") {
        "没有访问文件的权限，请选择可写目录。"
    } else if detail.contains("No such file") {
        "文件或目录已不存在，请重新扫描录像库。"
    } else if detail.contains("Unknown encoder") {
        "当前 FFmpeg 缺少所需编码器，请安装支持 H.264 / AAC 的完整版本。"
    } else if detail.contains("Invalid data") {
        "视频格式损坏或不受支持。"
    } else {
        "处理失败。快速裁剪遇到格式不兼容时，请改用精确裁剪。"
    };
    Err(format!("{purpose}：{explanation}\n{detail}"))
}

fn numeric(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(number) => number.parse().ok(),
        _ => None,
    }?;
    number.is_finite().then_some(number)
}

fn rate(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    let number = if let Some(value) = value.as_str() {
        if let Some((numerator, denominator)) = value.split_once('/') {
            let numerator = numerator.parse::<f64>().ok()?;
            let denominator = denominator.parse::<f64>().ok()?;
            numerator / denominator
        } else {
            value.parse::<f64>().ok()?
        }
    } else {
        value.as_f64()?
    };
    (number.is_finite() && number > 0.0).then_some(number)
}

fn parse_probe(bytes: &[u8]) -> Result<MediaInfo, String> {
    let data: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("无法解析 ffprobe 返回的视频信息：{error}"))?;
    let streams = data["streams"]
        .as_array()
        .ok_or_else(|| "视频中没有可用的媒体流。".to_owned())?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"].as_str() == Some("video"))
        .ok_or_else(|| "文件不包含视频流。".to_owned())?;
    let duration = numeric(data.get("format").and_then(|format| format.get("duration")))
        .filter(|duration| *duration > 0.0)
        .or_else(|| numeric(video.get("duration")).filter(|duration| *duration > 0.0))
        .ok_or_else(|| "视频没有有效时长，无法安全裁剪。".to_owned())?;
    let dimension = |key| {
        video[key]
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .filter(|number| *number > 0)
            .ok_or_else(|| "视频没有有效的分辨率。".to_owned())
    };
    Ok(MediaInfo {
        duration,
        width: dimension("width")?,
        height: dimension("height")?,
        fps: rate(video.get("avg_frame_rate"))
            .or_else(|| rate(video.get("r_frame_rate")))
            .unwrap_or(0.0),
    })
}

pub fn probe(path: &Path, setting: &str) -> Result<MediaInfo, String> {
    validate_input(path)?;
    let ffmpeg = require_ffmpeg(setting)?;
    let ffprobe = find_ffprobe(&ffmpeg).ok_or_else(|| {
        "未找到 ffprobe。请将 ffprobe.exe 放在 ffmpeg.exe 同一目录，或将它加入 PATH。"
            .to_owned()
    })?;
    let mut command = hidden_command(&ffprobe);
    command.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration:stream=codec_type,width,height,avg_frame_rate,r_frame_rate,duration",
        "-of",
        "json",
    ]);
    command.arg(path);
    parse_probe(&run_command(&mut command, "读取视频信息")?.stdout)
}

fn validate_range(start: f64, end: f64, duration: f64) -> Result<(f64, f64), String> {
    if !start.is_finite() || !end.is_finite() || !duration.is_finite() || duration <= 0.0 {
        return Err("裁剪时间必须是有效数字，视频必须有有效时长。".to_owned());
    }
    if start < 0.0 || end <= start || start >= duration {
        return Err("裁剪起点必须大于或等于 0，终点必须晚于起点，且起点应在视频范围内。".to_owned());
    }
    // Allows only tiny rounding differences from the frontend's time display.
    if end > duration + 0.05 {
        return Err(format!("裁剪终点超过视频时长（{duration:.3} 秒）。"));
    }
    Ok((start, end.min(duration)))
}

fn timestamp(value: f64) -> String {
    format!("{value:.6}")
}

fn export_arguments(
    input: &Path,
    start: f64,
    end: f64,
    output: &Path,
    precise: bool,
) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-ss"]
        .iter()
        .map(OsString::from)
        .collect();
    arguments.push(timestamp(start).into());
    arguments.push("-i".into());
    arguments.push(input.as_os_str().to_owned());
    arguments.push("-t".into());
    arguments.push(timestamp(end - start).into());
    arguments.extend(["-map", "0:v:0", "-map", "0:a:0?"].iter().map(OsString::from));
    if precise {
        arguments.extend(
            [
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
                "-vf",
                "scale=trunc(iw/2)*2:trunc(ih/2)*2",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
            ]
            .iter()
            .map(OsString::from),
        );
    } else {
        arguments.extend(["-c", "copy"].iter().map(OsString::from));
    }
    arguments.extend(
        ["-avoid_negative_ts", "make_zero", "-movflags", "+faststart", "-f", "mp4"]
            .iter()
            .map(OsString::from),
    );
    arguments.push(output.as_os_str().to_owned());
    arguments
}

/// A private directory next to the destination guarantees same-volume publication.
/// Its name is generated by the application; it never contains source recordings.
struct StagedOutput {
    directory: PathBuf,
    file: PathBuf,
}

impl StagedOutput {
    fn create(destination: &Path) -> Result<Self, String> {
        if destination.exists() {
            return Err("目标文件已存在，请选择新的文件名；不会覆盖已有文件。".to_owned());
        }
        let parent = destination.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|error| format!("无法创建输出目录：{error}"))?;
        let filename = destination.file_name().ok_or_else(|| "输出文件名无效。".to_owned())?;
        let clock = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        for _ in 0..32 {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let directory = parent.join(format!(".wegame-media-{}-{clock}-{serial}", std::process::id()));
            match fs::create_dir(&directory) {
                Ok(()) => return Ok(Self { file: directory.join(filename), directory }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("无法创建临时输出目录：{error}")),
            }
        }
        Err("无法生成唯一临时输出目录，请重试。".to_owned())
    }

    fn publish(&self, destination: &Path) -> Result<(), String> {
        let size = fs::metadata(&self.file)
            .map_err(|_| "没有生成输出文件，请确认截取时间在视频范围内。".to_owned())?
            .len();
        if size == 0 {
            return Err("输出文件为空，视频可能已损坏或截取时间无效。".to_owned());
        }
        publish_without_replace(&self.file, destination)
            .map_err(|error| format!("无法保存输出文件（目标已存在或目录不可写）：{error}"))
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        // Delete only these exact, application-created paths, never recursively.
        let _ = fs::remove_file(&self.file);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(windows)]
fn publish_without_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let wide = |path: &Path| -> std::io::Result<Vec<u16>> {
        let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "文件路径包含空字符"));
        }
        value.push(0);
        Ok(value)
    };
    let source = wide(source)?;
    let destination = wide(destination)?;
    // Flags = 0 deliberately omits MOVEFILE_REPLACE_EXISTING. There is no
    // check-then-rename race that could overwrite another export's destination.
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn publish_without_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    // A hard link atomically creates the destination without replacing it.
    // Both files reside in the same parent filesystem. Drop removes the staging link.
    fs::hard_link(source, destination)
}

pub fn thumbnail(input: &Path, time: f64, output: &Path, setting: &str) -> Result<(), String> {
    validate_input(input)?;
    if !time.is_finite() || time < 0.0 {
        return Err("缩略图时间必须是大于或等于 0 的有效数字。".to_owned());
    }
    let ffmpeg = require_ffmpeg(setting)?;
    let staged = StagedOutput::create(output)?;
    let mut command = hidden_command(&ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-ss"]);
    command.arg(timestamp(time));
    command.arg("-i").arg(input);
    command.args(["-frames:v", "1", "-vf", "scale=480:-2", "-q:v", "3"]);
    command.arg(&staged.file);
    run_command(&mut command, "生成缩略图")?;
    staged.publish(output)
}

pub fn export_clip(
    input: &Path,
    start: f64,
    end: f64,
    output: &Path,
    precise: bool,
    setting: &str,
) -> Result<(), String> {
    if !output
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
    {
        return Err("导出文件必须使用 .mp4 扩展名。".to_owned());
    }
    let info = probe(input, setting)?;
    let (start, end) = validate_range(start, end, info.duration)?;
    let ffmpeg = require_ffmpeg(setting)?;
    let staged = StagedOutput::create(output)?;
    let mut command = hidden_command(&ffmpeg);
    command.args(export_arguments(input, start, end, &staged.file, precise));
    run_command(&mut command, "导出 MP4")?;
    staged.publish(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_video_stream_duration_and_fractional_fps() {
        let data = br#"{"streams":[{"codec_type":"audio"},{"codec_type":"video","width":1920,"height":1080,"avg_frame_rate":"60000/1001","duration":"18.25"}],"format":{"duration":"18.3"}}"#;
        let info = parse_probe(data).unwrap();
        assert_eq!((info.width, info.height, info.duration), (1920, 1080, 18.3));
        assert!((info.fps - 59.94005994).abs() < 0.000_001);
    }

    #[test]
    fn falls_back_to_stream_duration_and_real_frame_rate() {
        let data = br#"{"streams":[{"codec_type":"video","width":1280,"height":720,"avg_frame_rate":"0/0","r_frame_rate":"30/1","duration":"10.5"}],"format":{"duration":"N/A"}}"#;
        assert_eq!(parse_probe(data).unwrap(), MediaInfo { duration: 10.5, width: 1280, height: 720, fps: 30.0 });
    }

    #[test]
    fn rejects_audio_only_and_invalid_duration() {
        assert!(parse_probe(br#"{"streams":[{"codec_type":"audio"}],"format":{"duration":"10"}}"#).is_err());
        for duration in ["NaN", "inf", "-10", "0", "N/A"] {
            let data = format!(r#"{{"streams":[{{"codec_type":"video","width":1920,"height":1080}}],"format":{{"duration":"{duration}"}}}}"#);
            assert!(parse_probe(data.as_bytes()).is_err(), "{duration}");
        }
    }

    #[test]
    fn bounds_clip_range_and_only_tolerates_display_rounding() {
        assert_eq!(validate_range(2.0, 10.01, 10.0).unwrap(), (2.0, 10.0));
        assert_eq!(validate_range(0.0, 10.0, 10.0).unwrap(), (0.0, 10.0));
        for (start, end, duration) in [
            (-1.0, 3.0, 10.0),
            (3.0, 3.0, 10.0),
            (4.0, 3.0, 10.0),
            (10.0, 11.0, 10.0),
            (0.0, 11.0, 10.0),
            (f64::NAN, 2.0, 10.0),
            (0.0, f64::INFINITY, 10.0),
            (0.0, 2.0, 0.0),
        ] {
            assert!(validate_range(start, end, duration).is_err());
        }
    }

    #[test]
    fn commands_keep_paths_as_single_arguments_and_select_codecs() {
        let input = Path::new("账户录像/round & $(command).mp4");
        let output = Path::new("导出目录/highlight.mp4");
        let precise = export_arguments(input, 1.25, 4.75, output, true);
        assert!(precise.contains(&input.as_os_str().to_owned()));
        assert!(precise.contains(&"3.500000".into()));
        assert!(precise.contains(&"libx264".into()));
        assert!(precise.contains(&"aac".into()));
        assert_eq!(precise.last(), Some(&output.as_os_str().to_owned()));
        let fast = export_arguments(input, 0.0, 2.0, output, false);
        assert!(fast.contains(&"copy".into()));
        assert!(!fast.contains(&"libx264".into()));
        assert!(fast.windows(2).any(|arguments| arguments == [OsString::from("-f"), OsString::from("mp4")]));
    }

    #[test]
    fn publication_does_not_overwrite_and_cleans_staging() {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let directory = env::temp_dir().join(format!("wegame-media-test-{}-{serial}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let destination = directory.join("clip.mp4");
        let staged = StagedOutput::create(&destination).unwrap();
        fs::write(&staged.file, b"new export").unwrap();
        // Simulates another export creating the destination after our initial check.
        fs::write(&destination, b"existing recording").unwrap();
        assert!(staged.publish(&destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"existing recording");
        let staging_directory = staged.directory.clone();
        drop(staged);
        assert!(!staging_directory.exists());
        fs::remove_file(&destination).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn successful_publication_is_nonempty_and_removes_staging() {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let directory = env::temp_dir().join(format!("wegame-media-success-{}-{serial}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let destination = directory.join("clip.mp4");
        let staged = StagedOutput::create(&destination).unwrap();
        assert!(staged.publish(&destination).is_err());
        fs::write(&staged.file, b"complete export").unwrap();
        staged.publish(&destination).unwrap();
        drop(staged);
        assert_eq!(fs::read(&destination).unwrap(), b"complete export");
        fs::remove_file(&destination).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    #[ignore = "Requires local FFmpeg and ffprobe binaries"]
    fn ffmpeg_smoke() {
        let ffmpeg = find_ffmpeg("").expect("Install ffmpeg-static and ffprobe-static first");
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let clock = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let directory = env::temp_dir().join(format!("wegame-ffmpeg-smoke-{}-{clock}-{serial}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let input = directory.join("账户 & fixture.mp4");
        let image = directory.join("frame.jpg");
        let precise = directory.join("precise.mp4");
        let fast = directory.join("fast.mp4");
        let mut generator = hidden_command(&ffmpeg);
        generator.args([
            "-hide_banner", "-loglevel", "error", "-nostdin", "-n",
            "-f", "lavfi", "-i", "testsrc=size=160x90:rate=30",
            "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
            "-t", "3", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac",
        ]);
        generator.arg(&input);
        run_command(&mut generator, "生成媒体测试片段").unwrap();
        let original_size = fs::metadata(&input).unwrap().len();
        let info = probe(&input, "").unwrap();
        assert!((info.duration - 3.0).abs() < 0.15);
        assert_eq!((info.width, info.height, info.fps), (160, 90, 30.0));
        thumbnail(&input, 1.0, &image, "").unwrap();
        assert!(fs::metadata(&image).unwrap().len() > 100);
        export_clip(&input, 0.4, 1.4, &precise, true, "").unwrap();
        let clipped = probe(&precise, "").unwrap();
        assert!((clipped.duration - 1.0).abs() < 0.15, "{}", clipped.duration);
        assert_eq!((clipped.width, clipped.height), (160, 90));
        export_clip(&input, 1.0, 2.0, &fast, false, "").unwrap();
        assert!(probe(&fast, "").unwrap().duration > 0.0);
        let saved_size = fs::metadata(&precise).unwrap().len();
        assert!(export_clip(&input, 0.0, 1.0, &precise, true, "").is_err());
        assert_eq!(fs::metadata(&precise).unwrap().len(), saved_size);
        assert!(export_clip(&input, 0.0, 30.0, &directory.join("invalid.mp4"), true, "").is_err());
        assert!(!directory.join("invalid.mp4").exists());
        assert_eq!(fs::metadata(&input).unwrap().len(), original_size);
        for path in [input, image, precise, fast] {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }
}
