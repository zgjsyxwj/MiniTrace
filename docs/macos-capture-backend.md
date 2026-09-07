# macOS Frida 双明文边界后端

MiniTrace 的 macOS 后端由 `src-tauri/src/macos_backend.rs` 启动仓库根目录
的 `capture_miniapp_protocol.py --transport macos`。Python/Frida 只在用户
明确授权的目标进程内被动观察，不发送业务请求，不修改目标内存中的业务
数据。Rust 侧负责停止、校验和发布统一会话。

## 边界与资格

macOS 采集必须同时具备以下四个入口：

- XWeb/Flue HTTP 明文写入（请求）和读取（响应）；
- XWeb/Flue WebSocket 明文写入（请求）和读取（响应）。

`MACOS_AGENT` 只接受显式配置的已核对符号，或目标 Framework 符号表中
安全别名的精确匹配。任一入口找不到时，agent 发送 `attach-failure`，不
发送 `ready`。`ready.json` 只有在 `coverage.http`、`coverage.websocket`
均为 `true` 时才会被 Rust 后端接受。

macOS 内部符号和地址会随微信构建变化。后端支持通过环境变量配置已核对
的符号：

```text
MINITRACE_MACOS_MODULE
MINITRACE_MACOS_HTTP_WRITE_SYMBOL
MINITRACE_MACOS_HTTP_READ_SYMBOL
MINITRACE_MACOS_WEBSOCKET_WRITE_SYMBOL
MINITRACE_MACOS_WEBSOCKET_READ_SYMBOL
```

不因为看见 `WeChatAppEx Framework`、Flue 或 XWeb 模块就把平台标记为
“支持环境”；模块存在只代表候选进程检查通过。必须再有双向 HTTP/HTTPS
和 WebSocket 的真实字节证据，才能更新平台验收结论。

## 统一事件和原始证据

每次 Frida 明文回调先写入 staging 目录的 `observations/*.bin`，并在
`timeline.jsonl` 写入 `kind: raw_observation`。随后：

- WebSocket 字节按 WebSocket 分片重组，再按既有游戏帧格式拆出
  `protocol_frame`；
- HTTP/HTTPS 解密后的字节不套 WebSocket 头，无法拆帧的部分保持为
  `unparsed_payload`；
- 每条统一观察都保留连接、方向、传输类型、采集分段、时间、长度和
  SHA-256。多边界产生相同字节时不静默去重。

Rust 在复制证据前检查相对路径、长度和 SHA-256，最终把原始字节写入会话
的 `raw/`，通过单一 `capture://event` 发送 `raw_observation` 和
`protocol_frame`。原始文件和时间线是权威证据，SQLite 只是可重建索引。

## 停止和失败

`stop_capture` 创建 staging 下的 `stop.requested`，等待 Python 自己写出
`summary.json` 并退出；正常路径才发布 `stopped`、`summary`。标准附加、
Frida 模块、目标进程或 Hardened Runtime 出错时，已有证据发布为
`incomplete`，同时保留错误码和说明。不能用强杀、只读 TLS 密文或单一
WebSocket Hook 充当正常完成。

MiniTrace 不关闭 SIP，不修改或重签微信，不绕过证书固定。若标准 Frida
附加失败，代理路径需要另行决策和明确授权，本 Ticket 不自动切换。

## 当前实机验收记录（2026-09-07）

当前开发机只完成只读预检：

- macOS `26.5.2`，`arm64`；
- `/Applications/WeChat.app` 的 `CFBundleShortVersionString` 为 `4.1.13`；
- WeChatAppEx 二进制与 `WeChatAppEx Framework` 为 Apple Silicon/Universal
  Mach-O，并带 Hardened Runtime；
- 受限终端无法读取运行进程清单，Python 环境未安装 `frida`，当前没有
  可验证的小游戏运行链和标准附加结果。

因此 issue 03 的登录、资源、双向心跳、移动、切图、战斗六项真实流量
验收仍为“未验收”。以上实现和 fixture 测试不构成真实支持环境证据。

## 可重复检查

```bash
python3 -m py_compile capture_miniapp_protocol.py
python3 capture_miniapp_protocol.py --self-check
cargo test --manifest-path src-tauri/Cargo.toml --offline
cargo check --manifest-path src-tauri/Cargo.toml --offline --target aarch64-apple-darwin
npm run check
npm test -- --run
npm run build
```
