# 世界村在线 NPC 获取与本地资源

2026-09-06 已通过用户正常重启、进村及 NPC 对话操作取得数据，并完成资源下载与本地组合预览。没有修改恢复版 NPC 配置；本次验收是数据、资源和离线预览，不是 Android 运行集成。

## 实测清单

会话：`captures/miniapp-protocol/session-20260906-090442/`，传输 flue，网络 PID `20764`（只适用于本轮）。81 个游戏帧、2 个未解析载荷，83 个原始文件 SHA-256 全部通过。按忽略列表跳过 14 帧；30 种其他消息职责未确认，保留在 [完整时序分析](../captures/miniapp-protocol/session-20260906-090442/analysis.json)，未加入忽略列表。采集器摘要 fatalError 为 null、timedOut 为 false；终端 Ctrl+C 返回码为 1，但摘要已写出。

登录响应 `2` 中解析出 10 个 NPC 对象。运行时地图 `30555`、原始地图 `302`、出生位置 `(13,8)`、dataFlag `5315966`。完整帧偏移 36 起为世界数据头；NPC 块位于 `[2840,3542)`，前三字节 `00 02 bb` 对应后续 699 字节，与解析终点一致。`10503` 后续只返回同地图与坐标，dataFlag 为 0。不能把这次首次列表归到 `10519`。

| ID | 名称 | 模型 | 格坐标 |
| ---: | --- | ---: | --- |
| 1 | 世界村村长 | 5003 | (12,12) |
| 2 | 云裳 | 15090 | (16,10) |
| 3 | 东海 | 5024 | (23,7) |
| 4 | 小悦 | 5026 | (27,17) |
| 5 | 悲剧哥 | 5012 | (13,21) |
| 6 | 轩姐 | 318 | (5,17) |
| 7 | 碰我进入战斗 | 600 | (26,10) |
| 8 | 井 | 1223 | (19,17) |
| 11 | 小偷 | 15073 | (22,20) |
| 12 | 碰我进行传送 | 1338 | (9,19) |

完整的主对话、状态、任务条件和菜单见 [NPC 清单](../captures/miniapp-protocol/session-20260906-090442/npcs.json)。这是一份服务端返回的对象集合，不代表 10 个对象在任意任务阶段都可见；战斗对象和传送门存在任务条件。

NPC `4` 返回任务 `1105/1106`，村长 `1` 返回任务 `1109`，NPC `5` 返回任务 `1107`。任务对象原始二进制已独立保留，当前只解析任务 ID、标题及包围结构，任务内部其余字段未完整命名。

## 下载与验证

按已保存 v63 版本清单下载 10 个模型及片段，共 67 个文件、55,999 字节；没有猜测 CDN 文件名。全部 JSON/PNG 解析通过，所有 78 帧的片段引用和动作帧索引通过；首帧预览已目视检查。

- [资源目录](../offline-source/wechat-miniapp/v63/assets/sprite/)
- [下载地址、大小与 SHA-256 清单](../offline-source/wechat-miniapp/v63/downloads/npc-resources-report.json)
- [验证结果](../captures/miniapp-protocol/session-20260906-090442/resource-validation.json)
- [本地预览](../captures/miniapp-protocol/session-20260906-090442/npc-preview.png)
- [Android 内置资源审计](../captures/miniapp-protocol/session-20260906-090442/android-resource-audit.json)

Android APK 已有并可解析：318、600、1223、1338。内置模型定义缺失：5003、15090、5024、5026、5012、15073；项目文件搜索也未找到这些编号的独立 SPR。该审计覆盖本地 APK 和源码规定的资源包路径，不代表读取过在线进程的全部缓存。

小游戏 JSON/图集不能直接当作 Android SPR/FR。下载原件已齐备；接入恢复版前仍需按 Android Smali 核对转换格式及模型入口。小游戏原始地图 302 和 Android 旧快照的地图布局、坐标与任务阶段不同，不能把两份坐标直接合并到当前离线地图。

## 可重复步骤

使用带 Pillow 的本地 Python 运行；所有输出保存在本项目内：

```powershell
python parse_miniapp_npcs.py --self-check
python parse_miniapp_npcs.py captures/miniapp-protocol/session-20260906-090442/frames/000011-response-2.bin --offset 2840 --output captures/miniapp-protocol/session-20260906-090442/npcs.json
python download_miniapp_npc_resources.py --self-check
python download_miniapp_npc_resources.py captures/miniapp-protocol/session-20260906-090442/npcs.json
python validate_miniapp_npc_resources.py
```

解析器偏移参数必须由原始帧和调用链确认，不能用于其他帧的固定偏移。预览校验器明确固定本次样本。下载器固定读取本地版本响应选择清单；新版本需重新保存版本响应及清单，不能把 v63 当作永久最新版。已有文件会先解析，不会覆盖原件。资源报告中的 existing 表示复用，此次原始下载报告均为 downloaded。

## 获取链路依据

[小游戏主包](../wechat-miniapp/subpackages/main/main.min.js)：`117000` 附近登录成功处理器调用 `processDataBlockFlagMsg`；`20768` 解析世界头和数据标志；`20843` 的 `DATA_NPC_BASE=1024` 调用 `parseNPCData`；`53557/53592` 解析列表和对象；`53509` 通过 `14501` 按需补任务。`53252` 的 `10519` 补取调用来自国家建筑入口，不能假定普通世界村会调用。

资源：`25281` 路径定义，`46529` 的 `getRealUrl` 根据清单哈希映射真实 URL（ad 前缀变为 dirad）；`61763/61790` 读取模型和 files，`104219` 读取片段。模型 JSON、片段 PNG 及图集配置独立落盘。

## 采集失败教训

此前 WinHTTP 会话 `session-20260906-085827` 只有 ready 记录，没有业务帧，[失败记录](../captures/miniapp-protocol/session-20260906-085827/review-result.json) 已保留。用户明确指定 flue 后重新采集；启动时先确认双向心跳真实落盘，再请用户重启小游戏。进程存活或钩子 ready 都不能替代此检查。没有修改项目采集技能的传输选择规则。
