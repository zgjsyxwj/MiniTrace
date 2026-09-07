"""本轮只读解析：复用任务解析器，校验所有帧，保留未解析尾部。"""
import hashlib
import json
import struct
import subprocess
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from parse_miniapp_missions import parse, self_check
from parse_miniapp_npcs import Reader, parse_block, self_check as npc_self_check
from decode_data import monster_data, npc_status, battle_reward, battle_plans, mission_reward, boolean, wire_string, skill, equipment_response, self_check as data_self_check
S = Path(__file__).resolve().parent
self_check()
npc_self_check()
data_self_check()
protobuf_rows = json.loads(subprocess.run(['node', str(S/'decode_protobuf.cjs')], check=True, capture_output=True, encoding='utf8').stdout)
(S/'protobuf.json').write_text(json.dumps(protobuf_rows,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
protobuf_by_sequence = {row['sequence']:row for row in protobuf_rows}
serialization = json.loads(subprocess.run(['node', str(S/'check_serialization.cjs')], check=True, capture_output=True, encoding='utf8').stdout)
(S/'serialization-check.json').write_text(json.dumps(serialization,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
serialization_by_sequence = {row['sequence']:row for row in serialization['frames']}
timeline_rows = [json.loads(x) for x in (S/'timeline.jsonl').read_text().splitlines()]
for row in timeline_rows:
    blob = (S/row['rawFile']).read_bytes()
    assert hashlib.sha256(blob).hexdigest() == row['sha256']
    assert len(blob) == row.get('actualLength',row.get('unparsedLength'))
rows = [row for row in timeline_rows if 'messageType' in row]
summary = json.loads((S/'summary.json').read_text())
assert len(rows) == summary['frameCount'] == 386
ignore = {10505,12533,13510,16001,16021,20401,24102,27403,27404}
history = {}
for timeline in (ROOT/'captures/miniapp-protocol').rglob('timeline.jsonl'):
    if timeline.parent == S: continue
    for line in timeline.read_text(encoding='utf8').splitlines():
        old = json.loads(line)
        if old.get('sha256'): history.setdefault(old['sha256'], str((timeline.parent/old['rawFile']).relative_to(ROOT)))
missions, maps = [], []
def world(r, row):
    fields = dict(zip(('result','runtimeMapId','mapId','x','y','dataFlag'), r.read('bihbbi')))
    assert fields['result'] == 0 and fields['mapId'] in (4,16)
    if fields['dataFlag'] == 0: return fields
    # 本轮两个切图样本均为 7488；先读其他玩家及队伍空表，再读地图字符串与 NPC 块。
    assert fields['dataFlag'] == 7488
    assert r.read('h') == 0 and r.read('h') == 0
    fields.update(name=wire_string(r), description=wire_string(r), mapByte=r.read('b'))
    npc = parse_block(r.data, r.pos)
    assert npc['endOffset'] == r.pos + 3 + int.from_bytes(r.data[r.pos:r.pos+3], 'big')
    assert (fields['mapId'],npc['offset'],npc['endOffset'],npc['npcCount']) in ((4,82,1417,23),(16,88,2663,36))
    r.pos = npc['endOffset']
    npc.update(source=row['rawFile'], sha256=row['sha256'], map=fields.copy())
    maps.append(npc)
    fields.update(npcCount=npc['npcCount'], npcOffset=npc['offset'], npcEndOffset=npc['endOffset'])
    fields['monsterData'] = monster_data(r)
    return fields
def task_block(r, row):
    size = int.from_bytes(r.data[r.pos:r.pos+3], 'big'); r.pos += 3
    offset = r.pos
    blob = r.data[offset:offset+size]
    assert len(blob) == size
    task = parse(blob); r.pos += size
    output = S/'missions'/f"{row['sequence']:06d}-{task['id']}.bin"
    output.parent.mkdir(exist_ok=True); output.write_bytes(blob)
    entry = dict(source=row['rawFile'], frameSha256=row['sha256'], offset=offset,
                 file=str(output.relative_to(S)), sha256=hashlib.sha256(blob).hexdigest(), task=task)
    missions.append(entry)
    return task
for row in rows:
    data = (S/row['rawFile']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == row['sha256']
    assert len(data) == row['actualLength']
    row['previousIdenticalFrame'] = history.get(row['sha256'])
    row['classification'] = 'ignored' if row['messageType'] in ignore else 'unconfirmed'
    if row['classification'] == 'ignored': continue
    r = Reader(data, 4); t = row['messageType']; request = row['direction'] == 'request'
    fields = {}
    try:
        if row['sequence'] in protobuf_by_sequence:
            decoded = protobuf_by_sequence[row['sequence']]
            assert decoded['sha256'] == row['sha256']
            fields = dict(schema=decoded['schema'], data=decoded['data'])
            r.pos = len(data)
        elif request:
            if t == 10506: fields = dict(zip(('mapId','x','y','dataFlag','flag'),r.read('hbbi?')))
            elif t == 10503: fields = dict(zip(('runtimeMapId','flag','dataFlag','byte1','byte2'),r.read('h?ibb')))
            elif t == 11003: fields['attributes'] = [r.read('hi') for _ in range(r.read('B'))]
            elif t == 11171:
                fields['mode'] = r.read('b'); count = r.read('i')
                assert 0 <= count <= (len(data)-r.pos)//4
                fields['guideIds'] = [r.read('i') for _ in range(count)]
            elif t == 12502:
                fields = dict(zip(('startHP','startMP','endHP','endMP','seed','monsterGroupId'),r.read('iiiihh')))
                fields.update(battle_plans(r))
                padding = serialization_by_sequence[row['sequence']]
                assert padding['sha256'] == row['sha256'] and padding['paddingOffset'] == r.pos
                assert padding['exactRebuild'] and data[r.pos:].hex() == padding['paddingHex']
                fields['serializationPadding'] = dict(offset=r.pos, hex=padding['paddingHex'], source='original ByteArray allocation; exactRebuild verified')
                r.pos = len(data)
            elif t == 14501:
                fields['npcId'] = r.read('h')
                fields['cachedGroups'] = [r.read('h') for _ in range(r.read('B'))]
                fields['cachedMonsters'] = [r.read('h') for _ in range(r.read('B'))]
            elif t == 14502: fields = dict(zip(('npcId','missionId'), r.read('hh')))
            elif t == 14503: fields = dict(zip(('npcId','missionId','selection'), r.read('hhi')))
            elif t == 10518: fields = dict(zip(('mode','target'), r.read('bh')))
            elif t == 10504: fields = dict(zip(('x','y'), r.read('bb')))
            elif t == 12501: fields['monsterGroupId'] = r.read('h')
            elif t == 12019: fields['slotPos'] = r.read('h')
            elif t == 12001:
                fields = dict(zip(('bagMode','operation','slotPos','quantity','equipPosition'),r.read('bbhib')))
                assert (fields['bagMode'],fields['operation']) == (9,1)
                fields.update(confirm=boolean(r), itemType=r.read('b'))
        else:
            if t in (10503,10506): fields = world(r,row)
            elif t == 11003:
                fields['result'] = r.read('b')
                assert fields['result'] == 0
                fields.update(zip(('cp','hp','mp','str','con','agi','ilt','wis','money1','money2','money3','combatPower'),r.read('hiihhhhhiiii')))
            elif t == 14501:
                fields['npcId'] = r.read('h'); count = r.read('b'); fields['missionCount'] = count
                if count >= 0:
                    fields['missionIds'] = [task_block(r,row)['id'] for _ in range(count)]
                    fields['monsterData'] = monster_data(r)
            elif t == 14591: fields['missionId'] = task_block(r,row)['id']
            elif t in (14502,14503,12501,12502):
                fields['result'] = r.read('b')
                if t == 12502 and fields['result'] == 2:
                    fields['reward'] = battle_reward(r)
                elif t in (14502,14503) and fields['result'] == 0:
                    fields['npcStatus'] = npc_status(r)
                    if t == 14502:
                        fields['directSubmit'] = boolean(r)
                        if fields['directSubmit']:
                            fields['submitResult'] = r.read('b')
                            assert fields['submitResult'] == 0
                            fields['reward'] = mission_reward(r)
                    else:
                        fields['reward'] = mission_reward(r)
            elif t == 10518:
                fields['result'] = r.read('b'); fields['segments'] = r.read('b')
                assert fields['result'] == 0 and fields['segments'] >= 0
                fields['transitions'] = [dict(zip(('exitX','exitY','mapId','entryX','entryY'),r.read('bbhbb'))) for _ in range(fields['segments'])]
                fields.update(zip(('x','y'),r.read('bb')))
            elif t == 11004:
                fields['increments'] = [r.read('hi') for _ in range(r.read('B'))]
                fields['absolute'] = r.read('hi')
            elif t == 10509: fields.update(displayMode=r.read('b'), text=wire_string(r))
            elif t == 11002:
                fields['mailCounts'] = [dict(zip(('type','read','unRead'),r.read('hhh'))) for _ in range(8)]
            elif t == 11014: fields.update(mailId=r.read('q'), text=wire_string(r))
            elif t == 12019: fields.update(result=r.read('b'), money3=r.read('i'))
            elif t == 12049:
                fields = dict(zip(('combatPower','ignore_back','ignore_magic_back','ignore_block','ignore_insight','ignore_wil','ignore_touch'),r.read('iiiiiii')))
            elif t == 12001: fields = equipment_response(r)
            elif t == 11114 and row['sequence'] == 173:
                # 仅提取能按客户端 Skill.fromBaseBytes 验证的内部片段；外层读取方尚未定位。
                nested = Reader(data,12)
                fields['embeddedSkill'] = dict(offset=12, data=skill(nested), endOffset=nested.pos)
                fields['boundaryStatus'] = 'inferred embedded skill; outer prefix/tail unconfirmed'
        row['fields'] = fields
        row['unparsedOffset'] = r.pos
        row['unparsedHex'] = data[r.pos:].hex()
        row['classification'] = 'parsed' if r.pos == len(data) else 'partial' if fields else 'unconfirmed'
    except (ValueError,struct.error,UnicodeError,AssertionError) as error:
        row['parseError'] = f'{error}; offset={r.pos}'
        row['classification'] = 'parse-error'
payloads = []
for path in sorted((S/'unparsed-payloads').glob('*.bin')):
    data = path.read_bytes()
    payloads.append(dict(file=str(path.relative_to(S)),length=len(data),sha256=hashlib.sha256(data).hexdigest(),hex=data.hex(),meaning='unconfirmed'))
assert len(payloads) == summary['unparsedPayloadCount'] == 2
assert len(maps) == 2
assert not any(row.get('parseError') for row in rows), [(row['sequence'],row.get('parseError')) for row in rows if row.get('parseError')]
for row in rows:
    if row['messageType'] == 12502 and row['direction'] == 'request':
        assert row['classification'] == 'parsed'
    if row['direction'] == 'response' and row['messageType'] in (10506,14501,14502,14503,12502,10509,11002,11014,12001,12019,12049,25013,32109,20301):
        assert row['classification'] == 'parsed', (row['sequence'],row.get('unparsedHex'))
# 真实非空分支的截断回归；拒绝少读字段后仍报告成功。
for filename, offset, decoder in (
    ('000029-response-10506.bin',1417,monster_data),
    ('000314-response-10506.bin',2663,monster_data),
    ('000373-response-12502.bin',5,battle_reward),
    ('000206-response-14503.bin',8,mission_reward),
    ('000265-response-12001.bin',4,equipment_response),
):
    blob = (S/'frames'/filename).read_bytes()
    r = Reader(blob,offset); decoder(r); assert r.pos == len(blob)
    try:
        decoder(Reader(blob[:-1],offset))
    except (ValueError,AssertionError,struct.error):
        pass
    else:
        raise AssertionError(f'未拒绝截断 {filename}')
blob = (S/'frames/000099-request-12502.bin').read_bytes()[:-4]
r = Reader(blob,24)
plans = battle_plans(r)
assert r.pos == len(blob) and [p['skillId'] for p in plans['plans']] == [731,731]
try:
    battle_plans(Reader(blob[:-1],24))
except (ValueError,AssertionError,struct.error):
    pass
else:
    raise AssertionError('未拒绝截断行动计划')
blob = (S/'frames/000173-response-11114.bin').read_bytes()[:61]
r = Reader(blob,12)
extracted_skill = skill(r)
assert (r.pos, extracted_skill['id'], extracted_skill['name'], extracted_skill['level'], extracted_skill['useMP']) == (61,731,'天雷空破',2,25)
try:
    skill(Reader(blob[:-1],12))
except (ValueError,AssertionError,struct.error):
    pass
else:
    raise AssertionError('未拒绝截断技能片段')
result = dict(summary=summary,sha256Verified=len(rows),unparsedPayloads=payloads,ignoredFrames=sum(x['classification']=='ignored' for x in rows),
              unconfirmedMessages=sorted({x['messageType'] for x in rows if x['classification']=='unconfirmed'}),
              partialMessages=sorted({x['messageType'] for x in rows if x['classification']=='partial'}),frames=rows)
(S/'analysis.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
(S/'missions.json').write_text(json.dumps(missions,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
(S/'npcs.json').write_text(json.dumps(maps,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print('verified', len(rows), 'ignored', result['ignoredFrames'], 'unconfirmed', result['unconfirmedMessages'], 'partial', result['partialMessages'])
for m in missions:
    t = m['task']; print(m['source'], t['id'], t['title'], 'map', t['mapId'], 'acceptMap',t['acceptMapId'],'npc',t['npcId'], 'next',t.get('nextMissionId'))
for r in rows:
    if r.get('parseError'): print('ERROR',r['sequence'],r['parseError'])
    elif r.get('fields') and r['messageType'] in (14501,14502,14503,10518,12501): print(r['sequence'],r['direction'],r['messageType'],r['fields'])

