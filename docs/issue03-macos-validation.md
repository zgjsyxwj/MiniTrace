# issue 03 验收证据：macOS 真实明文持续采集

日期：2026-09-07

## 结论

实现已完成，真实平台能力尚未验收。当前没有可提交的《世界 Online》
小游戏运行链、Frida 标准附加结果或 HTTP/WebSocket 双向业务流量原件，
因此不宣称 issue 03 的实机验收通过。

## 已完成实现

| 项目 | 证据 |
| --- | --- |
| macOS 后端启动和停止 | `src-tauri/src/macos_backend.rs` 的 `MacosLaunchSpec`、停止文件和正常收尾路径 |
| Frida 双边界 agent | `capture_miniapp_protocol.py` 的 `MACOS_AGENT`，缺少 HTTP 或 WebSocket 任一方向即 `attach-failure` |
| HTTP/HTTPS 明文 | `transport: http`，保留 XWeb/Flue 解密边界原始观察；不把 TLS 密文当作明文 |
| WebSocket 明文 | `transport: websocket`，复用 WebSocket 分片重组和游戏帧拆分 |
| 原始观察 | staging `observations/*.bin` → 会话 `raw/`，校验相对路径、长度和 SHA-256 |
| 未分类载荷 | `unparsed_payload` 保留在 staging；统一事件以带分类的 `raw_observation` 发布 |
| 统一事件 | `capture://event` 的 `ready`、`raw_observation`、`protocol_frame`、`stopped`、`summary`、`error` |
| 安全边界 | 不关闭 SIP，不修改/重签微信，不自动切换 HTTPS 代理，不发送业务请求 |

## 只读环境检查

执行命令：

```text
uname -a
uname -m
sw_vers -productVersion
plutil -p /Applications/WeChat.app/Contents/Info.plist
codesign -dv --verbose=4 /Applications/WeChat.app/Contents/MacOS/WeChatAppEx.app/Contents/MacOS/WeChatAppEx
python3 -c 'import importlib.util; print(bool(importlib.util.find_spec("frida")))'
```

实际结果：

- Darwin 25.5.0，`arm64`；
- macOS `26.5.2`；
- WeChat `CFBundleShortVersionString=4.1.13`；
- WeChatAppEx 为 Apple Silicon/Universal Mach-O，代码签名包含
  `flags=0x10000(runtime)`；
- 当前 Python 没有 `frida` 模块；
- 受限终端读取进程清单返回 `operation not permitted`，无法证明目标
  进程存在或完成标准附加。

## 实机验收缺口

以下每项都需要同一或连续采集会话中的真实原始观察、目标模块身份、
连接/方向和 SHA-256；当前均为“未验收”：

1. 登录请求和响应；
2. 资源/HTTP 或 HTTPS 请求和响应；
3. WebSocket 双向心跳；
4. 移动；
5. 切图；
6. 战斗。

没有目标实例时，不能用现有 Windows flue 会话、fixture、静态二进制分析
或 Python self-check 替代这些证据。完成后应补充会话目录、模块身份、
微信版本、CPU 架构、Frida 授权结果、双向边界覆盖和六项操作的帧号。

## 自动化结果

```text
python3 -m py_compile capture_miniapp_protocol.py       PASS
python3 capture_miniapp_protocol.py --self-check        PASS
cargo test --manifest-path src-tauri/Cargo.toml --offline PASS (27 lib + 1 acceptance + 3 diagnostics + 5 recovery)
cargo check --manifest-path src-tauri/Cargo.toml --offline --target aarch64-apple-darwin PASS
npm run check                                            PASS
npm test -- --run                                        PASS (6 tests)
npm run build                                            PASS
```
