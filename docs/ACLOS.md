# ACLOS 数据适配与验证记录

检查日期：2026-10-13。这里只读取已经生成的本地录像与缓存。程序不访问游戏进程，不调用录制引擎，不修改官方元数据，也不把 ACLOS 缓存当成应用自己的 SQLite 数据库。

## 参考来源

已下载并检查 [WonderfulUI](https://github.com/WizisCool/WonderfulUI)，固定提交为 `e6925a083598b1faecce6478a02357d89cf5be01`。参考的是目录、编码和事件语义，界面及此项目实现独立编写。参考项目采用 GPL-3.0-or-later；没有把它的播放器、界面、资源或源码复制到此项目。

相关固定版本源文件：

- [ACLOS 格式文档](https://github.com/WizisCool/WonderfulUI/blob/e6925a083598b1faecce6478a02357d89cf5be01/docs/ACLOS_FORMAT.md)
- [Rust 加密派生规则](https://github.com/WizisCool/WonderfulUI/blob/e6925a083598b1faecce6478a02357d89cf5be01/src-tauri/src/parser/crypto.rs)
- [字段模型](https://github.com/WizisCool/WonderfulUI/blob/e6925a083598b1faecce6478a02357d89cf5be01/src-tauri/src/parser/model.rs)
- [事件验证与时间映射](https://github.com/WizisCool/WonderfulUI/blob/e6925a083598b1faecce6478a02357d89cf5be01/src-tauri/src/library/events.rs)
- [参考项目保存的 ACLOS safeUtils 格式记录，含 WeGame AES128 分支](https://github.com/WizisCool/WonderfulUI/blob/e6925a083598b1faecce6478a02357d89cf5be01/packages/parser/src/schema/_acl-source/safeUtils.js)

这些是开源项目的兼容性观察，不是腾讯承诺稳定的公开 API。

## 已验证的实际输入

参考源代码与本机只读检查共同确认：

| 内容 | 路径 / 格式 | 本项目行为 |
| --- | --- | --- |
| ACLOS 账户缓存 | `%APPDATA%\ACLOS\WonderfulDb\<十进制账户ID>`，无扩展名 | SHA256 派生 AES256-CBC 解密，再严格验证 JSON 账户数组 |
| 可选身份缓存 | 同目录的 `snapshot<账户ID>` | 解密后读取最新昵称；缺失或损坏不影响录像 |
| WeGame 账户缓存 | `%APPDATA%\ACLOS\WeGameWonderfulDb\<十进制账户ID>` | SHA256 派生 AES128-CBC 解密，再严格验证 JSON 账户数组 |
| 视频路径 | `videos[].video_src`，本机观察为 Windows 绝对路径 | 同时索引仍存在及已缺失的视频，不猜默认录像盘符 |
| 事件树 | `videos[].rounds[].round_clips[].clip_events[]` | 验证身份及时间后转换为播放秒数 |

本机发现 7 个账户缓存及对应的可选身份缓存。使用 .NET 的独立 SHA256/AES 实现只读解密了全部真实账户缓存：两组各 7 个账户、324 场对局、1,140 条录像路径，均零解析失败；其中 1,008 条路径的原文件仍存在。AES256 与 AES128 分别成功，列表键、视频字段和事件字段吻合上述源代码。实际 `WonderfulDb` 中 1,136 条录像具有事件树；实际 `WeGameWonderfulDb` 只有视频列表，没有事件树，因此此分支不会生成虚假的击杀标记。两目录会重复引用同一视频，应按路径去重。部分历史视频原路径已不存在，程序应显示“原文件缺失”。没有保存或发布真实账户、玩家或对局数据。

本机验证确认“能够读取真实缓存”。合成夹具测试确认“时间映射及错误隔离按预期”。最终用户正在使用的 ACLOS 版本仍可能变更格式，错误会出现在诊断中，不会写回缓存。

随后实际编译并运行了此 Rust 适配器，读取全部缓存得到 1,140 条路径、1,008 个仍存在的视频和 12,272 个可播放事件（8,314 次击杀、3,958 次死亡），源文件解析错误与越界事件均为 0。对同批真实数据独立运行 WonderfulUI 的 TypeScript 事件状态机，按路径、事件类型、毫秒时间和显示目标生成排序摘要，得到完全相同的事件摘要 SHA256：`8055e0c882c1e0bc63834fb164762c2b03163bf0272efa46df0fcb5df593ea91`。独立检查了 21 个官方账户/身份文件的运行前后 SHA256，内容变化数量为 0。142 个视频存在被隔离的残缺或不匹配事件，其余可用事件仍可正常呈现。

## 加密及 JSON 合同

缓存正文为 ASCII 十六进制密文，AES-CBC 使用 PKCS#7 padding。`digest = lowercase_hex(SHA256(account_id_utf8))`。

- WonderfulDb：key 为 `digest[0..32]` 的 ASCII 字节，IV 为 `digest[0..16]` 的 ASCII 字节。
- WeGameWonderfulDb：key 和 IV 都为 `digest[0..16]` 的 ASCII 字节。
- key/IV 使用的是十六进制字符本身，不能再次 hex decode。
- 主列表必须为 `key_wonderful_list_<账户ID>`，值必须是数组。不回退到“任意首个数组”。
- 可选身份列表为 `key_snapshot_list<账户ID>`。
- 数字账户文件不可改名；改名会导致 key 无法派生。外部导入请保留原始文件名，或使用下面的标准 JSON 交换格式。
- 相同列表结构的明文 JSON 导出也可导入；只有明确的列表键会被接受。

只读读取采用普通文件读取，不打开官方数据库写连接。SQLite 文件头会触发“表结构未验证”诊断，并被隔离。本项目 SQLite 仅用于自己的资料库。

## 事件时间

`video_duration`、`clip_sTime` 和 `event_sTime` 都是毫秒；应用 DTO 和播放器时间都使用秒。

| 视频类型 | 接受事件 | 公式 |
| --- | --- | --- |
| `击杀集锦` | `kill` | `event_sTime / 1000` |
| `死亡集锦` | `death` | `event_sTime / 1000` |
| 其他独立时刻 | `kill` | `(clip_sTime + event_sTime) / 1000` |

独立时刻若 `clip_sTime` 缺失或为负数，使用已经落在视频范围内的 `event_sTime`。事件必须在视频时长内，具备 `EventName=Shot`、双方玩家名、匹配的特工名、明确的本地玩家标记、受击部位及可解析的 `EventTime`，事件发生时间须落在对局开始/结束时间的 ±30 秒范围。缺少依据、双方标记矛盾、视频类型不匹配或越界时隔离并报告。事件数量不能当作整局 K/D 统计。

## 外部电脑导入

可以导入视频，再在导入对话框选择：

1. 保持数值文件名的 ACLOS 账户缓存；
2. 带 `key_wonderful_list_<账户ID>` 的明文 JSON；
3. 本项目 v1 交换 JSON，参考同目录 `metadata-v1.example.json`。

原电脑与新电脑路径不同，可以用唯一文件名匹配。若元数据有两个相同文件名且没有完全一致的路径，程序拒绝自动关联，以免把别人的事件套到录像上。

标准交换格式：

```json
{
  "schema": "wegame-moments",
  "version": 1,
  "timeUnit": "seconds",
  "recordings": [
    {
      "file": "clip.mp4",
      "title": "三杀时刻",
      "account": "本地账号",
      "duration": 30,
      "events": [
        { "id": "kill-1", "kind": "kill", "time": 17, "label": "击杀" }
      ]
    }
  ]
}
```

`schema`、`version`、`timeUnit`、`recordings`、每项的 `file` 必填。时间单位必须明确为秒，不能从数字大小猜测。事件类型支持 `kill`、`death`、`assist`、`round`、`unknown`；时间不能为负，提供 duration 时不能超过时长。标题、账户、duration、events 可省略。事件 id 和 label 可省略，适配器提供显示值。相对路径以 JSON 所在目录为基准。文件 `clip.moments.json` 会在扫描 `clip.mp4` 时自动读取。

## 增量更新及限制

适配器以文件路径、长度和修改时间缓存已解析结果；新增或变更账户缓存/身份缓存会失效重读。视频实体和收藏由应用自己的 SQLite 保存。读取上限为每文件 128 MiB；非普通文件、符号链接、远程 URL、未知 schema 和未验证数据库表结构隔离。

没有读取账户登录凭据，没有进行联网账号查询，不处理录制、不注入游戏，不支持云同步或手动标签备注。新格式应添加独立适配器和夹具，不能通过猜字段或改官方文件修复。

## 测试夹具

`tests/fixtures/aclos-plaintext.json` 完全由合成身份和时间组成；两个 `.hex` 夹具由 .NET AES 产生，可独立验证 Rust 解密。测试覆盖 AES256/AES128、错误账户密钥、错误密文、montage 与 moment 时间映射、越界事件、缺失证据、显式秒单位、版本隔离及重名文件关联拒绝。夹具不包含真实个人数据。

9 个适配器 Rust 单元测试已在 Windows GNU 工具链实际运行并全部通过，另验证缓存文件新增/删除会失效重读，以及未知 SQLite 文件经过读取后字节不变。项目内复现命令为 `cargo test --manifest-path src-tauri/Cargo.toml adapters::tests`；开发环境需要 Tauri 2 的 Windows 原生构建依赖。

实际资料库集成验证也已完成：直接编译项目的 `library.rs`、`db.rs`、`models.rs`、适配器和媒体模块，21 个核心测试通过（另一个需要 FFmpeg 的测试由媒体验证流程单独运行）。真实数据扫描在 `work/` 内创建临时应用索引，同时添加两个重复引用同一录像的 ACLOS 来源，分别检查两种添加顺序。两种顺序都得到 1,140 条唯一录像和 12,272 个事件；重复扫描的全部录像 DTO 与 ID 保持一致，索引收藏和自定义分类没有丢失。冷缓存首轮扫描约 16 秒，后续无变化扫描约 0.25–0.32 秒；另一种顺序的首轮在适配器缓存已热时约 6.7 秒。此轮再次验证 21 个官方文件的 SHA256 无变化，测试结束后删除了临时应用数据库，没有发布真实资料库数据。这些是本机开发构建的测量结果，不代表所有电脑的性能。
