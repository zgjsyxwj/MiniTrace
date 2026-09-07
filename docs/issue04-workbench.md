# issue 04 验收记录：实时协议检查工作台

日期：2026-09-07

## 实现边界

- `src/lib/workbench.ts` 集中定义实时记录、消息号/协议名/方向/未分类筛选、
  有界窗口和冻结快照状态。
- `src/App.svelte` 展示实时协议列表、筛选控件、冻结/恢复和滚动跟随状态，
  以及选中记录的分段、连接、原始文件、SHA-256 和完整字节。
- `read_raw_bytes` 是只读 Tauri 命令，只允许读取当前会话 `raw/` 目录；原始
  文件和 `timeline.jsonl` 仍是权威证据，窗口淘汰不会触碰它们。
- fake backend 固定产生四条 WebSocket 双向协议帧和两条 HTTP 请求/响应未分类
  明文，供公开命令验收使用。该 fixture 不代表真实微信流量能力。

## 逐项验收

| 条目 | 结果 | 证据 |
| --- | --- | --- |
| 实时列表显示时间、方向、消息号、协议名、长度、哈希 | 通过 | `App.svelte` 工作台表格；fake 帧事件包含四组字段 |
| 消息号、协议名、请求、响应、未分类筛选 | 通过 | `src/lib/workbench.test.ts` 的筛选测试；Rust acceptance 检查 10504/16001、协议名及双向未分类事件 |
| 原始字节、采集分段和原始文件定位 | 通过 | `read_raw_bytes` 命令、工作台详情面板；Rust acceptance 读取 `raw/` 并校验长度/十六进制长度 |
| 冻结或滚动时后台继续接收并写盘 | 通过 | `LiveViewState` 将 `records` 与 `visibleRecords` 分离；冻结测试验证新记录仍进入后台窗口并累计待查看数量 |
| 有限窗口与证据保留 | 通过 | 默认窗口 120；单元测试验证窗口淘汰旧显示项且不修改输入记录，后端会话仍保存全部观察 |
| 停止后历史读取 | 通过 | Rust acceptance 正常停止后读取 `get_session`、重建索引和原始字节 |

## 验证命令

```text
npm test -- --run                  PASS（6 tests）
npm run check                      PASS（0 errors/warnings）
npm run build                      PASS
cargo fmt --manifest-path src-tauri/Cargo.toml --all  PASS
cargo test --manifest-path src-tauri/Cargo.toml --offline --no-fail-fast  PASS
```

真实 macOS 微信双向流量尚未完成 issue 03 的实机验收；本 issue 的工作台和
fixture 只验证统一事件契约及本地证据读取，不扩大平台采集能力声明。
