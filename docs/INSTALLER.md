# ValoPlayer Windows 安装引导

版本：0.2.1，Windows x64。运行 `ValoPlayer_0.2.1_x64-setup.exe`，选择语言后按向导安装。支持简体中文和英文，可选择安装目录，并创建开始菜单及可选桌面快捷方式。默认安装至当前用户的 `%LOCALAPPDATA%\ValoPlayer`，无需为应用本身请求管理员权限。

安装包内含 `ValoPlayer.exe`、WebView2 Loader、FFmpeg / ffprobe 及其许可证，无需用户安装 Node.js 或 Rust。若系统没有 WebView2 Runtime，安装器会联网下载微软引导程序并安装运行时；这不是离线 WebView2 安装包。

ValoPlayer 继续使用 `%LOCALAPPDATA%\com.local.wegame-moments` 保存已有索引、收藏副本、缩略图、分类和设置。产品名称、程序名称与快捷方式均为 ValoPlayer；旧内部数据标识、交换 JSON schema 及 `.moments.json` 文件约定保留以兼容已有数据。

卸载器只移除已安装的程序文件、快捷方式和卸载注册信息，不提供删除应用数据的选项。重装后可继续访问旧的应用数据；原始录像仍在原来位置。

## 构建

先准备 [Tauri Windows 开发环境](https://v2.tauri.app/start/prerequisites/)，然后在项目目录运行：

```powershell
npm install
npm run setup:media
npm run installer:build
```

安装器输出至 `src-tauri/target/release/bundle/nsis/`；使用显式 Rust target 时，请以命令输出的目录为准。`npm run desktop:build` 生成独立程序；`npm run package:windows` 整理便携版至 `dist-windows`。品牌图可以通过 `powershell -NoProfile -File scripts/generate-installer-art.ps1` 重新生成。

安装流程使用 [Tauri NSIS 打包](https://v2.tauri.app/distribute/windows-installer/)；保留数据的模板基线、变更和许可见 [模板来源说明](https://github.com/ddxd001/ValoPlayer/tree/main/src-tauri/installer)。当前安装包未做代码签名。

0.2.1 补齐多尺寸透明应用图标，并将桌面及开始菜单快捷方式明确关联到安装目录内的 `icons/ValoPlayer.ico`。覆盖安装会更新已有的 ValoPlayer 快捷方式，并通知 Windows 刷新对应图标。

安装器通过实际文件句柄解析快捷方式目标及 ICO 的最终文件路径，避免打包 Windows 应用的目录重定向造成桌面无法找到程序或图标。不会写死开发环境或某个宿主应用的包目录。
