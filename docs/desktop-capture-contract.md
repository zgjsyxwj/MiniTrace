# MiniTrace 桌面端最小采集契约

issue 01 建立 Tauri 2 命令与事件边界。后续平台后端只替换命令背后的
backend，不改变前端使用的载荷和会话文件模型。

macOS 后端使用 `macos-frida` 标识，启动
`capture_miniapp_protocol.py --transport macos`，要求 XWeb/Flue 的 HTTP
和 WebSocket 请求、响应四个明文方向均成功安装 Hook。未完成真实双向
流量验收时，界面和验收文档只能写“待实机验收”，不能把模块存在或
`ready` 单独当作平台采集能力。

Windows 后端使用 `windows-flue` 标识，候选来自 `WeChatAppEx.exe`/`WeApp.exe`
及其 `flue.dll` 模块诊断，并复用项目既有的 Python/Frida flue 采集器和拆帧
逻辑。Windows 当前只有“平台接入、未验证”状态；没有真实 Windows 微信验收
证据时，诊断不会开启正式采集会话。

## 公开命令

| 命令 | 输入 | 输出 | 说明 |
| --- | --- | --- | --- |
| `get_app_state` | 无 | `AppSnapshot` | 返回已保存工作区和当前采集状态 |
| `set_workspace` | `{ path }` | `WorkspaceInfo` | 创建、保存并切换采集工作区 |
| `pick_workspace` | 无 | `WorkspaceInfo` | 打开系统目录选择器并复用 `set_workspace` 语义 |
| `diagnose_environment` | 无 | `EnvironmentDiagnosis` | 只读检查 macOS、CPU、微信版本、角色和关键模块；不创建会话 |
| `list_candidates` | 无 | `CandidateProcess[]` | 返回当前诊断中已通过资格检查的目标进程 |
| `start_capture` | `{ candidateId }` | `CaptureSessionInfo` | 建立会话并异步启动 worker |
| `stop_capture` | 无 | `SessionSummary` | 通知 worker、等待正常结束和最终摘要 |
| `list_sessions` | 无 | `SessionSummary[]` | 从工作区读取已完成会话 |
| `get_session` | `{ sessionId }` | `SessionDetail` | 读取会话摘要、原始观察索引和协议帧索引 |
| `read_raw_bytes` | `{ sessionId, rawFile }` | `RawBytes` | 只读取当前会话 `raw/` 下的原始字节并校验路径 |
| `rebuild_session_index` | `{ sessionId }` | `SessionIndex` | 从 `timeline.jsonl` 重建查询索引 |

命令的错误统一通过 Tauri `Result` 返回字符串。未设置工作区、候选不存在、
已有采集或会话编号不合法时，命令不会创建新的正式会话。

`diagnose_environment` 返回当前环境的 `mode`、`captureAllowed`、候选进程和
诊断原因。只有 `mode: "supported"` 且用户明确传入某个候选 `candidateId`
时，`start_capture` 才允许建立正式采集会话。未知或未验证的微信版本、非
Apple Silicon、缺少 `WeChatAppEx Framework`/Flue/XWeb 明文边界时，结果为
兼容性诊断，不创建会话。多个合格候选会设置
`requiresCandidateConfirmation: true`，界面必须要求用户选择运行链。

## 事件

所有采集事件使用同一个名称 `capture://event`，payload 结构如下：

```json
{
  "kind": "started | ready | raw_observation | protocol_frame | stopped | summary | error",
  "sessionId": "session-…",
  "timestampMs": 1760000000000,
  "data": {}
}
```

一次正常 fake backend 采集的顺序为：

```text
started → ready → raw_observation → protocol_frame → stopped → summary
```

`raw_observation` 保存边界、分段、连接、方向、传输类型、长度、SHA-256 和
原始文件相对路径。macOS 每次 Frida 明文回调先保存完整
`observations/*.bin`，再归档至会话 `raw/`；HTTP 无法拆帧时带
`unparsed_payload` 分类。`protocol_frame` 在对应原始观察之上增加消息号和协议名。
多个边界产生的相同字节不能通过事件层静默去重。

## 工作区文件

工作区由 `sessions/<sessionId>/` 分隔会话。完成一次正常采集后至少包含：

```text
session.json               # 会话清单与文件路径
summary.json               # 最终摘要
index.sqlite               # 可删除、可由时间线重建的查询索引
timeline.jsonl             # 追加式权威事件时间线
transport-events.jsonl     # 传输边界事件
raw/000001-observation.bin # 原始字节
```

原始文件和时间线是权威证据；`index.sqlite` 仅供查询。索引事务完成后使用
临时文件加 `rename`，避免正常停止时发布半个 SQLite 文件。

## 实时协议检查工作台

工作台把 `protocol_frame` 和明确标记为 `unparsed_payload` 的
`raw_observation` 转换为实时视图记录，展示时间、方向、消息号、协议名称、
长度和 SHA-256。消息号与协议名称是独立的包含筛选条件，方向可筛选为请求或
响应，也可以只查看未分类明文载荷。

实时窗口默认最多保留 120 条记录。冻结或滚动离开最新位置时，只冻结当前
可见快照；事件接收、后台采集和磁盘追加继续运行，恢复实时视图后显示最新
窗口。窗口淘汰只丢弃 UI 投影，不删除 `raw/`、`timeline.jsonl` 或索引中的
任何原始观察。

选中实时或历史记录后，界面使用 `read_raw_bytes` 读取完整原始字节，并同时
显示采集分段、连接、传输边界、SHA-256 和会话内原始文件相对路径。该命令
拒绝绝对路径、父目录和 `raw/` 之外的文件。

## 本地运行和验收

```bash
npm install
npm run tauri dev
cargo test --manifest-path src-tauri/Cargo.toml --offline
npm test
npm run check
npm run build
```

Rust 集成测试通过 `tauri::test::mock_app` 注册 `AppState`，调用上述公开命令，
并记录 `capture://event` 的事件序列；不依赖真实微信或内部 Svelte 组件状态。
