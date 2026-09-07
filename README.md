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
