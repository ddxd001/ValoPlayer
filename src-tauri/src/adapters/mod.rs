//! Read-only metadata adapters. ACLOS is encrypted JSON, not SQLite.
//! Format facts were checked against WonderfulUI and local ACLOS files;
//! this implementation intentionally rejects unknown layouts.

use crate::models::MomentEvent;
use aes::{Aes128, Aes256};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use chrono::NaiveDateTime;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeSet, HashMap}, fs, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}, time::UNIX_EPOCH};

const MAX_METADATA_BYTES: u64 = 128 * 1024 * 1024;
const EXCHANGE_SCHEMA: &str = "wegame-moments";

#[derive(Debug, Clone)]
pub struct Metadata {
    pub title: Option<String>,
    pub account: Option<String>,
    pub duration: Option<f64>,
    pub events: Vec<MomentEvent>,
    pub status: String,
    pub diagnostics: Vec<String>,
}

impl Default for Metadata {
    fn default() -> Self {
        Self { title: None, account: None, duration: None, events: vec![], status: "video-only".into(), diagnostics: vec![] }
    }
}

#[derive(Debug, Clone)]
struct IndexedVideo { path: PathBuf, metadata: Metadata }

#[derive(Debug, Default)]
struct SourceIndex { videos: Vec<IndexedVideo>, diagnostics: Vec<String> }

struct CachedSource { fingerprint: Vec<(PathBuf, u64, u128)>, index: Arc<SourceIndex> }
static SOURCE_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedSource>>> = OnceLock::new();

/// Discover all video paths referenced by metadata, including absent originals.
/// Callers can preserve unavailable recordings in their app-owned index.
pub fn enumerate_source_videos(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let source = cached_source(root);
    let paths = source.videos.iter().map(|v| v.path.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    (paths, source.diagnostics.clone())
}

pub fn inspect_source(root: &Path) -> Vec<String> { cached_source(root).diagnostics.clone() }

pub fn inspect_video(path: &Path, source_root: &Path, explicit_metadata: Option<&Path>) -> Metadata {
    if let Some(metadata_path) = explicit_metadata {
        return inspect_metadata_file(path, metadata_path);
    }
    // Deliberately recognize our documented sidecar rather than interpreting
    // unrelated JSON next to a video as event data.
    let sidecar = path.with_extension("moments.json");
    if sidecar.is_file() { return inspect_metadata_file(path, &sidecar); }
    let index = cached_source(source_root);
    select_video(path, &index.videos).unwrap_or_else(|| {
        let mut out = Metadata::default();
        if !index.diagnostics.is_empty() {
            out.status = "metadata-unmatched".into();
            out.diagnostics = index.diagnostics.clone();
        }
        out
    })
}

fn inspect_metadata_file(video: &Path, metadata: &Path) -> Metadata {
    match parse_metadata_file(metadata) {
        Ok(index) => select_video(video, &index.videos).unwrap_or_else(|| Metadata {
            status: "metadata-unmatched".into(),
            diagnostics: vec!["元数据中未找到唯一对应的视频；请检查文件名是否重复。".into()],
            ..Metadata::default()
        }),
        Err(error) => Metadata { status: "unsupported".into(), diagnostics: vec![error], ..Metadata::default() },
    }
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").trim_start_matches("//?/").to_lowercase()
}

fn path_filename(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").rsplit('/').next().unwrap_or_default().to_lowercase()
}

fn select_video(path: &Path, videos: &[IndexedVideo]) -> Option<Metadata> {
    let exact = normalized_path(path);
    let mut matched = videos.iter().filter(|v| normalized_path(&v.path) == exact);
    if let Some(first) = matched.next() {
        // WonderfulDb carries richer event data than WeGameWonderfulDb.
        let chosen = matched.fold(first, |best, next| if next.metadata.events.len() > best.metadata.events.len() { next } else { best });
        return Some(chosen.metadata.clone());
    }
    // Allows an external computer's absolute video path to be relocated, but
    // ambiguous duplicate filenames never inherit another video's events.
    let filename = path_filename(path);
    let candidates: Vec<_> = videos.iter().filter(|v| path_filename(&v.path) == filename).collect();
    if candidates.len() == 1 { Some(candidates[0].metadata.clone()) } else { None }
}

fn metadata_files(root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![root.to_path_buf()];
    for relative in ["WonderfulDb", "WeGameWonderfulDb", "ACLOS/WonderfulDb", "ACLOS/WeGameWonderfulDb"] {
        let candidate = root.join(relative);
        if candidate.is_dir() { dirs.push(candidate); }
    }
    let mut files = vec![];
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(info) = fs::symlink_metadata(&path) else { continue };
            if !info.is_file() || info.file_type().is_symlink() { continue; }
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_account(&name) || name.strip_prefix("snapshot").is_some_and(is_account)
                || matches!(name.as_str(), "moments.json" | "metadata.json")
                || matches!(path.extension().and_then(|e| e.to_str()), Some("db" | "sqlite" | "sqlite3")) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn cached_source(root: &Path) -> Arc<SourceIndex> {
    let files = metadata_files(root);
    let fingerprint: Vec<_> = files.iter().filter_map(|p| fs::metadata(p).ok().map(|m| (
        p.clone(), m.len(), m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0)
    ))).collect();
    let cache = SOURCE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(locked) = cache.lock() {
        if let Some(stored) = locked.get(root) { if stored.fingerprint == fingerprint { return stored.index.clone(); } }
    }
    let mut index = SourceIndex::default();
    let mut nicknames: HashMap<String, String> = HashMap::new();
    for file in &files {
        let name = file.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        if let Some(account) = name.strip_prefix("snapshot").filter(|n| is_account(n)) {
            match parse_snapshot(file, account) {
                Ok(Some(nick)) => { nicknames.insert(account.to_string(), nick); },
                Ok(None) => {},
                Err(error) => index.diagnostics.push(format!("可选身份缓存无法读取：{error}")),
            }
        }
    }
    for file in &files {
        let name = file.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        if name.starts_with("snapshot") { continue; }
        match parse_metadata_file(file) {
            Ok(mut parsed) => {
                for video in &mut parsed.videos {
                    if let Some(nick) = video.metadata.account.as_ref().and_then(|id| nicknames.get(id)) { video.metadata.account = Some(nick.clone()); }
                }
                index.videos.extend(parsed.videos);
                index.diagnostics.extend(parsed.diagnostics);
            },
            Err(error) => index.diagnostics.push(format!("{}：{error}", name)),
        }
    }
    let index = Arc::new(index);
    if let Ok(mut locked) = cache.lock() { locked.insert(root.to_path_buf(), CachedSource { fingerprint, index: index.clone() }); }
    index
}

fn is_account(name: &str) -> bool { !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) }

fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    let info = fs::symlink_metadata(path).map_err(|e| format!("读取元数据失败：{e}"))?;
    if !info.is_file() || info.file_type().is_symlink() { return Err("元数据必须是普通文件。".into()); }
    if info.len() > MAX_METADATA_BYTES { return Err("元数据超过 128 MiB 安全读取上限。".into()); }
    fs::read(path).map_err(|e| format!("读取元数据失败：{e}"))
}

fn parse_metadata_file(path: &Path) -> Result<SourceIndex, String> {
    let bytes = bounded_read(path)?;
    if bytes.starts_with(b"SQLite format 3\0") {
        return Err("尚未验证此 SQLite 元数据表结构；已隔离，未写入官方数据库。".into());
    }
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    let (payload, cipher) = if is_account(name) {
        decode_account(&bytes, name)?
    } else {
        (serde_json::from_slice::<Value>(&bytes).map_err(|_| "不支持的元数据：仅接受 ACLOS 账户文件或 wegame-moments v1 JSON。".to_string())?, "plain-json")
    };
    if let Some(schema) = payload.get("schema") {
        if schema.as_str() != Some(EXCHANGE_SCHEMA) { return Err("未识别的交换元数据 schema；已隔离。".into()); }
        return parse_exchange(&payload, path);
    }
    parse_official(&payload, path, cipher)
}

fn decode_account(bytes: &[u8], account: &str) -> Result<(Value, &'static str), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "ACLOS 文件不是 UTF-8 十六进制文本。".to_string())?.trim();
    let mut cipher = hex::decode(text).map_err(|_| "ACLOS 文件不是有效十六进制，可能正在写入或格式已变更。".to_string())?;
    if cipher.is_empty() || cipher.len() % 16 != 0 { return Err("ACLOS 密文长度无效，稍后重试扫描。".into()); }
    let hash = hex::encode(Sha256::digest(account.as_bytes()));
    let expected_main = format!("key_wonderful_list_{account}");
    let expected_snapshot = format!("key_snapshot_list{account}");
    let original = cipher.clone();
    let plain256 = cbc::Decryptor::<Aes256>::new_from_slices(&hash.as_bytes()[..32], &hash.as_bytes()[..16])
        .map_err(|e| e.to_string())?.decrypt_padded_mut::<Pkcs7>(&mut cipher).ok().map(|p| p.to_vec());
    cipher.copy_from_slice(&original);
    let plain128 = cbc::Decryptor::<Aes128>::new_from_slices(&hash.as_bytes()[..16], &hash.as_bytes()[..16])
        .map_err(|e| e.to_string())?.decrypt_padded_mut::<Pkcs7>(&mut cipher).ok().map(|p| p.to_vec());
    for (plain, variant) in [(plain256, "aclos-aes256"), (plain128, "aclos-wegame-aes128")] {
        if let Some(plain) = plain {
            if let Ok(value) = serde_json::from_slice::<Value>(&plain) {
                if value.get(&expected_main).is_some_and(Value::is_array) || value.get(&expected_snapshot).is_some_and(Value::is_array) {
                    return Ok((value, variant));
                }
                // An empty optional snapshot is valid but cannot provide identity.
                if value.as_object().is_some_and(|v| v.is_empty()) { return Ok((value, variant)); }
            }
        }
    }
    Err("ACLOS 解密或账户列表校验失败；可能正在写入、账户文件改名或格式已变更。".into())
}

fn parse_exchange(payload: &Value, metadata_path: &Path) -> Result<SourceIndex, String> {
    if payload.get("version").and_then(Value::as_u64) != Some(1) || payload.get("timeUnit").and_then(Value::as_str) != Some("seconds") {
        return Err("交换元数据必须是 version=1、timeUnit=seconds；不猜测时间单位。".into());
    }
    let recordings = payload.get("recordings").and_then(Value::as_array).ok_or("交换元数据缺少 recordings 数组。")?;
    let mut index = SourceIndex::default();
    for (record_index, row) in recordings.iter().enumerate() {
        let file = string(row, "file").ok_or("交换录像缺少 file 字段。")?;
        let path = local_video_path(&file, metadata_path.parent().unwrap_or(Path::new(".")))?;
        let duration = number(row, "duration").filter(|n| *n > 0.0);
        let mut result = Metadata { title: string(row, "title"), account: string(row, "account"), duration, status: "exchange-v1".into(), ..Metadata::default() };
        if let Some(event_value) = row.get("events") {
            let events = event_value.as_array().ok_or("交换元数据 events 必须是数组。")?;
            for (event_index, event) in events.iter().enumerate() {
                let time = number(event, "time").ok_or("交换事件缺少有效的秒时间 time。")?;
                if time < 0.0 || duration.is_some_and(|d| time > d) { return Err("交换事件时间超出录像范围。".into()); }
                let kind = string(event, "kind").ok_or("交换事件缺少 kind。")?;
                if !matches!(kind.as_str(), "kill" | "death" | "assist" | "round" | "unknown") { return Err(format!("不支持的交换事件类型：{kind}")); }
                result.events.push(MomentEvent { id: string(event, "id").unwrap_or_else(|| format!("exchange-{record_index}-{event_index}")), kind, time, label: string(event, "label").unwrap_or_else(|| "事件".into()) });
            }
        }
        result.events.sort_by(|a, b| a.time.total_cmp(&b.time));
        index.videos.push(IndexedVideo { path, metadata: result });
    }
    Ok(index)
}

fn parse_official(payload: &Value, metadata_path: &Path, variant: &str) -> Result<SourceIndex, String> {
    let obj = payload.as_object().ok_or("ACLOS 明文必须是对象。")?;
    let account_lists: Vec<_> = obj.iter().filter_map(|(key, value)| key.strip_prefix("key_wonderful_list_").filter(|id| is_account(id)).and_then(|id| value.as_array().map(|v| (id, v)))).collect();
    if account_lists.is_empty() { return Err("未找到已验证的 key_wonderful_list_<account> 数组；未尝试未知表结构。".into()); }
    let mut index = SourceIndex::default();
    for (account, matches) in account_lists {
        for game in matches {
            let Some(videos) = game.get("videos").and_then(Value::as_array) else { continue };
            for video in videos {
                let Some(src) = string(video, "video_src") else { continue };
                let path = match local_video_path(&src, metadata_path.parent().unwrap_or(Path::new("."))) {
                    Ok(path) => path,
                    Err(error) => { index.diagnostics.push(error); continue; }
                };
                let duration = number(video, "video_duration").filter(|n| *n > 0.0).map(|ms| ms / 1000.0);
                let (events, quarantined) = official_events(game, video);
                let mut diagnostics = vec![];
                if quarantined > 0 { diagnostics.push(format!("已隔离 {quarantined} 个缺少时间或身份依据的 ACLOS 事件。")); }
                if variant == "aclos-wegame-aes128" && video.get("rounds").is_none() { diagnostics.push("WeGame 元数据没有事件树；可播放视频，不生成猜测的击杀时间。".into()); }
                index.videos.push(IndexedVideo { path, metadata: Metadata {
                    title: string(video, "video_name").or_else(|| string(video, "video_type")), account: Some(account.into()), duration, events, status: variant.into(), diagnostics,
                }});
            }
        }
    }
    Ok(index)
}

fn local_video_path(value: &str, base: &Path) -> Result<PathBuf, String> {
    if value.contains("://") || value.starts_with("data:") { return Err("远程或 URI 视频路径未经过验证，已隔离。".into()); }
    let path = PathBuf::from(value);
    // Windows drive paths remain absolute when tests run on another OS.
    let has_drive = value.as_bytes().get(1) == Some(&b':');
    if has_drive && !matches!(value.as_bytes().get(2), Some(b'\\' | b'/')) { return Err("不接受含糊的 Windows 盘符相对路径。".into()); }
    let windows_absolute = has_drive || value.starts_with("\\\\");
    Ok(if path.is_absolute() || windows_absolute { path } else { base.join(path) })
}

fn parse_snapshot(path: &Path, account: &str) -> Result<Option<String>, String> {
    let bytes = bounded_read(path)?;
    let (payload, _) = decode_account(&bytes, account)?;
    let key = format!("key_snapshot_list{account}");
    let Some(records) = payload.get(key).and_then(Value::as_array) else { return Ok(None); };
    let best = records.iter().filter(|r| r.get("snapshot").is_some_and(|s| string(s, "ss_nick").is_some())).max_by(|a, b| number(a, "matches_time").unwrap_or(0.0).total_cmp(&number(b, "matches_time").unwrap_or(0.0)));
    Ok(best.and_then(|r| r.get("snapshot")).and_then(|s| string(s, "ss_nick").map(|nick| string(s, "ss_nick_id").map(|tag| format!("{nick}#{tag}")).unwrap_or(nick))))
}

fn string(value: &Value, key: &str) -> Option<String> { value.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string) }
fn number(value: &Value, key: &str) -> Option<f64> { value.get(key).and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).filter(|n| n.is_finite()) }

fn official_events(game: &Value, video: &Value) -> (Vec<MomentEvent>, usize) {
    let mut result = vec![];
    let mut quarantined = 0;
    let Some(rounds) = video.get("rounds").and_then(Value::as_array) else { return (result, 0); };
    let mut ids = BTreeSet::new();
    for round in rounds {
        let Some(clips) = round.get("round_clips").and_then(Value::as_array) else { continue };
        for clip in clips {
            let Some(events) = clip.get("clip_events").and_then(Value::as_array) else { continue };
            for event in events {
                // Other event kinds are valid source data, but have no verified
                // playback mapping in this adapter.
                if !string(event, "event_type").is_some_and(|kind| matches!(kind.to_lowercase().as_str(), "kill" | "death")) { continue; }
                match official_event(game, video, clip, event) {
                    Some(resolved) => {
                        let identity = format!("{}|{}|{}", resolved.kind, resolved.time, resolved.label);
                        if ids.insert(identity) { result.push(resolved); }
                    },
                    None => { quarantined += 1; },
                }
            }
        }
    }
    result.sort_by(|a, b| a.time.total_cmp(&b.time));
    (result, quarantined)
}

fn official_event(game: &Value, video: &Value, clip: &Value, event: &Value) -> Option<MomentEvent> {
    let kind = string(event, "event_type")?.to_lowercase();
    let ext = event.get("event_ext")?.as_object()?;
    let ext = Value::Object(ext.clone());
    if !string(&ext, "EventName")?.eq_ignore_ascii_case("Shot") { return None; }
    let killer = string(&ext, "KillerPlayerName")?;
    let victim = string(&ext, "KilledPlayerName")?;
    let agent = string(&ext, "AgentName")?;
    if !agent.eq_ignore_ascii_case(&string(game.get("agent")?, "agent_name")?) { return None; }
    let occurred = local_time_ms(&string(&ext, "EventTime")?)?;
    let start = local_time_ms(&string(game, "gameStartTime")?)?;
    let end = local_time_ms(&string(game, "gameEndTime")?)?;
    if end < start || occurred < start - 30_000 || occurred > end + 30_000 { return None; }
    let is_me_killer = number(&ext, "KillerIsMe")?;
    let is_me_victim = number(&ext, "KilledIsMe")?;
    let part = number(&ext, "GetShotRolePart")?;
    if !matches!(kind.as_str(), "kill" | "death") { return None; }
    if (kind == "kill" && (is_me_killer != 1.0 || is_me_victim != 0.0)) || (kind == "death" && (is_me_killer != 0.0 || is_me_victim != 1.0)) { return None; }
    let time = resolve_official_time(&string(video, "video_type")?, number(video, "video_duration")?, number(clip, "clip_sTime"), number(event, "event_sTime")?, &kind)?;
    let target = if kind == "kill" { victim } else { killer };
    Some(MomentEvent { id: string(event, "event_id").unwrap_or_else(|| format!("aclos-{occurred}-{time}")), kind: kind.clone(), time, label: format!("{} {}{}", if kind == "kill" { "击杀" } else { "死亡" }, target, if part == 1.0 { " · 爆头" } else { "" }) })
}

fn resolve_official_time(video_type: &str, duration_ms: f64, clip_ms: Option<f64>, event_ms: f64, kind: &str) -> Option<f64> {
    if duration_ms <= 0.0 || event_ms < 0.0 { return None; }
    let ms = match video_type {
        "击杀集锦" if kind == "kill" => event_ms,
        "死亡集锦" if kind == "death" => event_ms,
        "击杀集锦" | "死亡集锦" => return None,
        _ if kind == "kill" => clip_ms.filter(|n| *n >= 0.0).unwrap_or(0.0) + event_ms,
        _ => return None,
    };
    if !ms.is_finite() || ms > duration_ms { None } else { Some(ms / 1000.0) }
}

fn local_time_ms(value: &str) -> Option<i64> {
    NaiveDateTime::parse_from_str(&value.replace('T', " "), "%Y-%m-%d %H:%M:%S%.f").ok().map(|time| time.and_utc().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preserves_montage_and_moment_time_units() {
        assert_eq!(resolve_official_time("击杀集锦", 140_000.0, Some(70_000.0), 130_000.0, "kill"), Some(130.0));
        assert_eq!(resolve_official_time("三杀时刻", 30_000.0, Some(11_000.0), 6_000.0, "kill"), Some(17.0));
        assert_eq!(resolve_official_time("三杀时刻", 30_000.0, Some(-1.0), 6_000.0, "kill"), Some(6.0));
        assert_eq!(resolve_official_time("死亡集锦", 30_000.0, Some(11_000.0), 6_000.0, "death"), Some(6.0));
    }

    #[test]
    fn rejects_mismatches_and_out_of_bounds_events() {
        assert!(resolve_official_time("击杀集锦", 30_000.0, None, 1000.0, "death").is_none());
        assert!(resolve_official_time("三杀时刻", 30_000.0, Some(27_000.0), 6_000.0, "kill").is_none());
        assert!(resolve_official_time("击杀集锦", 30_000.0, None, -1.0, "kill").is_none());
        assert!(resolve_official_time("击杀集锦", 0.0, None, 0.0, "kill").is_none());
        assert!(local_time_ms("2026-99-90 21:00:00").is_none());
    }

    #[test]
    fn exchange_requires_explicit_version_and_seconds() {
        let valid: Value = serde_json::from_str(include_str!("../../../tests/fixtures/exchange-v1.json")).unwrap();
        let index = parse_exchange(&valid, Path::new("/import/metadata.json")).unwrap();
        assert_eq!(index.videos[0].metadata.events[0].time, 3.25);
        assert_eq!(index.videos[0].metadata.account.as_deref(), Some("演示账号"));
        let mut wrong = valid.clone(); wrong["timeUnit"] = json!("milliseconds");
        assert!(parse_exchange(&wrong, Path::new("metadata.json")).is_err());
        let mut wrong = valid; wrong["version"] = json!(2);
        assert!(parse_exchange(&wrong, Path::new("metadata.json")).is_err());
    }

    #[test]
    fn actual_format_fixture_maps_events_and_quarantines_missing_evidence() {
        let payload: Value = serde_json::from_str(include_str!("../../../tests/fixtures/aclos-plaintext.json")).unwrap();
        let index = parse_official(&payload, Path::new("123"), "plain-json").unwrap();
        assert_eq!(index.videos.len(), 2);
        assert_eq!(index.videos[0].metadata.events[0].time, 17.0);
        assert_eq!(index.videos[1].metadata.events[0].time, 13.0);
        assert_eq!(index.videos[0].metadata.events.len(), 1);
        assert_eq!(index.videos[0].metadata.diagnostics.len(), 1);
    }

    #[test]
    fn independent_dotnet_encrypted_fixtures_decode_both_variants() {
        let aes256 = include_bytes!("../../../tests/fixtures/aclos-aes256.hex");
        let aes128 = include_bytes!("../../../tests/fixtures/aclos-wegame-aes128.hex");
        assert_eq!(decode_account(aes256, "123").unwrap().1, "aclos-aes256");
        assert_eq!(decode_account(aes128, "123").unwrap().1, "aclos-wegame-aes128");
        assert!(decode_account(aes256, "124").is_err());
        assert!(decode_account(b"00ab9", "123").is_err());
    }

    #[test]
    fn unsupported_shapes_do_not_become_false_events() {
        assert!(parse_official(&json!({"random": [{"videos": []}]}), Path::new("metadata.json"), "plain-json").is_err());
        assert!(parse_exchange(&json!({"schema":"wegame-moments", "version":1,"timeUnit":"seconds", "recordings":[{"file":"a.mp4","events":[{"kind":"kill","time":-1}]}]}), Path::new("metadata.json")).is_err());
        assert!(parse_exchange(&json!({"schema":"wegame-moments", "version":1,"timeUnit":"seconds", "recordings":[{"file":"a.mp4","events":{}}]}), Path::new("metadata.json")).is_err());
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("metadata.json");
        fs::write(&file, br#"{"schema":"unknown","key_wonderful_list_123":[]}"#).unwrap();
        assert!(parse_metadata_file(&file).unwrap_err().contains("schema"));
    }

    #[test]
    fn relocated_files_match_only_unique_basenames() {
        assert!(local_video_path("C:relative.mp4", Path::new("metadata.json")).is_err());
        let mut index = parse_exchange(&json!({"schema":"wegame-moments","version":1,"timeUnit":"seconds","recordings":[{"file":"C:\\old\\clip.mp4","title":"first"}]}), Path::new("metadata.json")).unwrap();
        assert_eq!(select_video(Path::new("D:\\new\\clip.mp4"), &index.videos).unwrap().title.as_deref(), Some("first"));
        index.videos.push(IndexedVideo { path: PathBuf::from("E:\\another\\clip.mp4"), metadata: Metadata::default() });
        assert!(select_video(Path::new("D:\\new\\clip.mp4"), &index.videos).is_none());
    }

    #[test]
    fn source_cache_detects_additions_and_keeps_missing_originals() {
        let directory = tempfile::tempdir().unwrap();
        assert!(enumerate_source_videos(directory.path()).0.is_empty());
        fs::write(directory.path().join("123"), include_bytes!("../../../tests/fixtures/aclos-aes256.hex")).unwrap();
        let (videos, diagnostics) = enumerate_source_videos(directory.path());
        assert_eq!(videos.len(), 2);
        assert!(diagnostics.is_empty());
        assert!(!videos[0].exists());
        let mapped = inspect_video(&directory.path().join("demo.mp4"), directory.path(), None);
        assert_eq!(mapped.events[0].time, 17.0);
        fs::remove_file(directory.path().join("123")).unwrap();
        assert!(enumerate_source_videos(directory.path()).0.is_empty());
    }

    #[test]
    fn sqlite_is_read_only_and_rejected_without_guessing_tables() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unknown.db");
        let original = b"SQLite format 3\0synthetic-unverified-table-layout";
        fs::write(&path, original).unwrap();
        assert!(parse_metadata_file(&path).unwrap_err().contains("SQLite"));
        assert_eq!(fs::read(path).unwrap(), original);
    }
}
