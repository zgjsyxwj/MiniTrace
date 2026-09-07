"""校验本轮道具使用会话，并解码相关业务帧。"""
import hashlib
import json
import struct
import sys
from collections import Counter
from pathlib import Path

S = Path(__file__).resolve().parent
ROOT = S.parents[2]
OLD = S.parent / "session-20260906-194937"
sys.path[:0] = [str(ROOT), str(OLD)]

from parse_miniapp_npcs import Reader
from decode_data import add_items, boolean, fields, wire_string

IGNORED = {10505, 12533, 13510, 16001, 16021, 20401, 24102, 27403, 27404}
ANCILLARY = {11002, 11014, 14563}


def decode(row, data):
    r = Reader(data, 4)
    message_type = row["messageType"]
    request = row["direction"] == "request"

    if message_type == 12001 and request:
        result = fields(r, "bagMode operation slotPos quantity", "bbhi")
        result.update(confirm=boolean(r), itemType=r.read("b"))
    elif message_type == 12001:
        mode = r.read("b")
        result = {"bagMode": mode}
        if mode == 11:
            result.update(fields(r, "exp gainLevel currentExp expMax sp cp exp2 gainLevel2", "ibiiHbIb"))
        elif mode == 4:
            result["extraConsumedQuantity"] = r.read("b")
            if result["extraConsumedQuantity"]:
                result.update(fields(r, "extraConsumedSlot extraConsumedId", "hi"))
            result["items"] = add_items(r)
        else:
            raise AssertionError(f"未覆盖背包响应模式 {mode}")
    elif message_type == 12049 and not request:
        result = fields(r, "combatPower ignore_back ignore_magic_back ignore_block ignore_insight ignore_wil ignore_touch", "iiiiiii")
    elif message_type == 11014 and not request:
        result = {"mailId": r.read("q"), "sender": wire_string(r)}
    elif message_type == 11002 and request:
        result = {}
    elif message_type == 11002:
        result = {"counts": [fields(r, "type read unread", "hhh") for _ in range(8)]}
    elif message_type == 14563 and not request:
        result = {"achievementTypes": [r.read("b") for _ in range(r.read("b"))]}
    else:
        raise AssertionError(f"未覆盖消息 {message_type}")
    return result, r.pos


summary = json.loads((S / "summary.json").read_text(encoding="utf8"))
raw_rows = [json.loads(line) for line in (S / "timeline.jsonl").read_text(encoding="utf8").splitlines()]
assert len(raw_rows) == summary["frameCount"] == 41
assert not any("messageType" not in row for row in raw_rows)

rows = []
for row in raw_rows:
    data = (S / row["rawFile"]).read_bytes()
    assert len(data) == row["actualLength"] == row["declaredLength"]
    assert hashlib.sha256(data).hexdigest() == row["sha256"]
    assert struct.unpack_from(">HH", data) == (len(data), row["messageType"])
    if row["messageType"] in IGNORED:
        row["classification"] = "ignored"
    else:
        row["fields"], end = decode(row, data)
        assert end == len(data), (row["sequence"], end, len(data))
        row["classification"] = "ancillary" if row["messageType"] in ANCILLARY else "parsed"
    rows.append(row)

assert Counter(row["direction"] for row in rows) == summary["counts"]
assert not any(event.get("kind") == "frida-error" for event in map(json.loads, (S / "transport-events.jsonl").read_text(encoding="utf8").splitlines()))

for sequence in (27, 35):
    row = next(row for row in rows if row["sequence"] == sequence)
    try:
        _, end = decode(row, (S / row["rawFile"]).read_bytes()[:-1])
    except (AssertionError, ValueError, struct.error, UnicodeError):
        pass
    else:
        assert end != row["actualLength"] - 1, f"未拒绝截断帧 {sequence}"

analysis = {
    "summary": summary,
    "classifications": Counter(row["classification"] for row in rows),
    "frames": rows,
}
(S / "analysis.json").write_text(json.dumps(analysis, ensure_ascii=False, indent=2) + "\n", encoding="utf8")
print(json.dumps(analysis["classifications"], ensure_ascii=False))
