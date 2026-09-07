# 微信小游戏桌面抓包应用参考包

建议阅读顺序：

1. `CURRENT_SESSION.md`
2. `CONTEXT.md`
3. `.agents/skills/miniapp-protocol-capture/SKILL.md`
4. `docs/微信小游戏明文协议采集.md`
5. `wechat-miniapp/网络分析.md`
6. `docs/miniapp-protocol-useful-list.md`
7. `docs/miniapp-protocol-ignore-list.md`

核心实现入口：

- `capture_miniapp_protocol.py`：被动持续采集。
- `request_miniapp_protocol.py`：复用当前连接主动发送已确认请求。
- `wechat-miniapp/api-cg-index.csv`：消息编号索引。

代表性证据位于 `captures/miniapp-protocol/`。完整历史证据仍以原项目目录为准。

## 桌面端 MiniTrace

当前仓库包含一个 Tauri 2 + Rust + Svelte 5 + Tailwind CSS 4 桌面端最小闭环。
它使用可控的假采集后端建立公开命令、事件和会话文件契约，不能据此宣称已经
通过真实微信环境验收。先选择采集工作区，再选择候选进程即可体验开始、实时
协议检查工作台、正常停止和历史读取流程。工作台的实时窗口有界，冻结或淘汰
只影响显示，不删除会话中的原始证据。

```bash
npm install
npm run tauri dev
```

Rust 端到端验收和前端检查：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --offline
npm run check
npm run build
```

公开边界和会话文件结构见 [`docs/desktop-capture-contract.md`](docs/desktop-capture-contract.md)。
实时工作台验收记录见 [`docs/issue04-workbench.md`](docs/issue04-workbench.md)。
桌面端各 Ticket 的统一验收现状见 [`docs/acceptance-status.md`](docs/acceptance-status.md)。
