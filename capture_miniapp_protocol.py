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
DEFAULT_MACOS_MODULE = "WeChatAppEx Framework"

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

# macOS 微信 4.x 的 WeChatAppEx 是带 Hardened Runtime 的 Chromium/Flue
# 宿主。与 Windows flue.dll 的固定 RVA 不同，macOS 版本的内部符号和地址
# 随构建变化，因此只接受显式配置的已核对符号，或从目标 Framework 的符号
# 表中找到完整的 HTTP 与 WebSocket 四个方向入口。缺少任何一个方向时，
# agent 直接报告 attach-failure，不把单一边界伪装成完整采集。
MACOS_AGENT = r"""
'use strict';

const moduleName = __MACOS_MODULE__;
const maxBytes = __MAX_BYTES__;
const configured = {
  httpWrite: __MACOS_HTTP_WRITE__,
  httpRead: __MACOS_HTTP_READ__,
  websocketWrite: __MACOS_WEBSOCKET_WRITE__,
  websocketRead: __MACOS_WEBSOCKET_READ__
};

const DEFAULT_SYMBOLS = {
  httpWrite: [
    'XWebHttpWrite', 'XWebHTTPWrite', 'xweb_http_write',
    'flue_http_write', 'FlueHttpWrite', 'HttpWrite'
  ],
  httpRead: [
    'XWebHttpRead', 'XWebHTTPRead', 'xweb_http_read',
    'flue_http_read', 'FlueHttpRead', 'HttpRead'
  ],
  websocketWrite: [
    'XWebWebSocketWrite', 'XWebWebsocketWrite', 'xweb_websocket_write',
    'flue_websocket_write', 'FlueWebSocketWrite', 'WebSocketWrite'
  ],
  websocketRead: [
    'XWebWebSocketRead', 'XWebWebsocketRead', 'xweb_websocket_read',
    'flue_websocket_read', 'FlueWebSocketRead', 'WebSocketRead'
  ]
};

function stringList(value, fallback) {
  if (typeof value !== 'string' || value.trim().length === 0) return fallback;
  return value.split('|').map((item) => item.trim()).filter((item) => item.length > 0);
}

function findModule() {
  const modules = Process.enumerateModules();
  const wanted = (moduleName || '').toLowerCase();
  const exact = modules.find((item) => item.name.toLowerCase() === wanted);
  if (exact) return exact;
  const byPath = modules.find((item) => {
    const value = `${item.name} ${item.path}`.toLowerCase();
    return wanted.length > 0 && value.includes(wanted);
  });
  if (byPath) return byPath;
  return modules.find((item) => {
    const value = `${item.name} ${item.path}`.toLowerCase();
    return value.includes('wechatappex framework') || value.includes('flue') || value.includes('xweb');
  });
}

function symbolName(symbol) {
  return (symbol.name || '').replace(/^_/, '').toLowerCase();
}

function findSymbol(module, configuredName, fallbackNames) {
  const names = stringList(configuredName, fallbackNames);
  for (const name of names) {
    try {
      const exported = module.findExportByName(name);
      if (exported) return { address: exported, name, source: 'export' };
    } catch (_) {
      // Some Frida versions expose getExportByName but not findExportByName.
      try {
        const exported = module.getExportByName(name);
        if (exported) return { address: exported, name, source: 'export' };
      } catch (_) {
        // Continue with the complete symbol table below.
      }
    }
  }

  let symbols = [];
  try {
    symbols = module.enumerateSymbols();
  } catch (_) {
    return null;
  }
  for (const name of names) {
    const wanted = name.replace(/^_/, '').toLowerCase();
    const exact = symbols.find((item) => symbolName(item) === wanted && item.type === 'function');
    if (exact) return { address: exact.address, name: exact.name, source: 'symbol' };
  }
  return null;
}

function pointerString(value) {
  try { return value.toString(); } catch (_) { return 'unknown'; }
}

function boundedLength(value) {
  try {
    const length = value.toInt32();
    return length > 0 && length <= maxBytes ? length : 0;
  } catch (_) {
    return 0;
  }
}

function emitBuffer(transport, direction, connection, buffer, length, boundary) {
  const bounded = boundedLength(length);
  if (buffer.isNull() || bounded <= 0) return;
  send({
    kind: 'plaintext-buffer', transport, direction,
    connection: pointerString(connection), boundary, length: bounded
  }, buffer.readByteArray(bounded));
}

function attachWrite(hook, transport, boundary) {
  Interceptor.attach(hook.address, {
    onEnter(args) {
      // 统一约定为 (connection, buffer, length)，与既有 flue 采集器相同。
      emitBuffer(transport, 'request', args[0], args[1], args[2], boundary);
    }
  });
}

function attachRead(hook, transport, boundary) {
  Interceptor.attach(hook.address, {
    onEnter(args) {
      this.connection = args[0];
      this.buffer = args[1];
      this.capacity = boundedLength(args[2]);
    },
    onLeave(retval) {
      const length = boundedLength(retval);
      if (length > 0 && length <= this.capacity) {
        emitBuffer(transport, 'response', this.connection, this.buffer, length, boundary);
      }
    }
  });
}

const module = findModule();
if (!module) {
  send({
    kind: 'attach-failure', code: 'plaintext-boundary-module-not-found',
    message: `未找到 macOS 明文边界模块：${moduleName || 'WeChatAppEx Framework / Flue / XWeb'}`
  });
  throw new Error('macOS 明文边界模块未加载');
}

const hooks = {
  httpWrite: findSymbol(module, configured.httpWrite, DEFAULT_SYMBOLS.httpWrite),
  httpRead: findSymbol(module, configured.httpRead, DEFAULT_SYMBOLS.httpRead),
  websocketWrite: findSymbol(module, configured.websocketWrite, DEFAULT_SYMBOLS.websocketWrite),
  websocketRead: findSymbol(module, configured.websocketRead, DEFAULT_SYMBOLS.websocketRead)
};
const missing = Object.keys(hooks).filter((key) => !hooks[key]);
if (missing.length > 0) {
  send({
    kind: 'attach-failure', code: 'plaintext-boundary-hook-not-found',
    module: module.name, missing,
    message: `macOS HTTP/WebSocket 明文边界入口不完整，缺少：${missing.join(', ')}`
  });
  throw new Error(`缺少 macOS 明文边界入口：${missing.join(', ')}`);
}

attachWrite(hooks.httpWrite, 'http', 'xweb-http');
attachRead(hooks.httpRead, 'http', 'xweb-http');
attachWrite(hooks.websocketWrite, 'websocket', 'xweb-websocket');
attachRead(hooks.websocketRead, 'websocket', 'xweb-websocket');

send({
  kind: 'ready', pid: Process.id, arch: Process.arch, module: module.name,
  modulePath: module.path, strategy: 'macos-symbols',
  coverage: { http: true, websocket: true, request: true, response: true },
  hooks: Object.fromEntries(Object.entries(hooks).map(([key, value]) => [key, {
    name: value.name, address: pointerString(value.address), source: value.source
  }]))
});
"""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def parse_integer(value: str) -> int:
    return int(value, 0)


def build_agent(args: argparse.Namespace) -> str:
    if args.transport == "winhttp":
        return WINHTTP_AGENT.replace("__MAX_BYTES__", str(args.max_bytes))
    if args.transport == "macos":
        return (
            MACOS_AGENT.replace("__MACOS_MODULE__", json.dumps(args.macos_module))
            .replace("__MACOS_HTTP_WRITE__", json.dumps(args.macos_http_write_symbol or ""))
            .replace("__MACOS_HTTP_READ__", json.dumps(args.macos_http_read_symbol or ""))
            .replace("__MACOS_WEBSOCKET_WRITE__", json.dumps(args.macos_websocket_write_symbol or ""))
            .replace("__MACOS_WEBSOCKET_READ__", json.dumps(args.macos_websocket_read_symbol or ""))
            .replace("__MAX_BYTES__", str(args.max_bytes))
        )
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


class HttpReassembler:
    """保存 XWeb/Flue 已解密的 HTTP 片段，不把它误套成 WebSocket 帧。"""

    def feed(self, _direction: str, _connection: str, data: bytes) -> list[tuple[int | None, bytes]]:
        return [(None, data)] if data else []


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


def resolve_output_dir(path: Path | None, *, allow_external: bool = False) -> Path:
    if path is None:
        stamp = datetime.now().strftime("session-%Y%m%d-%H%M%S")
        return PROJECT_ROOT / "captures" / "miniapp-protocol" / stamp
    resolved = (PROJECT_ROOT / path).resolve() if not path.is_absolute() else path.resolve()
    if not allow_external and not resolved.is_relative_to(PROJECT_ROOT):
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
    http = HttpReassembler()
    assert http.feed("response", "http-1", b"HTTP/2 plaintext") == [(None, b"HTTP/2 plaintext")]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="记录微信小游戏运行时明文游戏协议。")
    parser.add_argument("--transport", choices=("winhttp", "flue", "macos"), default="winhttp")
    parser.add_argument("--pid", type=int)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--duration", type=int, default=3600)
    parser.add_argument("--api-index", type=Path, default=DEFAULT_API_INDEX)
    parser.add_argument("--max-bytes", type=int, default=16 * 1024 * 1024)
    parser.add_argument("--flue-module", default="flue.dll")
    parser.add_argument("--flue-write-rva", type=parse_integer, default=DEFAULT_FLUE_WRITE_RVA)
    parser.add_argument("--flue-read-rva", type=parse_integer, default=DEFAULT_FLUE_READ_RVA)
    parser.add_argument("--macos-module", default=DEFAULT_MACOS_MODULE)
    parser.add_argument("--macos-http-write-symbol")
    parser.add_argument("--macos-http-read-symbol")
    parser.add_argument("--macos-websocket-write-symbol")
    parser.add_argument("--macos-websocket-read-symbol")
    parser.add_argument(
        "--allow-external-output",
        action="store_true",
        help="允许桌面端把会话临时证据写入工作区；仅由 MiniTrace 后端使用",
    )
    parser.add_argument(
        "--stop-file",
        type=Path,
        help="存在该文件时请求采集器正常停止并写出 summary.json",
    )
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

    output_dir = resolve_output_dir(
        args.output_dir,
        allow_external=args.allow_external_output,
    )
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
    http = HttpReassembler()
    stop_requested = False
    target_detached = False
    detach_detail = None
    fatal_error = None
    frame_sequence = 0
    payload_sequence = 0
    observation_sequence = 0
    counts = {"request": 0, "response": 0}
    type_counts = defaultdict(int)

    def append_jsonl(path: Path, value: dict) -> None:
        with path.open("a", encoding="utf-8") as output:
            output.write(json.dumps(value, ensure_ascii=False) + "\n")

    def save_payload(
        direction: str,
        connection: str,
        opcode: int | None,
        payload: bytes,
        timestamp: str,
        observed_at_ms: int,
        transport: str,
        observation_key: str,
    ) -> bool:
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
                "kind": "protocol_frame",
                "sequence": frame_sequence, "timestamp": timestamp, "direction": direction,
                "connection": connection, "webSocketOpcode": opcode, "messageType": message_type,
                "transport": transport, "observedAtMs": observed_at_ms,
                "observationKey": observation_key,
                "protocolName": names.get(message_type), "declaredLength": declared_length,
                "actualLength": len(frame), "sha256": hashlib.sha256(frame).hexdigest(),
                "rawFile": f"frames/{filename}",
            })
        if remainder:
            payload_sequence += 1
            filename = f"{payload_sequence:06d}-{direction}-unparsed.bin"
            (payloads_dir / filename).write_bytes(remainder)
            append_jsonl(timeline_path, {
                "kind": "unparsed_payload",
                "timestamp": timestamp, "direction": direction, "connection": connection,
                "webSocketOpcode": opcode, "transport": transport,
                "observedAtMs": observed_at_ms, "observationKey": observation_key,
                "unparsedLength": len(remainder),
                "sha256": hashlib.sha256(remainder).hexdigest(),
                "rawFile": f"unparsed-payloads/{filename}",
            })
        return bool(remainder)

    def on_message(message, data) -> None:
        nonlocal fatal_error, stop_requested, observation_sequence
        timestamp = utc_now()
        if message.get("type") != "send":
            fatal_error = message
            append_jsonl(transport_path, {"timestamp": timestamp, "kind": "frida-error", "message": message})
            stop_requested = True
            return
        payload = message.get("payload", {})
        if payload.get("kind") == "ready":
            ready_path.write_text(json.dumps({"timestamp": timestamp, **payload}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if payload.get("kind") == "attach-failure":
            append_jsonl(transport_path, {"timestamp": timestamp, **payload})
            fatal_error = payload
            stop_requested = True
            return
        if payload.get("kind") != "plaintext-buffer" or data is None:
            append_jsonl(transport_path, {"timestamp": timestamp, **payload})
            return
        direction = payload["direction"]
        connection = payload["connection"]
        transport = payload.get("transport") or ("websocket" if args.transport == "flue" else "http")
        raw = bytes(data)
        observed_at_ms = int(time.time() * 1000)
        observation_key = ""
        observation_sha256 = hashlib.sha256(raw).hexdigest()
        if args.transport == "macos":
            observation_sequence += 1
            observation_key = f"{observation_sequence:06d}-{direction}-{transport}-{connection}"
            observation_filename = f"{observation_sequence:06d}-{direction}-{transport}.bin"
            observation_path = output_dir / "observations" / observation_filename
            observation_path.parent.mkdir(parents=True, exist_ok=True)
            observation_path.write_bytes(raw)
        append_jsonl(transport_path, {
            "timestamp": timestamp, "kind": "plaintext-buffer", "direction": direction,
            "connection": connection, "transport": transport,
            "boundary": payload.get("boundary"), "bufferType": payload.get("bufferType"),
            "length": len(raw), "sha256": observation_sha256,
            "previewHex": raw[:16].hex(),
        })
        if args.transport == "flue":
            messages = websocket.feed(direction, connection, raw)
        elif args.transport == "macos" and transport == "websocket":
            messages = websocket.feed(direction, connection, raw)
        elif args.transport == "macos":
            messages = http.feed(direction, connection, raw)
        else:
            messages = winhttp.feed(direction, payload.get("bufferType"), raw)
        unparsed = any(
            split_game_frames(message_payload)[1]
            for _opcode, message_payload in messages
        )
        if args.transport == "macos":
            # 先归档完整明文边界，再归档其协议帧/未分类投影。Rust 侧通过
            # observationKey 建立父子关系；顺序稳定也让 JSONL 可直接审计。
            append_jsonl(timeline_path, {
                "kind": "raw_observation", "timestamp": timestamp,
                "observedAtMs": observed_at_ms, "direction": direction,
                "connection": connection, "transport": transport,
                "boundary": payload.get("boundary"), "observationKey": observation_key,
                "classification": "unparsed_payload" if unparsed else None,
                "length": len(raw), "actualLength": len(raw),
                "sha256": observation_sha256,
                "rawFile": f"observations/{observation_filename}",
            })
        for opcode, message_payload in messages:
            save_payload(
                direction,
                connection,
                opcode,
                message_payload,
                timestamp,
                observed_at_ms,
                transport,
                observation_key,
            )

    def request_stop(_signum, _frame) -> None:
        nonlocal stop_requested
        stop_requested = True

    def on_detached(reason, crash=None) -> None:
        """Frida 在目标进程退出或会话失效时唤醒采集主循环。"""
        nonlocal target_detached, detach_detail
        target_detached = True
        detach_detail = {
            "reason": str(reason),
            "crash": str(crash) if crash is not None else None,
        }

    signal.signal(signal.SIGINT, request_stop)
    signal.signal(signal.SIGTERM, request_stop)
    session = None
    script = None
    try:
        import frida

        session = frida.get_local_device().attach(args.pid)
        session.on("detached", on_detached)
        script = session.create_script(build_agent(args))
        script.on("message", on_message)
        script.load()
        deadline = time.monotonic() + args.duration
        stop_file = args.stop_file.resolve() if args.stop_file else None
        while not stop_requested and not target_detached and time.monotonic() < deadline:
            if stop_file is not None and stop_file.is_file():
                stop_requested = True
                break
            time.sleep(0.1)
        if target_detached and not stop_requested:
            fatal_error = {
                "type": "TargetDetached",
                "message": f"目标进程已断开：{detach_detail}",
                "diagnosticCode": "target-process-terminated",
                "detail": detach_detail,
            }
            append_jsonl(
                transport_path,
                {"timestamp": utc_now(), "kind": "target-detached", **fatal_error},
            )
    except Exception as error:
        message = str(error)
        lowered = message.lower()
        if "permission" in lowered or "access" in lowered or "attach" in lowered or "hardened" in lowered:
            diagnostic_code = "standard-frida-attach-unavailable"
        elif "frida" in lowered or "no module named" in lowered:
            diagnostic_code = "frida-runtime-unavailable"
        else:
            diagnostic_code = "collector-start-failed"
        fatal_error = {
            "type": type(error).__name__,
            "message": message,
            "diagnosticCode": diagnostic_code,
            "hint": "不关闭 SIP、不修改或重签微信；请确认标准 Frida 授权和目标进程仍在运行",
        }
        append_jsonl(
            transport_path,
            {"timestamp": utc_now(), "kind": "attach-failure", **fatal_error},
        )
    finally:
        summary = {
            "finishedAt": utc_now(), "pid": args.pid, "transport": args.transport,
            "counts": counts, "typeCounts": dict(sorted(type_counts.items())),
            "fatalError": fatal_error, "timedOut": not stop_requested,
            "frameCount": frame_sequence, "unparsedPayloadCount": payload_sequence,
            "observationCount": observation_sequence,
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
