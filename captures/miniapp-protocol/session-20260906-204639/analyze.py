"""补采会话：校验所有原件，复用已有解码器，未知部分显式保留。"""
import hashlib
import json
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

S = Path(__file__).resolve().parent
ROOT = S.parents[2]
OLD = S.parent/'session-20260906-194937'
sys.path[:0] = [str(ROOT), str(OLD)]
from parse_miniapp_npcs import Reader, parse_block
from parse_miniapp_missions import parse as parse_task
from decode_data import (fields, boolean, count, wire_string, item_template,
                         add_items, monster_data, battle_reward, battle_plans, self_check)

def save(name, data):
    (S/name).write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n', encoding='utf8')

def shop(r):
    d = {'result': r.read('b')}
    if d['result'] != 0:
        d['error'] = wire_string(r)
        return d
    d['countWire'] = r.read('h')
    d['items'] = []
    for _ in range(d['countWire'] & 255):
        it = item_template(r)
        it.update(fields(r, 'viewOrder oMoney1 oMoney2 oMoney3 discount money1 money2 money3 status maxBuy allCount hotTypeOrNewItem typeForShop', 'biiibiiibhhbb'))
        it['timeLimited'] = boolean(r)
        if it['timeLimited']:
            it.update(startTime=wire_string(r), endTime=wire_string(r))
        d['items'].append(it)
    return d

def gift(r):
    d = {'bagMode': r.read('b')}
    if d['bagMode'] < 0:
        d['error'] = wire_string(r)
        return d
    assert d['bagMode'] == 4, '未覆盖背包响应模式'
    d['extraConsumedQuantity'] = r.read('b')
    if d['extraConsumedQuantity']:
        d.update(fields(r, 'extraConsumedSlot extraConsumedId', 'hi'))
    d['items'] = add_items(r)
    return d

def suits(r):
    n = r.read('b')
    assert n >= -1, '未覆盖套装表负计数'
    return dict(countWire=n, entries=[dict(id=r.read('B'), effects=[fields(r, 'requiredPieces power value vipLevel active', 'bhhbb')
                                        for _ in range(count(r))]) for _ in range(max(0,n))])

def world(r):
    d = fields(r, 'result runtimeMapId mapId x y dataFlag', 'bihbbi')
    assert d['result'] == 0
    if not d['dataFlag']:
        return d
    assert d['dataFlag'] == 7488 and r.read('h') == 0 and r.read('h') == 0
    d.update(name=wire_string(r), description=wire_string(r), mapByte=r.read('b'))
    d['npcData'] = parse_block(r.data, r.pos)
    assert d['npcData']['endOffset'] == r.pos + 3 + int.from_bytes(r.data[r.pos:r.pos+3], 'big')
    r.pos = d['npcData']['endOffset']
    d['monsterData'] = monster_data(r)
    return d

def decode(row, data):
    r = Reader(data, 4)
    t, req = row['messageType'], row['direction'] == 'request'
    d = {}
    if row['sequence'] in protobuf:
        p = protobuf[row['sequence']]
        assert p['sha256'] == row['sha256']
        return dict(schema=p['schema'], data=p['data']), len(data)
    if req:
        if t == 10504: d = fields(r, 'x y', 'bb')
        elif t == 10503: d = fields(r, 'runtimeMapId flag dataFlag byte1 byte2', 'h?ibb')
        elif t == 10506: d = fields(r, 'mapId x y dataFlag flag', 'hbbi?')
        elif t == 11004 or t == 12036: pass
        elif t == 11006: d['setting'] = r.read('i')
        elif t == 11501: d['suitIds'] = [r.read('b') for _ in range(count(r))]
        elif t == 12004: d['shopId'] = r.read('h')
        elif t == 12005:
            d = fields(r, 'shopId itemId quantity', 'hib')
            d['countryShop'] = boolean(r)
            if d['countryShop']: d.update(fields(r, 'money1 money2 money3', 'iii'))
        elif t == 12006: d = fields(r, 'shopId itemId slotPos quantity', 'hihb')
        elif t == 12016: d['itemId'] = r.read('i')
        elif t == 12019: d['slotPos'] = r.read('h')
        elif t == 12001:
            d = fields(r, 'bagMode operation slotPos quantity', 'bbhi')
            assert (d['bagMode'],d['operation']) == (4,3)
            d.update(confirm=boolean(r), itemType=r.read('b'))
        elif t == 12501: d['monsterGroupId'] = r.read('h')
        elif t == 12502:
            d = fields(r, 'startHP startMP endHP endMP seed monsterGroupId', 'iiiihh')
            d.update(battle_plans(r))
            p = serialization[row['sequence']]
            assert p['sha256'] == row['sha256'] and p['paddingOffset'] == r.pos and p['exactRebuild']
            d['serializationPadding'] = p['paddingHex']
            r.pos = len(data)
        elif t == 14501:
            d['npcId'] = r.read('h')
            d['cachedGroups'] = [r.read('h') for _ in range(count(r))]
            d['cachedMonsters'] = [r.read('h') for _ in range(count(r))]
    else:
        if t in (10503,10506): d = world(r)
        elif t == 11006: d['ackByte'] = r.read('b')
        elif t == 11501: d['suits'] = suits(r)
        elif t == 12016:
            d['result'] = r.read('b')
            if d['result'] < 0: d['error'] = wire_string(r)
            else: d.update(item=item_template(r), suits=suits(r))
        elif t == 14555: d['text'] = wire_string(r)
        elif t == 11004:
            d['increments'] = [r.read('hi') for _ in range(count(r))]
            d['absolute'] = r.read('hi')
        elif t == 12004: d = shop(r)
        elif t == 12001: d = gift(r)
        elif t == 12005:
            d['result'] = r.read('b')
            if d['result'] < 0: d['error'] = wire_string(r)
            elif d['result'] == 0:
                d.update(fields(r, 'itemId shortWire quantity money1 money2 money3 maxBuy allCount', 'ihbiiihh'))
        elif t == 12006:
            d['result'] = r.read('b')
            if d['result'] < 0: d['error'] = wire_string(r)
            else: d['money3'] = r.read('i')
        elif t == 12019: d = fields(r, 'result money3', 'bi')
        elif t == 12049: d = fields(r, 'combatPower ignore_back ignore_magic_back ignore_block ignore_insight ignore_wil ignore_touch', 'iiiiiii')
        elif t == 12036:
            d['count'] = r.read('h')
            assert d['count'] == 0, '非空自动出售表未覆盖'
        elif t == 12501:
            d['result'] = r.read('b')
            if d['result'] < 0: d['error'] = wire_string(r)
        elif t == 12502:
            d['result'] = r.read('b')
            assert d['result'] == 2
            d['reward'] = battle_reward(r)
        elif t == 14501:
            d['npcId'] = r.read('h')
            d['tasks'] = []
            for _ in range(count(r)):
                size = (r.read('B') << 16) | r.read('H')
                assert r.pos + size <= len(data)
                d['tasks'].append(parse_task(data[r.pos:r.pos+size]))
                r.pos += size
            d['monsterData'] = monster_data(r)
    return d, r.pos

self_check()
summary = json.loads((S/'summary.json').read_text())
raw_rows = [json.loads(s) for s in (S/'timeline.jsonl').read_text().splitlines()]
for row in raw_rows:
    data = (S/row['rawFile']).read_bytes()
    assert len(data) == row.get('actualLength', row.get('unparsedLength'))
    assert hashlib.sha256(data).hexdigest() == row['sha256']
    if 'messageType' in row:
        assert struct.unpack_from('>HH', data) == (len(data), row['messageType'])
rows = [r for r in raw_rows if 'messageType' in r]
assert len(rows) == summary['frameCount'] == 266
assert len(raw_rows)-len(rows) == summary['unparsedPayloadCount'] == 9
assert summary['fatalError'] is None and not summary['timedOut']
protobuf_rows = json.loads(subprocess.run(['node', str(OLD/'decode_protobuf.cjs'), str(S)], check=True, capture_output=True, encoding='utf8').stdout)
save('protobuf.json', protobuf_rows)
protobuf = {r['sequence']:r for r in protobuf_rows}
rebuild = json.loads(subprocess.run(['node', str(OLD/'check_serialization.cjs'), str(S)], check=True, capture_output=True, encoding='utf8').stdout)
save('serialization-check.json', rebuild)
serialization = {r['sequence']:r for r in rebuild['frames']}
ignore = {10505,12533,13510,16001,16021,20401,24102,27403,27404}
for row in rows:
    row['classification'] = 'ignored' if row['messageType'] in ignore else 'unconfirmed'
    if row['classification'] == 'ignored': continue
    data = (S/row['rawFile']).read_bytes()
    try:
        row['fields'], end = decode(row, data)
        row.update(unparsedOffset=end, unparsedHex=data[end:].hex())
        row['classification'] = 'parsed' if end == len(data) else 'partial' if row['fields'] else 'unconfirmed'
    except (ValueError, AssertionError, struct.error, UnicodeError) as e:
        row.update(classification='parse-error', parseError=str(e))
save('analysis.json', dict(summary=summary, classifications=Counter(r['classification'] for r in rows), frames=rows,
                           unparsedPayloads=[r for r in raw_rows if 'messageType' not in r]))
for name, types in [('shops.json',{12004}), ('gift.json',{12001}), ('maps.json',{10506}), ('transactions.json',{12005,12006}), ('missions.json',{14501})]:
    save(name, [r for r in rows if r['messageType'] in types])
assert all(r['classification'] in ('parsed','ignored') for r in rows), '存在未完成的帧解码'
# 真实样本截断检查覆盖商城图鉴嵌套、礼包列表和物品详情末尾套装表。
for sequence in (56,186,188,190,194,156,159,58,230,257):
    row = next(r for r in rows if r['sequence'] == sequence)
    data = (S/row['rawFile']).read_bytes()
    try:
        _, end = decode(row, data[:-1])
        assert end == len(data)-1
    except (ValueError, AssertionError, struct.error, UnicodeError):
        pass
    else:
        raise AssertionError(f'未拒绝截断帧 {sequence}')
assert len(protobuf_rows) == 15 and len(rebuild['frames']) == 4
assert Counter(r['direction'] for r in rows) == summary['counts']
print(json.dumps(Counter(r['classification'] for r in rows)))
for row in rows:
    if row['classification'] in ('parse-error','partial','unconfirmed'):
        print(row['sequence'], row['direction'], row['messageType'], row.get('parseError'), row.get('unparsedHex','')[:90])
