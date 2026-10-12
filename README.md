# ValoPlayer · 无畏时刻增强播放器

ValoPlayer 是实际可运行的 Windows 本地 MVP，使用 Tauri 2 + Rust + Vue 3 + TypeScript + 应用自有 SQLite。界面及实现独立开发，参考 WonderfulUI 的 ACLOS 格式与事件语义。

## Windows 安装

运行 `ValoPlayer_0.2.1_x64-setup.exe`，按安装引导选择安装位置和快捷方式，然后启动 `ValoPlayer.exe`。安装引导支持简体中文和英文，默认仅为当前 Windows 用户安装，并包含 FFmpeg / ffprobe 媒体工具。

安装后保留个人录像、收藏副本和索引在应用数据目录。卸载不会清空应用数据。0.2.1 修复桌面图标，并在升级时更新已有快捷方式。详见 [安装引导说明](docs/INSTALLER.md)。ValoPlayer 沿用 `com.local.wegame-moments` 数据目录标识，以兼容早期版本的数据；这个内部标识不影响正式产品名称。

## 从源码启动

在项目目录打开 PowerShell：

```powershell
npm install
npm run setup:media
npm run desktop
```

本次开发环境已安装依赖和媒体工具；该工作目录还提供自动识别的独立 Rust/MinGW 工具链，因此可以直接 `npm run desktop`。复制源码到别处时，需要正常安装 [Tauri Windows 开发前置环境](https://v2.tauri.app/start/prerequisites/)（Rust、Microsoft C++ Build Tools、WebView2）及 Node.js 22.12 或更新版本。普通 Windows MSVC 环境无需本次使用的 MinGW 工具链。

```powershell
npm run build              # TypeScript 检查与前端生产构建
npm test                   # 前端业务测试
npm run test:rust          # Rust 核心测试
npm run test:rust -- media::tests::ffmpeg_smoke -- --ignored
npm run test:rust -- library::tests::recycle_bin_smoke -- --ignored
npm run desktop:build      # 构建 Windows 可执行文件，不制作安装包
npm run package:windows    # 将程序、WebView2 Loader 和媒体工具整理到 dist-windows
npm run installer:build    # 构建 ValoPlayer 0.2.1 Windows NSIS 安装引导程序
```

桌面构建的程序名为 `ValoPlayer.exe`；安装包名为 `ValoPlayer_0.2.1_x64-setup.exe`，实际生成位置以构建输出为准。源码发布包使用 `ValoPlayer/` 作为项目目录前缀。

仅查看界面：`npm run dev`，打开 `http://127.0.0.1:1420/`。浏览器默认显示空状态；“体验界面演示”使用明确标记的公共花卉视频和样例事件。本机读取、回收站及导出操作在桌面端执行。

## 已实现

- 自动发现 `%APPDATA%\ACLOS\WonderfulDb` 和 `WeGameWonderfulDb`，跨本机多个账户显示录像；按路径去重，并优先使用有完整事件的缓存。
- 手动连接录像目录，导入外部视频与 ACLOS 账号缓存、明文 ACLOS JSON 或标准 v1 交换元数据；外部绝对路径可按唯一文件名重新关联。
- SQLite 索引、网格/列表、搜索、账号与击杀数筛选、排序、缺失录像状态、自定义分类、批量分类与批量文件管理。
- 从播放器返回时恢复进入录像前的列表位置；网格、列表、Esc 返回及连续播放都保留原来的浏览位置。
- 混合收藏：索引收藏与永久复制。永久副本存入应用数据目录；原文件删除后可播放副本。取消收藏或切换模式保留已生成的副本，需在“删除管理”中明确删除副本。
- 原文件/副本删除均使用 Windows 回收站；失败时报告并保留文件，不回退到永久删除。移除索引会记录忽略项，重扫不自动重新加入，显式导入可以恢复索引。
- HTML5 本地视频播放、倍速、暂停逐帧步进、快捷键、全屏、连续播放、事件跳转、进度条悬停缩略图。
- FFmpeg 精确 H.264/AAC MP4 裁剪、快速复制裁剪、按每个验证过的击杀事件批量导出。
- 目录变化监测、2 秒防抖、45 秒补充扫描，视频和元数据变化增量更新；缩略图按需生成与缓存。

## 第一次使用

启动 ValoPlayer 后自动扫描本机 ACLOS；初次索引可能需要十几秒。来源不存在时，通过左侧“录像来源”连接目录，或“导入录像”选择外部视频。数值命名的 ACLOS 账号缓存没有扩展名，导入时使用“所有文件”即可选择，务必保持其原文件名。

录像库工具栏的“全部击杀数”可选择 1、2、3、4、5 杀、6 杀及以上或“无击杀事件”，能与账号、搜索、分类及排序共同使用。1–5 杀按该录像中已验证的击杀事件数精确匹配，不从标题猜测，也不是整局 KDA。“重置筛选”会同时清除击杀数、账号和搜索条件。观看录像后，点击“返回录像库”、关闭按钮或 Esc 会回到进入时的位置；切换侧栏来源或修改筛选条件则从新结果顶部开始。

收藏时选“索引收藏”或“永久副本”。原文件删除前，在详情里确认“收藏副本”位置；批量删除对话框会明确区分原始文件、永久副本与库内索引。移除来源只停止该来源扫描，不删除录像文件或现有索引。

打开录像后用“裁剪与导出”设置秒数范围。精确裁剪重新编码；取消精确裁剪使用视频流复制，实际起点可能对齐到关键帧。事件面板可设置击杀前后秒数，每个击杀输出一个独立 MP4，不自动合并重叠事件。

快捷键：`Space / K` 播放暂停；`← / →` 跳转 5 秒；`J / L` 跳转 10 秒；`,` / `.` 前后帧；`M` 静音；`F` 全屏；`Esc` 返回；`Ctrl+K` 搜索。文本输入及管理弹窗中屏蔽播放器快捷键。

## 数据边界与当前限制

官方 ACLOS 文件按普通只读文件读取，从未打开可写数据库连接，没有游戏进程注入、录制、云同步、手动标签或备注。ACLOS 不是稳定的官方公共接口；已验证 AES256/AES128 及对应字段，未知格式或不完整事件会进入诊断。详情见 [ACLOS 验证记录](docs/ACLOS.md) 与 [交换元数据示例](docs/metadata-v1.example.json)。

播放器依赖 WebView2 的系统解码能力；某些 MKV/AVI/HEVC 视频可能无法直接播放，可通过 FFmpeg 导出 H.264 MP4。逐帧功能按 ffprobe 报告的帧率调整 HTML5 播放位置；可变帧率视频及浏览器寻帧精度下，不保证剪辑软件级别的逐帧精确度。

永久副本仍保存在同一电脑，不能替代异地备份。个人视频与 SQLite 数据不随源码或便携程序分发。单文件导入保留原路径和一次性元数据快照；自动增量发现针对已经连接的来源目录。暂未提供分类重命名、视频流转码播放、字幕、安装器签名或自动更新。

应用数据位置：`%LOCALAPPDATA%\com.local.wegame-moments\`（兼容早期版本的数据目录标识，以设置里的实际路径为准），包含 `library.sqlite3`、`favorites/` 和 `thumbnails/`。媒体工具自动查找项目或程序旁的 `tools/`，也可以在设置指定 `ffmpeg.exe`（建议与 `ffprobe.exe` 同目录）。详见 [媒体工具说明](docs/MEDIA.md)。

Rust 核心测试以 `--no-default-features` 运行，与桌面窗口及文件选择器隔离；桌面构建默认启用 `desktop`。两个需要本机媒体工具/回收站的测试默认跳过，以上命令已分别显式运行并通过。回收站测试只使用自动生成的临时文件。

## 项目结构

`src/` 是 Vue 界面与播放器；`src-tauri/src/library.rs` 是索引与文件管理；`db.rs` 是应用 SQLite；`adapters/` 是隔离的 ACLOS 与交换 JSON 解析；`media.rs` 是独立 FFmpeg 管线。

[WonderfulUI 参考提交](https://github.com/WizisCool/WonderfulUI/tree/e6925a083598b1faecce6478a02357d89cf5be01)。本项目以 GPL-3.0-or-later 发布；不是 Riot、腾讯或 WeGame 的官方软件。媒体工具及第三方依赖分别遵循其自身许可证。
