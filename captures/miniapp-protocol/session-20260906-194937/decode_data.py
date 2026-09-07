"""解码本轮已出现的怪物与奖励分支；未知分支显式停止，不越过未知字节。"""
from parse_miniapp_npcs import Reader


def fields(r, names, fmt):
    values = r.read(fmt)
    return dict(zip(names.split(), values if len(names.split()) > 1 else [values]))


def count(r):
    n = r.read('b')
    if n < 0:
        raise ValueError(f'负列表数量，偏移 {r.pos-1}')
    return n


def boolean(r):
    value = r.read('B')
    if value not in (0, 1):
        raise ValueError(f'非法布尔值 {value}，偏移 {r.pos-1}')
    return bool(value)


def wire_string(r):
    size = (r.read('B') << 16) | r.read('H')
    end = r.pos + size*2
    if end > len(r.data):
        raise ValueError(f'字符串截断，偏移 {r.pos}')
    text = r.data[r.pos:end].decode('utf-16-be')
    r.pos = end
    return text


def skill(r, for_item=False):
    s = fields(r, 'id level', 'Hb')
    s.update(name=wire_string(r), type=r.read('b'), info=wire_string(r), skillWeapon=r.read('b'))
    s['powers'] = [r.read('hh') for _ in range(3)]
    if s['type'] != 1:
        s.update(fields(r, 'skillAtkType area round atk_time useMP useHP anime1 anime2 anime3 position statusBit1 statusBit2 statusBit3', 'bbbbHHbhhbbbb'))
    if not for_item:
        s['isLearnByBook'] = boolean(r)
    return s


def monster_data(r):
    result = {'groups': []}
    for _ in range(count(r)):
        g = fields(r, 'groupId type nextBattleGroupID flag coefficient', 'HbHbi')
        g['monsters'] = [r.read('h') for _ in range(20 if g['flag'] & 1 else 10)]
        g['npcs'] = [r.read('h') for _ in range(4)] if g['flag'] & 2 else []
        result['groups'].append(g)
    result['monsters'] = []
    for _ in range(count(r)):
        m = {'id': r.read('i')}
        if m['id'] < 0:
            result['monsters'].append(m)
            continue
        m.update(name=wire_string(r), icon1=r.read('q'))
        m.update(fields(r, 'level monstertype hpMax mpMax speed atkType atkMin atkMax atk_time', 'bbiiibiib'))
        names = 'dodge atk_str atk_str_nearby atk_str_range atk_agi atk_agi_nearby atk_agi_range atk_magic def_str def_str_nearby def_str_range def_agi def_agi_nearby def_agi_range def_magic hitrate hitMagic'
        m.update(fields(r, names, 'i'*len(names.split())))
        m['critical'] = r.read('h')
        names = 'ignoreCritical criticalDmg ignore_back ignore_magic_back ignore_block ignore_insight ignore_touch ignore_wil wil tough block brkArmor insight def_field back magic_back life_absorption mana_absorption magic_penetration'
        m.update(fields(r, names, 'i'*len(names.split())))
        m.update(bornStatus=r.read('q'), newReflection=r.read('i'))
        result['monsters'].append(m)
    result.update(skills=[], ai=[])
    if result['monsters']:
        result['skills'] = [skill(r) for _ in range(count(r))]
        for _ in range(count(r)):
            ai = {'id': r.read('h'), 'rules': []}
            for _ in range(count(r)):
                rule = fields(r, 'conditionType conditionValue', 'bi')
                rule.update(message=wire_string(r), skill1=r.read('hb'), skill2=r.read('hb'), targetType=r.read('b'))
                ai['rules'].append(rule)
            result['ai'].append(ai)
    return result


def skill_updates(r):
    updates = []
    for _ in range(count(r)):
        mode = r.read('b')
        if mode in (1,2):
            updates.append(dict(mode=mode, skill=skill(r), addLevel=r.read('b')))
        elif mode == 3:
            updates.append(dict(mode=mode, skillId=r.read('i')))
        else:
            raise ValueError(f'未知技能更新模式 {mode}')
    return dict(updates=updates, autoSkillIds=[r.read('h') for _ in range(count(r))], activeSkillId=r.read('h'))


def equipment_response(r):
    mode = r.read('b')
    assert mode == 9
    return dict(bagMode=mode, itemSetData=[r.read('h') for _ in range(count(r))], skillData=skill_updates(r))


def npc_status(r):
    return [fields(r, 'npcId sign', 'bb') for _ in range(count(r))]


def level_reward(r):
    d = fields(r, 'exp gainLevel', 'ib')
    if d['gainLevel'] > 0:
        d.update(fields(r, 'currentExp expMax sp cp', 'iihb'))
    d.update(fields(r, 'exp2 gainLevel2', 'ib'))
    if d['gainLevel2'] > 0:
        d.update(fields(r, 'currentExp2 expMax2', 'ii'))
    return d


def pet_reward(r):
    d = {'experience': r.read('i'), 'gainLevel': r.read('b')}
    if d['gainLevel'] > 0:
        d.update(fields(r, 'sp gainGrowthLevel', 'hb'))
        if d['gainGrowthLevel'] > 0:
            d.update(fields(r, 'str con agi ilt wis', 'hhhhh'))
        d.update(fields(r, 'currentExp expMax cp', 'iib'))
        d['skills'] = [skill(r) for _ in range(count(r))]
    return d


def add_items(r):
    items = []
    for _ in range(count(r)):
        d = fields(r, 'mode quantity', 'bb')
        if d['mode'] == 2:
            d.update(fields(r, 'itemId slotPos', 'ii'))
        elif d['mode'] == 1:
            d['expectedSlotPos'] = r.read('i')
            d['item'] = item(r)
            assert d['expectedSlotPos'] == d['item']['slotPos']
        else:
            raise ValueError(f'未知物品新增模式 {d}')
        items.append(d)
    return items


def item(r):
    d = fields(r, 'quantity petProvideExp durability attachDone attachPower attachValue slotPos status', 'hiibhhhb')
    if d['status'] & 1:
        d['expireMinutesWire'] = r.read('i')
    d.update(item_template(r))
    return d


def item_template(r):
    d = {'type': r.read('b')}
    hand_book = boolean(r)
    if hand_book:
        d['handBookItem'] = True
        d['isHandBookActivated'] = boolean(r)
        d['illusion'] = [fields(r, 'currency day price', 'bii') for _ in range(count(r))]
        d.update(fields(r, 'illusionItemId illusionEndTime', 'iq'))
        if d['illusionItemId']:
            d['itemInfo'] = item_template(r)
    d.update(id=r.read('i'), name=wire_string(r), bagIcon=r.read('i'), info=wire_string(r))
    d.update(fields(r, 'grade autoBinding stackNum', 'bbb'))
    d['decomCHest'] = boolean(r)
    kind = d['type']
    if kind in (25, 29, 33):
        d['price'] = r.read('i')
    elif kind == 30:
        d.update(fields(r, 'price power1 powerValue1', 'ihh'))
    elif kind == 40:
        d.update(fields(r, 'power1 price powerValueBlood1 powerValueBloodMax1', 'hiii'))
    elif kind in (44, 46, 47):
        d['powerValueBlood1'] = r.read('i')
    elif kind == 42:
        d.update(fields(r, 'petProvideExp price', 'ii'))
    elif kind == 43:
        d.update(fields(r, 'seal_grade seal_type', 'ii'))
        d['sealPowers'] = [r.read('hh') for _ in range(count(r))]
    elif kind in (26, 27, 28, 31, 45):
        d.update(fields(r, 'reqLv ownTime durMax price round area power1 powerValue1 power2 powerValue2 vipLevelReq', 'bHiibbhhhhb'))
        if d['power1'] == 181:
            d.update(fields(r, 'power3 powerValue3', 'hh'))
        d['needShowRate'] = boolean(r)
    elif 0 <= kind <= 24 or 34 <= kind <= 39:
        weapon = kind >= 13
        d.update(fields(r, 'icon ownTime durMax price attachCount itemSet reqLv reqStr reqCon reqAgi reqIlt reqWis', 'hHiibbbhhhhh'))
        d.update(fields(r, 'def_str def_agi def_mag', 'hhh'))
        d['powers'] = [r.read('hh') for _ in range(3)]
        d['skill'] = skill(r, True) if boolean(r) else None
        d['combinUpSkills'] = [skill(r, True) if boolean(r) else None for _ in range(count(r))]
        d['skillDesc'] = wire_string(r)
        d['bindPowers'] = [r.read('hh') for _ in range(2)]
        if weapon:
            d.update(fields(r, 'atkType atk_time atkMin atkMax', 'bbhh'))
        d.update(fields(r, 'ascensionStar star', 'bb'))
        if weapon:
            d['hitrate'] = r.read('b')
        elif kind == 12:
            d['fashionIcons'] = r.read('qqq')
        d['vipLevelReq'] = r.read('b')
        d['enchantPowers'] = [r.read('hh') for _ in range(2)]
        d['enchantLevel'] = r.read('h')
        for key in ('cssLeftChannel', 'cssRightChannel', 'cssLeftHasItem', 'cssRightHasItem'):
            d[key] = boolean(r)
        d.update(fields(r, 'cssLeftPowerPer cssRightPowerPer', 'hh'))
        for side in ('Left', 'Right'):
            if d[f'css{side}HasItem']:
                css = fields(r, 'id grade icon', 'ibh')
                css['powers'] = [r.read('hh') for _ in range(count(r))]
                d[f'css{side}Item'] = css
        d['isUpgradeItem'] = boolean(r)
        if d['isUpgradeItem']:
            d['upgradePowers'] = [r.read('hh') for _ in range(4)]
        d.update(fields(r, 'upgradeAscensionStar upgradeStar', 'bb'))
    else:
        raise ValueError(f'物品类型待补齐 {kind}，偏移 {r.pos}')
    return d


def battle_reward(r):
    d = {'money3': r.read('i'), 'level': level_reward(r), 'bagFull': boolean(r)}
    d['hasPetReward'] = boolean(r)
    if d['hasPetReward']:
        d['petReward'] = pet_reward(r)
    d['kills'] = [fields(r, 'monsterId quantity', 'hb') for _ in range(count(r))]
    d['hasEscortReward'] = boolean(r)
    assert not d['hasEscortReward'], '未覆盖护送奖励'
    d['items'] = add_items(r)
    d['npcStatus'] = npc_status(r)
    d['powerValid'] = boolean(r)
    d['expiredMercenaries'] = [dict(flag=boolean(r), id=r.read('h'), name=wire_string(r)) for _ in range(count(r))]
    d.update(changeMap=boolean(r), countryPowerValid=boolean(r))
    return d


def battle_plans(r):
    d = {'hasPet': boolean(r), 'buddyCount': count(r), 'roundCount': count(r), 'plans': []}
    for _ in range(d['roundCount'] * (1 + int(d['hasPet']) + d['buddyCount'])):
        size = (r.read('B') << 16) | r.read('H')
        end = r.pos + size
        p = {'offset': r.pos, 'length': size, 'action': r.read('b')}
        if p['action'] in (1, 2, 3):
            p['targetPosition'] = r.read('b')
        if p['action'] == 2:
            p['skillId'] = r.read('h')
        elif p['action'] == 3:
            p['slotPos'] = r.read('b')
        assert p['action'] in (0, 1, 2, 3, 4) and r.pos == end, '未覆盖行动计划'
        d['plans'].append(p)
    return d


def mission_reward(r):
    d = {'finishMission': boolean(r)}
    d.update(fields(r, 'costMoney1 costMoney2 costMoney3 gainMoney2 gainMoney3', 'hiiii'))
    d['level'] = level_reward(r)
    d['hasPetReward'] = boolean(r)
    if d['hasPetReward']:
        d['petReward'] = pet_reward(r)
    d['removedItems'] = [fields(r, 'itemId slotPos quantity', 'iib') for _ in range(count(r))]
    d['items'] = add_items(r)
    d.update(refreshCity=boolean(r), hasCountryReward=boolean(r))
    if d['hasCountryReward']:
        d.update(fields(r, 'honor2 cityDegree cityArmy countryLand prosperity prosperityBonus countryPeople countryWood countryStone countryIron countryArmy', 'i'+'h'*10))
    return d


def self_check():
    r = Reader(b'\x00\x00\x01\x4e\x2d')
    assert wire_string(r) == '中' and r.pos == 5
    for blob in (b'\x00\x00\x01\x4e', b'\x00'):
        try:
            wire_string(Reader(blob))
        except ValueError:
            pass
        else:
            raise AssertionError('未拒绝截断字符串')
    assert monster_data(Reader(b'\0\0')) == dict(groups=[], monsters=[], skills=[], ai=[])
    for fn, blob in ((count, b'\xff'), (boolean, b'\x02')):
        try:
            fn(Reader(blob))
        except ValueError:
            pass
        else:
            raise AssertionError('未拒绝非法计数/布尔值')
