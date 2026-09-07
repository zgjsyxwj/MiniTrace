"""复用微信小游戏当前登录连接发送协议请求，并归档原始帧。"""

import argparse
import hashlib
import json
import secrets
import struct
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

import frida

from capture_miniapp_protocol import WebSocketReassembler, split_game_frames
from parse_miniapp_npcs import parse_block


ROOT = Path(__file__).resolve().parent
WRITE_RVA = 0x573550
READ_RVA = 0x4C06990

AGENT = r"""
'use strict';
const module = Process.getModuleByName('flue.dll');
const writeAddress = module.base.add(__WRITE_RVA__);
const readAddress = module.base.add(__READ_RVA__);
const write = new NativeFunction(writeAddress, 'int', ['pointer', 'pointer', 'int']);

function emit(direction, connection, buffer, length) {
  if (buffer.isNull() || length <= 0 || length > 16777216) return;
  send({kind: 'buffer', direction, connection: connection.toString(), length},
       buffer.readByteArray(length));
}

Interceptor.attach(writeAddress, {onEnter(args) {
  emit('request', args[0], args[1], args[2].toInt32());
}});
Interceptor.attach(readAddress, {
  onEnter(args) { this.connection = args[0]; this.buffer = args[1]; this.capacity = args[2].toInt32(); },
  onLeave(retval) {
    const length = retval.toInt32();
    if (length > 0 && length <= this.capacity) emit('response', this.connection, this.buffer, length);
  }
});
rpc.exports.sendframe = function(connection, hex) {
  const bytes = hex.match(/../g).map(value => parseInt(value, 16));
  const buffer = Memory.alloc(bytes.length);
  buffer.writeByteArray(bytes);
  return write(ptr(connection), buffer, bytes.length);
};
send({kind: 'ready', pid: Process.id, moduleBase: module.base.toString()});
"""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def websocket_frame(payload: bytes) -> bytes:
    size = len(payload)
    if size < 126:
        header = bytes((0x82, 0x80 | size))
    elif size <= 0xFFFF:
        header = b"\x82\xfe" + struct.pack(">H", size)
    else:
        raise ValueError("请求帧不能超过 65535 字节")
    mask = secrets.token_bytes(4)
    return header + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(payload))


def game_frame(message_type: int, payload: bytes) -> bytes:
    size = 4 + len(payload)
    if not 0 <= message_type <= 0xFFFF or size > 0xFFFF:
        raise ValueError("消息号或请求长度超限")
    return struct.pack(">HH", size, message_type) + payload


class Reader:
    def __init__(self, data: bytes, offset: int = 4):
        self.data, self.pos = data, offset

    def take(self, size: int) -> bytes:
        end = self.pos + size
        if end > len(self.data):
            raise ValueError("响应字段截断")
        value, self.pos = self.data[self.pos:end], end
        return value

    def u8(self) -> int:
        return self.take(1)[0]

    def i8(self) -> int:
        return struct.unpack(">b", self.take(1))[0]

    def i16(self) -> int:
        return struct.unpack(">h", self.take(2))[0]

    def i32(self) -> int:
        return struct.unpack(">i", self.take(4))[0]

    def text(self) -> str:
        chars = int.from_bytes(self.take(3), "big")
        return self.take(chars * 2).decode("utf-16-be")

    def done(self) -> None:
        if self.pos != len(self.data):
            raise ValueError(f"响应存在 {len(self.data) - self.pos} 个未解析字节")


def parse_regions(frame: bytes) -> list[dict]:
    reader = Reader(frame)
    rows = [dict(id=reader.u8(), name=reader.text(), px=reader.i16(), py=reader.i16(), state=reader.i8())
            for _ in range(reader.u8())]
    reader.done()
    return rows


def parse_region_maps(frame: bytes) -> list[dict]:
    reader = Reader(frame)
    result = reader.i8()
    if result < 0:
        raise RuntimeError(reader.text())
    rows = [dict(entryId=reader.i16(), name=reader.text(), info=reader.text(),
                 state=reader.i8(), isNeedItem=reader.i8()) for _ in range(reader.u8())]
    reader.done()
    return rows


DATA_PLAYER_LIST_BASE = 64
DATA_MAP_NORMAL = 256
DATA_MAP_MINI = 512
DATA_NPC_BASE = 1024
ALLOCATED_MAP_RANGES = ((10000, 19999), (20000, 29999), (30000, 31999))


def allocated_map(map_id: int) -> bool:
    return any(start <= map_id <= end for start, end in ALLOCATED_MAP_RANGES)


def npc_categories(npcs: list[dict]) -> dict:
    counts = {}
    for npc in npcs:
        kind = str(npc["type"])
        counts[kind] = counts.get(kind, 0) + 1
    return {
        "ordinaryTask": counts.get("0", 0),
        "enemy": counts.get("1", 0),
        "portal": counts.get("3", 0),
        "otherType": {key: value for key, value in counts.items() if key not in ("0", "1", "3")},
    }


def parse_10523_response(frame: bytes) -> dict:
    """解析10523回调前缀和7488场景块，NPC块之后保留未解码尾部。"""
    reader = Reader(frame)
    result = reader.i8()
    if result < 0:
        error = reader.text()
        reader.done()
        return {"result": result, "serverError": error}

    item_flag = reader.u8()
    item = None
    if item_flag == 1:
        item = {"slot": reader.i16(), "itemId": reader.i32()}
    runtime_map_id = reader.i32()
    org_map_id = reader.i16() if allocated_map(runtime_map_id) else runtime_map_id
    x, y = reader.i8(), reader.i8()
    data_flag = reader.i32()
    result = {
        "result": result,
        "itemFlag": item_flag,
        "item": item,
        "runtimeMapId": runtime_map_id,
        "orgMapId": org_map_id,
        "x": x,
        "y": y,
        "dataFlag": data_flag,
    }
    if data_flag & DATA_PLAYER_LIST_BASE:
        other_count = reader.i16()
        if other_count != 0:
            raise ValueError(f"未解析的其他玩家列表：{other_count}")
        team_count = reader.i16()
        if team_count != 0:
            raise ValueError(f"未解析的队伍列表：{team_count}")
    if data_flag & (DATA_MAP_NORMAL | DATA_MAP_MINI):
        result.update(mapName=reader.text(), mapDescription=reader.text(), mapByte=reader.i8())
    if data_flag & DATA_NPC_BASE:
        npc = parse_block(frame, reader.pos)
        reader.pos = npc["endOffset"]
        result.update(npcCount=npc["npcCount"], npcOffset=npc["offset"], npcEndOffset=npc["endOffset"],
                     npcCategories=npc_categories(npc["npcs"]), npcs=npc["npcs"])
    else:
        result.update(npcCount=0, npcCategories=npc_categories([]), npcs=[])
    result["npcParsed"] = bool(data_flag & DATA_NPC_BASE)
    result["unparsedOffset"] = reader.pos
    result["unparsedBytes"] = len(frame) - reader.pos
    return result


def load_catalog(path: Path) -> list[dict]:
    catalog = json.loads(path.read_text(encoding="utf-8"))
    entries = [dict(regionId=region["id"], regionName=region["name"], **entry)
               for region in catalog["regions"] for entry in region["maps"]]
    if len(entries) != catalog.get("mapEntryCount", len(entries)):
        raise ValueError("地图目录条目数与 result.json 不一致")
    return entries


def traverse_catalog(client: "Client", catalog_path: Path, data_flag: int) -> dict:
    entries = load_catalog(catalog_path)
    output = client.output_dir
    records, maps = [], {}
    with (output / "traversal.jsonl").open("w", encoding="utf-8") as log:
        for index, entry in enumerate(entries, 1):
            record = {"index": index, "total": len(entries), **entry}
            archive = None
            try:
                response = client.request(10523, struct.pack(">ii", entry["entryId"], data_flag))
                archive = dict(client.last_archive)
                parsed = parse_10523_response(response)
                record.update(archive, **{key: value for key, value in parsed.items() if key != "npcs"})
                record["status"] = "success" if parsed["result"] >= 0 else "failed"
                if parsed["result"] >= 0 and parsed.get("npcs"):
                    map_key = str(parsed["orgMapId"])
                    mapped = maps.setdefault(map_key, {
                        "runtimeMapId": parsed["runtimeMapId"], "orgMapId": parsed["orgMapId"],
                        "mapName": parsed.get("mapName", ""), "npcCount": parsed["npcCount"],
                        "npcCategories": parsed["npcCategories"], "npcs": parsed["npcs"],
                        "entries": [], "source": [],
                    })
                    mapped["entries"].append({"regionId": entry["regionId"], "regionName": entry["regionName"],
                                              "entryId": entry["entryId"], "entryName": entry["name"]})
                    mapped["source"].append({"request": archive["request"], "response": archive["response"],
                                             "responseSha256": archive["responseSha256"]})
                    record["npcFile"] = f"npcs/map-{parsed['orgMapId']}.json"
                elif parsed["result"] >= 0:
                    record["parseStatus"] = "no-npc-block"
            except Exception as error:
                record.update(status="failed", error=str(error))
                if archive:
                    record.update(archive)
                elif getattr(client, "last_attempt", None):
                    record.update(client.last_attempt)
            records.append(record)
            log.write(json.dumps(record, ensure_ascii=False) + "\n")
            log.flush()
            print(json.dumps({"index": index, "total": len(entries), "entryId": entry["entryId"],
                              "status": record["status"], "orgMapId": record.get("orgMapId"),
                              "npcCount": record.get("npcCount", 0), "error": record.get("serverError", record.get("error"))},
                             ensure_ascii=False), flush=True)

    npc_dir = output / "npcs"
    for map_id, mapped in maps.items():
        (npc_dir / f"map-{map_id}.json").parent.mkdir(parents=True, exist_ok=True)
        (npc_dir / f"map-{map_id}.json").write_text(json.dumps(mapped, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    failures = [record for record in records if record["status"] != "success"]
    success = [record for record in records if record["status"] == "success"]
    category_totals = {key: sum(mapped["npcCategories"].get(key, 0) for mapped in maps.values())
                       for key in ("ordinaryTask", "enemy", "portal")}
    category_totals["otherType"] = sum(sum(mapped["npcCategories"].get("otherType", {}).values())
                                       for mapped in maps.values())
    result = {
        "generatedAt": utc_now(), "catalog": str(catalog_path), "entryCount": len(entries),
        "successCount": len(success), "failureCount": len(failures),
        "uniqueOriginalMapCount": len(maps), "npcTotal": sum(mapped["npcCount"] for mapped in maps.values()),
        "npcCategoryTotals": category_totals, "heartbeatRequests": sum(row[2] in (16001, 16021) and row[0] == "request" for row in client.frames),
        "heartbeatResponses": sum(row[2] in (16001, 16021) and row[0] == "response" for row in client.frames),
        "entries": records, "failedEntries": failures,
    }
    (output / "result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return result


class Client:
    def __init__(self, pid: int, output_dir: Path, timeout: float, write_rva: int, read_rva: int):
        self.output_dir, self.timeout = output_dir, timeout
        self.output_dir.mkdir(parents=True, exist_ok=False)
        self.reassembler = WebSocketReassembler()
        self.condition = threading.Condition()
        self.frames = []
        self.connection = None
        self.sequence = 0
        self.session = frida.get_local_device().attach(pid)
        source = AGENT.replace("__WRITE_RVA__", hex(write_rva)).replace("__READ_RVA__", hex(read_rva))
        self.script = self.session.create_script(source)
        self.script.on("message", self._on_message)
        self.script.load()

    def _on_message(self, message, data) -> None:
        if message.get("type") != "send" or data is None:
            return
        meta = message.get("payload", {})
        if meta.get("kind") != "buffer":
            return
        direction, connection = meta["direction"], meta["connection"]
        messages = self.reassembler.feed(direction, connection, bytes(data))
        with self.condition:
            for _, payload in messages:
                for frame in split_game_frames(payload)[0]:
                    message_type = struct.unpack_from(">H", frame, 2)[0]
                    self.frames.append((direction, connection, message_type, frame))
                    if direction == "request" and message_type in (16001, 16021):
                        self.connection = connection
            self.condition.notify_all()

    def _wait(self, predicate):
        deadline = time.monotonic() + self.timeout
        with self.condition:
            while True:
                value = predicate()
                if value is not None:
                    return value
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError("等待小游戏连接或响应超时")
                self.condition.wait(remaining)

    def request(self, message_type: int, payload: bytes) -> bytes:
        connection = self._wait(lambda: self.connection)
        request = game_frame(message_type, payload)
        with self.condition:
            start = len(self.frames)
        self.sequence += 1
        request_name = f"{self.sequence:03d}-request-{message_type}.bin"
        response_name = f"{self.sequence:03d}-response-{message_type}.bin"
        (self.output_dir / request_name).write_bytes(request)
        request_hash = hashlib.sha256(request).hexdigest()
        self.last_attempt = {"request": request_name, "requestSha256": request_hash}
        try:
            result = self.script.exports_sync.sendframe(connection, websocket_frame(request).hex())
            if result <= 0:
                raise RuntimeError(f"flue 写入失败：{result}")
            response = self._wait(lambda: next((row[3] for row in self.frames[start:]
                                               if row[0] == "response" and row[2] == message_type), None))
        except Exception as error:
            with (self.output_dir / "requests.jsonl").open("a", encoding="utf-8") as output:
                output.write(json.dumps({
                    "timestamp": utc_now(), "messageType": message_type,
                    "request": request_name, "requestSha256": request_hash,
                    "response": None, "error": str(error),
                }, ensure_ascii=False) + "\n")
            raise
        (self.output_dir / response_name).write_bytes(response)
        self.last_archive = {
            "request": request_name, "requestSha256": request_hash,
            "response": response_name, "responseSha256": hashlib.sha256(response).hexdigest(),
        }
        with (self.output_dir / "requests.jsonl").open("a", encoding="utf-8") as output:
            output.write(json.dumps({
                "timestamp": utc_now(), "messageType": message_type,
                **self.last_archive,
            }, ensure_ascii=False) + "\n")
        return response

    def close(self) -> None:
        self.script.unload()
        self.session.detach()


def output_dir(value: Path | None) -> Path:
    path = value or ROOT / "captures" / "miniapp-requests" / datetime.now().strftime("session-%Y%m%d-%H%M%S")
    path = path.resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError("输出目录必须位于项目内")
    return path


def self_check() -> None:
    frame = game_frame(10521, b"")
    wire = websocket_frame(frame)
    cursor = 2 if len(frame) < 126 else 4
    mask = wire[cursor:cursor + 4]
    assert bytes(value ^ mask[index % 4] for index, value in enumerate(wire[cursor + 4:])) == frame
    sample = game_frame(1, bytes((1, 1, 0, 0, 1, 0x4E, 0x2D, 0, 1, 0, 2, 0)))
    assert parse_regions(sample) == [{"id": 1, "name": "中", "px": 1, "py": 2, "state": 0}]
    assert game_frame(10523, struct.pack(">ii", 37, 7232)).hex() == "000c291b0000002500001c40"
    sample_response = ROOT / "captures/miniapp-protocol/session-20260907-next-map-worker/frames/000086-response-10523.bin"
    if sample_response.exists():
        parsed = parse_10523_response(sample_response.read_bytes())
        assert (parsed["result"], parsed["orgMapId"], parsed["x"], parsed["y"], parsed["dataFlag"], parsed["npcCount"]) == (0, 17, 17, 9, 7488, 25)
    print("self-check ok")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--timeout", type=float, default=20)
    parser.add_argument("--write-rva", type=lambda value: int(value, 0), default=WRITE_RVA)
    parser.add_argument("--read-rva", type=lambda value: int(value, 0), default=READ_RVA)
    parser.add_argument("--self-check", action="store_true")
    commands = parser.add_subparsers(dest="command")
    send = commands.add_parser("send")
    send.add_argument("message_type", type=int)
    send.add_argument("--payload-hex", default="")
    enter = commands.add_parser("enter-map")
    enter.add_argument("entry_id", type=int)
    enter.add_argument("--data-flag", type=int, default=7232)
    traverse = commands.add_parser("traverse-catalog")
    traverse.add_argument("--catalog", type=Path,
                          default=ROOT / "captures/miniapp-requests/world-catalog-20260907/result.json")
    traverse.add_argument("--data-flag", type=int, default=7232)
    commands.add_parser("world-catalog")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        return
    if args.pid is None or args.command is None:
        parser.error("需要 --pid 和子命令")

    destination = output_dir(args.output_dir)
    client = Client(args.pid, destination, args.timeout, args.write_rva, args.read_rva)
    try:
        if args.command == "send":
            response = client.request(args.message_type, bytes.fromhex(args.payload_hex))
            result = {"messageType": args.message_type, "responseHex": response.hex()}
        elif args.command == "enter-map":
            response = client.request(10523, struct.pack(">ii", args.entry_id, args.data_flag))
            result = {"entryId": args.entry_id, "messageType": 10523, "responseBytes": len(response)}
        elif args.command == "traverse-catalog":
            result = traverse_catalog(client, args.catalog.resolve(), args.data_flag)
        else:
            regions = parse_regions(client.request(10521, b""))
            for region in regions:
                region["maps"] = parse_region_maps(client.request(10522, struct.pack(">i", region["id"])))
            result = {"generatedAt": utc_now(), "regionCount": len(regions),
                      "mapEntryCount": sum(len(region["maps"]) for region in regions), "regions": regions}
        (destination / "result.json").write_text(
            json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"outputDir": str(destination), **{key: value for key, value in result.items()
                                                              if key not in ("regions", "responseHex")}}, ensure_ascii=False))
    finally:
        client.close()


if __name__ == "__main__":
    main()
