# 微信小游戏已采集的有用协议列表

本文是微信小游戏动态明文协议的长期索引。只记录对 Android 恢复版角色、场景、任务、战斗、背包和伙伴玩法有直接价值，且已有运行时样本的消息。

原始帧是证据，字段含义以动态样本和小程序读写代码共同确认。小程序帧不能直接重放给 Android；实现前仍需核对 Android DEX/Smali。

## 追加规则

- 每个消息号只维护一节。新样本追加到该节的“代表性样本”，不新建重复说明。
- 只有覆盖新分支、新字段、不同结果、非空列表或明显不同数据形态的样本才追加。完全相同的响应不重复登记。
- 每个样本记录操作、请求值、响应值、原始帧路径和 SHA-256。未解析字段明确标记为“未确认”。
- 失败响应必须单独保留，不能用成功样本推断失败布局。
- 明确不需要的协议登记到[忽略协议列表](miniapp-protocol-ignore-list.md)。

## 总览

| 消息号 | 协议 | 已覆盖内容 | 当前价值 |
| ---: | --- | --- | --- |
| `10002` | `CG_LOGIN_CREATEACTOR` | 创建成功 | 本地角色初始字段和外观样本 |
| `10503` | `CG_SCENE_WORLDDATA` | 切图后的世界数据 | 运行时地图与原始地图双 ID |
| `10504` | `CG_SCENE_PLAYER_MOVING` | 多个目标格上报 | 本地移动完成后的坐标持久化 |
| `10506` | `CG_SCENE_GO_MAP` | 成功切图 | 地图切换入口和数据块结构 |
| `10518` | `CG_SCENE_FINDPATH_MISSION` | 六个主线目标 | 任务编号与目标格样本 |
| `11004` | `CG_ACTOR_ATTRIBUTE_UPDATE` | 战后属性刷新 | 属性 ID 与绝对值更新格式 |
| `12501` | `CG_FIGHT_ENTER_LOCALBATTEL` | 本地战斗进入成功 | 怪物组到战斗入口的确认链 |
| `12502` | `CG_FIGHT_RUN_LOCALBATTLE` | 两场胜利结算 | 本地战斗结果、奖励和任务击杀进度 |
| `14501` | `CG_TASK_GETMISSIONDATA` | NPC 任务、怪物组和怪物数据 | 恢复 NPC 可接/可交任务 |
| `14502` | `CG_TASK_ACCEPT` | 五次成功接取 | 本地任务接取状态转换 |
| `14503` | `CG_TASK_DELIVER` | 六次成功提交 | 奖励结算和下一主线推进 |

## `10002` 创建角色

请求字段顺序：

| 字段 | 类型 | 代表值 |
| --- | --- | --- |
| `sex` | byte | `0` |
| `race` | byte | `1` |
| `job` | byte | `4` |
| `icon1` | long | `2237346` |
| `icon2` | long | `0` |
| `icon3` | long | `0` |
| `name` | string | `斯特特里古` |
| `clientType` | string | `"1"` |

成功响应先读 `result=0`，再返回角色：`id=2586841`、同名、`sex/race/job=0/1/4`、`level=1`、`vipLevel=0`、`mapName=世界村`、`icon1/2/3=2237346/6208/2957952`、`status=0`。请求和响应均无剩余字节。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 创建角色成功 | [`01-request-10002.bin`](../captures/miniapp-protocol/create-role/session-20260905-flue/01-request-10002.bin)，SHA-256 `14bc72fa841a8e8a6a964aa28f6deed62f6360faf30d5e635278ff17d726442b` | [`02-response-10002.bin`](../captures/miniapp-protocol/create-role/session-20260905-flue/02-response-10002.bin)，SHA-256 `9a40b453d1040b7834b62dceac86821b9a5ba5ea197aef97212131ed8df1be9a` |

## `10503` 世界数据

已确认请求字段：运行时地图 `short=30680`、`boolean=true`、`int=0`、两个零 byte。响应为 `result=0`、运行时 `mapId=30680`、原始 `orgMapId=900`、位置 `(20,8)`、`dataFlag=0`。

这组样本只证明双地图 ID 机制。`30680` 是在线运行时 ID，不能写入离线长期存档。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 切图后获取世界数据 | [`000089-request-10503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000089-request-10503.bin)，SHA-256 `55b5acc5ef683ba868a61e0feb051c66ecf9954991b500e229e32eb9e1dee294` | [`000090-response-10503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000090-response-10503.bin)，SHA-256 `bf9251afff4b189a5f31ca677c05810a24ed4a274ea3b70c1947856c764769c7` |


新增地图样本：2026-09-06 重新登录后收到运行时地图 `30555`、原始地图 `302`、位置 `(13,8)`、`dataFlag=0`。此前登录响应 `2` 已含完整 NPC 列表，本帧不重复下发。原始响应 [000030-response-10503.bin](../captures/miniapp-protocol/session-20260906-090442/frames/000030-response-10503.bin)，SHA-256 `576ba4dc56ac2151df60a6b445fccf1f4ea99fb5766cb2b0eddfe8709503bbc6`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"runtimeMapId": 30942, "flag": true, "dataFlag": 0, "byte1": 0, "byte2": 0}（完整解析）：[000030-request-10503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000030-request-10503.bin)，SHA-256 `35d0e16a8649b0ae6c084d6b40ce85910a924ddc45304cf4e6740f90ec38e6d5`。
- response {"result": 0, "runtimeMapId": 30942, "mapId": 4, "x": 29, "y": 6, "dataFlag": 0}（完整解析）：[000032-response-10503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000032-response-10503.bin)，SHA-256 `bd547faeb9a3eac1f7a95dddcdf13886649c1567d0c9dd786e95c81c5324717a`。
- request {"runtimeMapId": 30943, "flag": true, "dataFlag": 0, "byte1": 0, "byte2": 0}（完整解析）：[000315-request-10503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000315-request-10503.bin)，SHA-256 `ec9edc62c85bb4b8ae91899d4329d59cc22317107f87ebd357c1549f1646a847`。
- response {"result": 0, "runtimeMapId": 30943, "mapId": 16, "x": 3, "y": 44, "dataFlag": 0}（完整解析）：[000316-response-10503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000316-response-10503.bin)，SHA-256 `6c43c08190a48228bd78cd5bea48b30a890adb7472da7a7e6e8f2da399289f34`。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"runtimeMapId": 30944, "flag": true, "dataFlag": 0, "byte1": 0, "byte2": 0}（完整读取）：[000231-request-10503.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000231-request-10503.bin)，SHA-256 `3bfbcfcbab5c23f444624693557adb0edec7aa01a96eff18bf6be92de6381ea9`。
- response {"result": 0, "runtimeMapId": 30944, "mapId": 4, "x": 36, "y": 33, "dataFlag": 0}（完整读取）：[000232-response-10503.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000232-response-10503.bin)，SHA-256 `f8ea2404703600f2fa7f461a480b93d22fae33f8d82299eccfe7de00e296097c`。
- request {"runtimeMapId": 30945, "flag": true, "dataFlag": 0, "byte1": 0, "byte2": 0}（完整读取）：[000258-request-10503.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000258-request-10503.bin)，SHA-256 `7ba5989e7b8f0b23ea1de86afa0afddeaa644a2212a076b12a12932f812676bd`。
- response {"result": 0, "runtimeMapId": 30945, "mapId": 16, "x": 3, "y": 44, "dataFlag": 0}（完整读取）：[000260-response-10503.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000260-response-10503.bin)，SHA-256 `519e4ad41a084f69c4081fd91648387c428b7517df29f0b73f4973a245f10567`。

## `10504` 玩家移动

请求体只有目标格 `x:byte, y:byte`。已采集 `(22,21)`、`(22,19)`、`(15,21)`、`(13,13)`、`(6,17)` 和 `(21,11)`。没有同号响应。

离线用途是角色完成合法移动后保存最新格坐标。该消息不能证明服务端没有碰撞或位置纠正。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 移动到 `(22,21)` | [`000015-request-10504.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000015-request-10504.bin)，SHA-256 `b386a509943f830028755ec7d6367282bb4a7924fc6d30d269a6b4cb8964bf99` | 无同号响应 |
| 移动到 `(21,11)` | [`000045-request-10504.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000045-request-10504.bin)，SHA-256 `c45dd2c8e0e246646b4bc8d8e64a0ec212aba6762812c633556ae85d276d3e92` | 无同号响应 |

## `10506` 切图

请求字段：目标原始地图 `short=900`、目标格 `(20,8)`、`dataFlag=7232`、`boolean=false`。

成功响应头：`result=0`、运行时 `mapId=30680`、原始 `orgMapId=900`、位置 `(20,8)`、`dataFlag=7488`。后续 7488 标志块包含角色、任务、背包、技能、地图、NPC、怪物等数据，尚未逐字段完成解码。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 切到原始地图 `900` | [`000087-request-10506.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000087-request-10506.bin)，SHA-256 `c57b979fec043ff656137fcdd971f1b57fe20fb4ec9a926fbc70f3c7123bf145` | [`000088-response-10506.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000088-response-10506.bin)，SHA-256 `5a436aa0ad479e4e237e85a927c48cb7da3a27d66ab4ce632ced33fe828fcb3a` |

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

NPC 块分别完整解析 23、36 个对象；怪物组、怪物及空 AI 表已完整解码。神剑广场稳定编号为 16，30943 为本次在线运行时编号。

- request {"mapId": 4, "x": 29, "y": 6, "dataFlag": 7232, "flag": false}（完整解析）：[000028-request-10506.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000028-request-10506.bin)，SHA-256 `c04cb344e8e05155d5bf532f89bde7b22cc93d278aae6cb53c2ba61cf84773f9`。
- response {"result": 0, "runtimeMapId": 30942, "mapId": 4, "x": 29, "y": 6, "dataFlag": 7488, "name": "迎客殿", "description": "如果你知道该如何进行任务，可以点选任务列表查看。", "mapByte": 0, "npcCount": 23, "npcOffset": 82, "npcEndOffset": 1417, "groupIds": [2012, 2013, 2015, 2016], "monsterIds": [2016, 2019, 2017, 2049, 2020, 2048, 2021, 2047], "skillCount": 0, "aiCount": 0}（完整解析）：[000029-response-10506.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000029-response-10506.bin)，SHA-256 `f3536bc94da4e4a03fac447fcaee721e40fcf59a5719f7ef75d2a6414b24b526`。
- request {"mapId": 16, "x": 3, "y": 44, "dataFlag": 7232, "flag": false}（完整解析）：[000313-request-10506.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000313-request-10506.bin)，SHA-256 `7df2205d05acea768cecee45091b945591c0a92c7c8eae171c5b2d0fce0a32f3`。
- response {"result": 0, "runtimeMapId": 30943, "mapId": 16, "x": 3, "y": 44, "dataFlag": 7488, "name": "神剑广场", "description": "等级提升后，会获得3点属性点。可在人物菜单中提升哦。", "mapByte": 0, "npcCount": 36, "npcOffset": 88, "npcEndOffset": 2663, "groupIds": [2019, 2018, 2021, 2020, 2024, 2047, 2023], "monsterIds": [2023, 2022, 2019, 2029, 2050, 2025, 2032, 2031, 2046], "skillCount": 0, "aiCount": 0}（完整解析）：[000314-response-10506.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000314-response-10506.bin)，SHA-256 `72ce01c6c08281b4e79cbc179a7316a29e7d656f31a18d526bf1ee0a99313454`。

后续解码校验（同一批原始帧）：本轮 10506、14501、14502、14503 和 12502 响应均已完整解析到帧尾，覆盖怪物、战斗组、实际奖励物品、升级和 NPC 标记；12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量，原始字节保留；此前未确认边界已消除。以上更新替代本节本轮早期的“尾部未完整解码”边界。完整字段和原始哈希见 [解码报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"mapId": 4, "x": 36, "y": 33, "dataFlag": 7232, "flag": false}（完整读取）：[000229-request-10506.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000229-request-10506.bin)，SHA-256 `725fc558f796d8c86077651022c2a5ed0480288b6c430e4a596eb8ee29dc9e22`。
- response {"mapId": 4, "runtimeMapId": 30944, "name": "迎客殿", "x": 36, "y": 33, "npcCount": 23}（完整读取）：[000230-response-10506.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000230-response-10506.bin)，SHA-256 `d3fc9bb3335e7ba98f6cc04a53205d92717214a05e3c0a4b214cc2ee0c396f7c`。
- response {"mapId": 16, "runtimeMapId": 30945, "name": "神剑广场", "x": 3, "y": 44, "npcCount": 36}（完整读取）：[000257-response-10506.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000257-response-10506.bin)，SHA-256 `90770b6a5e6a9b666590be928852629bd9c51d6432b4e0250565a8593cdbaba5`。

## `10521` / `10522` / `10523` 世界地图进入

2026-09-07 复用当前登录连接主动请求，未修改既有写缓冲：

- `10521` 空请求返回 14 个世界区域。每项为 `id(byte)、name(string)、px(short)、py(short)、state(byte)`。请求：[000044-request-10521.bin](../captures/miniapp-protocol/session-20260907-next-map-worker/frames/000044-request-10521.bin)，SHA-256 `18845789cb52e6b0625c268637014be21a2826ded0ebeca7604fb1d48f5d3f75`；响应：[000046-response-10521.bin](../captures/miniapp-protocol/session-20260907-next-map-worker/frames/000046-response-10521.bin)，SHA-256 `e94a6ebe8fd3b3d301da943b85b60dc0c222f80072090fe2303c96d466a98c5f`。
- 随后对 14 个区域逐一请求 `10522`，完整返回 57 个地图入口；44 个 `isNeedItem=1`，13 个为 `0`。嵌套区域、入口 ID、名称、说明和状态见 [完整世界地图目录](../captures/miniapp-requests/world-catalog-20260907/result.json)，对应 15 组原始请求响应及哈希见同目录 `requests.jsonl`。
- `10522` 请求 `{regionId: 8}` 返回蜀山 4 个地图入口：`35` 迎客殿、`36` 神剑广场、`37` 蜀山后山、`38` 镇妖塔。每项为 `id(short)、name(string)、info(string)、state(byte)、isNeedItem(byte)`。请求：[000073-request-10522.bin](../captures/miniapp-protocol/session-20260907-next-map-worker/frames/000073-request-10522.bin)，SHA-256 `f804e43c55e19ca4b1c13a5d87180b88fa7ba7b86b94ccd5c0793e25b9147792`；响应：[000074-response-10522.bin](../captures/miniapp-protocol/session-20260907-next-map-worker/frames/000074-response-10522.bin)，SHA-256 `9c97c60cfb378e92fb611780e337af3239fcf7258f024ee6c881e0b5bb155df6`。
- `10523` 请求 `{entryId: 37, dataFlag: 7232}` 成功进入原始地图 `17`“蜀山后山”，坐标 `(17,9)`，响应 `dataFlag=7488`。NPC 块完整解析 25 个对象：普通/任务 NPC 11、敌人 12、传送点 2。请求：[000085-request-10523.bin](../captures/miniapp-protocol/session-20260907-next-map-worker/frames/000085-request-10523.bin)，SHA-256 `3d9c8d486d5ac2b29a9db83d57e02a6c4c6ba293d2eb2004e0b5c9dec817d43a`；响应：[000086-response-10523.bin](../captures/miniapp-protocol/session-20260907-next-map-worker/frames/000086-response-10523.bin)，SHA-256 `85dd63d1fb84a309e5feb8dbe8594ef8e7822972b08e4ab3ff5c40afe1f86539`；解析结果：[npcs-map-17.json](../captures/miniapp-protocol/session-20260907-next-map-worker/npcs-map-17.json)。

边界：`10522` 的入口 ID 不等同于原始地图 ID；本次 `37 → 17`。`10523` 会改变在线角色位置，不是只读查询。NPC 块之后的怪物数据本轮未做增量语义解析。

### 2026-09-07 目录入口串行尝试

复用当前登录连接，按 57 个目录入口逐个串行发送 `10523(entryId, dataFlag=7232)`，不并发、不递归传送点。完整结果、失败服务端文本、入口到原始地图映射和 57 组请求/响应哈希见 [`world-traverse-20260907-serial-01/result.json`](../captures/miniapp-requests/world-traverse-20260907-serial-01/result.json)、[`traversal.jsonl`](../captures/miniapp-requests/world-traverse-20260907-serial-01/traversal.jsonl) 和 [`requests.jsonl`](../captures/miniapp-requests/world-traverse-20260907-serial-01/requests.jsonl)。

- 成功 4 个：入口 `20 → orgMapId=901 风之森林`，`21 → 902 亡者之地`，`36 → 16 神剑广场`，`37 → 17 蜀山后山`；坐标分别为 `(24,6)`、`(20,26)`、`(3,44)`、`(17,9)`，均为 `dataFlag=7488`。
- 失败 53 个：`-5` 等级不足 51 个（入口 `1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,22,23,24,25,26,27,28,29,30,31,32,33,34,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,55,56,57`）；`-6` 没有传送石 2 个（入口 `19,35`）。
- 四个唯一原始地图共 107 个 NPC：普通/任务 40、敌人 55、传送点 5、其他类型 7。完整对象文件：[`npcs/`](../captures/miniapp-requests/world-traverse-20260907-serial-01/npcs/)。
- 所有原始请求/响应文件和 SHA-256 已核对；NPC 块完整解析，块后的怪物组/模型只保留原始响应，未将未解析尾部冒充完整语义。失败原因是本账号本次服务端返回的实测状态，不等同于地图不存在。

可复现入口：

```powershell
python request_miniapp_protocol.py --self-check
python request_miniapp_protocol.py --pid <当前WeChatAppEx网络PID> `
  --output-dir captures/miniapp-requests/<新目录> traverse-catalog
```

## `10518` 任务寻路

请求为 `mode:byte=2 + target:short`。响应为 `result:byte=0 + segmentCount:byte=0 + targetX:byte + targetY:byte`。

已采集目标：

| 目标 short | 终点 |
| ---: | --- |
| `1105` | `(27,17)` |
| `1106` | `(13,21)` |
| `1107` | `(19,17)` |
| `1108` | `(12,12)` |
| `1109` | `(5,17)` |
| `1110` | `(9,19)` |

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 寻路到主线目标 `1105` | [`000019-request-10518.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000019-request-10518.bin)，SHA-256 `fec1486ae6277a0e48a6cbee05c4e557bb9aee56eb47a04f8ff8bea5c92f0eb2` | [`000020-response-10518.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000020-response-10518.bin)，SHA-256 `fd75f73b6d0cd4518ec428e5e0769264d5a1340295ba1e9da6f8e28579987df2` |

### 2026-09-06 17:43 flue 补采

任务 2201→(22,7)，2202 战前→(21,12)、战后→(22,7)，2203→(22,16)，2204 战前→(29,16)、战后→(22,16)，2205→(14,23)，2206→(20,31)。八对请求响应完整解析。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- request {"mode": 2, "target": 2201}：[000032-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000032-request-10518.bin)，SHA-256 `deca95141d8a97595102283dfde459c77d8867bff7fafeca660661e876b7dff5`。
- response {"result": 0, "segments": 0, "x": 22, "y": 7}：[000033-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000033-response-10518.bin)，SHA-256 `ed05d9c8cbaa1a0ccf7b1cd67252c15056fd38a08d798f5f2e4f6a907f777c92`。
- request {"mode": 2, "target": 2202}：[000046-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000046-request-10518.bin)，SHA-256 `ae52d8ef691079a6ac393b31523f814c2c5b8294723cdfd95d70e4aae7550cf4`。
- response {"result": 0, "segments": 0, "x": 21, "y": 12}：[000047-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000047-response-10518.bin)，SHA-256 `59e56e23aa3fc66f2afdd2c246e777e97da1c2a6be949fa0f5387ccd73c23c8b`。
- request {"mode": 2, "target": 2202}：[000062-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000062-request-10518.bin)，SHA-256 `ae52d8ef691079a6ac393b31523f814c2c5b8294723cdfd95d70e4aae7550cf4`。
- response {"result": 0, "segments": 0, "x": 22, "y": 7}：[000063-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000063-response-10518.bin)，SHA-256 `ed05d9c8cbaa1a0ccf7b1cd67252c15056fd38a08d798f5f2e4f6a907f777c92`。
- request {"mode": 2, "target": 2203}：[000073-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000073-request-10518.bin)，SHA-256 `fdc891f6798de29bca102196bb5463247a79b55f4fb5dfac47be7f09a749fb62`。
- response {"result": 0, "segments": 0, "x": 22, "y": 16}：[000074-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000074-response-10518.bin)，SHA-256 `efa70dcdf4c47e17d7bb9a6034acd250a0c6fef16d9a16ffcfc2873d6dfce6c1`。
- request {"mode": 2, "target": 2204}：[000081-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000081-request-10518.bin)，SHA-256 `5a1c9289ec1e6f437e7bf9baf4e4ea279602b3fca5d197766d622ee25a37278b`。
- response {"result": 0, "segments": 0, "x": 29, "y": 16}：[000082-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000082-response-10518.bin)，SHA-256 `92526c4df7f19612ce9dcdd8b1d7318efdffd4aa6fba92549f0a6ab979fed41d`。
- request {"mode": 2, "target": 2204}：[000097-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000097-request-10518.bin)，SHA-256 `5a1c9289ec1e6f437e7bf9baf4e4ea279602b3fca5d197766d622ee25a37278b`。
- response {"result": 0, "segments": 0, "x": 22, "y": 16}：[000098-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000098-response-10518.bin)，SHA-256 `efa70dcdf4c47e17d7bb9a6034acd250a0c6fef16d9a16ffcfc2873d6dfce6c1`。
- request {"mode": 2, "target": 2205}：[000114-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000114-request-10518.bin)，SHA-256 `abc11567d4ea6a1b40dc0f29c58e2350c901e7748da35de0100202c356ff5478`。
- response {"result": 0, "segments": 0, "x": 14, "y": 23}：[000115-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000115-response-10518.bin)，SHA-256 `b5fdfba43c2daccc5be73f1111bc95ca9f27d47b5796f7410a0ef7680d443b91`。
- request {"mode": 2, "target": 2206}：[000125-request-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000125-request-10518.bin)，SHA-256 `7171976a9ac203acbb0b76568b1b271b144c20cada25b0ebccb652e07390d4b6`。
- response {"result": 0, "segments": 0, "x": 20, "y": 31}：[000126-response-10518.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000126-response-10518.bin)，SHA-256 `33f7df5fe45522fb8b8b207d31f472d63486a180040f584fc71bb9e841539d49`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"mode": 2, "target": 2207}（完整解析）：[000201-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000201-request-10518.bin)，SHA-256 `f74e625c8fe40282bd8165e95b494d8794ce29c5e0be238695a8b50b6c505b7a`。
- request {"mode": 2, "target": 2243}（完整解析）：[000215-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000215-request-10518.bin)，SHA-256 `44a34e708182f4fb29775072943b5df8798c31c7edee54643938100799047360`。
- response {"result": 0, "segments": 0, "transitions": [], "x": 16, "y": 31}（完整解析）：[000216-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000216-response-10518.bin)，SHA-256 `46a07a768df4ac21cb520213786c78c9f03806b8b6429e8611b4f58d196488e8`。
- request {"mode": 2, "target": 2208}（完整解析）：[000226-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000226-request-10518.bin)，SHA-256 `57b796af81ed71199dd8a61cb2f26190c20bfba8c16b77611cbfe9ac6de01cdd`。
- response {"result": 0, "segments": 0, "transitions": [], "x": 12, "y": 35}（完整解析）：[000227-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000227-response-10518.bin)，SHA-256 `90c5c73a5fef47ab044dd6609290869fdf8034e5000b318e2c46a8c49272dec6`。
- request {"mode": 2, "target": 2209}（完整解析）：[000240-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000240-request-10518.bin)，SHA-256 `764a832a05458e119634dde98369e1f4a48aff5e19e37d1a8f7a086c644a8d83`。
- response {"result": 0, "segments": 0, "transitions": [], "x": 9, "y": 34}（完整解析）：[000242-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000242-response-10518.bin)，SHA-256 `8e3bd9be5a00f9215e080ad4f2ae9f18b8ba4a02c29bb9bf401e514c0271c9f9`。
- request {"mode": 2, "target": 2210}（完整解析）：[000277-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000277-request-10518.bin)，SHA-256 `9ebe4062273abe6bd49c7760ab3066fdbbbfe15ca998a577f313be03b0da6c09`。
- response {"result": 0, "segments": 0, "transitions": [], "x": 29, "y": 28}（完整解析）：[000278-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000278-response-10518.bin)，SHA-256 `8c7566b0aa50c4b4017302443c3be969ed5afc336b8a2a89d90fc032c8cf34bc`。
- request {"mode": 2, "target": 2211}（完整解析）：[000310-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000310-request-10518.bin)，SHA-256 `d2ca5a778648ce6d20ed015ad90be2fe30f0a370cb1733d21877f44dea0a90d1`。
- response {"result": 0, "segments": 1, "transitions": [{"exitX": 34, "exitY": 33, "mapId": 16, "entryX": 3, "entryY": 44}], "x": 5, "y": 44}（完整解析）：[000311-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000311-response-10518.bin)，SHA-256 `f282481aa38d276b311b193147bf10fd22a9afeb62b1ec5f0c5c45edfdc6ab9b`。
- request {"mode": 2, "target": 2212}（完整解析）：[000330-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000330-request-10518.bin)，SHA-256 `309b7e56b85e8a9e10c5784ed7a1c750d823e2befa13aa72c1d4e9ea1ccba214`。
- response {"result": 0, "segments": 0, "transitions": [], "x": 7, "y": 44}（完整解析）：[000331-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000331-response-10518.bin)，SHA-256 `c5b201a2c5392cfb680ab89dd9e9edae8bc46893e4a089e83f7e1182d2238553`。
- request {"mode": 2, "target": 2213}（完整解析）：[000346-request-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000346-request-10518.bin)，SHA-256 `ab1e1c5843cf7bb2e455a3f8ba5efc6a5dfaf7cb7dcad3bccef4695b7c4a1a15`。
- response {"result": 0, "segments": 0, "transitions": [], "x": 18, "y": 45}（完整解析）：[000347-response-10518.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000347-response-10518.bin)，SHA-256 `911d0853b1c21820eb136611cdf336753d5ff11398125c8fb2ff1078ec11091f`。

## `11004` 角色属性刷新

请求为空。响应布局为：`增量属性数量:byte`，随后若干 `属性ID:short + 增量:int`，最后固定一个 `属性ID:short + 绝对值:int`。

当前战后样本为增量数量 `0`，最后属性 `122=3773`。2026-09-06 已用 Android DEX/Smali 与小程序 `ModelConst.COMBATPOINT=122` 确认为战斗力；此前“名称待确认”结论已补齐。12049首个int也是战斗力，样本3816；这些下发值不包含服务端公式，详见[离线战力评分](小程序协议到Android离线适配.md#离线战力评分2026-09-06)。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 战斗结束后刷新属性 | [`000013-request-11004.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000013-request-11004.bin)，SHA-256 `0ae17e88b62541928dd721e9fcbaab37a13ae00516cd18ee067391173be5a44a` | [`000014-response-11004.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000014-response-11004.bin)，SHA-256 `905f20b11f87b5f46e569af7403cb9dfd744694794acc4de9be794438c2b6a2d` |

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- response {"increments": [], "absolute": [122, 3942]}（完整解析）：[000104-response-11004.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000104-response-11004.bin)，SHA-256 `0fe6d92f7327e2013d03c132bc82cb5524a58242cd35c6d9c094131df382ccae`。
- response {"increments": [], "absolute": [122, 4028]}（完整解析）：[000200-response-11004.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000200-response-11004.bin)，SHA-256 `be5d0a5a134fc020f5711bd8d803577c1d3283aee6b86b080b13a9c55139ec14`。
- response {"increments": [], "absolute": [122, 4071]}（完整解析）：[000255-response-11004.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000255-response-11004.bin)，SHA-256 `d5e4182aeb1757df1ac854791da9030a1b963a265cda5c4efade92c5ff9c6d4c`。
- response {"increments": [], "absolute": [122, 4176]}（完整解析）：[000295-response-11004.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000295-response-11004.bin)，SHA-256 `e8c59063ac3eb923dbba0ebc88990874825320aec74f692238d3c1af7e4c9265`。
- response {"increments": [], "absolute": [122, 4278]}（完整解析）：[000362-response-11004.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000362-response-11004.bin)，SHA-256 `997b50d40278989355b24f14543186288c44a45c0a35963980dc9f40f53f8120`。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- response {"increments": [], "absolute": [122, 4295]}（完整读取）：[000046-response-11004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000046-response-11004.bin)，SHA-256 `c93e466ab551e8c4763abd1203aa660fd0ef9421aa8306d09ec051bda40dac4a`。

## `12501` 进入本地战斗

请求体是怪物组 `short`。当前请求为怪物组 `261`。响应首 byte 为结果，当前 `result=0`，表示成功进入战斗；失败分支还会返回错误字符串，尚未采集。

该样本补齐了此前缺失的战斗进入链：伙伴出战列表检查后，客户端发送 `12501(261)`，成功后进入本地战斗对象。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 进入怪物组 `261` 的本地战斗 | [`000034-request-12501.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000034-request-12501.bin)，SHA-256 `9cccd5c264aa834e150d54bf3414b436c4583304ff3ed948339438295a3664f0` | [`000036-response-12501.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000036-response-12501.bin)，`result=0`，SHA-256 `fb8d5c832f0288ea2eb59ab146b66222369cb00f30f8dabaf0d1362e58a67040` |

### 2026-09-06 17:43 flue 补采

怪物组 2012、2013 两次进入成功，result=0。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- request {"monsterGroupId": 2012}：[000050-request-12501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000050-request-12501.bin)，SHA-256 `648a899e7b5f0a3710d63c0a004f005a6cbf9343412e773725d983797c544ef8`。
- request {"monsterGroupId": 2013}：[000086-request-12501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000086-request-12501.bin)，SHA-256 `e29292ee6e738bbe07ffbb4cc7284b233e8b954c8c0b5e712d81df5ef1afefeb`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"monsterGroupId": 2014}（完整解析）：[000188-request-12501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000188-request-12501.bin)，SHA-256 `b9e9643045fa368e151bb121292aa00883e134cd29f383204d17898b6dbe50d4`。
- request {"monsterGroupId": 2015}（完整解析）：[000247-request-12501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000247-request-12501.bin)，SHA-256 `1f420a497fd5cb887a1bf4bc8a4d458db2df1ef1445f2c8148763268c83dd3e7`。
- request {"monsterGroupId": 2016}（完整解析）：[000283-request-12501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000283-request-12501.bin)，SHA-256 `77ea62458fc4fe4cb114bf5a7acabafcb31223434296ecb524b90fef24bba0a7`。
- request {"monsterGroupId": 2019}（完整解析）：[000352-request-12501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000352-request-12501.bin)，SHA-256 `6d8d7848746fe3cf3945582167ba8d28ebe071959d0ab8e4a68c7ce2a66a19af`。

## `12502` 本地战斗结算

请求已确认前缀：

| 字段 | 类型 | 2026-09-06 样本 |
| --- | --- | ---: |
| `startHP` | int | `135` |
| `startMP` | int | `176` |
| `endHP` | int | `135` |
| `endMP` | int | `176` |
| `seed` | short | `4` |
| `monsterGroupId` | short | `261` |
| `hasPet` | boolean | `false` |
| `buddyNum` | byte | `0` |
| `planCount` | byte | `2` |
| 行动计划 | length-prefixed bytes | 两组 `01 05`；帧末两字节含义未确认 |

成功响应 `result=2` 表示右方胜利。本次奖励为 `MONEY3 +0`、`EXP +20`、未升级、第二经验 `+0`、无宠物奖励、击杀怪物 `22 × 1`、无护送奖励、无物品、无 NPC 标志变化、无佣兵失效、未切图。响应末尾还包含战斗力和国家战力有效标志。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 三轮行动，结束 MP `169` | [`000010-request-12502.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000010-request-12502.bin)，SHA-256 `fe89cb01dba072940c3c89bc0320a9df5da51eb34a605e90db318b39d18765d8` | [`000012-response-12502.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000012-response-12502.bin)，SHA-256 `53e883d0468c80575d6aceef94281082e4512b20d3ead713e8f5efa2e9ebfa9f` |
| 两轮行动，HP/MP 无消耗，确认击杀 `22 × 1` | [`000039-request-12502.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000039-request-12502.bin)，SHA-256 `a448f9cce233913a4a3ea6c8bfcb49f10dc9bc55af0531b91cea800d1307a9bb` | [`000041-response-12502.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000041-response-12502.bin)，响应与上一样本完全相同，不重复保存解析副本 |

`12533` 是紧邻结算的服务端校验数据，按[忽略协议列表](miniapp-protocol-ignore-list.md)处理，不纳入结算规则。

### 2026-09-06 17:43 flue 补采

怪物组 2012、2013 的后续结算结果均为 2，右方胜利；本轮行动计划和结算尾未完整解码。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- request {}：[000056-request-12502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000056-request-12502.bin)，SHA-256 `874450ccc0c38640c7f3d86e94eb235cec9b8855af34dd4877eb61c06c5ded1f`。
- response {"result": 2}：[000059-response-12502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000059-response-12502.bin)，SHA-256 `ea3e0ed6c43d7c75ff961afe4b4e0c9b043e775e910303e7a1855eb6cb7050cd`。
- request {}：[000089-request-12502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000089-request-12502.bin)，SHA-256 `09ef1ccf47e762ebf6a5ca186b862bad6866133a0a008b1d9d4403e6e12a1f1c`。
- response {"result": 2}：[000092-response-12502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000092-response-12502.bin)，SHA-256 `a9fcf91fbbde5784e90a4cc7cf1bc0c139c23692cd7f54b3acd56234a4d79d3f`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"startHP": 244, "startMP": 174, "endHP": 244, "endMP": 154, "seed": 3, "monsterGroupId": 2012, "hasPet": false, "buddyCount": 0, "roundCount": 2, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 2, "skillId": 731}, {"offset": 37, "length": 4, "action": 2, "targetPosition": 7, "skillId": 731}], "serializationPadding": {"offset": 41, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000099-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000099-request-12502.bin)，SHA-256 `21ae101e82afa4b25c069232bc8d12d54565f7c4e3a5d2c7c0f0c5d2818c6148`。
- request {"startHP": 244, "startMP": 174, "endHP": 244, "endMP": 154, "seed": 4, "monsterGroupId": 2013, "hasPet": false, "buddyCount": 0, "roundCount": 2, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 5, "skillId": 731}, {"offset": 37, "length": 4, "action": 2, "targetPosition": 6, "skillId": 731}], "serializationPadding": {"offset": 41, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000135-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000135-request-12502.bin)，SHA-256 `6724d5cbf5f3b912995f21494317a7030a014e7430bc622f014e1b370f100149`。
- request {"startHP": 324, "startMP": 194, "endHP": 324, "endMP": 174, "seed": 5, "monsterGroupId": 2014, "hasPet": false, "buddyCount": 0, "roundCount": 2, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 4, "skillId": 731}, {"offset": 37, "length": 4, "action": 2, "targetPosition": 4, "skillId": 731}], "serializationPadding": {"offset": 41, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000193-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000193-request-12502.bin)，SHA-256 `86b52f3a2c772471223de29e3a0438553ee87b19a97612079f48d14aa28a7ced`。
- response {"result": 2, "reward": {"money3": 11, "level": {"exp": 30, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "bagFull": false, "hasPetReward": false, "kills": [{"monsterId": 2018, "quantity": 1}], "hasEscortReward": false, "items": [], "npcStatus": [{"npcId": 11, "sign": 1}], "powerValid": false, "expiredMercenaries": [], "changeMap": false, "countryPowerValid": false}}（完整解析）：[000197-response-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000197-response-12502.bin)，SHA-256 `f48a48721103a5379e0e03504fcd1b60c5d3290ea7b2531319b1aa6af63c2c59`。
- request {"startHP": 364, "startMP": 204, "endHP": 364, "endMP": 184, "seed": 6, "monsterGroupId": 2015, "hasPet": false, "buddyCount": 0, "roundCount": 2, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 2, "skillId": 731}, {"offset": 37, "length": 4, "action": 2, "targetPosition": 4, "skillId": 731}], "serializationPadding": {"offset": 41, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000250-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000250-request-12502.bin)，SHA-256 `a5eda05de64b4d94fdf1892ca767c398fe35000298d8992a9fceb330a9a41c6f`。
- response {"result": 2, "reward": {"money3": 3, "level": {"exp": 12, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "bagFull": false, "hasPetReward": false, "kills": [{"monsterId": 2020, "quantity": 3}], "hasEscortReward": false, "items": [], "npcStatus": [{"npcId": 13, "sign": 1}], "powerValid": false, "expiredMercenaries": [], "changeMap": false, "countryPowerValid": false}}（完整解析）：[000253-response-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000253-response-12502.bin)，SHA-256 `d9baf1e1f68835b39fc6fa3e8c84e5816c6a04c477369e21557f808042fb8363`。
- request {"startHP": 364, "startMP": 204, "endHP": 364, "endMP": 175, "seed": 7, "monsterGroupId": 2016, "hasPet": false, "buddyCount": 0, "roundCount": 2, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 7, "skillId": 731}, {"offset": 37, "length": 4, "action": 2, "targetPosition": 4, "skillId": 731}], "serializationPadding": {"offset": 41, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000290-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000290-request-12502.bin)，SHA-256 `bdf679937f1e44b0256af32141d95fa54396557c6dba5239c4672555b7c2e3ef`。
- response {"result": 2, "reward": {"money3": 3, "level": {"exp": 12, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "bagFull": false, "hasPetReward": false, "kills": [{"monsterId": 2021, "quantity": 3}], "hasEscortReward": false, "items": [], "npcStatus": [{"npcId": 16, "sign": 1}], "powerValid": false, "expiredMercenaries": [], "changeMap": false, "countryPowerValid": false}}（完整解析）：[000293-response-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000293-response-12502.bin)，SHA-256 `6541880958a67579bf87c0392618d1a61d1ae17067449ce301a1476336b21c34`。
- request {"startHP": 404, "startMP": 260, "endHP": 404, "endMP": 250, "seed": 8, "monsterGroupId": 2019, "hasPet": false, "buddyCount": 0, "roundCount": 1, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 5, "skillId": 731}], "serializationPadding": {"offset": 34, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000355-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000355-request-12502.bin)，SHA-256 `d54c3b176984d381210e5c8188b4f244c765b3e8e7628afad8b766bc01ed3389`。
- response {"result": 2, "reward": {"money3": 11, "level": {"exp": 30, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "bagFull": false, "hasPetReward": false, "kills": [{"monsterId": 2022, "quantity": 1}, {"monsterId": 2023, "quantity": 1}], "hasEscortReward": false, "items": [], "npcStatus": [{"npcId": 5, "sign": 1}], "powerValid": false, "expiredMercenaries": [], "changeMap": false, "countryPowerValid": false}}（完整解析）：[000358-response-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000358-response-12502.bin)，SHA-256 `b3109fe61fbadf2ab3a1dd322601dd41b0af47492801095ca29ce7b328f47570`。
- request {"startHP": 404, "startMP": 260, "endHP": 404, "endMP": 240, "seed": 9, "monsterGroupId": 2019, "hasPet": false, "buddyCount": 0, "roundCount": 2, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 3, "skillId": 731}, {"offset": 37, "length": 4, "action": 2, "targetPosition": 5, "skillId": 731}], "serializationPadding": {"offset": 41, "hex": "00000000", "source": "original ByteArray allocation; exactRebuild verified"}}（完整解析）：[000370-request-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000370-request-12502.bin)，SHA-256 `8def871e2556bbb5c7ca2e8c9171634235f04fe7769cf554c48a87bb7a8c8736`。
- response {"result": 2, "reward": {"money3": 11, "level": {"exp": 30, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "bagFull": false, "hasPetReward": false, "kills": [{"monsterId": 2022, "quantity": 1}, {"monsterId": 2023, "quantity": 1}], "hasEscortReward": false, "items": [], "npcStatus": [], "powerValid": false, "expiredMercenaries": [], "changeMap": false, "countryPowerValid": false}}（完整解析）：[000373-response-12502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000373-response-12502.bin)，SHA-256 `26395427ad42eb014d9b2b5da5c57227d59fb72c44224d2c789fbe1643de2e70`。

后续解码校验（同一批原始帧）：本轮 10506、14501、14502、14503 和 12502 响应均已完整解析到帧尾，覆盖怪物、战斗组、实际奖励物品、升级和 NPC 标记；12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量，原始字节保留；此前未确认边界已消除。以上更新替代本节本轮早期的“尾部未完整解码”边界。完整字段和原始哈希见 [解码报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"startHP": 404, "startMP": 267, "endHP": 404, "endMP": 257, "seed": 16, "monsterGroupId": 2018, "hasPet": false, "buddyCount": 0, "roundCount": 1, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 3, "skillId": 731}], "serializationPadding": "00000000"}（完整读取）：[000039-request-12502.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000039-request-12502.bin)，SHA-256 `0d883476e828b1c06e657eaf179cb351fd0d06416c1513590227f733771a4b75`。
- request {"startHP": 404, "startMP": 267, "endHP": 404, "endMP": 257, "seed": 17, "monsterGroupId": 2018, "hasPet": false, "buddyCount": 0, "roundCount": 1, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 4, "skillId": 731}], "serializationPadding": "00000000"}（完整读取）：[000096-request-12502.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000096-request-12502.bin)，SHA-256 `4cc9881f870bae1cbc5e5d5b2e080d284f7236c69b23e8af90aebeeb15feca86`。
- request {"startHP": 404, "startMP": 267, "endHP": 404, "endMP": 257, "seed": 18, "monsterGroupId": 2018, "hasPet": false, "buddyCount": 0, "roundCount": 1, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 3, "skillId": 731}], "serializationPadding": "00000000"}（完整读取）：[000110-request-12502.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000110-request-12502.bin)，SHA-256 `22e1e9c4e18ae334242a2c0d1011241b4305fe6bf289521af51270680b15235c`。
- response {"result": 2, "reward": {"money3": 11, "level": {"exp": 30, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "bagFull": false, "hasPetReward": false, "kills": [{"monsterId": 2022, "quantity": 1}, {"monsterId": 2023, "quantity": 1}], "hasEscortReward": false, "items": [{"mode": 1, "quantity": 1, "expectedSlotPos": 42, "item": {"quantity": 1, "petProvideExp": 0, "durability": 0, "attachDone": 0, "attachPower": 0, "attachValue": 0, "slotPos": 42, "status": 0, "type": 28, "id": 101108, "name": "十五周年福袋", "bagIcon": 303, "info": "开启可随机获得【世界生日会】称号、【十五周年残卷①】或日常消耗品等奖励", "grade": 2, "autoBinding": 1, "stackNum": 99, "decomCHest": false, "reqLv": 0, "ownTime": 0, "durMax": 0, "price": 50, "round": 0, "area": -1, "power1": 102, "powerValue1": 10959, "power2": 103, "powerValue2": 1, "vipLevelReq": 0, "needShowRate": false}}], "npcStatus": [], "powerValid": false, "expiredMercenaries": [], "changeMap": false, "countryPowerValid": false}}（完整读取）：[000113-response-12502.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000113-response-12502.bin)，SHA-256 `c3eb02beab184452e1c97366ae77fef9156382fedc9b8a3e60a47f2e95a67e61`。
- request {"startHP": 404, "startMP": 267, "endHP": 404, "endMP": 257, "seed": 19, "monsterGroupId": 2019, "hasPet": false, "buddyCount": 0, "roundCount": 1, "plans": [{"offset": 30, "length": 4, "action": 2, "targetPosition": 5, "skillId": 731}], "serializationPadding": "00000000"}（完整读取）：[000128-request-12502.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000128-request-12502.bin)，SHA-256 `d9643076cc1b7ad9546e7cd385250f61dec327cec18a1bf5d4eadfb6efe9633b`。

## `14501` NPC 任务数据

按任务 ID 查询详情的 `13535` 失败样本见本文末尾；不要把本协议的 NPC 编号替换为任务编号。

请求字段：`npcId:short`，已有怪物组数量及若干 `groupId:short`，已有怪物数量及若干 `monsterId:short`。

响应字段：`npcId:short`、任务数量及 length-prefixed 任务对象、怪物组列表、怪物列表；存在怪物时继续读取怪物 AI。它用于给 NPC 填充可接/可交任务及相关怪物定义。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 获取 NPC 任务及完整数据 | [`000027-request-14501.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000027-request-14501.bin)，SHA-256 `a3220a736038daa5ae61ea823880e2272074d3c9efa536dc0b784f38bcc73cea` | [`000028-response-14501.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000028-response-14501.bin)，650 字节，SHA-256 `da210fdd0f159b777ccae7ec565bd2f50fc06bf2434d67c0009c7646b33b716e` |
| NPC `2`，客户端已有怪物组 `260/261` 和怪物 `21/22` | [`000046-request-14501.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000046-request-14501.bin)，SHA-256 `cf1d621cd461f8794eceaba0c4990df98f107c08c3b4c63663aa92abcbb269c8` | [`000047-response-14501.bin`](../captures/miniapp-protocol/session-20260906-065158/frames/000047-response-14501.bin)：NPC `2`，任务/怪物组/怪物数量均为 `0`，SHA-256 `7a2576872e7ff502bcc06318a94c1d1ff750e8562013d8f3327d8ec31ef13dbd` |


2026-09-06 新增代表性样本（用户进村并打开 NPC）：

- NPC `4`：请求 [000023-request-14501.bin](../captures/miniapp-protocol/session-20260906-090442/frames/000023-request-14501.bin)，SHA-256 `b811d3712f1732fc08c6f9c34f3d4e9b62b3b2aeb71e68d024213480d9bdd315`；响应 [000048-response-14501.bin](../captures/miniapp-protocol/session-20260906-090442/frames/000048-response-14501.bin)，SHA-256 `67535d0cc8d21492e53ab72a32ad04d385151d1f0e982a7fa2110fabb70d1674`，返回任务 `1105/1106`。任务对象已分离并解析 ID/标题，内部其余字段未完整命名；怪物组与怪物数量均为 0。
- NPC `1`：请求 [000060-request-14501.bin](../captures/miniapp-protocol/session-20260906-090442/frames/000060-request-14501.bin)，SHA-256 `3de747afe5dcccb93d8966359a9658598845f9aca95ca9b07d8152e6839ff384`；响应 [000061-response-14501.bin](../captures/miniapp-protocol/session-20260906-090442/frames/000061-response-14501.bin)，SHA-256 `f599d4ff1911aad3d81a5eba5d84eecd167ef76e84af3edb348357a8f861db37`，返回任务 `1109`。任务对象已分离并解析 ID/标题，内部其余字段未完整命名；怪物组与怪物数量均为 0。

同轮 NPC `5` 的 1107 响应与原 650 字节样本 SHA-256 相同，不重复登记。

### 2026-09-06 17:43 flue 补采

地图字段为 4 的 NPC 2/5/10/11 返回任务 2202/2203、2204/2205、2206、2207/2243。前三响应怪物组和怪物为空，完整消费；NPC 11 响应有 1 个怪物组，组与怪物尾未完整解码。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- request {"npcId": 2, "cachedGroups": [260, 261, 2012, 2013, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2019, 2020, 2021, 2047, 2048, 2049]}：[000029-request-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000029-request-14501.bin)，SHA-256 `1e4ed35601ab20ba7826e179493134a28ff370057fd61a70b4a0a5dbfd252ba4`。
- response {"npcId": 2, "missionCount": 2, "missionIds": [2202, 2203], "monsterGroupCount": 0, "monsterCount": 0}：[000030-response-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000030-response-14501.bin)，SHA-256 `6f3a47a1037214376beb72048289df79e207c4f19d81a99a439c03cd00a9c95e`。
- request {"npcId": 5, "cachedGroups": [260, 261, 2012, 2013, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2019, 2020, 2021, 2047, 2048, 2049]}：[000071-request-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000071-request-14501.bin)，SHA-256 `3d1505336010c0304f3854be26f41618f715acbdf9f84006369d10e3f8a51f13`。
- response {"npcId": 5, "missionCount": 2, "missionIds": [2204, 2205], "monsterGroupCount": 0, "monsterCount": 0}：[000072-response-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000072-response-14501.bin)，SHA-256 `f4f51d3ceb4961f88aaf0e448f5a453e119495065c803f61b555baa2933b0c76`。
- request {"npcId": 10, "cachedGroups": [260, 261, 2012, 2013, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2019, 2020, 2021, 2047, 2048, 2049]}：[000112-request-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000112-request-14501.bin)，SHA-256 `baf0c36316751b4af44b0f35cffb49d55a92550c5f725eaa44a2ab1f75da3ad5`。
- response {"npcId": 10, "missionCount": 1, "missionIds": [2206], "monsterGroupCount": 0, "monsterCount": 0}：[000113-response-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000113-response-14501.bin)，SHA-256 `0be8a2233fef924cf85c61e69669746f3416ed25887cc3b44e9adc6c5b3c4f61`。
- request {"npcId": 11, "cachedGroups": [260, 261, 2012, 2013, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2019, 2020, 2021, 2047, 2048, 2049]}：[000123-request-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000123-request-14501.bin)，SHA-256 `1f0e37d0ff170a3d0ad5e1d882288e05cb9ff309fb783512004cc2851d7a08b0`。
- response {"npcId": 11, "missionCount": 2, "missionIds": [2207, 2243], "monsterGroupCount": 1}：[000124-response-14501.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000124-response-14501.bin)，SHA-256 `4d457c6d7e1348b5998f176612727d3401952063de0879d5748dd69d24c576a7`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"npcId": 88, "cachedGroups": [260, 261, 2012, 2013, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2019, 2020, 2021, 2047, 2048, 2049]}（完整解析）：[000031-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000031-request-14501.bin)，SHA-256 `c94b5d806f02254e4754edd5c22d013ca64ed21e47306a21fcd7074b6d520da8`。
- request {"npcId": 51, "cachedGroups": [260, 261, 2012, 2013, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2019, 2020, 2021, 2047, 2048, 2049]}（完整解析）：[000033-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000033-request-14501.bin)，SHA-256 `d9005932979f4e6d793d81f1c97d978c2b1a5c8e232fc90117083bd7adf3c30e`。
- response {"npcId": 88, "missionCount": 1, "missionIds": [1124], "groupIds": [], "monsterIds": [], "skillCount": 0, "aiCount": 0}（完整解析）：[000034-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000034-response-14501.bin)，SHA-256 `4fae22281e6af6ca730baa686894e619e24a11f3a6de2b71831f82aea6079ae3`。
- response {"npcId": 51, "missionCount": 2, "missionIds": [2201, 2199], "groupIds": [], "monsterIds": [], "skillCount": 0, "aiCount": 0}（完整解析）：[000035-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000035-response-14501.bin)，SHA-256 `34bcf19f159c0910a13c50f164ec51b91eb2a3953b4b04c15e75504fb302a0ca`。
- request {"npcId": 16, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2047, 2048, 2049]}（完整解析）：[000213-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000213-request-14501.bin)，SHA-256 `bb957d30746893d1add4021ff44cba37ffa5ec1c0da2ac6e0eb038b6bfe2f3d9`。
- response {"npcId": 16, "missionCount": 2, "missionIds": [2208, 2211], "groupIds": [], "monsterIds": [], "skillCount": 0, "aiCount": 0}（完整解析）：[000214-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000214-response-14501.bin)，SHA-256 `d6e9c14a90f081af498d27ec75d87a8536bcbc0fad728ed1445ace7bef6d1d12`。
- request {"npcId": 13, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2047, 2048, 2049]}（完整解析）：[000224-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000224-request-14501.bin)，SHA-256 `a2f4bad03a82d1c284345cae238e4954d9f52b720a916e7a3e5f2f7da77d8426`。
- response {"npcId": 13, "missionCount": 2, "missionIds": [2209, 2210], "groupIds": [], "monsterIds": [], "skillCount": 0, "aiCount": 0}（完整解析）：[000225-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000225-response-14501.bin)，SHA-256 `5df2a83cd31940122fe513b70f1ab224695cb270a0f884e201bd2df3949ce62a`。
- request {"npcId": 1, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016, 2018, 2019, 2020, 2021, 2023, 2024, 2047], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2025, 2029, 2031, 2032, 2046, 2047, 2048, 2049, 2050]}（完整解析）：[000317-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000317-request-14501.bin)，SHA-256 `361486de23429deddecf79140acf5a5928c6695a104ad8aec91c20358a6e012e`。
- response {"npcId": 1, "missionCount": 1, "missionIds": [2212], "groupIds": [], "monsterIds": [], "skillCount": 0, "aiCount": 0}（完整解析）：[000318-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000318-response-14501.bin)，SHA-256 `141c2c3406ca49cb9c88a111c03a52c1adb2fe2403bec7c04e538fb6e2b13ecb`。
- request {"npcId": 2, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016, 2018, 2019, 2020, 2021, 2023, 2024, 2047], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2025, 2029, 2031, 2032, 2046, 2047, 2048, 2049, 2050]}（完整解析）：[000328-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000328-request-14501.bin)，SHA-256 `917a69fc1ea1fb227a4a010f46bdad301bb339b5c4b25a52af10344874477f67`。
- response {"npcId": 2, "missionCount": 3, "missionIds": [2213, 2215, 1181], "groupIds": [], "monsterIds": [], "skillCount": 0, "aiCount": 0}（完整解析）：[000329-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000329-response-14501.bin)，SHA-256 `5e89811d4017a3f5d420e1b3a9accd844827922edcd86a5d91d49ebde88a05ab`。
- request {"npcId": 5, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016, 2018, 2019, 2020, 2021, 2023, 2024, 2047], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2025, 2029, 2031, 2032, 2046, 2047, 2048, 2049, 2050]}（完整解析）：[000359-request-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000359-request-14501.bin)，SHA-256 `1690a398a7920549e54d9d4af3283c545b41e61c61789e0a17fe56388e505b73`。
- response {"npcId": 5, "missionCount": 2, "missionIds": [2214, 2244], "groupIds": [2017], "monsterIds": [2028], "skillCount": 0, "aiCount": 0}（完整解析）：[000360-response-14501.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000360-response-14501.bin)，SHA-256 `892bbb53723371deab5e4b2576aeb1f663e4915dc9f60e983ae3e1e846846143`。

后续解码校验（同一批原始帧）：本轮 10506、14501、14502、14503 和 12502 响应均已完整解析到帧尾，覆盖怪物、战斗组、实际奖励物品、升级和 NPC 标记；12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量，原始字节保留；此前未确认边界已消除。以上更新替代本节本轮早期的“尾部未完整解码”边界。完整字段和原始哈希见 [解码报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"npcId": 66, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016, 2017, 2018, 2019, 2020, 2021, 2023, 2024, 2047], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2025, 2028, 2029, 2031, 2032, 2046, 2047, 2048, 2049, 2050]}（完整读取）：[000053-request-14501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000053-request-14501.bin)，SHA-256 `9541e39706dbcd867f1293cdbf7c5be7e4b2795989f376ca6a457699eddc240f`。
- response {"npcId": 66, "tasks": [], "detail": "本会话 missions.json"}（完整读取）：[000054-response-14501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000054-response-14501.bin)，SHA-256 `4db1f55bab318c3727cd861aa1669fa601a59c514075678c138beb128be5cbb5`。
- request {"npcId": 51, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016, 2017, 2018, 2019, 2020, 2021, 2023, 2024, 2047], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2025, 2028, 2029, 2031, 2032, 2046, 2047, 2048, 2049, 2050]}（完整读取）：[000242-request-14501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000242-request-14501.bin)，SHA-256 `ef216125e10668a5bddd5c5e6e29ada94e7f333ae09456d977efec66673a3582`。
- response {"npcId": 51, "tasks": [], "detail": "本会话 missions.json"}（完整读取）：[000243-response-14501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000243-response-14501.bin)，SHA-256 `f8d0b4c49b8c02c1d1e28a3d323fbc893e4311441dbf785e9b1e7f291740db8c`。
- request {"npcId": 5, "cachedGroups": [260, 261, 2012, 2013, 2014, 2015, 2016, 2017, 2018, 2019, 2020, 2021, 2023, 2024, 2047], "cachedMonsters": [21, 22, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2025, 2028, 2029, 2031, 2032, 2046, 2047, 2048, 2049, 2050]}（完整读取）：[000259-request-14501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000259-request-14501.bin)，SHA-256 `f9bbfa3a1c6585766a0420a492c14d8ef5419e85b4888e9662a60c7a78d371b9`。
- response {"npcId": 5, "tasks": [{"id": 2214, "title": "击败魔教屠夫"}, {"id": 2244, "title": "桥已打通"}], "detail": "本会话 missions.json"}（完整读取）：[000261-response-14501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000261-response-14501.bin)，SHA-256 `7ef7b6f2d7de44f70076f212b7fc20c31db6b8e51a13b99d99f79fc66279c0dd`。

## `14502` 接取任务

请求固定写两个 short。已采集 `(4,1106)`、`(5,1107)`、`(8,1108)`、`(1,1109)`、`(6,1110)`。五个响应均以 `result=0` 成功，长度 11 字节；后续 NPC 状态对和末 byte 的精确含义尚未完全确认。

代表性样本：

| 操作 | 请求 | 响应 |
| --- | --- | --- |
| 接取主线 `1106` | [`000025-request-14502.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000025-request-14502.bin)，SHA-256 `da66b0f57ab69cb2459d84f946ba200bf512d3527947c8fcc760e0e1315bcd54` | [`000026-response-14502.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000026-response-14502.bin)，SHA-256 `7ca8f5c34cfa9550332e44a2e7a14ed9493db3ff155d527b21b4e8f047740371` |

### 2026-09-06 17:43 flue 补采

成功接取 2201～2206，六个结果字节均为 0；状态尾未完整解码。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- request {"npcId": 51, "missionId": 2201}：[000027-request-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000027-request-14502.bin)，SHA-256 `a991e3f881c99df834a46125d200889834775cb7cd5619049f10dd8dbf67f50a`。
- response {"result": 0}：[000028-response-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000028-response-14502.bin)，SHA-256 `39112da00002d0a349162758de54809fa71a8b46b388dc54026853eb62713b22`。
- request {"npcId": 2, "missionId": 2202}：[000044-request-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000044-request-14502.bin)，SHA-256 `d4d59fd649154bae56030b135628f001f8c46a277ab7d576c711d9bb5a31b1ac`。
- response {"result": 0}：[000045-response-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000045-response-14502.bin)，SHA-256 `7d5ce057c380147552ca5eadb53cf5e6e6b9b6e05b909be42cef7aa1fe862eeb`。
- request {"npcId": 2, "missionId": 2203}：[000069-request-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000069-request-14502.bin)，SHA-256 `8315be3facef30e3fec1d0add8447d620f166f941abd45537d44ada16b2ff6cd`。
- response {"result": 0}：[000070-response-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000070-response-14502.bin)，SHA-256 `88b85d531a5786bb3ce378c054db72ac3388f69475be8dfda6563ea989712ee2`。
- request {"npcId": 5, "missionId": 2204}：[000079-request-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000079-request-14502.bin)，SHA-256 `d9fababc4d7c0bcb925bcd93deff5d4dbe06367b648e46c4bcd2b894ea1e5369`。
- response {"result": 0}：[000080-response-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000080-response-14502.bin)，SHA-256 `8a04eafb4a38bd4243303b5e4f50c32b6e8109790d126b8b7732355fb37ae93e`。
- request {"npcId": 5, "missionId": 2205}：[000110-request-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000110-request-14502.bin)，SHA-256 `4c2cdf33b32abb3aefb7ca6b8e696df9ad240d9fb09601b9314f8730ac8d1cc6`。
- response {"result": 0}：[000111-response-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000111-response-14502.bin)，SHA-256 `544a7b8d40718f0911e9c87268eb695e5310ef53582c0da20891cea5ac3fca5d`。
- request {"npcId": 10, "missionId": 2206}：[000121-request-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000121-request-14502.bin)，SHA-256 `a40abd9bbb7c5384c96550ab8082ee7475b5867a383f5380d6a37acf1872cca7`。
- response {"result": 0}：[000122-response-14502.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000122-response-14502.bin)，SHA-256 `9355ac73297635f0872f20a80b2d23af6e8d266d86836e1d65939bb92253884e`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"npcId": 11, "missionId": 2207}（完整解析）：[000184-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000184-request-14502.bin)，SHA-256 `14263246ae3791d88737fdf75148c08ffbbb2df1cbf47680d5ec19f7299031af`。
- response {"result": 0, "npcStatus": [{"npcId": 11, "sign": 2}], "directSubmit": false}（完整解析）：[000185-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000185-response-14502.bin)，SHA-256 `b22f8ce503559bfbc00cc4649f87f11133eef7efd1b63de5a4e8a3e01a0faf55`。
- request {"npcId": 11, "missionId": 2243}（完整解析）：[000211-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000211-request-14502.bin)，SHA-256 `90a346f92813dcfb64a74f707d7bad1cd49f7d7db1b8b51337f82affed3ab4f5`。
- response {"result": 0, "npcStatus": [{"npcId": 16, "sign": 1}, {"npcId": 11, "sign": 0}], "directSubmit": false}（完整解析）：[000212-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000212-response-14502.bin)，SHA-256 `c096912d1e194f73f5dc62fb16064b485bbca35efe8213342866dc7729a67032`。
- request {"npcId": 16, "missionId": 2208}（完整解析）：[000222-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000222-request-14502.bin)，SHA-256 `e0139fa4a3699ee1b9c803c75b101eedf08ec860ca588036fd818f2f62e0bf63`。
- response {"result": 0, "npcStatus": [{"npcId": 16, "sign": 0}, {"npcId": 13, "sign": 1}], "directSubmit": false}（完整解析）：[000223-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000223-response-14502.bin)，SHA-256 `da1dec79f0f042d689af1f5c906d05102249809ba13e7b6a2fe098b84afc833a`。
- request {"npcId": 13, "missionId": 2209}（完整解析）：[000238-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000238-request-14502.bin)，SHA-256 `88059d6e55004db00624b1d7b8e2734c4f01344283fb032bc492ba786cdfcad5`。
- response {"result": 0, "npcStatus": [{"npcId": 13, "sign": 2}], "directSubmit": false}（完整解析）：[000239-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000239-response-14502.bin)，SHA-256 `fea0cb3a820c76c7643adfeccd9724d1f597454fd64c4260f35ce7751da91200`。
- request {"npcId": 13, "missionId": 2210}（完整解析）：[000275-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000275-request-14502.bin)，SHA-256 `6184d354ac9d5902d6d5b735c622f92bb4bce6c448a31a0d293ea1c6115e5117`。
- response {"result": 0, "npcStatus": [{"npcId": 16, "sign": 2}, {"npcId": 13, "sign": 0}], "directSubmit": false}（完整解析）：[000276-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000276-response-14502.bin)，SHA-256 `b1b61adadfad47769b38f49d0aeeed40c3b585c176c52d54c5f57ab07086c655`。
- request {"npcId": 16, "missionId": 2211}（完整解析）：[000308-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000308-request-14502.bin)，SHA-256 `01bfe4abef852252af6df0fc9a00eb96a64851f9180efea3efc8096703d83b5e`。
- response {"result": 0, "npcStatus": [{"npcId": 16, "sign": 0}], "directSubmit": false}（完整解析）：[000309-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000309-response-14502.bin)，SHA-256 `05e596d0d41c752e0deebca6d4a95302594c16a138fa79fd0a3dd4da0b8c4a96`。
- request {"npcId": 1, "missionId": 2212}（完整解析）：[000326-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000326-request-14502.bin)，SHA-256 `e6d313e1301944267b3f28e9dabeb957d731466690bee18a2d71e6d3f73d9917`。
- request {"npcId": 2, "missionId": 2213}（完整解析）：[000343-request-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000343-request-14502.bin)，SHA-256 `12a3e9c2ac979f7cd5d01649736e4b1152dc6b0a8b094f59a4f6d5717459dfab`。
- response {"result": 0, "npcStatus": [{"npcId": 2, "sign": 0}, {"npcId": 5, "sign": 2}], "directSubmit": false}（完整解析）：[000344-response-14502.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000344-response-14502.bin)，SHA-256 `aeb2e386fd48adc38224de88df0bacd2e08980735ef70bd55767f506d61f8ee5`。

后续解码校验（同一批原始帧）：本轮 10506、14501、14502、14503 和 12502 响应均已完整解析到帧尾，覆盖怪物、战斗组、实际奖励物品、升级和 NPC 标记；12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量，原始字节保留；此前未确认边界已消除。以上更新替代本节本轮早期的“尾部未完整解码”边界。完整字段和原始哈希见 [解码报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

## `14503` 提交任务

请求固定为三个值：`short + short + int`。已采集：

| 主线阶段 | 请求值 | 已确认结果 |
| ---: | --- | --- |
| `1105` | `(4,1105,-1)` | `MONEY3 +400`；资源绝对值变为 `800` |
| `1106` | `(5,1106,-1)` | 成功；未确认非零奖励 |
| `1107` | `(8,1107,-1)` | `EXP +220`；升 1 级；当前 `EXP=70/400`；`SP+10`；`CP+3` |
| `1108` | `(1,1108,-1)` | 成功；未确认非零奖励 |
| `1109` | `(6,1109,-1)` | `EXP +210` |
| `1110` | `(12,1110,-1)` | 成功；主线推进到 `1125` |

响应以结果 byte `0` 开始，后续可能包含任务完成标志、NPC 状态、三类货币、经验与升级、宠物、背包物品及国家/城市字段。每次提交后还收到 `14591` 主线状态推送；`14591` 是小程序版本的状态更新证据，不要求 Android 模拟同号网络消息。

代表性样本：

| 操作 | 请求 | 相关响应 |
| --- | --- | --- |
| 提交 `1105`，包含长奖励尾 | [`000021-request-14503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000021-request-14503.bin)，SHA-256 `cabdabad233af7074e1b665efceb0134163beabf76bc55b7cc1eccff8697e268` | [`000024-response-14503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000024-response-14503.bin)，174 字节，SHA-256 `cac412ee4c2414ad3a9f8b50d214d7fa4c488fbaf1186ca6e544a38f8c7e429f`；相关 [`000022-response-25013.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000022-response-25013.bin) 和 [`000023-response-14591.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000023-response-14591.bin) |
| 提交 `1107` 并升级 | [`000043-request-14503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000043-request-14503.bin)，SHA-256 `9d4826de0a1eb2d2a66aa6af5ad42e1806d21be890b13801ac6683c347bc0ebf` | [`000049-response-14503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000049-response-14503.bin)，53 字节，SHA-256 `b67f96b7c7680bf956c4d96718df7568c270915fb3e61a0e051fa0ec29bee2e0` |
| 提交 `1110` 并推进到 `1125` | [`000084-request-14503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000084-request-14503.bin)，SHA-256 `eb48e0959ec1f67c01b079477e6f1c56cd30820ea4e5bb2ec551aea673cee1b7` | [`000086-response-14503.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000086-response-14503.bin)，SHA-256 `a7cf194500e5a7e79df95a2e8bd8567d357b14d33c7da79f5e2b34793e10a8a3`；相关 [`000085-response-14591.bin`](../captures/miniapp-protocol/in-game/session-20260905/frames/000085-response-14591.bin) |

### 2026-09-06 17:43 flue 补采

成功提交 1124、2199、2201～2205；2202 和 2204 的选择值分别为 2001、2025，其他为 -1。七个结果字节均为 0；奖励尾未完整解码。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- request {"npcId": 88, "missionId": 1124, "selection": -1}：[000019-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000019-request-14503.bin)，SHA-256 `313a17d2b300975f0d2b14526d4794cb6e82435845cd209784c47175ca4d9a1d`。
- response {"result": 0}：[000021-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000021-response-14503.bin)，SHA-256 `7826476baa432370e6ef6546cb61134c7b9f6e8777e46e57cdf16df7abd1b542`。
- request {"npcId": 51, "missionId": 2199, "selection": -1}：[000023-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000023-request-14503.bin)，SHA-256 `ed5a9affa799d8e15190047b8b7bf78789b83dae6901ffe3503c0a6f8376cec9`。
- response {"result": 0}：[000026-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000026-response-14503.bin)，SHA-256 `7cea0d31019ce4927beda772fc8f3522fc5f9b5ff3dacaf0c2e2b28d725707d1`。
- request {"npcId": 2, "missionId": 2201, "selection": -1}：[000034-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000034-request-14503.bin)，SHA-256 `fc6132bdf7a52719b27fd93c96741e94ff7cd62cdaef71f52d36304485dea060`。
- response {"result": 0}：[000042-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000042-response-14503.bin)，SHA-256 `16565aa3e5be415e568919fce1af99977971ed38a81b843d58362096e3e777df`。
- request {"npcId": 2, "missionId": 2202, "selection": 2001}：[000064-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000064-request-14503.bin)，SHA-256 `2c87b57231a2f32f5b086037375d5c0ace27b87896c8965a0e6be7ea90ad9716`。
- response {"result": 0}：[000067-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000067-response-14503.bin)，SHA-256 `338f745ae921969d7d0837e44dac29e10b66a5d9d63f0ed99c231de87964b42a`。
- request {"npcId": 5, "missionId": 2203, "selection": -1}：[000075-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000075-request-14503.bin)，SHA-256 `23c86ab9eeea9450bde53940ad2e22940f665a8989916397cae95250b8a63078`。
- response {"result": 0}：[000078-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000078-response-14503.bin)，SHA-256 `282ba1dd6be991069239d28d09a0499264a18c3a4ff35ef8ff28934754541503`。
- request {"npcId": 5, "missionId": 2204, "selection": 2025}：[000099-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000099-request-14503.bin)，SHA-256 `d83c03a6609ea265d54aecc2d07da795ad0db9521e7f89b23a4f68a29321810c`。
- response {"result": 0}：[000105-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000105-response-14503.bin)，SHA-256 `88e6bf7e7506773a95955b13addf5c01c0bb557a15b0b1918357e23c3cb2124b`。
- request {"npcId": 10, "missionId": 2205, "selection": -1}：[000117-request-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000117-request-14503.bin)，SHA-256 `fe5bbd219eb58b293af3abf21dc1c400e2816077bc3d8661f9f7807170b5e8ad`。
- response {"result": 0}：[000120-response-14503.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000120-response-14503.bin)，SHA-256 `3937aefef928dadf27fb07812ba8cd01c872dd8ffde71c85309ec9ecd39b4e8c`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"npcId": 22, "missionId": 1121, "selection": -1}（完整解析）：[000025-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000025-request-14503.bin)，SHA-256 `e68da03c12ac2493350171e8b027b3fb96e9cc68abb500c19a12f199bde90153`。
- response {"result": 0, "npcStatus": [{"npcId": 22, "sign": 0}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 0, "level": {"exp": 0, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000027-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000027-response-14503.bin)，SHA-256 `3c6422b73ba7ca0f1512611c6102a63b289d1e66778cb208d835058167477a01`。
- response {"result": 0, "npcStatus": [{"npcId": 88, "sign": 0}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 0, "level": {"exp": 0, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 40067, "slotPos": 31}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000062-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000062-response-14503.bin)，SHA-256 `784722c74b7d8578bf18dfc5644275bdd887aef44daa53b758db9de74499da67`。
- response {"result": 0, "npcStatus": [{"npcId": 2, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 100, "level": {"exp": 100, "gainLevel": 1, "currentExp": 60, "expMax": 800, "sp": 20, "cp": 3, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000085-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000085-response-14503.bin)，SHA-256 `ccd49c651b1ecdbde189872ed54d081d2213001425d2cba90a3001f88ab8c8a2`。
- request {"npcId": 2, "missionId": 2202, "selection": 2101}（完整解析）：[000108-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000108-request-14503.bin)，SHA-256 `8b1209d50184a15930acb91ff9d4acd9d59dcdf096c01cc128f917fc977238be`。
- response {"result": 0, "npcStatus": [{"npcId": 2, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 400, "level": {"exp": 300, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 2101, "slotPos": 32}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000111-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000111-response-14503.bin)，SHA-256 `74b190243c9b65027ee20518d72973727f6b3e02646b0f612717eb53842e8e00`。
- request {"npcId": 5, "missionId": 2204, "selection": 2125}（完整解析）：[000143-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000143-request-14503.bin)，SHA-256 `99bb56f4d76f3d0017f901aa22c4d955bb0b680591f974b3ab9a35971ad722ab`。
- response {"result": 0, "npcStatus": [{"npcId": 5, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 1000, "level": {"exp": 1000, "gainLevel": 1, "currentExp": 672, "expMax": 1200, "sp": 30, "cp": 3, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 2125, "slotPos": 33}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000149-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000149-response-14503.bin)，SHA-256 `24a50b66bcf51ca2adfcfdd784b8c26ff5fbc77875b8c2c7c0456b2caa5fb1f9`。
- request {"npcId": 11, "missionId": 2206, "selection": -1}（完整解析）：[000172-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000172-request-14503.bin)，SHA-256 `fc0620563c547f766d849b9ae98505a0b8fb70aafb813cadd7a093631d86b436`。
- response {"result": 0, "npcStatus": [{"npcId": 11, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 250, "level": {"exp": 250, "gainLevel": 1, "currentExp": 222, "expMax": 1600, "sp": 40, "cp": 3, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000181-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000181-response-14503.bin)，SHA-256 `b65fa6f0be600360c03b85c191a5870ee88b7e05b84416f35f02ec45dc0bc132`。
- request {"npcId": 11, "missionId": 2207, "selection": 2117}（完整解析）：[000203-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000203-request-14503.bin)，SHA-256 `1465dd447cc277f3a99b272036c1a35bbf1eafab0e4f51927010efa8896b9a1b`。
- response {"result": 0, "npcStatus": [{"npcId": 11, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 550, "level": {"exp": 1250, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 20303, "slotPos": 34}, {"mode": 1, "quantity": 1, "itemId": 20003, "slotPos": 35}, {"mode": 1, "quantity": 1, "itemId": 2117, "slotPos": 36}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000206-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000206-response-14503.bin)，SHA-256 `d5b3f6ea293de2553e70f903eea6d88183fd735200606326b7532f18c9b9d839`。
- request {"npcId": 16, "missionId": 2243, "selection": -1}（完整解析）：[000218-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000218-request-14503.bin)，SHA-256 `721c49fca7608ce6263c5351392688cf1ca13238c102772b89cde0a92775b35d`。
- response {"result": 0, "npcStatus": [{"npcId": 16, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 0, "level": {"exp": 0, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000221-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000221-response-14503.bin)，SHA-256 `d1f2b47619fb116083962987a741013bd82467a6f6031db870a338270edfd013`。
- request {"npcId": 13, "missionId": 2208, "selection": -1}（完整解析）：[000228-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000228-request-14503.bin)，SHA-256 `615ff8a220fdcd472d1a874fde8e815010c456b3611e54245fcd4e4c5229c33b`。
- response {"result": 0, "npcStatus": [{"npcId": 13, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 500, "level": {"exp": 1000, "gainLevel": 1, "currentExp": 902, "expMax": 2200, "sp": 50, "cp": 3, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 20103, "slotPos": 37}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000235-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000235-response-14503.bin)，SHA-256 `d113a6755744d9a83898da39beeb26ca9e210c353a730291acdfab46a498b695`。
- request {"npcId": 13, "missionId": 2209, "selection": 20004}（完整解析）：[000258-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000258-request-14503.bin)，SHA-256 `28a061e57aa509e83a9098ff3b4c2c2b69485e59ae43fb70414bdb08e4f093f2`。
- response {"result": 0, "npcStatus": [{"npcId": 13, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 500, "level": {"exp": 500, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 2009, "slotPos": 38}, {"mode": 1, "quantity": 1, "itemId": 2109, "slotPos": 39}, {"mode": 1, "quantity": 1, "itemId": 20004, "slotPos": 40}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000261-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000261-response-14503.bin)，SHA-256 `16876e78400b0c8ffcfd095be4a6d3d805ee8b1c1a5503e445766c4d15823f79`。
- request {"npcId": 16, "missionId": 2210, "selection": 2102}（完整解析）：[000299-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000299-request-14503.bin)，SHA-256 `41ccc34e5dc356c136234d45e68d4840cd8ed578f0de5459d40d4eff116f4323`。
- response {"result": 0, "npcStatus": [{"npcId": 16, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 500, "level": {"exp": 500, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [{"mode": 1, "quantity": 1, "itemId": 2102, "slotPos": 41}], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000302-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000302-response-14503.bin)，SHA-256 `1b1de5d03d59df01f61df4347c2902aa169a8497ba30a99d28a76f462cb4dcdc`。
- request {"npcId": 1, "missionId": 2211, "selection": -1}（完整解析）：[000321-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000321-request-14503.bin)，SHA-256 `bf4b7d4a93fe78d83fa3572728c5c73a5e1a24f738f64fdc3852eeba1ee6d77f`。
- response {"result": 0, "npcStatus": [{"npcId": 1, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 200, "level": {"exp": 200, "gainLevel": 0, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000325-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000325-response-14503.bin)，SHA-256 `664a7ff8cead45ec1277933c1535a652e9e4e9e613bbf921515ca5556eb839b2`。
- request {"npcId": 2, "missionId": 2212, "selection": -1}（完整解析）：[000334-request-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000334-request-14503.bin)，SHA-256 `d8b2474d18a967b1e133493a514b99f30b6f2bb4a7f1e324d162f4feafb582e0`。
- response {"result": 0, "npcStatus": [{"npcId": 2, "sign": 3}], "reward": {"finishMission": true, "costMoney1": 0, "costMoney2": 0, "costMoney3": 0, "gainMoney2": 0, "gainMoney3": 300, "level": {"exp": 300, "gainLevel": 1, "currentExp": 226, "expMax": 2800, "sp": 60, "cp": 3, "exp2": 0, "gainLevel2": 0}, "hasPetReward": false, "removedItems": [], "items": [], "refreshCity": false, "hasCountryReward": false}}（完整解析）：[000342-response-14503.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000342-response-14503.bin)，SHA-256 `58309d1d29d7eae1fc323a092a1d573a41752957207b6aa366fd33377e5ad3ce`。

后续解码校验（同一批原始帧）：本轮 10506、14501、14502、14503 和 12502 响应均已完整解析到帧尾，覆盖怪物、战斗组、实际奖励物品、升级和 NPC 标记；12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量，原始字节保留；此前未确认边界已消除。以上更新替代本节本轮早期的“尾部未完整解码”边界。完整字段和原始哈希见 [解码报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

## 仍需补充的代表性样本

- `10002`：重名、非法名称和其他失败响应。
- `10506`：不同地图，以及 `dataFlag` 各数据块的完整解码。
- `12501`：进入失败响应。
- `12502`：失败、逃跑、升级、物品掉落、宠物或伙伴参战、地图变化。
- `14501`：已解析世界村 1100～1110 的完整任务对象；仍需其他地图、非空 AI 和其他条件类型样本。
- `14502/14503`：失败、重复操作和背包已满分支。
- `20300/20301`：非空出战伙伴列表；当前空样本不作为代表性数据。

## `2` 登录成功中的世界村 NPC 列表

2026-09-06，用户重启小游戏重新进入世界村。本轮登录响应包含 `DATA_NPC_BASE=1024`：运行时地图 `30555`、原始地图 `302`、玩家 `(13,8)`、dataFlag `5315966`，NPC 数据块偏移 `[2840,3542)`，10 个对象全部按小游戏 `parseNPCData/fromBytes` 解析。含村长 `id=1/model=5003/(12,12)`，主对话“这里是世界之村！”，以及其他 9 个对象的模型、对话、任务条件和菜单。

原始响应：[000011-response-2.bin](../captures/miniapp-protocol/session-20260906-090442/frames/000011-response-2.bin)，SHA-256 `438920d59844273b73aa3627a883dfcac432bbaf5f35977ad180cb1002480ced`。该响应的全部其他角色数据块尚未完整解码，不能声称整帧零剩余。完整结果见 [获取报告](世界村在线NPC获取.md) 与 [NPC 清单](../captures/miniapp-protocol/session-20260906-090442/npcs.json)。Android 需单独匹配资源与地图，不能重放此帧。

## 2026-09-06 世界村任务补采

用户指定改用 flue。会话 `session-20260906-104921` 保存 107 个完整游戏帧、23 个未拆分载荷，原始文件和 SHA-256 见该目录 `analysis.json`。按已有忽略列表分类；未知消息保留待确认，本轮不增加忽略项。写方向在重连后未完整还原，只有心跳/校时请求；不能将本轮业务响应冒充完整请求响应对。

- `14501`：村长、云裳、东海、小悦的任务列表补齐 1100～1106；结合历史响应得到世界村 1100～1110 的 11 个定义。东海 `000075-response-14501.bin` SHA-256 `da0edf77532a4859abf892b1a8fee2753b2b106f27fba10ae95b7c72871dc85a`，确认 1103 要求哈士奇 21×1，奖励铜币 400，下一任务 1104。
- `14591`：1100 起始任务 `000058-response-14591.bin` SHA-256 `0ff7094b2eab9a0929bdcebec13ac7dc3fb5620d4e46809c0edf4dfde25dc0ac`。`canAccept` 和击杀计数属于采集时状态，不能固化为本地任务规则。
- `2`：`000025-response-2.bin` SHA-256 `11fc89c37de2cb07fcf5c8904aabaa2a87c164dd441774ad03126fdb55ea8111`。偏移 3159、3362 是怪物 21、22，最大生命分别 60、80；偏移 1700 是开山剑模板。整条登录响应尚未全部解码。
- `12502`：`000085-response-12502.bin`、`000104-response-12502.bin` 分别给两场胜利经验 10、20；后者 SHA-256 `7090de1d4ee7a4ea126d8879c36a9f2f5e466397a39ab9357bf39ddc35a638b1`。失败/逃跑仍没有动态样本，本地仅在原战斗引擎胜利时更新任务。
- `14503`：历史 `in-game/session-20260905/frames/000024-response-14503.bin` 的偏移 64～172 已完整读取体质头巾模板，物品 25、类型 0、防御 6/6/13、属性 3+3。其 SHA-256 为 `cac412ee4c2414ad3a9f8b50d214d7fa4c488fbaf1186ca6e544a38f8c7e429f`。任务奖励简表不包含这些装备属性，不能直接作为完整背包物品。

复现入口：`parse_miniapp_missions.py --export-assets`、`analysis-tools/extract-village-equipment.py`。它们输出本地 JSON 业务数据；不向 Android 重放小程序帧。

## `13535` 查看引用任务详情：离线角色失败分支

2026-09-06 单次查询任务 1106。静态证据 `wechat-miniapp/subpackages/main/main.min.js:20215` 的 `createChatSeeMission` 写入 `actorId:int + count:byte + missionIds:short[]`；`:58686` 的调用方使用被查看角色 ID。此消息读取任务详情，不发送聊天。

本次试探 `actorId=0, count=1, missionIds=[1106]`，请求 [000002-request-13535.bin](../captures/miniapp-protocol/task-1106-query/frames/000002-request-13535.bin)，SHA-256 `25dee63989be457e3ee6a554e5c46e0dc1132560611adfbcb4bc5858fe264399`。

实测响应 [000004-response-13535.bin](../captures/miniapp-protocol/task-1106-query/frames/000004-response-13535.bin)，SHA-256 `aac2ddbaad6340d20d241931148d3d78fe750f51a93b1464d35babb7313bf752`。头部后为 `result:byte=-1`、三字节字符串长度 `10`、UTF-16BE 文本“对方已经下线无法查看”，完整消费 28 字节。没有返回任务对象；不能据此判断 1106 不存在，也不能确认使用任意在线角色 ID 即可读取任意任务。

会话使用 flue，PID 39904；共 8 帧（4 请求、4 响应），其中 6 帧为心跳/校时，未解析载荷 0、fatalError=null。独立写入返回 17 字节，原心跳写入仍返回 10 字节；查询后心跳与校时均继续响应，采集结束后进程存活。证据与可重复入口见 [采集说明](微信小游戏明文协议采集.md) 的单次查询章节。

### 使用当前真实角色 ID：任务不可查看分支

同日从小游戏角色资料读取 `a2586968`，截图 [role-profile.png](../captures/miniapp-protocol/task-1106-role-query/role-profile.png)。源码 `main.min.js:26130 getClientUid` 经 `:6085 getAreaID` 添加区域前缀，数字部分保持原 id，故查询 actorId=2586968；不得沿用历史角色 ID，也不得把 0 当作当前角色。

对 1106 的单次请求 [000002-request-13535.bin](../captures/miniapp-protocol/task-1106-role-query/frames/000002-request-13535.bin)，SHA-256 `5d9354a5be3887a851c9109bc66325f376d15e893a3f9fc43f2b865b4a72292a`；响应 [000004-response-13535.bin](../captures/miniapp-protocol/task-1106-role-query/frames/000004-response-13535.bin)，SHA-256 `62f115b97918b856f272daac7839db0a5f54392ba580c83a0fa68a88c7def277`。返回 `result=-2`、字符串长度 11、UTF-16BE 文本“对方的任务已经不可查看”，完整消费 30 字节，未返回任务数据。

当时地图界面显示“初入世界 6”，本地任务定义对应 1105。推断该查询受被查看角色的任务状态约束，不能作为任意任务配置导出接口；尚未用角色已接任务验证成功分支。会话 flue/PID 39904 共 10 帧，1 次业务请求和 1 次业务失败响应，其余 8 帧心跳/校时，未解析载荷 0、fatalError=null，查询后心跳继续正常。全部帧哈希通过校验，解析结果见同目录 result.json，missions.json 未修改。

## `14591` 主线任务状态推送

### 2026-09-06 17:43 flue 补采

主线状态依次推送 2199、2201、2202、2203、2204、2205、2206。每个任务对象完整消费，mapId/acceptMapId 均为 4；运行时 canAccept 不应固化为离线规则。 本轮不包含世界之门 NPC 基础列表；完整范围见 [会话报告](../captures/miniapp-protocol/session-20260906-174356/result.md)。

- response {"missionId": 2199}：[000020-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000020-response-14591.bin)，SHA-256 `368fb209bf28ca8dfbcd8a9386fb3eb1062fe9095ea7e9dfacf29c4f0d74d024`。
- response {"missionId": 2201}：[000025-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000025-response-14591.bin)，SHA-256 `c6c42230dfa0af43c9791328d62039332b130a267a895ed4bc62af460916dc0a`。
- response {"missionId": 2202}：[000040-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000040-response-14591.bin)，SHA-256 `d2f29dfce2ed6155f02839e068ac1e3f7ec9a7020502cf66f3abaaea3c1bd438`。
- response {"missionId": 2203}：[000066-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000066-response-14591.bin)，SHA-256 `8ba9852df8b9d46a648d044f1051e7d988263a6b71629556b382b947911a5fa6`。
- response {"missionId": 2204}：[000077-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000077-response-14591.bin)，SHA-256 `f55e0b58735982acc9c1593714c55f8792ef317f5902e15d136174c04338fd73`。
- response {"missionId": 2205}：[000103-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000103-response-14591.bin)，SHA-256 `5db167aa423a2dfa7585fcd4501682b38bf1278d2b8e91a77716f7001322f73d`。
- response {"missionId": 2206}：[000119-response-14591.bin](../captures/miniapp-protocol/session-20260906-174356/frames/000119-response-14591.bin)，SHA-256 `ac2e81b179d587a3c1631bc18a57f5727ca01206b9a6581c86023ada86cad793`。

### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- response {"missionId": 1124}（完整解析）：[000026-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000026-response-14591.bin)，SHA-256 `6b5a721aecbff4672ce729cd885d0747f3fe550daf0bbecbe5756b6740ba6330`。
- response {"missionId": 2207}（完整解析）：[000179-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000179-response-14591.bin)，SHA-256 `76a898fb7d89bdcb918f4f61d6b6486efade09db52e311810e652402d9edd65d`。
- response {"missionId": 2243}（完整解析）：[000205-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000205-response-14591.bin)，SHA-256 `c548dce92b7945f43b0f0245671c08f38ddfb48936ea2b531fb7f92bfbc4200d`。
- response {"missionId": 2208}（完整解析）：[000219-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000219-response-14591.bin)，SHA-256 `368db77789deaaed420ba1074d79746bd7cb3ac893c6e6f9be30c17e9bf1e48c`。
- response {"missionId": 2209}（完整解析）：[000233-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000233-response-14591.bin)，SHA-256 `4b5177d663a03082dc86e2652396e9b0ff4f905c19b2f229464bc2164c143431`。
- response {"missionId": 2210}（完整解析）：[000260-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000260-response-14591.bin)，SHA-256 `fd1d552c3032523e7ed9041f9f818f6d9a3c14189af7151c89d4b103dd0bf4bb`。
- response {"missionId": 2211}（完整解析）：[000301-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000301-response-14591.bin)，SHA-256 `a6704328153ae90e3c0ed8785121badc15df334f98e0c8466d73b7c87ebf7a40`。
- response {"missionId": 2212}（完整解析）：[000324-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000324-response-14591.bin)，SHA-256 `2c03cc1e43b17d944a33c790921797172c39b0ac5ed43411c039c6b0996c4a88`。
- response {"missionId": 2213}（完整解析）：[000340-response-14591.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000340-response-14591.bin)，SHA-256 `a1598402ff6b834cc69a1a1cc0e659a149140c11d33ad2c4cbaeea6e67afd4fc`。

## `11003` 角色加点


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

字段依据 main.min.js:19499、88692，成功响应完整消费；失败分支未采集。

- request {"attributes": [[10, 2]]}（完整解析）：[000381-request-11003.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000381-request-11003.bin)，SHA-256 `890469c4dea458529e197c1fe86bb50af2df65ae7f111436ceff3feeaea700b8`。
- response {"result": 0, "cp": 19, "hp": 404, "mp": 260, "str": 4, "con": 5, "agi": 5, "ilt": 8, "wis": 10, "money1": 0, "money2": 0, "money3": 5821, "combatPower": 4295}（完整解析）：[000382-response-11003.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000382-response-11003.bin)，SHA-256 `0b069e5b904a423964b8c10e16b4bafc8deea0f3b838df5a3083535003cc7748`。

## `11171` 新手引导记录


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

字段依据 main.min.js:201647。仅上行引导记录，未收到同号响应。

- request {"mode": 0, "guideIds": [300101, 300102, 300103, 300104, 300105, 300106]}（完整解析）：[000209-request-11171.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000209-request-11171.bin)，SHA-256 `c01cd68931cff8e876f836d78b92dd2b05c5afe40f5d707d9fbb74dfc89007f8`。

## `12001` 背包穿戴操作


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- request {"bagMode": 9, "operation": 1, "slotPos": 38, "quantity": 1, "equipPosition": -1, "confirm": false, "itemType": 2}（完整解析）：[000263-request-12001.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000263-request-12001.bin)，SHA-256 `6bbe58efe98fa5bbff07929177f5df200b12d172be36f44008746dee1cb355d1`。
- response {"bagMode": 9, "itemSetData": [], "skillData": {"updates": [], "autoSkillIds": [0, 0, 0, 0], "activeSkillId": 731}}（完整解析）：[000265-response-12001.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000265-response-12001.bin)，SHA-256 `046796ad4cef2743830d1d94c8364b46897362404b9c4da6739ca80c8e8340a9`。
- request {"bagMode": 9, "operation": 1, "slotPos": 39, "quantity": 1, "equipPosition": -1, "confirm": false, "itemType": 2}（完整解析）：[000267-request-12001.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000267-request-12001.bin)，SHA-256 `34b637a44bd83214fcc53d67c7227ac4883c4d3c9626faabb91838514ad885a7`。
- request {"bagMode": 9, "operation": 1, "slotPos": 40, "quantity": 1, "equipPosition": -1, "confirm": false, "itemType": 19}（完整解析）：[000271-request-12001.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000271-request-12001.bin)，SHA-256 `4827910d4a1ec442d0553ac2f14c7b945133b002b34da0489654bafe07fd96fc`。
- request {"bagMode": 9, "operation": 1, "slotPos": 41, "quantity": 1, "equipPosition": -1, "confirm": false, "itemType": 1}（完整解析）：[000304-request-12001.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000304-request-12001.bin)，SHA-256 `4d8859cce2589382540ca26b26c249466ec5e8948981a017ecb6dbbdd09b1bc6`。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"bagMode": 4, "operation": 3, "slotPos": 31, "quantity": 1, "confirm": false, "itemType": 28}（完整读取）：[000155-request-12001.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000155-request-12001.bin)，SHA-256 `cbdcfe02f4fd024ea4fd9354d2f58ef5ac6083b77ebac52d225619d9f585ab30`。
- response {"bagMode": 4, "extraConsumedQuantity": 0, "items": [{"id": 1324, "name": "20级秘宝盒", "quantity": 1, "slotPos": 31}, {"id": 40006, "name": "宠物经验卷(30000）", "quantity": 1, "slotPos": 43}, {"id": 40007, "name": "人物经验卷(30000）", "quantity": 1, "slotPos": 44}]}（完整读取）：[000156-response-12001.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000156-response-12001.bin)，SHA-256 `83e3d8448b147c11cdf41c3321cd174bb1a240dc44811026bf397f9e7f9cba14`。

### 2026-09-07 道具使用补采

范围、槽位连续性和退出恢复边界见 [会话报告](../captures/miniapp-protocol/session-20260907-050333/result.md)。

- request {"bagMode": 11, "operation": 3, "slotPos": 44, "quantity": 1, "confirm": false, "itemType": 28}（槽位由前序会话确认为 40007“人物经验卷(30000）”；完整读取）：[000012-request-12001.bin](../captures/miniapp-protocol/session-20260907-050333/frames/000012-request-12001.bin)，SHA-256 `fbc0f1178b8aad5cd42ae15b32ff735860765f1ed31a87ac61b5e45e4feee462`。
- response {"bagMode": 11, "exp": 30000, "gainLevel": 5, "currentExp": 6686, "expMax": 9300, "sp": 450, "cp": 15, "exp2": 0, "gainLevel2": 0}（完整读取）：[000027-response-12001.bin](../captures/miniapp-protocol/session-20260907-050333/frames/000027-response-12001.bin)，SHA-256 `c5ed7b1199e6f256bc31fd919116b016f0a1d8789ad2d298c4b178a582901200`。
- request `bagMode=4、operation=3、slotPos=31、quantity=1、type=28` 与上一会话已登记的新手大礼包请求逐字节相同；当前槽位由前序响应确认为 1324“20级秘宝盒”，不重复登记相同请求哈希。
- response {"bagMode": 4, "extraConsumedQuantity": 0, "items": [{"id": 413, "name": "加1敏捷宝石(绑）", "quantity": 1, "slotPos": 31}]}（完整读取）：[000035-response-12001.bin](../captures/miniapp-protocol/session-20260907-050333/frames/000035-response-12001.bin)，SHA-256 `c6177d1dcd2bad1738e433c4f0e924d5998a48636913cb740e434c42b448725c`。

## `12019` 装备修理


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- response {"result": 1, "money3": 792}（完整解析）：[000022-response-12019.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000022-response-12019.bin)，SHA-256 `f992c554db02b16012a634afbcbeec8d217e811b70978b1b10a6429d11408630`。
- response {"result": 1, "money3": 5499}（完整解析）：[000337-response-12019.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000337-response-12019.bin)，SHA-256 `c3c4d87aa121ac172f1e35b5ea75143451892495f280bbaa09bdc54cc44e40eb`。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"slotPos": -1}（完整读取）：[000171-request-12019.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000171-request-12019.bin)，SHA-256 `c98d4a25203170103b034d3cb0465951154e017ad733642cf69c110fa0c0faa5`。
- response {"result": 1, "money3": 5835}（完整读取）：[000174-response-12019.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000174-response-12019.bin)，SHA-256 `9a45d7a6a7b2ce17f312106d92989663d70aa44aeb24f0965cefe228ed40fcd4`。

## `12049` 角色战力及忽略属性


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- response {"combatPower": 3899, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000021-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000021-response-12049.bin)，SHA-256 `219b6741ec8d3a2ac33c0ffa1fb023a7a4b96c3dc0b94d01bc3c17a2fddb8133`。
- response {"combatPower": 3942, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000084-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000084-response-12049.bin)，SHA-256 `e41cd725c9ffe3684aea9c709b74b751a0bf91c56ba4d76e68358b4178e4c506`。
- response {"combatPower": 3985, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000148-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000148-response-12049.bin)，SHA-256 `e3b0f6bcd65d181cd6c467c550e310b9a6fd30359bce11bbb2e5f7b5818a7670`。
- response {"combatPower": 4028, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000180-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000180-response-12049.bin)，SHA-256 `81beaea078e34444e17b6ead8c36ba7a8fe63b8a972c662da7d5fe372b207835`。
- response {"combatPower": 4071, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000234-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000234-response-12049.bin)，SHA-256 `149ec35a5ca30eb175f8fe27ab67957e0593ec00b52814b888b473564f47d15d`。
- response {"combatPower": 4130, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000266-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000266-response-12049.bin)，SHA-256 `7de1b063c1f3d24f15fca0ba6fe4262b3c4391f16181ffa42119d32d3a2e9445`。
- response {"combatPower": 4122, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000270-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000270-response-12049.bin)，SHA-256 `68be574a21ce02456e31f207e73293d7191eff1f4c769f54b99376b8f7a8ac96`。
- response {"combatPower": 4176, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000274-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000274-response-12049.bin)，SHA-256 `3caea702c0d04ac9e721acc6088db67bb004cadc71f9c9b2c5b0753df16ec34b`。
- response {"combatPower": 4235, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000307-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000307-response-12049.bin)，SHA-256 `c37b8dc69981dce686033bb6bc14bdda6577541cd9db84b37eb15d2856fcef55`。
- response {"combatPower": 4278, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整解析）：[000341-response-12049.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000341-response-12049.bin)，SHA-256 `125332c817f93e94c8026091091ddb997d2a8248fb4571f8ed97e46a5b5634e3`。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- response {"combatPower": 4295, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整读取）：[000157-response-12049.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000157-response-12049.bin)，SHA-256 `fa42c46379fe00a7ad517fd4aaec395a43123e03137ab3f80128e4332f38b861`。

### 2026-09-07 道具使用补采

人物经验卷升级及随后使用秘宝盒时均同步相同战力值 4563；相同响应帧只登记一次。范围见 [会话报告](../captures/miniapp-protocol/session-20260907-050333/result.md)。

- response {"combatPower": 4563, "ignore_back": 0, "ignore_magic_back": 0, "ignore_block": 0, "ignore_insight": 0, "ignore_wil": 0, "ignore_touch": 0}（完整读取）：[000028-response-12049.bin](../captures/miniapp-protocol/session-20260907-050333/frames/000028-response-12049.bin)，SHA-256 `e0addb6e99f314b2bc044ad5fa5119d321bf4df770d9b15974e2c4dd66ce2f89`。

## `25013` 货币余额同步


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 792}]}}（完整解析）：[000020-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000020-response-25013.bin)，SHA-256 `9856a0377c7e651a1d56dbc204f969d769d3c6ec40e3a8c54033c2a289aca2ec`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 892}]}}（完整解析）：[000066-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000066-response-25013.bin)，SHA-256 `c15fb1037934a9fe61f6cdc4485bbe95a3598aee2f57a6e2d536e55cf08709da`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 992}]}}（完整解析）：[000082-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000082-response-25013.bin)，SHA-256 `c15eea0a66ecb005e17104223f9d59e1fbd0349cd3c8e15c55a9a2bdc171c8df`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 994}]}}（完整解析）：[000100-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000100-response-25013.bin)，SHA-256 `83c700b171ad386ae64725b5ec0fd07115e743c78a9384722e35bb317aff29db`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 1394}]}}（完整解析）：[000109-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000109-response-25013.bin)，SHA-256 `3dd5b18cc7160f7b3453fd4a53ce9d070b4584d79ee2e05c70d1aa464bf2fa87`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 1494}]}}（完整解析）：[000121-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000121-response-25013.bin)，SHA-256 `f559fb3e03757bf4cb9be25edeaa7b9de93beb85bd02f89a0366a4e32df4d988`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 1496}]}}（完整解析）：[000136-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000136-response-25013.bin)，SHA-256 `0bee093e9ae6c0a145f8b2469d793a57b13d30214d96d99b54f18bfbcef678bd`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 2496}]}}（完整解析）：[000146-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000146-response-25013.bin)，SHA-256 `992736f62d36efc779d168a2205217845c71d0b9d6279e7abd5dcd7a8fe510a1`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 2996}]}}（完整解析）：[000160-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000160-response-25013.bin)，SHA-256 `eeb6898ea5de6dd3bb3c184a88a6e92ba207fa2e9fe3d423b52f185e3ad46526`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 3246}]}}（完整解析）：[000177-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000177-response-25013.bin)，SHA-256 `ed517bfed0724f21adb516033f16e884100070eaf3aab4dd3b77a5082fa6d21b`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 3257}]}}（完整解析）：[000195-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000195-response-25013.bin)，SHA-256 `d43d30fdcf8428f942ff4801aa41e24422493036730303af7f950d077c8cb6d9`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 3807}]}}（完整解析）：[000204-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000204-response-25013.bin)，SHA-256 `33fc7733be5da8cb7ae518437100e441dfb6c706030534a26c4547d3a7b3bee0`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 4307}]}}（完整解析）：[000232-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000232-response-25013.bin)，SHA-256 `a49dedc0bd6f97ab178ce8c017256c1e8b4d51758c6464b690bc2a11e1bfa7c2`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 4310}]}}（完整解析）：[000251-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000251-response-25013.bin)，SHA-256 `f67672aab896bb0b629597618e56c682c0a3a48ba2ceec8b74a2cc20b9bfdd91`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 4810}]}}（完整解析）：[000259-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000259-response-25013.bin)，SHA-256 `69256887463ddae19c6ebe945da50c53ce7ef8fe1775b27188562c8464244185`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 4813}]}}（完整解析）：[000291-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000291-response-25013.bin)，SHA-256 `f40863efabc6f21fdd0eaf09cb5c7cf88eb9df7201385fc4b941d87f4553a28c`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5313}]}}（完整解析）：[000300-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000300-response-25013.bin)，SHA-256 `1e1fa4c6f07de4df83e2cd4a3e8bb061ed3b644090ce8f7d04c14fc323989161`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5513}]}}（完整解析）：[000323-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000323-response-25013.bin)，SHA-256 `889a3e5ad9303ca87ad9c3d11be189dfa2c149201e65b05961f3522a53136719`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5499}]}}（完整解析）：[000335-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000335-response-25013.bin)，SHA-256 `29c44722dc9e0ac8f5242349d74ef597695e8fd7b12c9c262c1eab80fcc3f946`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5799}]}}（完整解析）：[000339-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000339-response-25013.bin)，SHA-256 `f67878d08a8036bece3ddfd5f8bd947fdd5766f09daec6781a7840252ef39bb9`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5810}]}}（完整解析）：[000356-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000356-response-25013.bin)，SHA-256 `4d15f534ba3c060cf2ece2c7e106084da27d2f8982f38390ca678ca1ebe00c6d`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5821}]}}（完整解析）：[000371-response-25013.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000371-response-25013.bin)，SHA-256 `ee58c2d8525065e1fd4c8e4df9a82b03a84f90b15c9deb007d85db00b36d4688`。


### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5834}]}}（完整读取）：[000041-response-25013.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000041-response-25013.bin)，SHA-256 `33b9f97118ba5b7058eec3e543b02e137a533bc1e6dcc4f8ab622c55e3f31eed`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5334}]}}（完整读取）：[000072-response-25013.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000072-response-25013.bin)，SHA-256 `afad9745fd1d4be176165c8c851ec2e18e79dfb6d89a6a7b816263755acaeb52`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5845}]}}（完整读取）：[000097-response-25013.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000097-response-25013.bin)，SHA-256 `13f83b6f08aaa2b13cb350dfb3ff208d9baac4595873c36b4065fa3551fa06a9`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5856}]}}（完整读取）：[000111-response-25013.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000111-response-25013.bin)，SHA-256 `5d257039bd6743cdbf43af804a3cd69e9c43d1f76328fd2e0e63da5161dba368`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5867}]}}（完整读取）：[000129-response-25013.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000129-response-25013.bin)，SHA-256 `a18a8e8f0d58d3ce7da6b3e2a3188b58417fa63411ad184090fd12cf086ca97e`。
- response {"schema": "s2c_ResUpdate", "data": {"info": [{"type": -1, "num": 0}, {"type": -2, "num": 0}, {"type": -3, "num": 5835}]}}（完整读取）：[000172-response-25013.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000172-response-25013.bin)，SHA-256 `f082d02880995d8bd2a37dad9becbc2a02809640eaa8086c858f2981bc63a77b`。

## `32109` 图鉴属性生效状态


### 2026-09-06 19:49 迎客殿与神剑广场补采

迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/session-20260906-194937/result.md)。

- response {"schema": "s2c_HandBookPowerInfo", "data": [{"group": 1, "attributeCount": 15, "effectiveCount": 0}, {"group": 2, "attributeCount": 12, "effectiveCount": 0}, {"group": 3, "attributeCount": 4, "effectiveCount": 0}, {"group": 11, "attributeCount": 1, "effectiveCount": 0}]}（完整解析）：[000264-response-32109.bin](../captures/miniapp-protocol/session-20260906-194937/frames/000264-response-32109.bin)，SHA-256 `68e6de8b687b0d8f4e5977ce200b8d8701601d237ee424c19b96af4e5ba4e8b7`。

## `11501` 套装属性

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"suitIds": [74]}（完整读取）：[000028-request-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000028-request-11501.bin)，SHA-256 `67cccd33861110a0ab45b48a996e8b80b03a4102eeeb0973540c4e6055e56368`。
- response {"suits": {"countWire": 0, "entries": []}}（完整读取）：[000030-response-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000030-response-11501.bin)，SHA-256 `cad203b101aa18557936ee9eb40dd704f3c4d21a670dc18b72505f4665684c43`。
- request {"suitIds": [14, 74, 104, 44, 74, 44]}（完整读取）：[000057-request-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000057-request-11501.bin)，SHA-256 `f1e02ccd90042fad966e76f9d162e91103e7a0dd021a14fceeb85000b695de0a`。
- response {"suits": {"countWire": 2, "entries": [{"id": 44, "effects": [{"requiredPieces": 7, "power": 55, "value": 25, "vipLevel": 0, "active": 1}]}, {"id": 14, "effects": [{"requiredPieces": 7, "power": 7, "value": 10, "vipLevel": 0, "active": 1}]}]}}（完整读取）：[000058-response-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000058-response-11501.bin)，SHA-256 `4ec953089b68ac399454a3e017d2af2282f9c3cb05d81afe74b2bc183556f491`。
- request {"suitIds": [-79]}（完整读取）：[000191-request-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000191-request-11501.bin)，SHA-256 `d63393e22f924bd8155180a770c023394b833abd22f83ae6912d4f50bb2d6ef0`。
- response {"suits": {"countWire": 1, "entries": [{"id": 177, "effects": [{"requiredPieces": 1, "power": 27, "value": 20, "vipLevel": 4, "active": 0}, {"requiredPieces": 2, "power": 130, "value": 4, "vipLevel": 5, "active": 0}]}]}}（完整读取）：[000192-response-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000192-response-11501.bin)，SHA-256 `2c7e8334f6fe5c1de8ef9614cfe740d7d82b9d2c46bd75f6a49fdd29ba98fe61`。
- request {"suitIds": [-78, -82]}（完整读取）：[000195-request-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000195-request-11501.bin)，SHA-256 `8e3faca60ade1e0ede8cc565f3547652c3a9998d86713013fd8eaa93805ac654`。
- response {"suits": {"countWire": 2, "entries": [{"id": 178, "effects": [{"requiredPieces": 1, "power": 171, "value": 400, "vipLevel": 8, "active": 0}]}, {"id": 174, "effects": [{"requiredPieces": 1, "power": 171, "value": 400, "vipLevel": 8, "active": 0}]}]}}（完整读取）：[000196-response-11501.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000196-response-11501.bin)，SHA-256 `0d0a5cd5743c18968640f36bd3d1fb83e226d7f982cf2737f98452b0b79cd23a`。


## `11006` 角色设置

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"setting": 0}（完整读取）：[000029-request-11006.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000029-request-11006.bin)，SHA-256 `68aa301f732f9590570babc42e8678e33f68785ed000d49eed543da7348f9f2c`。
- response {"ackByte": 0}（完整读取）：[000031-response-11006.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000031-response-11006.bin)，SHA-256 `61cac423705b74ae9a9383a4af9eb4025e857d4860b5d80047abea575e8e6c31`。


## `12004` 物品商店库存

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"shopId": 200}（完整读取）：[000055-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000055-request-12004.bin)，SHA-256 `d76d75e071f1fbf35b033afdb476297e95bd388f95fe7452f9162b8f8d4049a1`。
- response {"shopId": 200, "itemCount": 18, "detail": "本会话 shop-inventory.json"}（完整读取）：[000056-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000056-response-12004.bin)，SHA-256 `4b6af4c261213ba7bb80911546b62301b85e39837d4f91bb57bf2111868f793d`。
- request {"shopId": 207}（完整读取）：[000067-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000067-request-12004.bin)，SHA-256 `10c345b4826c1ef17cb44a08799c1f9ffd3d3e95f7734286d80cac1a25e77d07`。
- response {"shopId": 207, "itemCount": 9, "detail": "本会话 shop-inventory.json"}（完整读取）：[000068-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000068-response-12004.bin)，SHA-256 `898795b445c9d6d0ee220119e2520b8efcf6816e8a5d93da2803548497c8986e`。
- request {"shopId": 202}（完整读取）：[000083-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000083-request-12004.bin)，SHA-256 `c49600fd8146450e58dde3d7e590b0bed517a781e870c983e57359cdfba4db5d`。
- response {"shopId": 202, "itemCount": 4, "detail": "本会话 shop-inventory.json"}（完整读取）：[000084-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000084-response-12004.bin)，SHA-256 `7506669ba799081613ad67c41c366af91f9f592e2f3d7582a966aa45d46a5cc5`。
- request {"shopId": 50}（完整读取）：[000185-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000185-request-12004.bin)，SHA-256 `5f03c5131538ea84b8e83fb49f125c3b33ce1463deb5bdd3aab80a4fdeee662a`。
- response {"shopId": 50, "itemCount": 111, "detail": "本会话 shop-inventory.json"}（完整读取）：[000186-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000186-response-12004.bin)，SHA-256 `2c4d4bb4fa6b8e3ff281cc26af8d01307059c869874fb9f3baa70231606088c0`。
- request {"shopId": 24000}（完整读取）：[000187-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000187-request-12004.bin)，SHA-256 `200e889d8d0832f21d7eaf4de652d2ffc8674d95e4863b39de0af6ff32a3d24a`。
- response {"shopId": 24000, "itemCount": 30, "detail": "本会话 shop-inventory.json"}（完整读取）：[000188-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000188-response-12004.bin)，SHA-256 `869aeee906695fdf1742bb9d30dc924e85a8181cfbf73d51dbf1fb6b61f9390f`。
- request {"shopId": 2154}（完整读取）：[000189-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000189-request-12004.bin)，SHA-256 `0b960534858350161acdcce0201f20bf1e43ffcc9664f21bdd60899c6ed101a4`。
- response {"shopId": 2154, "itemCount": 77, "detail": "本会话 shop-inventory.json"}（完整读取）：[000190-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000190-response-12004.bin)，SHA-256 `cc6ab4f38171a2f50a6d753f805f9e8cc0a440141a798c0de63f5c0cbe37b94e`。
- request {"shopId": 2000}（完整读取）：[000193-request-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000193-request-12004.bin)，SHA-256 `6e2fd6b1fff04bc0b099cad048b50a3c1b5f7497f87b6d690c44433d3cec1c76`。
- response {"shopId": 2000, "itemCount": 39, "detail": "本会话 shop-inventory.json"}（完整读取）：[000194-response-12004.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000194-response-12004.bin)，SHA-256 `f2bf06430f84812b0752011579b700252accc5c7936f98ea15474b01f90f7792`。


## `12005` 商店购买

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"shopId": 207, "itemId": 20803, "quantity": 1, "countryShop": false}（完整读取）：[000071-request-12005.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000071-request-12005.bin)，SHA-256 `8d4cce93b256fb4942463b3d1f7f99086fb5b4450b34626de9be659be60d5bcf`。
- response {"result": 0, "itemId": 20803, "shortWire": 42, "quantity": 1, "money1": 0, "money2": 0, "money3": 5334, "maxBuy": -1, "allCount": -1}（完整读取；shortWire 由原客户端读后丢弃，未命名其语义）：[000073-response-12005.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000073-response-12005.bin)，SHA-256 `faf3da91c016f6f0cae1480309c4cb6683952f54c4257270047a83372361d88e`。


## `12006` 商店出售

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"shopId": 207, "itemId": 20803, "slotPos": 42, "quantity": 1}（完整读取）：[000079-request-12006.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000079-request-12006.bin)，SHA-256 `67c164729e2ef05dcf889425ec95822001ebb259e8e55eddf6c4901f7d654d99`。
- response {"result": 0, "money3": 5834}（完整读取）：[000081-response-12006.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000081-response-12006.bin)，SHA-256 `779e12f5776fc1e0bfac6e814be21b2d07cf7bdb309f8449ae655ffd4af90ad9`。


## `14555` 活跃度提示

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- response {"text": "恭喜您完成活跃度内容\"鉴定一件装备\",获得20活跃度值。"}（完整读取）：[000082-response-14555.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000082-response-14555.bin)，SHA-256 `f9f2a28d03cdef64813c05665a70c47c08b1a47a3fe26ed26809cd7e77f6e9f1`。


## `12016` 物品模板详情

### 2026-09-06 世界之门奖励模板补采

三次单独查询成功，原件及逐帧 SHA-256 在 [gate-item-2017](../captures/miniapp-protocol/gate-item-2017/timeline.jsonl)、[gate-item-20304](../captures/miniapp-protocol/gate-item-20304/timeline.jsonl)、[gate-item-2002](../captures/miniapp-protocol/gate-item-2002/timeline.jsonl)。每次均有一对12016请求/响应和双向16001心跳。模板后出现非空套装表，不能固定假设 count=-1；解码至帧末的回归入口为 `analysis-tools/prepare-gate-gameplay.py`。发送边界见“微信小游戏明文协议采集”的“单次物品模板查询”。

### 2026-09-06 20:46 神剑广场商店与礼包补采

范围与字段边界见 [会话报告](../captures/miniapp-protocol/session-20260906-204639/result.md)。

- request {"itemId": 40006}（完整读取）：[000158-request-12016.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000158-request-12016.bin)，SHA-256 `db2d41177867ea9296f7606c126e3569330a1416b603238b8ad25e31944dbd66`。
- response {"result": 0, "id": 40006, "name": "宠物经验卷(30000）", "suits": {"countWire": -1, "entries": []}, "detail": "本会话 analysis.json"}（完整读取）：[000159-response-12016.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000159-response-12016.bin)，SHA-256 `32e3540a96c5516d113963280b091dbf868152b784ce6c5fec5976b218155f20`。
- request {"itemId": 1324}（完整读取）：[000161-request-12016.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000161-request-12016.bin)，SHA-256 `481fc35aade87c6079626eb7b195fc1b9c36f953f4759ae6e14ee93e5317d1ff`。
- response {"result": 0, "id": 1324, "name": "20级秘宝盒", "suits": {"countWire": -1, "entries": []}, "detail": "本会话 analysis.json"}（完整读取）：[000162-response-12016.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000162-response-12016.bin)，SHA-256 `5f2bcc64975f8ca0e4bcdafe058b772a82f080d98eff8ce0d9c24d76992fec41`。
- request {"itemId": 40007}（完整读取）：[000165-request-12016.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000165-request-12016.bin)，SHA-256 `582e1a5cdc9023f625a23c2e20eec03f35d3587e10eb0480f1dc6923601479ec`。
- response {"result": 0, "id": 40007, "name": "人物经验卷(30000）", "suits": {"countWire": -1, "entries": []}, "detail": "本会话 analysis.json"}（完整读取）：[000166-response-12016.bin](../captures/miniapp-protocol/session-20260906-204639/frames/000166-response-12016.bin)，SHA-256 `96285b8c8b43002a82764ff15612df162a890d3ebc22c78dcff4d83cc51381df`。

