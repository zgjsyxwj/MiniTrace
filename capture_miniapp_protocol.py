import argparse
import csv
import hashlib
import json
import signal
import struct
import time
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent
DEFAULT_API_INDEX = PROJECT_ROOT / "wechat-miniapp" / "api-cg-index.csv"
DEFAULT_FLUE_WRITE_RVA = 0x573550
DEFAULT_FLUE_READ_RVA = 0x4C06990

WINHTTP_AGENT = r"""
'use strict';

const module = Process.getModuleByName('WINHTTP.dll');
const sendAddress = module.getExportByName('WinHttpWebSocketSend');
const receiveAddress = module.getExportByName('WinHttpWebSocketReceive');

function emit(direction, buffer, length, bufferType) {
  if (buffer.isNull() || length <= 0 || length > __MAX_BYTES__) return;
  send({ kind: 'plaintext-buffer', direction, connection: 'winhttp', bufferType, length },
       buffer.readByteArray(length));
}

Interceptor.attach(sendAddress, {
  onEnter(args) {
    emit('request', args[2], args[3].toUInt32(), args[1].toUInt32());
  }
});

Interceptor.attach(receiveAddress, {
  onEnter(args) {
    this.buffer = args[1];
    this.capacity = args[2].toUInt32();
    this.bytesRead = args[3];
    this.bufferType = args[4];
  },
  onLeave(retval) {
    if (retval.toUInt32() !== 0 || this.bytesRead.isNull()) return;
    const length = this.bytesRead.readU32();
    if (length <= this.capacity) {
      emit('response', this.buffer, length,
           this.bufferType.isNull() ? null : this.bufferType.readU32());
    }
  }
});

send({
  kind: 'ready', pid: Process.id, arch: Process.arch, module: module.name,
  hooks: { send: sendAddress.toString(), receive: receiveAddress.toString() }
});
"""

FLUE_AGENT = r"""
'use strict';

const module = Process.getModuleByName(__MODULE__);
const writeAddress = module.base.add(__WRITE_RVA__);
const readAddress = module.base.add(__READ_RVA__);

function emit(direction, connection, buffer, length) {
  if (buffer.isNull() || length <= 0 || length > __MAX_BYTES__) return;
  send({ kind: 'plaintext-buffer', direction, connection: connection.toString(), length },
       buffer.readByteArray(length));
}

Interceptor.attach(writeAddress, {
  onEnter(args) {
    emit('request', args[0], args[1], args[2].toInt32());
  }
});

Interceptor.attach(readAddress, {
  onEnter(args) {
    this.connection = args[0];
    this.buffer = args[1];
    this.capacity = args[2].toInt32();
  },
  onLeave(retval) {
    const length = retval.toInt32();
    if (length > 0 && length <= this.capacity) {
      emit('response', this.connection, this.buffer, length);
    }
  }
});

send({
  kind: 'ready', pid: Process.id, arch: Process.arch, module: module.name,
  moduleBase: module.base.toString(),
  hooks: { write: writeAddress.toString(), read: readAddress.toString() }
});
"""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def parse_integer(value: str) -> int:
    return int(value, 0)


def build_agent(args: argparse.Namespace) -> str:
    if args.transport == "winhttp":
        return WINHTTP_AGENT.replace("__MAX_BYTES__", str(args.max_bytes))
    return (
        FLUE_AGENT.replace("__MODULE__", json.dumps(args.flue_module))
        .replace("__WRITE_RVA__", hex(args.flue_write_rva))
        .replace("__READ_RVA__", hex(args.flue_read_rva))
        .replace("__MAX_BYTES__", str(args.max_bytes))
    )


def load_protocol_names(path: Path | None) -> dict[int, str]:
    if path is None or not path.exists():
        return {}
    with path.open("r", encoding="utf-8-sig", newline="") as source:
        return {
            int(row["id"]): row["name"]
            for row in csv.DictReader(source)
            if row.get("id", "").isdigit() and row.get("name")
        }


class WebSocketReassembler:
    def __init__(self) -> None:
        self.buffers = defaultdict(bytearray)
        self.fragments = defaultdict(bytearray)
        self.fragment_opcodes = {}

    def feed(self, direction: str, connection: str, data: bytes) -> list[tuple[int, bytes]]:
        key = (direction, connection)
        buffer = self.buffers[key]
        buffer.extend(data)
        messages = []
        while len(buffer) >= 2:
            first, second = buffer[:2]
            fin = bool(first & 0x80)
            opcode = first & 0x0F
            masked = bool(second & 0x80)
            length = second & 0x7F
            cursor = 2
            if length == 126:
                if len(buffer) < 4:
                    break
                length = struct.unpack_from(">H", buffer, cursor)[0]
                cursor += 2
            elif length == 127:
                if len(buffer) < 10:
                    break
                length = struct.unpack_from(">Q", buffer, cursor)[0]
                cursor += 8
            if length > 64 * 1024 * 1024:
                del buffer[0]
                continue
            mask = None
            if masked:
                if len(buffer) < cursor + 4:
                    break
                mask = bytes(buffer[cursor : cursor + 4])
                cursor += 4
            end = cursor + length
            if len(buffer) < end:
                break
            payload = bytes(buffer[cursor:end])
            del buffer[:end]
            if mask is not None:
                payload = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
            if opcode in (1, 2) and fin:
                messages.append((opcode, payload))
            elif opcode in (1, 2):
                self.fragments[key] = bytearray(payload)
                self.fragment_opcodes[key] = opcode
            elif opcode == 0:
                self.fragments[key].extend(payload)
                if fin:
                    messages.append((self.fragment_opcodes.pop(key, 2), bytes(self.fragments.pop(key))))
        return messages


class WinHttpReassembler:
    def __init__(self) -> None:
        self.fragments = defaultdict(bytearray)

    def feed(self, direction: str, buffer_type: int | None, data: bytes) -> list[tuple[int, bytes]]:
        if buffer_type in (1, 3):
            self.fragments[direction].extend(data)
            return []
        if buffer_type not in (0, 2):
            return []
        payload = bytes(self.fragments.pop(direction, bytearray())) + data
        return [(2 if buffer_type == 0 else 1, payload)]


def split_game_frames(payload: bytes) -> tuple[list[bytes], bytes]:
    frames = []
    offset = 0
    while offset + 4 <= len(payload):
        declared_length = struct.unpack_from(">H", payload, offset)[0]
        if declared_length < 4 or offset + declared_length > len(payload):
            break
        frames.append(payload[offset : offset + declared_length])
        offset += declared_length
    return frames, payload[offset:]


def resolve_output_dir(path: Path | None) -> Path:
    if path is None:
        stamp = datetime.now().strftime("session-%Y%m%d-%H%M%S")
        return PROJECT_ROOT / "captures" / "miniapp-protocol" / stamp
    resolved = (PROJECT_ROOT / path).resolve() if not path.is_absolute() else path.resolve()
    if not resolved.is_relative_to(PROJECT_ROOT):
        raise ValueError("输出目录必须位于项目内")
    return resolved


def self_check() -> None:
    frame = struct.pack(">HH", 6, 10002) + b"ok"
    frames, remainder = split_game_frames(frame)
    assert frames == [frame] and not remainder
    winhttp = WinHttpReassembler()
    assert not winhttp.feed("request", 1, frame[:3])
    assert winhttp.feed("request", 0, frame[3:]) == [(2, frame)]
    websocket = WebSocketReassembler()
    assert websocket.feed("response", "1", b"\x82\x06" + frame) == [(2, frame)]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="记录微信小游戏运行时明文游戏协议。")
    parser.add_argument("--transport", choices=("winhttp", "flue"), default="winhttp")
    parser.add_argument("--pid", type=int)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--duration", type=int, default=3600)
    parser.add_argument("--api-index", type=Path, default=DEFAULT_API_INDEX)
    parser.add_argument("--max-bytes", type=int, default=16 * 1024 * 1024)
    parser.add_argument("--flue-module", default="flue.dll")
    parser.add_argument("--flue-write-rva", type=parse_integer, default=DEFAULT_FLUE_WRITE_RVA)
    parser.add_argument("--flue-read-rva", type=parse_integer, default=DEFAULT_FLUE_READ_RVA)
    parser.add_argument("--self-check", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_check:
        self_check()
        print("self-check ok")
        return 0
    if args.pid is None:
        raise SystemExit("缺少 --pid")
    if args.duration <= 0 or args.max_bytes <= 0:
        raise SystemExit("--duration 和 --max-bytes 必须大于 0")

    output_dir = resolve_output_dir(args.output_dir)
    frames_dir = output_dir / "frames"
    payloads_dir = output_dir / "unparsed-payloads"
    frames_dir.mkdir(parents=True, exist_ok=True)
    payloads_dir.mkdir(parents=True, exist_ok=True)
    names = load_protocol_names(args.api_index)
    ready_path = output_dir / "ready.json"
    timeline_path = output_dir / "timeline.jsonl"
    transport_path = output_dir / "transport-events.jsonl"
    summary_path = output_dir / "summary.json"
    websocket = WebSocketReassembler()
    winhttp = WinHttpReassembler()
    stop_requested = False
    fatal_error = None
    frame_sequence = 0
    payload_sequence = 0
    counts = {"request": 0, "response": 0}
    type_counts = defaultdict(int)

    def append_jsonl(path: Path, value: dict) -> None:
        with path.open("a", encoding="utf-8") as output:
            output.write(json.dumps(value, ensure_ascii=False) + "\n")

    def save_payload(direction: str, connection: str, opcode: int, payload: bytes, timestamp: str) -> None:
        nonlocal frame_sequence, payload_sequence
        frames, remainder = split_game_frames(payload)
        for frame in frames:
            frame_sequence += 1
            declared_length, message_type = struct.unpack_from(">HH", frame, 0)
            filename = f"{frame_sequence:06d}-{direction}-{message_type}.bin"
            (frames_dir / filename).write_bytes(frame)
            counts[direction] += 1
            type_counts[f"{direction}:{message_type}"] += 1
            append_jsonl(timeline_path, {
                "sequence": frame_sequence, "timestamp": timestamp, "direction": direction,
                "connection": connection, "webSocketOpcode": opcode, "messageType": message_type,
                "protocolName": names.get(message_type), "declaredLength": declared_length,
                "actualLength": len(frame), "sha256": hashlib.sha256(frame).hexdigest(),
                "rawFile": f"frames/{filename}",
            })
        if remainder:
            payload_sequence += 1
            filename = f"{payload_sequence:06d}-{direction}-unparsed.bin"
            (payloads_dir / filename).write_bytes(remainder)
            append_jsonl(timeline_path, {
                "timestamp": timestamp, "direction": direction, "connection": connection,
                "webSocketOpcode": opcode, "unparsedLength": len(remainder),
                "sha256": hashlib.sha256(remainder).hexdigest(),
                "rawFile": f"unparsed-payloads/{filename}",
            })

    def on_message(message, data) -> None:
        nonlocal fatal_error, stop_requested
        timestamp = utc_now()
        if message.get("type") != "send":
            fatal_error = message
            append_jsonl(transport_path, {"timestamp": timestamp, "kind": "frida-error", "message": message})
            stop_requested = True
            return
        payload = message.get("payload", {})
        if payload.get("kind") == "ready":
            ready_path.write_text(json.dumps({"timestamp": timestamp, **payload}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if payload.get("kind") != "plaintext-buffer" or data is None:
            append_jsonl(transport_path, {"timestamp": timestamp, **payload})
            return
        direction = payload["direction"]
        connection = payload["connection"]
        raw = bytes(data)
        append_jsonl(transport_path, {
            "timestamp": timestamp, "kind": "plaintext-buffer", "direction": direction,
            "connection": connection, "bufferType": payload.get("bufferType"),
            "length": len(raw), "previewHex": raw[:16].hex(),
        })
        if args.transport == "flue":
            messages = websocket.feed(direction, connection, raw)
        else:
            messages = winhttp.feed(direction, payload.get("bufferType"), raw)
        for opcode, message_payload in messages:
            save_payload(direction, connection, opcode, message_payload, timestamp)

    def request_stop(_signum, _frame) -> None:
        nonlocal stop_requested
        stop_requested = True

    signal.signal(signal.SIGINT, request_stop)
    signal.signal(signal.SIGTERM, request_stop)
    session = None
    script = None
    try:
        import frida

        session = frida.get_local_device().attach(args.pid)
        script = session.create_script(build_agent(args))
        script.on("message", on_message)
        script.load()
        deadline = time.monotonic() + args.duration
        while not stop_requested and time.monotonic() < deadline:
            time.sleep(0.1)
    except Exception as error:
        fatal_error = {"type": type(error).__name__, "message": str(error)}
    finally:
        summary = {
            "finishedAt": utc_now(), "pid": args.pid, "transport": args.transport,
            "counts": counts, "typeCounts": dict(sorted(type_counts.items())),
            "fatalError": fatal_error, "timedOut": not stop_requested,
            "frameCount": frame_sequence, "unparsedPayloadCount": payload_sequence,
        }
        summary_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(summary, ensure_ascii=False))
        if script is not None:
            try:
                script.unload()
            except Exception:
                pass
        if session is not None:
            try:
                session.detach()
            except Exception:
                pass
    return 1 if fatal_error else 0


if __name__ == "__main__":
    raise SystemExit(main())
