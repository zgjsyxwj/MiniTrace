"""由本轮已验证解析产物生成报告，并按原始哈希增量登记协议。"""
import hashlib
import json
import re
import sys
from pathlib import Path

S = Path(__file__).resolve().parent
ROOT = S.parents[2]
sys.path.insert(0, str(ROOT))
from parse_miniapp_npcs import parse_block
from parse_miniapp_missions import parse
A = json.loads((S/'analysis.json').read_text(encoding='utf8'))
N = json.loads((S/'npcs.json').read_text(encoding='utf8'))
M = json.loads((S/'missions.json').read_text(encoding='utf8'))
rows = A['frames']
tasks = {m['task']['id']:m['task'] for m in M}
assert not any(r.get('parseError') for r in rows)
for m in M:
    assert hashlib.sha256((S/m['file']).read_bytes()).hexdigest() == m['sha256']
for m in N:
    assert hashlib.sha256((S/m['source']).read_bytes()).hexdigest() == m['sha256']

old_path = ROOT/'captures/miniapp-protocol/in-game/session-20260905/frames/000088-response-10506.bin'
old_data = old_path.read_bytes()
assert hashlib.sha256(old_data).hexdigest() == '5a436aa0ad479e4e237e85a927c48cb7da3a27d66ab4ce632ced33fe828fcb3a'
old_npcs = parse_block(old_data, 247)
assert old_npcs['endOffset'] == 2807 and old_npcs['npcCount'] == 29
old_task_path = ROOT/'captures/miniapp-protocol/in-game/session-20260905/frames/000085-response-14591.bin'
old_task_data = old_task_path.read_bytes()
assert hashlib.sha256(old_task_data).hexdigest() == 'adbe0cb50c694dbac6b60eeef394c349e900485120e3a26055affa3059855d92'
old_task = parse(old_task_data[7:7+int.from_bytes(old_task_data[4:7], 'big')])
assert (old_task['id'], old_task['mapId'], old_task['nextMissionId']) == (1125, 900, 2500)
comparison = {'maps': [], 'giftTasks': [tasks[1124], old_task], 'sources': [
    {'file': str(p.relative_to(ROOT)), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
    for p in (old_path, old_task_path)]}
for map_id, block in ((4, N[0]), (900, old_npcs)):
    p = ROOT/f'offline-source/wechat-miniapp/v63/assets/map/{map_id}.json'
    cfg = json.loads(p.read_text(encoding='utf8'))
    grid = [cfg['mW']//cfg['tW'], cfg['mH']//cfg['tH']]
    assert len(cfg['blocks']) == grid[0]*grid[1]
    comparison['maps'].append(dict(mapId=map_id, grid=grid, layers=len(cfg['layers']),
        tileGroups=len(cfg['tiles']), npcCount=block['npcCount'],
        npcs=block['npcs'], source=str(p.relative_to(ROOT)), sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
assert comparison['maps'][0]['grid'] == [40,40] and comparison['maps'][1]['grid'] == [30,40]
assert tasks[1124]['rewardItems'] == old_task['rewardItems']
comparison['sameNamedNpcs'] = sorted({n['name'] for n in N[0]['npcs'] if n['name']} & {n['name'] for n in old_npcs['npcs'] if n['name']})
(S/'map-comparison.json').write_text(json.dumps(comparison,ensure_ascii=False,indent=2)+'\n',encoding='utf8')

def frame_link(row, prefix=''):
    return f"[{Path(row['rawFile']).name}]({prefix}{row['rawFile']})，SHA-256 `{row['sha256']}`"

def brief_fields(row):
    f = row['fields'].copy()
    if f.get('schema') == 's2c_HandBookPowerInfo':
        f['data'] = [dict(group=g['group'], attributeCount=len(g['info']), effectiveCount=sum(bool(p['effect']) for p in g['info'])) for g in f['data']['infos']]
    if 'monsterData' in f:
        data = f.pop('monsterData')
        f.update(groupIds=[g['groupId'] for g in data['groups']], monsterIds=[m['id'] for m in data['monsters']],
            skillCount=len(data['skills']), aiCount=len(data['ai']))
    if 'reward' in f:
        reward = f['reward'] = f['reward'].copy()
        reward['items'] = [dict(mode=i['mode'], quantity=i['quantity'],
            itemId=i.get('item',{}).get('id',i.get('itemId')),
            slotPos=i.get('item',{}).get('slotPos',i.get('slotPos'))) for i in reward['items']]
    return json.dumps(f,ensure_ascii=False)

monster_events, reward_events, pending, current_maps = [], [], {}, {}
for row in rows:
    f = row.get('fields', {})
    connection = row['connection']
    key = (connection, row['messageType'])
    if row['direction'] == 'request' and row['messageType'] in (14502,14503,12502):
        assert key not in pending, '存在并发同号请求，不能仅按顺序配对'
        pending[key] = row
    if row['direction'] != 'response':
        continue
    if row['messageType'] == 10506 and f.get('result') == 0:
        current_maps[connection] = f['mapId']
    evidence = dict(sequence=row['sequence'], mapId=current_maps.get(connection), source=row['rawFile'], sha256=row['sha256'])
    if 'monsterData' in f and any(f['monsterData'].values()):
        monster_events.append(dict(**evidence, data=f['monsterData']))
    if key in pending:
        request = pending.pop(key)
        reward_events.append(dict(**evidence, messageType=row['messageType'],
            requestSource=request['rawFile'], requestSha256=request['sha256'],
            request=request['fields'], response=f))
assert not pending and len(reward_events) == 37
for filename, events in (('monsters.json',monster_events),('rewards.json',reward_events)):
    (S/filename).write_text(json.dumps(events,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
groups = {g['groupId'] for e in monster_events for g in e['data']['groups']}
monsters = {m['id'] for e in monster_events for m in e['data']['monsters']}
items = [i['item'] for e in reward_events for i in e['response'].get('reward',{}).get('items',[]) if 'item' in i]
assert (len(groups),len(monsters),len(items)) == (13,18,11)
assert [i['type'] for i in items if i['id'] == 40067] == [28]
battles = [e for e in reward_events if e['messageType'] == 12502]
assert len(battles) == 7 and sum(len(e['request']['plans']) for e in battles) == 13
assert all(p['action'] == 2 and p['skillId'] == 731 for e in battles for p in e['request']['plans'])
assert all(not e['response']['reward'][k] for e in battles for k in ('items','hasPetReward','hasEscortReward','expiredMercenaries','changeMap'))
assert all(e['response']['reward']['level']['gainLevel'] == 0 for e in battles)
assert sum(e['response'].get('reward',{}).get('level',{}).get('gainLevel',0) for e in reward_events) == 5
assert all(not e['response']['directSubmit'] for e in reward_events if e['messageType'] == 14502)
extra_types = {10509,11002,11014,11114,12001,12019,12049,20300,20301,25013,32109}
extra_rows = [r for r in rows if r['messageType'] in extra_types]
(S/'character-data.json').write_text(json.dumps(extra_rows,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
equipment_requests = [r for r in extra_rows if r['messageType'] == 12001 and r['direction'] == 'request']
assert [r['fields']['slotPos'] for r in equipment_requests] == [38,39,40,41]
books = [r for r in extra_rows if r['messageType'] == 32109]
assert len(books) == 4 and len({r['sha256'] for r in books}) == 1
assert [len(g['info']) for g in books[0]['fields']['data']['infos']] == [15,12,4,1]
assert all(not p['effect'] for g in books[0]['fields']['data']['infos'] for p in g['info'])
money_rows = [r for r in extra_rows if r['messageType'] == 25013]
assert len(money_rows) == 22
assert all([x['type'] for x in r['fields']['data']['info']] == [-1,-2,-3] for r in money_rows)

lines = ['# 迎客殿与神剑广场采集结果', '',
    'flue，PID 19804，2026-09-06 19:49～19:54（北京时间）。用户确认本轮完成后停止；进程已退出，summary 正常保存。', '',
    '386 个游戏帧（161 请求、225 响应）和 2 个未拆分载荷全部按时间线核对文件长度、SHA-256。fatalError=null，timedOut=false。', '',
    f"按既有忽略列表跳过 {A['ignoredFrames']} 帧：16001、16021、12533、20401、27403、27404。原始文件保留，没有新增忽略规则。", '',
    '## 地图与 NPC', '',
    '用户澄清：本次所说的“世界之门”指小游戏迎客殿，本轮已采到目标地图的 NPC 基础列表。实际协议为迎客殿 4 → 神剑广场 16。后续核对表明，本地地图 4 与恢复版当前采用的小游戏 900 资源在布局、NPC 和任务出口上均不同，不能仅改名称或编号替换。两份地图证据都来自小游戏，不足以归因为跨客户端改名。本轮没有创建角色或登录响应。', '',
    '| 地图 | 稳定编号 | 在线运行时编号 | 入口格 | NPC 对象数 | 块范围 |', '| --- | ---: | ---: | --- | ---: | --- |']
for n in N:
    m = n['map']
    lines.append(f"| {m['name']} | {m['mapId']} | {m['runtimeMapId']} | ({m['x']},{m['y']}) | {n['npcCount']} | [{n['offset']},{n['endOffset']}) |")
lines += ['', '两次 10506 均 result=0、dataFlag=7488；世界头、其他玩家空表、队伍空表、地图名称/说明、NPC、怪物组及怪物数据已完整解析到帧尾。NPC 三字节长度与解析终点一致，包含模型、名称、位置、对话、任务状态和类型专属菜单。神剑广场有 6 个名称为空的对象，按原始值保留；不能将对象数等同于具名可对话人物数。怪物 AI 技能表和规则表本轮均为空。', '',
    '两份后续 10503 均 dataFlag=0，不重复下发 NPC。NPC 同号只在对应地图内解释。完整清单见 [npcs.json](npcs.json)。', '']
for r in rows:
    if r['messageType'] == 10506:
        lines.append(f"- {r['direction']} {brief_fields(r)}：{frame_link(r)}。")
lines += ['', '## 任务与操作链', '',
    f"严格解析 {len(M)} 个任务对象，共 {len(tasks)} 个不同任务 ID；每个对象完整消费到尾部，原始片段与哈希见 [missions.json](missions.json)。运行时 canAccept、击杀计数不应固化为离线规则。", '',
    '| ID | 标题 | 接取地图/NPC | 交付地图/NPC | 下一任务 |', '| ---: | --- | --- | --- | --- |']
for key,t in sorted(tasks.items()):
    lines.append(f"| {key} | {t['title']} | {t['acceptMapId']}/{t['acceptNpcId']} | {t['mapId']}/{t['npcId']} | {t.get('nextMissionId','未提供')} |")
lines += ['', '16 次任务交付、14 次接取的结果字节均为 0；奖励、物品实例、升级和 NPC 标记变化已完整解析到响应帧尾。14 次接取均 directSubmit=false。7 次进入战斗结果为 0，7 次结算结果为 2（右方胜利）；7 份结算响应也完整解析。这些成功结果不能证明整张地图全部任务已完成。', '',
    '关键时序：1121 交付 → 10506 进入迎客殿 → 1124/2199 与 2201～2210、2243 推进 → 2211 跨图寻路 → 10506 进入神剑广场 → 交付 2211/2212 → 接取 2213；最后主线推送仍为 2213，另已取得 2214/2244 的任务定义。', '',
    '跨图寻路 10518：从出口 (34,33) 切往地图 16 的 (3,44)，最终目标 (5,44)。该帧完整解析；其余 17 个响应为同图目标格。', '',
    '11003：请求属性 ID 10 增加 2；成功响应完整解析 cp、HP/MP、五维属性、货币和战斗力。11171 请求记录引导模式和 ID 列表，没有同号响应。', '',
    '全部按时序字段、重复样本来源与未解析尾见 [analysis.json](analysis.json)。', '', '## 未确认与适用边界', '',
    f"职责或字段尚未确认的消息（可能仅某方向）：{A['unconfirmedMessages']}。部分解析消息：{A['partialMessages']}。", '',
    '协议索引按原始哈希增量登记；原有 109 个帧链接保留，本次另补充装备、战力、货币和图鉴样本，实际链接数见 report-check.log。相同哈希不重复登记。', '',
    '两个未拆分载荷分别为 65 字节上行和 4 字节下行，来自另一连接；含义未确认，不归类为心跳，也不丢弃。', '']
for p in A['unparsedPayloads']:
    lines.append(f"- [{p['file']}]({p['file']})，{p['length']} 字节，SHA-256 `{p['sha256']}`。")
lines += ['', '本轮未收到技能商店列表或物品商店列表消息，NPC 菜单不等于商店库存。已有地图 4/16 的本地布局属于历史资源，本轮未重新下载；旧 900 号地图的 29 对象列表仍属于独立历史证据，不能与本轮迎客殿 23 对象列表直接合并。未修改 Android 资源或存档。', '',
    '## 复现与静态依据', '',
    '`python captures/miniapp-protocol/session-20260906-194937/analyze.py`；再运行同目录 `write_report.py`。复用 parse_miniapp_npcs 与 parse_miniapp_missions 的自检；本轮固定帧数、两个 NPC 块边界、长度和哈希均有断言。', '',
    'main.min.js：20768 世界头与数据块，21030 队伍表，23099 UTF-16BE 字符串，53557 NPC 表，44307 跨图寻路，19499/88692 加点请求与响应，201647 引导记录，132144 战斗结果。任务对象依据已有解析器的 49171 调用链。', '']
lines += ['## 与现有世界之门资源核对', '',
    '复用原始 NPC、任务解析器重新读取历史帧，并校验 SHA-256；布局来自已保存的同一 v63 资源目录。完整比较及原件哈希见 [map-comparison.json](map-comparison.json)。', '',
    '| 项目 | 本次目标：迎客殿 4 | 恢复版现有来源：世界之门 900 |',
    '| --- | --- | --- |',
    '| 布局 | 960×960 像素，40×40 格，5 层 | 720×960 像素，30×40 格，6 层 |',
    '| NPC 对象 | 23 | 29 |',
    '| 主要人物 | 迎客道人、紫嫣、剑客韩松 | 狂战士珍尼、雷恩长老、魔导师 |',
    '| 新手大使 88 | (31,6)，模型 15176 | (22,9)，模型 15176 |',
    '| 礼包任务 | 1124 → 2199 | 1125 → 2500 |',
    '| 出口 | NPC 50，(34,33) → 神剑广场 16，(3,44) | NPC 0，(1,37) → 风之森林 901，(27,3) |', '',
    f"具名 NPC 名称交集仅为：{'、'.join(comparison['sameNamedNpcs'])}。两个礼包任务的奖励简表相同（40067×1），但任务 ID、地图和后继不同；此共同点不证明整图等价。", '',
    '推断：它们更像不同新手路线。当前证据未确定路线选择条件，不能断言由职业、种族或客户端版本决定。恢复版当前 1110→900→1125 链有历史证据，不能用本轮 4 号数据直接覆盖。', '',
    '后续解码已消除 10506、14501、14502、14503 和 12502 的缺口。12502 请求末尾 4 个零字节已由原客户端序列化代码逐字节重建，确认为分配缓冲区的余量，详见下文；此前“含义未确认”结论被本次证据替代。11114 外层和两个未拆分载荷仍保留。未触发的商店库存、玩家完整技能清单、礼包开包以及后续/其他支线仍需对应操作；不能声称已取得全部玩法数据。', '']
lines += ['## 怪物、战斗组与实际奖励补齐', '',
    f'完整解析 {len(groups)} 个不同战斗组 ID、{len(monsters)} 个不同怪物 ID、{len(items)} 个奖励物品实例。按每次出现保留地图与原始帧来源，不把跨地图同号对象直接覆盖。详情见 [monsters.json](monsters.json) 和 [rewards.json](rewards.json)。', '',
    '| 来源帧 | 地图 | 战斗组 ID | 怪物 ID |', '| --- | ---: | --- | --- |']
for e in monster_events:
    lines.append(f"| {e['sequence']} | {e['mapId']} | {', '.join(str(g['groupId']) for g in e['data']['groups'])} | {', '.join(str(m['id']) for m in e['data']['monsters'])} |")
lines += ['', '怪物定义包含名称、外观、等级、HP/MP、速度、攻击和防御属性、暴击、吸血/吸蓝、状态位等；战斗组保留敌方槽位、友方 NPC 槽位、系数和后续战斗组。空技能/AI 表只证明本轮未下发规则，不代表怪物永远没有技能。', '',
    '| 结算帧 | 战斗组 | 铜币 | 经验 | 击杀 |', '| --- | ---: | ---: | ---: | --- |']
for e in reward_events:
    if e['messageType'] != 12502: continue
    r = e['response']['reward']
    lines.append(f"| {e['sequence']} | {e['request']['monsterGroupId']} | {r['money3']} | {r['level']['exp']} | {', '.join(str(k['monsterId'])+'×'+str(k['quantity']) for k in r['kills'])} |")
lines += ['', '7 场均无升级、物品掉落、宠物/护送奖励、佣兵失效或结算切图；任务奖励中另有 5 次升级，每次 gainLevel=1。行动计划共 13 条，本轮均为技能指令 2、技能 ID 731；请求中没有该技能的完整定义。', '',
    '| 物品 ID | 名称 | 类型 | 背包槽位 |', '| ---: | --- | ---: | ---: |']
for i in items:
    lines.append(f"| {i['id']} | {i['name']} | {i['type']} | {i['slotPos']} |")
lines += ['', '物品 40067 本轮实际类型为 28（非战斗使用物品），已获得完整领取实例。旧恢复版因当时只有奖励简表而使用类型 33 的保留物品；旧限制对本轮在线证据已失效，但这里未改动 Android 配置，也未验证礼包开包内容。装备实例保留耐久、槽位、需求属性、攻击/防御、词条、镶嵌/附魔及星级字段，不能只用任务简表替代。', '',
    '可重复验证：运行 analyze.py，再运行 write_report.py。analyze.py 强制本轮对应响应全部读到末尾；额外覆盖两个非空地图怪物块、含三件装备的任务奖励和战斗奖励截断拒绝。负数量、非法布尔值和字符串截断自检通过。没有网络请求、原件变更或 Android 构建。', '',
    '静态依据：main.min.js:20977/20984/20995 列表，59948 怪物，81225 战斗组，74135 AI，44516 技能，21509 NPC 标记，21291 升级，21068 任务奖励，21359 入包，26993/27039/27289 物品，132144 战斗结算，104705/106635 行动计划。', '']
lines += ['## 装备、货币、图鉴与技能片段补齐', '',
    f"当前 386 帧中：{sum(r['classification']=='parsed' for r in rows)} 帧完整解析，{sum(r['classification']=='partial' for r in rows)} 帧部分解析，{sum(r['classification']=='unconfirmed' for r in rows)} 帧未确认，97 帧按既有规则跳过。完整字段与原件哈希见 [character-data.json](character-data.json)。", '',
    '- 12001：4 次穿戴请求，槽位依次 38、39、40、41，对应萍踪护腿、云卿锦裤、统御法杖、松绿锦袍；数量均为 1，目标槽位参数均为 -1（客户端自动选槽）。响应模式 9，套装表及技能增量表为空，4 个自动技能槽均为 0，主动技能 ID 为 731。请求响应均完整到帧尾。',
    '- 12019：两次修理请求 slotPos=-1，响应首字节为 1，随后铜币余额分别 792、5499；这些是修理后余额，不能当成修理费用。',
    '- 12049：11 次战力/抗性推送，保留 7 个 int；首值为服务端战力，后 6 个忽略属性本轮均为 0。',
    '- 25013：22 次货币绝对值同步，type=-1/-2/-3 对应三种货币。复用客户端 protobuf 生成代码解析，包含有符号负数，不把余额当奖励增量。',
    '- 32109：4 份完全相同的图鉴属性推送，分组 1/2/3/11，属性数 15/12/4/1，共 32 条。effect 全为 false；客户端只累加 effect 为真的属性，不能把本轮这些值直接加入离线角色。它不是图鉴物品库存。',
    '- 20300→20301：7 次 cmd=10 出战伙伴列表查询，均为空。已完整解析，但按已有边界不将空表登记为非空伙伴样本。',
    '- 10509：竞技场开放公告。11002：每帧固定 8 组邮件类型、已读数和未读数。11014：5 个邮件 ID 和“系统”文字。只确认通知和统计，未取得邮件正文或附件；没有扩大忽略规则。', '',
    '11114 的 000173 帧在 [12,61) 可按客户端 Skill.fromBaseBytes 解出 ID 731「天雷空破」、等级 2、类型 3、耗蓝 25、耗血 0、动画 1/1305/0、三组属性 86=150、82=20、154=20。内部字段及截断拒绝已验证；尚未定位外层读取方，因此片段边界仍标为推断，偏移 4～12 和 61～82 未确认，不称完整技能响应。另两份 11114 仅载荷 02 和 13，含义未确认。', '',
    '验证入口仍为 analyze.py → write_report.py。analyze.py 自动调用 decode_protobuf.cjs，复用项目内 protobuf.min.js 与 protobuf-bundle.min.js；40 份 protobuf 原帧均完成解码、逐字节重编码一致和截断拒绝检查。装备响应与技能片段新增截断检查通过。未修改原客户端脚本；未发网络请求。', '',
    '静态依据 main.min.js：20718 背包请求，127147 穿戴处理，40439 套装，43775 技能更新，44516 技能基础，11026 修理，88728 战力，126862 货币，186849 图鉴生效条件，19579 邮件统计，137048 邮件通知，148953 场景文字。protobuf-bundle.min.js：5469/5542 伙伴，21076 图鉴，35685 货币。', '']
lines += ['## 战斗请求末尾零字节的来源', '',
    '已直接执行本地原客户端的 ByteArray、Message 和 encodeMsg 函数，重建 7 份 12502 请求，含帧头逐字节一致。证据见 [serialization-check.json](serialization-check.json)，其中保留原始帧和静态代码 SHA-256。没有执行客户端启动或联网代码。', '',
    '根因：Message.putBytes 复用了写字符串的 putLength，按 2×计划长度+3 分配空间，却只写入 计划长度+3 个字节；ByteArray.bytes 返回整个底层数组，encodeMsg 也按数组容量封包。本轮最后一条行动计划长 4 字节，因此多发送 4 个初始零字节。这部分是缓冲余量，不是另一个业务 int 字段。', '',
    '6 份请求的逻辑载荷为 37 字节、分配载荷为 41 字节，另一份为 30/34；加 4 字节帧头后与原始长度完全一致。额外用计划长度 0/1/4/8 复现余量 0/1/4/8，证明它不固定为 4 字节。不能将此结论扩大为“所有协议的尾部零都可删除”。解析器只在同一原帧通过完整重建时标记 serializationPadding，并保存原字节。', '',
    '复现入口：node captures/miniapp-protocol/session-20260906-194937/check_serialization.cjs；analyze.py 已自动运行此检查。修改最后一字节会导致重建比较不一致。依据 main.min.js:23040/23084/23118/104705/110094 和 egret.min.js:10389/10436/10613。11114 在已保存的两个主包副本中仍只有常量定义，未找到同号处理方；其外层继续保留未确认。', '']
(S/'result.md').write_text('\n'.join(lines),encoding='utf8')

doc = ROOT/'docs/miniapp-protocol-useful-list.md'
text = doc.read_text(encoding='utf8')
marker = '### 2026-09-06 19:49 迎客殿与神剑广场补采'
types = {10503:'世界数据',10506:'切图',10518:'任务寻路',11003:'角色加点',11004:'角色属性刷新',11171:'新手引导记录',12501:'进入本地战斗',12502:'本地战斗结算',14501:'NPC 任务数据',14502:'接取任务',14503:'提交任务',14591:'主线任务状态推送'}
types.update({12001:'背包穿戴操作',12019:'装备修理',12049:'角色战力及忽略属性',25013:'货币余额同步',32109:'图鉴属性生效状态'})
updated = []
for kind,title in types.items():
    pattern = rf'(?m)^## `{kind}`[^\n]*\n'
    hit = re.search(pattern,text)
    if not hit:
        text += f'\n## `{kind}` {title}\n\n'; hit = re.search(pattern,text)
    end = text.find('\n## ',hit.end())
    if end < 0: end = len(text)
    section = text[hit.start():end]
    if marker in section: continue
    candidates = []; seen = set()
    for row in rows:
        if row['messageType'] != kind or not row.get('fields') or row['sha256'] in text or row['sha256'] in seen: continue
        if row['previousIdenticalFrame'] and kind not in (12001,12019,12049,25013,32109): continue
        seen.add(row['sha256']); candidates.append(row)
    if not candidates: continue
    addition = f'\n{marker}\n\n迎客殿 4 → 神剑广场 16；本轮范围、时序和未解析边界见 [会话报告](../captures/miniapp-protocol/{S.name}/result.md)。\n\n'
    if kind == 10506: addition += 'NPC 块分别完整解析 23、36 个对象；怪物尾未完整解码。神剑广场稳定编号为 16，30943 为本次在线运行时编号。\n\n'
    if kind == 11003: addition += '字段依据 main.min.js:19499、88692，成功响应完整消费；失败分支未采集。\n\n'
    if kind == 11171: addition += '字段依据 main.min.js:201647。仅上行引导记录，未收到同号响应。\n\n'
    for row in candidates:
        status = '完整解析' if row['classification']=='parsed' else f"未解析尾从偏移 {row['unparsedOffset']} 开始"
        addition += f"- {row['direction']} {brief_fields(row)}（{status}）：{frame_link(row,f'../captures/miniapp-protocol/{S.name}/')}。\n"
    text = text[:end] + addition + text[end:]; updated.append(kind)
deep_types = {10506,14501,14502,14503,12502}
for row in rows:
    if row['messageType'] not in deep_types or not row.get('fields'): continue
    link = f"../captures/miniapp-protocol/{S.name}/{row['rawFile']}"
    pattern = rf'(?m)^- [^\n]*\]\({re.escape(link)}\)，SHA-256 `{row["sha256"]}`。$'
    status = '完整解析' if row['classification']=='parsed' else f"未解析尾从偏移 {row['unparsedOffset']} 开始"
    replacement = f"- {row['direction']} {brief_fields(row)}（{status}）：{frame_link(row,f'../captures/miniapp-protocol/{S.name}/')}。"
    text = re.sub(pattern,lambda match: replacement,text)
for kind in sorted(deep_types):
    hit = re.search(rf'(?m)^## `{kind}`[^\n]*\n',text)
    end = text.find('\n## ',hit.end())
    if end < 0: end = len(text)
    section = text[hit.start():end]
    note = '后续解码校验（同一批原始帧）'
    if note in section: continue
    addition = f'\n{note}：本轮 10506、14501、14502、14503 和 12502 响应均已完整解析到帧尾，覆盖怪物、战斗组、实际奖励物品、升级和 NPC 标记；12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量。以上更新替代本节本轮早期的“尾部未完整解码”边界。完整字段和原始哈希见 [解码报告](../captures/miniapp-protocol/{S.name}/result.md)。\n'
    text = text[:end]+addition+text[end:]
text = text.replace('NPC 块分别完整解析 23、36 个对象；怪物尾未完整解码。神剑广场稳定编号为 16，30943 为本次在线运行时编号。',
    'NPC 块分别完整解析 23、36 个对象；怪物组、怪物及空 AI 表已完整解码。神剑广场稳定编号为 16，30943 为本次在线运行时编号。')
text = text.replace('12502 请求仅剩末尾 4 字节含义未确认。', '12502 请求末尾 4 字节已通过原客户端序列化重建确认为缓冲余量，原始字节保留；此前未确认边界已消除。')
doc.write_text(text,encoding='utf8')
checked = 0
for path,sha in re.findall(r'\]\(([^)]+)\)，SHA-256 `([a-f0-9]{64})`',text):
    if S.name not in path: continue
    assert hashlib.sha256((doc.parent/path).resolve().read_bytes()).hexdigest() == sha
    checked += 1
print(json.dumps(dict(frames=len(rows),payloads=len(A['unparsedPayloads']),npcCounts=[n['npcCount'] for n in N],missionObjects=len(M),uniqueMissions=len(tasks),groups=len(groups),monsters=len(monsters),rewardItems=len(items),rewardEvents=len(reward_events),updatedProtocols=updated,verifiedIndexLinks=checked),ensure_ascii=False))
