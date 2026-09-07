# Windows `flue.dll` 后端接入

MiniTrace 的 Windows 后端只负责平台适配。它复用仓库已有的
[`capture_miniapp_protocol.py`](../capture_miniapp_protocol.py)；Python/Frida
继续负责 `flue.dll` hook、WebSocket 重组、`split_game_frames` 拆帧、协议号
名称映射和未分类明文保存。Rust 适配层负责候选诊断、停止控制、证据路径
安全校验，以及把 Python 产物转换成统一的 `capture://event` 和会话文件。

## 候选诊断

Windows 端只把同时满足以下条件的进程列为候选：

- 进程角色为 `WeChatAppEx.exe` 或 `WeApp.exe`；
- 模块清单包含 `flue.dll`；
- PID 非零。

进程角色和模块清单来自只读的 `tasklist /m flue.dll /fo csv /nh`。主进程
`WeChat.exe` 不会被选作采集目标。候选 ID 使用 `windows-flue-pid-<PID>`，
供公开命令边界传递，不把首个 PID 当作目标。

## 采集流程

Windows 后端启动的命令等价于：

```text
python capture_miniapp_protocol.py \
  --transport flue \
  --pid <candidate.pid> \
  --output-dir <session>/windows-flue \
  --allow-external-output \
  --stop-file <session>/windows-flue/stop.requested \
  --flue-module flue.dll \
  --flue-write-rva 0x573550 \
  --flue-read-rva 0x4c06990
```

适配层轮询 Python 的 `ready.json` 与 `timeline.jsonl`。每个已拆出的协议帧
和未分类明文先校验原始文件的长度与 SHA-256，再写入统一会话的 `raw/`，
同时生成 `raw_observation` 事件；协议帧另外生成 `protocol_frame` 事件。
路径只允许位于本次会话临时目录，禁止 `..` 和绝对路径逃逸。

停止命令创建 `stop.requested`，等待 Python 自己写出 `summary.json` 并退出，
再发布 `stopped`、`summary`。Python 进程异常退出或停止超时时，已有证据以
`incomplete` 会话保留，错误通过统一 `error` 事件报告。

## 验收边界

当前开发机没有 Windows 微信实例。Windows 后端的进程筛选、启动参数、Python
时间线归一化和不完整会话路径均由 fixture/单元测试覆盖，但这不等于平台
采集能力。真实 Windows 微信版本、双向流量和正式自包含打包仍是后续工作。
界面在真实验收完成前固定显示“平台接入、未验证”。
