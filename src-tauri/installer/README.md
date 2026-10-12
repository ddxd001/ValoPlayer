# NSIS 卸载时保留个人数据

`installer.nsi` 基于本机 `@tauri-apps/cli 2.12.2` 对应的官方模板，删去“删除应用数据”选项及其执行分支，并为新建与已有快捷方式指定 ValoPlayer 的多尺寸图标。正常安装、升级和卸载程序文件的流程保留。

## 来源与许可证

- 仓库：<https://github.com/tauri-apps/tauri>
- 固定标签：`tauri-cli-v2.12.2`；该标签的 `tauri-bundler` 为 `2.10.2`。
- 上游文件：[`crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.2/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)。
- 上游原文件 SHA256：`166f9677d42e81c7f712a1bf80d9d8c089c21d2967d8398474768b5eac14b494`。
- 本修改版 SHA256（UTF-8、LF）：`8e743ad47d722b7c6a248e254a3ead17e4e4d098748d28c858af3f7729e6f954`。
- 上游 `tauri-bundler` 以 `Apache-2.0 OR MIT` 提供。本目录的模板采用其 MIT 许可；随附原样复制的 [LICENSE-MIT.txt](LICENSE-MIT.txt)，包含 Cargo-Bundle 和 Tauri Apps Contributors 的版权及许可声明。

许可证来源：[`tauri-bundler/License-MIT.md`](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.2/crates/tauri-bundler/License-MIT.md)。此模板的上游许可独立于 ValoPlayer 项目自身的 GPL-3.0-or-later。

## 精确修改范围

数据保护部分删去上游两处代码，共 55 行：

1. 删除原文件第 426–462 行：`DeleteAppDataCheckbox`、`DeleteAppDataCheckboxState`、仅供该复选框使用的常量、`un.ConfirmShow` / `un.ConfirmLeave` 及其页面回调绑定。标准卸载确认页、`un.SkipIfPassive` 和卸载执行页保留。
2. 删除原文件第 875–892 行：依赖复选框的条件分支，包括 `$APPDATA\${BUNDLEID}` 和 `$LOCALAPPDATA\${BUNDLEID}` 的递归删除，以及只在该分支中执行的安装位置/语言注册信息清理。卸载程序正常移除“已安装应用”注册项、程序关联和快捷方式的代码保留；未删除个人数据时，安装位置/语言记录的保留行为与上游默认行为一致。

此外，`SetValoPlayerShortcut` 宏使用 `CreateShortcut` 明确指定 `$INSTDIR\icons\ValoPlayer.ico`、工作目录与应用标识。在创建或更新桌面及开始菜单快捷方式的所有分支调用此宏，然后用 `SHChangeNotify(SHCNE_UPDATEITEM, SHCNF_PATHW, ...)` 通知 Windows 刷新该项；升级时同样修复已有快捷方式。安装执行段还会独立刷新指向本应用的既有桌面快捷方式，因此即使取消完成页的“创建桌面快捷方式”，已有快捷方式仍会修复；不存在的快捷方式仍由完成页选项决定。

安装资源、主程序、附加二进制及 `uninstall.exe` 仍按上游逻辑删除；资源目录和安装目录只尝试移除空目录。`/UPDATE`、重装与版本比较、WebView2 安装及安装器 hooks 均保持原样。

ValoPlayer 应继续使用 `identifier = com.local.wegame-moments`，个人数据仍位于原应用数据目录，包括 `library.sqlite3`、永久收藏副本、分类和设置。正常卸载或更新该安装器时，不提供清空个人数据的选项，也不执行上述两个应用数据目录的递归删除。用户通过播放器主动删除视频的操作不受本模板影响。

## 接入与验证

在 `tauri.conf.json` 中设置：

```json
{
  "bundle": {
    "windows": {
      "nsis": {
        "template": "installer/installer.nsi"
      }
    }
  }
}
```

模板保留了该 CLI 官方模板的全部 Handlebars 占位符，没有引入自定义变量。产品名、数据目录标识、主程序、资源与二进制、安装模式等均由 Tauri bundler 按上游规则提供；`utils.nsh` 等包含文件仍由 bundler 在生成目录准备，不应手动改为本目录内的路径。

已做静态核对：全部 Handlebars token 与原模板一致；已无 `DeleteAppDataCheckbox`、`un.ConfirmShow`、`un.ConfirmLeave` 或应用数据目录递归删除指令。未执行安装或卸载。实际 NSIS 编译和安装器验收由项目构建流程完成；更新 Tauri CLI 版本时，应重新对比对应标签的官方模板再保留数据保护和图标更新修改。

快捷方式创建前用 `GetFinalPathNameByHandleW` 解析程序和图标的物理文件路径，兼容由打包 Windows 应用启动安装器时的文件系统重定向。解析失败时保留输入路径；成功时移除扩展路径前缀，并保留 UNC 语义。

已有快捷方式的升级匹配同时接受旧逻辑路径和 resolver 返回的物理路径，因此 `/UPDATE`、`/NS` 和取消新增桌面快捷方式时也会刷新已存在的本应用快捷方式。
