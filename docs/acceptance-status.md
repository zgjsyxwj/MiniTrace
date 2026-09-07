# MiniTrace 桌面端验收现状

日期：2026-09-07

## 总体结论

桌面端最小采集闭环、macOS 环境诊断与目标选择、实时协议检查工作台、
会话恢复，以及 Windows 统一后端接入均已通过当前自动化验收。

平台采集能力尚未通过实机验收：macOS 缺少当前微信实例的标准 Frida 附加和
HTTP/HTTPS、WebSocket 双向真实流量证据；Windows 当前没有实机环境。因此，
界面和文档仍分别使用“待实机验收”和“平台接入、未验证”，不能声明为
“支持环境”。

## Ticket 状态

| Ticket | 当前状态 | 已确认范围 | 尚缺范围 |
| --- | --- | --- | --- |
| 01 建立桌面端最小采集闭环 | 自动化验收通过 | 工作区复用、候选选择、开始/ready/停止/摘要、原始证据、历史读取和索引重建 | 不包含真实微信流量能力 |
| 02 完成 macOS 环境诊断与目标选择 | 自动化验收通过 | 支持环境、未知版本、无候选、多候选及人工选择均由固定诊断场景覆盖 | 当前开发机尚未形成合格的真实采集运行链 |
| 03 打通 macOS 真实明文持续采集 | 实现完成，实机验收未通过 | macOS Frida 双明文边界、统一事件、证据校验、正常停止及失败诊断已实现 | 登录、资源请求、双向心跳、移动、切图和战斗六项真实流量证据均缺失 |
| 04 提供实时协议检查工作台 | 自动化验收通过 | 列表、组合筛选、原始字节、冻结、有限窗口和停止后历史读取 | 尚未随真实 macOS 流量执行人工工作台验收 |
| 05 保证长时间采集与会话可恢复 | 自动化验收通过 | 同一会话跨进程重启、多候选暂停选择、异常退出恢复、索引重建、磁盘水位告警和目录边界 | 尚未完成真实微信长时间运行及重启压力验收 |
| 06 接入 Windows 采集后端 | 平台接入验收通过 | 复用 `flue.dll` Python/Frida 路径、统一事件与会话契约、候选筛选和异常收尾 | Windows 真实微信双向流量及正式自包含打包不在当前已验收范围 |

## 本次验证结果

以下命令于 2026-09-07 在当前工作区实际执行：

```text
python3 -m py_compile capture_miniapp_protocol.py                         PASS
python3 capture_miniapp_protocol.py --self-check                          PASS
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check           PASS
cargo test --manifest-path src-tauri/Cargo.toml --offline --no-fail-fast  PASS
cargo check --manifest-path src-tauri/Cargo.toml --offline \
  --target aarch64-apple-darwin                                           PASS
npm test -- --run                                                         PASS
npm run check                                                             PASS
npm run build                                                             PASS
```

测试明细：

- Rust：27 项库测试、1 项端到端 acceptance、3 项诊断集成测试、5 项恢复集成测试，合计 36 项通过；
- 前端：2 个测试文件、6 项测试通过；
- Svelte：0 个错误、0 个警告；
- Vite：生产构建成功。

## 实机验收阻塞

macOS 当前只完成只读环境检查。已知环境为 Apple Silicon、macOS 26.5.2、
微信 4.1.13，WeChatAppEx 带 Hardened Runtime；当前 Python 环境没有
`frida`，且受限终端不能证明目标小游戏进程存在或完成标准附加。详细证据和
六项缺口见 [`issue03-macos-validation.md`](issue03-macos-validation.md)。

Windows 当前没有可用于验收的微信实例。平台接入边界见
[`windows-capture-backend.md`](windows-capture-backend.md)。

下一次可以提升平台结论的验收，必须保存同一或连续采集会话中的真实原始观察、
目标模块身份、连接、方向、长度和 SHA-256，并逐项标注登录、资源请求、
双向心跳、移动、切图和战斗对应的帧号。fixture、静态分析和自检结果不能替代
这些证据。
