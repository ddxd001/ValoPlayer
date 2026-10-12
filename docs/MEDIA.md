# 本地视频处理

播放器通过本地 FFmpeg / ffprobe 读取视频信息、生成缩略图和导出 MP4。程序直接传递参数给可执行文件，支持中文及包含空格的文件名，不执行 shell 命令，Windows 子进程隐藏窗口。

## 安装与查找

项目提供 `npm run setup:media`，下载并将 FFmpeg 与 ffprobe 准备到 `tools/`。也可以使用包含两个程序的完整工具包，在设置中指定 `ffmpeg.exe` 文件或其所在目录。优先查找设置路径；设置为空时检查 `FFMPEG_PATH`、PATH、应用/资源/开发目录中的 `tools/`、`bin/`、开发项目 `node_modules/ffmpeg-static`，再检查 `C:\ffmpeg\bin`、`C:\Program Files\ffmpeg\bin` 和 Scoop 标准路径。没有全盘搜索。

ffprobe 优先使用 ffmpeg 同目录的程序，其次检查 `FFPROBE_PATH`、PATH、应用/开发目录中的 `tools/`、`bin/` 及 `node_modules/ffprobe-static/bin/<平台>/<架构>`。Windows x64 对应 `win32/x64/ffprobe.exe`。运行：

```powershell
npm run setup:media
```

本次便携程序包含 FFmpeg 6.1.1 与 ffprobe 4.0.2，程序运行不需要 npm 或 Rust。`package:windows` 会将媒体工具复制到程序旁的 `tools/`。两个实际二进制均报告 GPL v3；ffprobe-static 的 MIT 许可证只覆盖 npm 包装代码。准备脚本同时保存二进制许可及 FFmpeg 构建/源码说明，便携程序会保留这些文件。源代码与发行来源见工具 README；FFmpeg 构建源码提交为 `e38092ef93`。

## 行为

- `probe` 使用 ffprobe JSON 读取时长、分辨率及有理数帧率。时长优先取容器，其次视频流；无有效视频流或时长会明确报错。未知帧率返回 0，不伪造固定帧率。
- 缩略图是宽度 480 像素、等比例高度的单帧，时间必须为非负有限数。未生成有效画面会报错。
- 精确裁剪使用 H.264（libx264，CRF 20）与 AAC 音频，像素格式 yuv420p，视频边长向下取偶数，并把 MP4 索引移到文件开头便于播放。没有音频的视频也可导出。
- 快速裁剪复制编码流，速度快，起点受关键帧位置影响，输出时长可能比选择范围略长。源编码不兼容 MP4 时应改用精确裁剪。
- 输出必须使用 `.mp4` 扩展名；起点、终点先与实际视频时长校验。仅接受最多 0.05 秒的界面四舍五入偏差，并将终点截至真实时长。
- 文件先写入目标目录中的私有临时目录，处理成功且非空后才发布最终文件。Windows 使用不带覆盖标志的 MoveFileExW；其他平台使用同分区硬链接。目标已存在时拒绝覆盖，包括处理期间另一操作创建目标的竞争情况。失败时只清理本次创建的临时文件与空目录。

本模块没有读取或写入官方数据库，没有操作游戏进程。原录像路径、输出目录和操作权限由调用它的资料库模块校验。

## 验证

Rust 单元测试覆盖 ffprobe 解析、有理数帧率回退、无视频及无效时长拒绝、时间范围、命令参数完整性、精确/快速编码选择、发布时不覆盖已有文件及临时目录清理。运行：

```powershell
npm run test:rust -- media::tests
npm run test:rust -- media::tests::ffmpeg_smoke -- --ignored
```

第二条命令执行媒体冒烟测试：生成含音频的 3 秒短片，验证时长读取、中文与特殊字符路径、缩略图、精确裁剪时长、快速裁剪、重复输出拒绝、越界裁剪拒绝和原文件未被改动。已在本机通过该测试，并在 Windows WebView2 播放器中播放了真实 ACLOS 游戏录像，显示真实缩略图、时长、分辨率和事件。ACLOS 解析及只读验证见 [ACLOS.md](ACLOS.md)，完整测试结果见 [TEST_REPORT.md](TEST_REPORT.md)。
