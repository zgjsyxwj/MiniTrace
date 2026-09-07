//! macOS Frida 明文采集后端适配。
//!
//! macOS 的微信承载进程使用带 Hardened Runtime 的 WeChatAppEx。Rust 侧不
//! 重写 Frida 或拆帧逻辑，而是启动仓库内的 `capture_miniapp_protocol.py`
//! macOS 模式。Python/Frida 负责在已经核对的 XWeb/Flue 符号上同时观察
//! HTTP/HTTPS 与 WebSocket 的请求、响应；本模块负责停止控制、原始证据
//! 校验和统一 `capture://event` 会话归档。
//!
//! 未找到标准 Frida、目标模块或任一明文方向时，采集器会写出明确的
//! `attach-failure`，本模块把已有证据发布为 `incomplete`，绝不退回 TLS
//! 密文、关闭 SIP、修改微信或把单一 Hook 当成完整能力。

use crate::backend::EventSink;
use crate::model::{CandidateProcess, SessionSummary};
#[cfg(any(target_os = "macos", test))]
use crate::model::{
    CaptureEvent, CaptureEventKind, ProtocolFrame, RawObservation, TrafficDirection, TransportKind,
};
use crate::recovery::RecoveryCommand;
#[cfg(target_os = "macos")]
use crate::recovery::{RecoveryAction, RecoveryBridge};
use crate::storage::SessionWriter;
#[cfg(any(target_os = "macos", test))]
use crate::storage::{now_ms, sha256_hex};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", test))]
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
#[cfg(any(target_os = "macos", test))]
use std::fs::{self, File};
#[cfg(any(target_os = "macos", test))]
use std::io::{Read, Seek, SeekFrom};
#[cfg(any(target_os = "macos", test))]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::{mpsc::Receiver, Arc};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

pub const MACOS_BACKEND_ID: &str = "macos-frida";
pub const MACOS_PLATFORM_STATUS: &str = "macOS Frida 双边界待实机验收";
pub const MACOS_MODULE_NAME: &str = "WeChatAppEx Framework";
pub const CAPTURE_SCRIPT_NAME: &str = "capture_miniapp_protocol.py";

const STAGING_DIRECTORY: &str = "macos-frida";
const STOP_FILE_NAME: &str = "stop.requested";
#[cfg(target_os = "macos")]
const READY_FILE_NAME: &str = "ready.json";
#[cfg(target_os = "macos")]
const SUMMARY_FILE_NAME: &str = "summary.json";
#[cfg(target_os = "macos")]
const TIMELINE_FILE_NAME: &str = "timeline.jsonl";
#[cfg(target_os = "macos")]
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(15);

/// Python macOS 采集器的启动配置。
///
/// 符号参数允许针对每个已核对的微信构建传入实际入口。默认值为空，
/// Python agent 只尝试安全的明确别名；找不到 HTTP 或 WebSocket 任一方向
/// 就拒绝进入 ready。发布物自包含运行时仍按 ADR 0001 在后续阶段处理。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MacosLaunchSpec {
    pub python: PathBuf,
    pub script: PathBuf,
    pub output_dir: PathBuf,
    pub stop_file: PathBuf,
    pub pid: u32,
    pub module: String,
    pub http_write_symbol: String,
    pub http_read_symbol: String,
    pub websocket_write_symbol: String,
    pub websocket_read_symbol: String,
    pub max_bytes: u64,
}

impl MacosLaunchSpec {
    pub fn for_session(session_root: &Path, pid: u32) -> Result<Self, String> {
        Self::for_segment(session_root, pid, "segment-001")
    }

    fn for_segment(session_root: &Path, pid: u32, segment_id: &str) -> Result<Self, String> {
        let output_dir = session_root.join(STAGING_DIRECTORY).join(segment_id);
        let stop_file = output_dir.join(STOP_FILE_NAME);
        let python = env::var_os("MINITRACE_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python3"));
        let script = env::var_os("MINITRACE_CAPTURE_SCRIPT")
            .map(PathBuf::from)
            .or_else(locate_capture_script)
            .ok_or_else(|| {
                "找不到 capture_miniapp_protocol.py；请设置 MINITRACE_CAPTURE_SCRIPT".to_string()
            })?;
        Ok(Self {
            python,
            script,
            output_dir,
            stop_file,
            pid,
            module: env::var("MINITRACE_MACOS_MODULE")
                .unwrap_or_else(|_| MACOS_MODULE_NAME.to_string()),
            http_write_symbol: env::var("MINITRACE_MACOS_HTTP_WRITE_SYMBOL").unwrap_or_default(),
            http_read_symbol: env::var("MINITRACE_MACOS_HTTP_READ_SYMBOL").unwrap_or_default(),
            websocket_write_symbol: env::var("MINITRACE_MACOS_WEBSOCKET_WRITE_SYMBOL")
                .unwrap_or_default(),
            websocket_read_symbol: env::var("MINITRACE_MACOS_WEBSOCKET_READ_SYMBOL")
                .unwrap_or_default(),
            max_bytes: 16 * 1024 * 1024,
        })
    }

    /// 暴露给契约测试的参数序列，确保 macOS 使用双明文边界模式。
    pub fn command_args(&self) -> Vec<String> {
        vec![
            self.script.to_string_lossy().into_owned(),
            "--transport".to_string(),
            "macos".to_string(),
            "--pid".to_string(),
            self.pid.to_string(),
            "--output-dir".to_string(),
            self.output_dir.to_string_lossy().into_owned(),
            "--allow-external-output".to_string(),
            "--stop-file".to_string(),
            self.stop_file.to_string_lossy().into_owned(),
            "--macos-module".to_string(),
            self.module.clone(),
            "--macos-http-write-symbol".to_string(),
            self.http_write_symbol.clone(),
            "--macos-http-read-symbol".to_string(),
            self.http_read_symbol.clone(),
            "--macos-websocket-write-symbol".to_string(),
            self.websocket_write_symbol.clone(),
            "--macos-websocket-read-symbol".to_string(),
            self.websocket_read_symbol.clone(),
            "--max-bytes".to_string(),
            self.max_bytes.to_string(),
        ]
    }
}

fn locate_capture_script() -> Option<PathBuf> {
    let mut directories = Vec::new();
    if let Ok(current) = env::current_dir() {
        directories.push(current);
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            directories.push(parent.to_path_buf());
            if let Some(grandparent) = parent.parent() {
                directories.push(grandparent.to_path_buf());
            }
        }
    }
    directories
        .into_iter()
        .map(|directory| directory.join(CAPTURE_SCRIPT_NAME))
        .find(|path| path.is_file())
}

#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
struct JsonlTail {
    offset: u64,
    pending: String,
}

#[cfg(any(target_os = "macos", test))]
impl JsonlTail {
    fn read(&mut self, path: &Path) -> Result<Vec<Value>, String> {
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let mut file =
            File::open(path).map_err(|error| format!("打开 macOS Python 时间线失败：{error}"))?;
        let file_length = file
            .metadata()
            .map_err(|error| format!("读取 macOS Python 时间线大小失败：{error}"))?
            .len();
        if file_length < self.offset {
            self.offset = 0;
            self.pending.clear();
        }
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|error| format!("定位 macOS Python 时间线失败：{error}"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("读取 macOS Python 时间线失败：{error}"))?;
        self.offset += bytes.len() as u64;
        self.pending.push_str(
            std::str::from_utf8(&bytes)
                .map_err(|error| format!("macOS Python 时间线不是 UTF-8：{error}"))?,
        );

        let complete_length = self.pending.rfind('\n').map(|index| index + 1).unwrap_or(0);
        let complete = self.pending[..complete_length].to_string();
        self.pending = self.pending[complete_length..].to_string();
        let mut values = Vec::new();
        for line in complete.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            values.push(
                serde_json::from_str::<Value>(line)
                    .map_err(|error| format!("解析 macOS Python 时间线条目失败：{error}"))?,
            );
        }
        Ok(values)
    }
}

#[cfg(any(target_os = "macos", test))]
fn value_string(value: &Value, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("macOS Python 时间线缺少字符串字段：{field}"))
}

#[cfg(any(target_os = "macos", test))]
fn optional_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

#[cfg(any(target_os = "macos", test))]
fn value_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("macOS Python timeline missing integer field: {field}"))
}

#[cfg(any(target_os = "macos", test))]
fn observed_at_ms(value: &Value) -> u64 {
    value
        .get("observedAtMs")
        .and_then(Value::as_u64)
        .unwrap_or_else(now_ms)
}

#[cfg(any(target_os = "macos", test))]
fn parse_direction(value: &str) -> Result<TrafficDirection, String> {
    match value {
        "request" => Ok(TrafficDirection::Request),
        "response" => Ok(TrafficDirection::Response),
        _ => Err(format!("macOS Python 时间线方向无效：{value}")),
    }
}

#[cfg(any(target_os = "macos", test))]
fn parse_transport(value: Option<&str>) -> Result<TransportKind, String> {
    match value.unwrap_or("websocket") {
        "websocket" => Ok(TransportKind::WebSocket),
        "http" => Ok(TransportKind::Http),
        other => Err(format!("macOS Python 时间线传输类型无效：{other}")),
    }
}

#[cfg(any(target_os = "macos", test))]
fn safe_evidence_path(staging: &Path, raw_file: &str) -> Result<PathBuf, String> {
    let relative = Path::new(raw_file);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("macOS Python 原始文件路径越界：{raw_file}"));
    }
    Ok(staging.join(relative))
}

#[cfg(any(target_os = "macos", test))]
fn evidence_bytes(staging: &Path, entry: &Value) -> Result<Vec<u8>, String> {
    let raw_file = value_string(entry, "rawFile")?;
    let path = safe_evidence_path(staging, &raw_file)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("读取 macOS Python 原始证据失败：{error}"))?;
    let expected_length = entry
        .get("actualLength")
        .or_else(|| entry.get("length"))
        .or_else(|| entry.get("unparsedLength"))
        .and_then(Value::as_u64);
    if let Some(length) = expected_length {
        if length != bytes.len() as u64 {
            return Err(format!(
                "macOS Python 原始证据长度不一致：声明 {length}，实际 {}",
                bytes.len()
            ));
        }
    }
    if let Some(expected) = entry.get("sha256").and_then(Value::as_str) {
        let actual = sha256_hex(&bytes);
        if expected != actual {
            return Err(format!(
                "macOS Python 原始证据 SHA-256 不一致：声明 {expected}，实际 {actual}"
            ));
        }
    }
    Ok(bytes)
}

#[cfg(any(target_os = "macos", test))]
#[allow(clippy::too_many_arguments)]
fn append_observation(
    writer: &mut SessionWriter,
    session_id: &str,
    segment_id: &str,
    raw_sequence: &mut u64,
    source_raw_file: Option<&str>,
    bytes: &[u8],
    direction: TrafficDirection,
    connection_id: String,
    transport: TransportKind,
    observed_at_ms: u64,
    classification: Option<&str>,
    sink: &Arc<dyn EventSink>,
) -> Result<RawObservation, String> {
    *raw_sequence += 1;
    let observation_id = format!("{session_id}-observation-{raw_sequence:06}");
    let raw_file = writer.write_raw(*raw_sequence, bytes)?;
    let sha256 = sha256_hex(bytes);
    let observation = RawObservation {
        observation_id: observation_id.clone(),
        segment_id: segment_id.to_string(),
        connection_id: connection_id.clone(),
        direction: direction.clone(),
        transport: transport.clone(),
        observed_at_ms,
        length: bytes.len(),
        sha256: sha256.clone(),
        raw_file: raw_file.clone(),
    };
    let mut data = serde_json::to_value(&observation)
        .map_err(|error| format!("序列化 macOS 原始观察失败：{error}"))?;
    if let Some(classification) = classification {
        data["classification"] = json!(classification);
    }
    writer.append_event(observed_at_ms, "raw_observation", data.clone())?;
    writer.append_transport_event(
        observed_at_ms,
        json!({
            "backend": MACOS_BACKEND_ID,
            "source": "capture_miniapp_protocol.py",
            "sourceRawFile": source_raw_file,
            "observationId": observation_id,
            "segmentId": segment_id,
            "connectionId": connection_id,
            "direction": direction,
            "transport": transport,
            "length": bytes.len(),
            "sha256": sha256,
            "classification": classification
        }),
    )?;
    writer.add_observation(observation.clone());
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::RawObservation,
            session_id: session_id.to_string(),
            timestamp_ms: observed_at_ms,
            data,
        },
    );
    Ok(observation)
}

#[cfg(any(target_os = "macos", test))]
/// 将 macOS Python 时间线条目转换成统一事件。
///
/// 一个明文边界回调先产生一条 `raw_observation`，其后的
/// `protocol_frame`/`unparsed_payload` 只描述这条原始观察的逻辑投影。
/// 因此 HTTP、HTTPS（在 XWeb/Flue 解密边界）和 WebSocket 都保留来源、
/// 方向、连接、分段、长度及 SHA-256，且不会静默去重。
#[allow(clippy::too_many_arguments)]
fn append_python_entry(
    writer: &mut SessionWriter,
    staging: &Path,
    session_id: &str,
    segment_id: &str,
    raw_sequence: &mut u64,
    observation_ids: &mut HashMap<String, String>,
    entry: &Value,
    sink: &Arc<dyn EventSink>,
) -> Result<(), String> {
    let kind = entry
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "raw_observation" => {
            let bytes = evidence_bytes(staging, entry)?;
            let direction = parse_direction(&value_string(entry, "direction")?)?;
            let connection_id = value_string(entry, "connection")?;
            let transport = parse_transport(optional_string(entry, "transport").as_deref())?;
            let classification = optional_string(entry, "classification");
            let observation = append_observation(
                writer,
                session_id,
                segment_id,
                raw_sequence,
                optional_string(entry, "rawFile").as_deref(),
                &bytes,
                direction,
                connection_id,
                transport,
                observed_at_ms(entry),
                classification.as_deref(),
                sink,
            )?;
            if let Some(key) = optional_string(entry, "observationKey") {
                observation_ids.insert(key, observation.observation_id);
            }
            Ok(())
        }
        "protocol_frame" => {
            let bytes = evidence_bytes(staging, entry)?;
            let direction = parse_direction(&value_string(entry, "direction")?)?;
            let connection_id = value_string(entry, "connection")?;
            let transport = parse_transport(optional_string(entry, "transport").as_deref())?;
            let observed_at_ms = observed_at_ms(entry);
            let observation_id = if let Some(key) = optional_string(entry, "observationKey") {
                if let Some(observation_id) = observation_ids.get(&key) {
                    observation_id.clone()
                } else {
                    // 兼容没有 raw_observation 条目的旧 fixture：协议帧本身
                    // 仍是可验证的原始边界，不丢弃也不伪造其父关联。
                    let observation = append_observation(
                        writer,
                        session_id,
                        segment_id,
                        raw_sequence,
                        optional_string(entry, "rawFile").as_deref(),
                        &bytes,
                        direction.clone(),
                        connection_id.clone(),
                        transport.clone(),
                        observed_at_ms,
                        Some("protocol_frame_fallback"),
                        sink,
                    )?;
                    observation_ids.insert(key, observation.observation_id.clone());
                    observation.observation_id
                }
            } else {
                let observation = append_observation(
                    writer,
                    session_id,
                    segment_id,
                    raw_sequence,
                    optional_string(entry, "rawFile").as_deref(),
                    &bytes,
                    direction.clone(),
                    connection_id.clone(),
                    transport.clone(),
                    observed_at_ms,
                    Some("protocol_frame_fallback"),
                    sink,
                )?;
                observation.observation_id
            };
            let message_id = value_u64(entry, "messageType")?;
            let message_id = u32::try_from(message_id)
                .map_err(|_| format!("macOS Python 协议号超出 u32 范围：{message_id}"))?;
            let protocol_name = optional_string(entry, "protocolName")
                .unwrap_or_else(|| format!("UNKNOWN_{message_id}"));
            *raw_sequence += 1;
            let raw_file = writer.write_raw(*raw_sequence, &bytes)?;
            let sha256 = sha256_hex(&bytes);
            let frame = ProtocolFrame {
                frame_id: format!("{session_id}-frame-{raw_sequence:06}"),
                observation_id,
                segment_id: segment_id.to_string(),
                connection_id,
                direction,
                transport,
                observed_at_ms,
                message_id,
                protocol_name,
                length: bytes.len(),
                sha256,
                raw_file,
            };
            let data = serde_json::to_value(&frame)
                .map_err(|error| format!("序列化 macOS 协议帧失败：{error}"))?;
            writer.append_event(observed_at_ms, "protocol_frame", data.clone())?;
            writer.add_frame(frame);
            emit(
                sink,
                CaptureEvent {
                    kind: CaptureEventKind::ProtocolFrame,
                    session_id: session_id.to_string(),
                    timestamp_ms: observed_at_ms,
                    data,
                },
            );
            Ok(())
        }
        "unparsed_payload" => {
            // raw_observation 已经保存完整边界字节。若旧或手工 fixture
            // 只有未分类条目，则补建一个带明确分类的观察。
            if let Some(key) = optional_string(entry, "observationKey") {
                if observation_ids.contains_key(&key) {
                    return Ok(());
                }
                let bytes = evidence_bytes(staging, entry)?;
                let direction = parse_direction(&value_string(entry, "direction")?)?;
                let observation = append_observation(
                    writer,
                    session_id,
                    segment_id,
                    raw_sequence,
                    optional_string(entry, "rawFile").as_deref(),
                    &bytes,
                    direction,
                    value_string(entry, "connection")?,
                    parse_transport(optional_string(entry, "transport").as_deref())?,
                    observed_at_ms(entry),
                    Some("unparsed_payload"),
                    sink,
                )?;
                observation_ids.insert(key, observation.observation_id);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(any(target_os = "macos", test))]
fn emit(sink: &Arc<dyn EventSink>, event: CaptureEvent) {
    // 会话文件是权威证据；窗口关闭或事件订阅端断开不能阻止 worker 收尾。
    let _ = sink.emit(event);
}

#[cfg(target_os = "macos")]
fn read_python_failure(staging: &Path) -> Option<String> {
    let summary_path = staging.join(SUMMARY_FILE_NAME);
    let bytes = fs::read(summary_path).ok()?;
    let summary: Value = serde_json::from_slice(&bytes).ok()?;
    let fatal = summary.get("fatalError")?;
    if fatal.is_null() {
        return None;
    }
    Some(
        fatal
            .get("message")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| fatal.to_string()),
    )
}

#[cfg(target_os = "macos")]
fn read_python_coverage(staging: &Path) -> Result<(bool, bool), String> {
    let ready_path = staging.join(READY_FILE_NAME);
    let bytes = fs::read(&ready_path)
        .map_err(|error| format!("读取 macOS Frida ready.json 失败：{error}"))?;
    let ready: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("解析 macOS Frida ready.json 失败：{error}"))?;
    let coverage = ready
        .get("coverage")
        .ok_or_else(|| "macOS Frida ready.json 缺少 coverage".to_string())?;
    let http = coverage
        .get("http")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let websocket = coverage
        .get("websocket")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok((http, websocket))
}

#[cfg(target_os = "macos")]
fn finish_capture(
    writer: SessionWriter,
    session_id: &str,
    started_at_ms: u64,
    candidate: &CandidateProcess,
    sink: &Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    let ended_at_ms = now_ms();
    let summary = writer.finish(session_id, started_at_ms, ended_at_ms, candidate)?;
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Stopped,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化 macOS 停止事件失败：{error}"))?,
        },
    );
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Summary,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化 macOS 摘要事件失败：{error}"))?,
        },
    );
    Ok(summary)
}

#[cfg(target_os = "macos")]
fn finish_incomplete_capture(
    writer: SessionWriter,
    session_id: &str,
    started_at_ms: u64,
    candidate: &CandidateProcess,
    sink: &Arc<dyn EventSink>,
    reason: String,
) -> Result<SessionSummary, String> {
    let ended_at_ms = now_ms();
    let summary = writer.finish_incomplete(session_id, started_at_ms, ended_at_ms, candidate)?;
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Stopped,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: json!({ "status": "incomplete", "reason": reason.clone() }),
        },
    );
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Summary,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化 macOS 不完整摘要失败：{error}"))?,
        },
    );
    Err(reason)
}

#[cfg(target_os = "macos")]
fn finish_incomplete_summary(
    writer: SessionWriter,
    session_id: &str,
    started_at_ms: u64,
    candidate: &CandidateProcess,
    sink: &Arc<dyn EventSink>,
    reason: String,
) -> Result<SessionSummary, String> {
    let ended_at_ms = now_ms();
    let summary = writer.finish_incomplete(session_id, started_at_ms, ended_at_ms, candidate)?;
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Stopped,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: json!({ "status": "incomplete", "reason": reason }),
        },
    );
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Summary,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化 macOS 不完整摘要失败：{error}"))?,
        },
    );
    Ok(summary)
}

#[cfg(target_os = "macos")]
struct ActiveCollector {
    child: std::process::Child,
    spec: MacosLaunchSpec,
    segment_id: String,
    pid: u32,
    timeline: JsonlTail,
    observation_ids: HashMap<String, String>,
    ready_seen: bool,
    stop_seen: bool,
    stop_deadline: Option<Instant>,
}

#[cfg(target_os = "macos")]
impl ActiveCollector {
    fn ready_path(&self) -> PathBuf {
        self.spec.output_dir.join(READY_FILE_NAME)
    }

    fn timeline_path(&self) -> PathBuf {
        self.spec.output_dir.join(TIMELINE_FILE_NAME)
    }

    fn request_stop(&mut self) -> Result<(), String> {
        if self.stop_seen {
            return Ok(());
        }
        fs::write(&self.spec.stop_file, b"stop\n")
            .map_err(|error| format!("写入 macOS 停止标记失败：{error}"))?;
        self.stop_seen = true;
        self.stop_deadline = Some(Instant::now() + STOP_GRACE_PERIOD);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn start_collector(
    session_root: &Path,
    candidate: &CandidateProcess,
    segment_id: &str,
) -> Result<ActiveCollector, String> {
    let spec = MacosLaunchSpec::for_segment(session_root, candidate.pid, segment_id)?;
    fs::create_dir_all(&spec.output_dir)
        .map_err(|error| format!("创建 macOS Frida 临时目录失败：{error}"))?;
    if spec.stop_file.exists() {
        fs::remove_file(&spec.stop_file)
            .map_err(|error| format!("清理 macOS 停止标记失败：{error}"))?;
    }
    let mut command = std::process::Command::new(&spec.python);
    command
        .args(spec.command_args())
        .current_dir(session_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let child = command
        .spawn()
        .map_err(|error| format!("启动 macOS Frida Python 采集器失败：{error}"))?;
    Ok(ActiveCollector {
        child,
        spec,
        segment_id: segment_id.to_string(),
        pid: candidate.pid,
        timeline: JsonlTail::default(),
        observation_ids: HashMap::new(),
        ready_seen: false,
        stop_seen: false,
        stop_deadline: None,
    })
}

#[cfg(target_os = "macos")]
fn process_is_alive(pid: u32) -> bool {
    // `ps` 在受限 macOS 宿主中可能被系统拒绝，即使目标仍存活。signal 0
    // 不发送实际信号，只检查 PID 是否存在及当前进程是否有权访问它。
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn abort_active(
    mut active: Option<ActiveCollector>,
    writer: SessionWriter,
    session_id: &str,
    started_at_ms: u64,
    candidate: &CandidateProcess,
    sink: &Arc<dyn EventSink>,
    reason: String,
) -> Result<SessionSummary, String> {
    if let Some(collector) = active.as_mut() {
        let _ = collector.child.kill();
        let _ = collector.child.wait();
    }
    finish_incomplete_capture(writer, session_id, started_at_ms, candidate, sink, reason)
}

#[cfg(target_os = "macos")]
pub fn run_macos_capture(
    mut writer: SessionWriter,
    session_id: String,
    started_at_ms: u64,
    candidate: CandidateProcess,
    stop_requested: Receiver<()>,
    recovery_requested: Receiver<RecoveryCommand>,
    sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    if candidate.backend != "macos" && candidate.backend != MACOS_BACKEND_ID {
        return finish_incomplete_capture(
            writer,
            &session_id,
            started_at_ms,
            &candidate,
            &sink,
            format!("macOS 后端不能接管候选：{}", candidate.backend),
        );
    }
    let session_root = writer.root_path().to_path_buf();
    let mut recovery = RecoveryBridge::new(
        session_id.clone(),
        started_at_ms,
        candidate.clone(),
        recovery_requested,
    );
    let mut raw_sequence = 0_u64;
    let mut active = match start_collector(
        &session_root,
        recovery.current_candidate(),
        recovery.current_segment_id(),
    ) {
        Ok(collector) => Some(collector),
        Err(reason) => {
            return finish_incomplete_capture(
                writer,
                &session_id,
                started_at_ms,
                recovery.current_candidate(),
                &sink,
                reason,
            )
        }
    };
    let mut stopping = false;
    let mut last_process_probe = Instant::now();
    let mut last_candidate_probe = Instant::now();

    loop {
        let action = recovery.process_commands(&mut writer, &sink, || {
            crate::diagnostics::rediscover_macos_candidates()
        })?;
        if action != RecoveryAction::None {
            if let Some(collector) = active.as_mut() {
                collector.request_stop()?;
            }
        }

        if !stopping {
            match stop_requested.try_recv() {
                Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    stopping = true;
                    if let Some(collector) = active.as_mut() {
                        collector.request_stop()?;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        if let Some(collector) = active.as_mut() {
            if collector.segment_id == recovery.current_segment_id()
                && !collector.ready_seen
                && collector.ready_path().is_file()
            {
                let (http, websocket) = read_python_coverage(&collector.spec.output_dir)?;
                if !http || !websocket {
                    return abort_active(
                        active,
                        writer,
                        &session_id,
                        started_at_ms,
                        recovery.current_candidate(),
                        &sink,
                        format!(
                            "macOS Frida ready 未覆盖双明文边界：HTTP={http}、WebSocket={websocket}"
                        ),
                    );
                }
                recovery.mark_ready(
                    &mut writer,
                    &sink,
                    MACOS_BACKEND_ID,
                    json!({
                        "module": collector.spec.module,
                        "source": "capture_miniapp_protocol.py",
                        "coverage": {"http": http, "websocket": websocket}
                    }),
                )?;
                collector.ready_seen = true;
            }

            for entry in collector.timeline.read(&collector.timeline_path())? {
                let kind = entry
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                append_python_entry(
                    &mut writer,
                    &collector.spec.output_dir,
                    &session_id,
                    &collector.segment_id,
                    &mut raw_sequence,
                    &mut collector.observation_ids,
                    &entry,
                    &sink,
                )?;
                if kind == "raw_observation" {
                    recovery.record_observation();
                } else if kind == "protocol_frame" {
                    recovery.record_frame();
                }
            }
        }

        if !stopping
            && active
                .as_ref()
                .is_some_and(|collector| collector.ready_seen)
            && last_process_probe.elapsed() >= Duration::from_millis(500)
        {
            last_process_probe = Instant::now();
            let pid = active
                .as_ref()
                .map(|collector| collector.pid)
                .unwrap_or_default();
            if !process_is_alive(pid) {
                let action = recovery.target_process_exited(
                    &mut writer,
                    &sink,
                    "承载小游戏的目标进程已退出",
                    crate::diagnostics::rediscover_macos_candidates(),
                )?;
                if action != RecoveryAction::None {
                    if let Some(collector) = active.as_mut() {
                        collector.request_stop()?;
                    }
                }
            }
        }

        let child_status = if let Some(collector) = active.as_mut() {
            collector
                .child
                .try_wait()
                .map_err(|error| format!("读取 macOS Frida 采集器状态失败：{error}"))?
        } else {
            None
        };
        if let Some(status) = child_status {
            let mut collector = active.take().expect("已退出的采集器必须存在");
            // 采集器退出后再读一次，确保最后一条边界已经归档。
            for entry in collector.timeline.read(&collector.timeline_path())? {
                let kind = entry
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                append_python_entry(
                    &mut writer,
                    &collector.spec.output_dir,
                    &session_id,
                    &collector.segment_id,
                    &mut raw_sequence,
                    &mut collector.observation_ids,
                    &entry,
                    &sink,
                )?;
                if kind == "raw_observation" {
                    recovery.record_observation();
                } else if kind == "protocol_frame" {
                    recovery.record_frame();
                }
            }
            if stopping {
                let previous_state = recovery.state();
                recovery.machine_mut().stop(now_ms());
                writer.update_lifecycle(
                    recovery.current_candidate(),
                    &recovery.machine().lifecycle(),
                )?;
                if collector.stop_seen && status.success() && collector.ready_seen {
                    return if matches!(
                        previous_state,
                        crate::model::ReattachState::Attached
                            | crate::model::ReattachState::Attaching
                    ) {
                        finish_capture(
                            writer,
                            &session_id,
                            started_at_ms,
                            recovery.current_candidate(),
                            &sink,
                        )
                    } else {
                        finish_incomplete_summary(
                            writer,
                            &session_id,
                            started_at_ms,
                            recovery.current_candidate(),
                            &sink,
                            "会话在等待重新附加时停止".to_string(),
                        )
                    };
                }
                return finish_incomplete_capture(
                    writer,
                    &session_id,
                    started_at_ms,
                    recovery.current_candidate(),
                    &sink,
                    "macOS Frida 采集器未能正常停止".to_string(),
                );
            }

            if collector.stop_seen && collector.segment_id != recovery.current_segment_id() {
                // 恢复命令可能先于旧分段的 ready 文件被 runner 消费。旧采集器
                // 此时只负责正常退出，绝不能用旧 ready 提前确认新分段。
            } else if recovery.state() == crate::model::ReattachState::Reattaching {
                // 旧分段是按恢复状态机要求停止的。下方会为新分段启动独立
                // staging，旧时间线和 observationKey 不会被重复读取。
            } else if collector.ready_seen && !process_is_alive(collector.pid) {
                recovery.target_process_exited(
                    &mut writer,
                    &sink,
                    "承载小游戏的目标进程已退出",
                    crate::diagnostics::rediscover_macos_candidates(),
                )?;
            } else {
                let reason = read_python_failure(&collector.spec.output_dir).unwrap_or_else(|| {
                    format!(
                        "macOS Frida 采集器异常结束（退出码 {:?}，ready={}）",
                        status.code(),
                        collector.ready_seen
                    )
                });
                if recovery.state() == crate::model::ReattachState::Attaching {
                    return finish_incomplete_capture(
                        writer,
                        &session_id,
                        started_at_ms,
                        recovery.current_candidate(),
                        &sink,
                        format!("macOS Frida 采集器附加失败：{reason}"),
                    );
                }
                let transition = recovery.machine_mut().attach_failed(reason);
                recovery
                    .machine()
                    .append_transition(&mut writer, &transition, now_ms(), &sink)?;
            }
        }

        if active.as_ref().is_some_and(|collector| {
            collector
                .stop_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        }) {
            return abort_active(
                active,
                writer,
                &session_id,
                started_at_ms,
                recovery.current_candidate(),
                &sink,
                "macOS Frida 采集器未在正常停止期限内退出".to_string(),
            );
        }

        if active.is_none() && !stopping {
            if recovery.state() == crate::model::ReattachState::Reattaching {
                match start_collector(
                    &session_root,
                    recovery.current_candidate(),
                    recovery.current_segment_id(),
                ) {
                    Ok(collector) => active = Some(collector),
                    Err(reason) => {
                        let transition = recovery.machine_mut().attach_failed(reason);
                        recovery.machine().append_transition(
                            &mut writer,
                            &transition,
                            now_ms(),
                            &sink,
                        )?;
                    }
                }
                last_process_probe = Instant::now();
            } else if matches!(
                recovery.state(),
                crate::model::ReattachState::Interrupted
                    | crate::model::ReattachState::AttachFailed
            ) && last_candidate_probe.elapsed() >= Duration::from_secs(1)
            {
                last_candidate_probe = Instant::now();
                recovery.discover_candidates(
                    &mut writer,
                    &sink,
                    crate::diagnostics::rediscover_macos_candidates(),
                )?;
            }
        }

        if stopping && active.is_none() {
            let previous_state = recovery.state();
            recovery.machine_mut().stop(now_ms());
            writer.update_lifecycle(
                recovery.current_candidate(),
                &recovery.machine().lifecycle(),
            )?;
            return if previous_state == crate::model::ReattachState::Attached {
                finish_capture(
                    writer,
                    &session_id,
                    started_at_ms,
                    recovery.current_candidate(),
                    &sink,
                )
            } else {
                finish_incomplete_summary(
                    writer,
                    &session_id,
                    started_at_ms,
                    recovery.current_candidate(),
                    &sink,
                    "会话在等待重新附加时停止".to_string(),
                )
            };
        }

        recovery.probe_disk_if_due(&mut writer, &sink)?;
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn run_macos_capture(
    _writer: SessionWriter,
    _session_id: String,
    _started_at_ms: u64,
    _candidate: CandidateProcess,
    _stop_requested: Receiver<()>,
    _recovery_requested: Receiver<RecoveryCommand>,
    _sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    Err("macOS Frida 后端仅在 macOS 上运行；当前环境未验证平台采集能力".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullEventSink;
    use crate::model::{CandidateProcess, CaptureEvent, SessionStatus};
    use crate::storage::SessionWriter;
    use std::fs;
    #[cfg(target_os = "macos")]
    use std::sync::{mpsc, Mutex};
    #[cfg(target_os = "macos")]
    use std::thread;
    #[cfg(target_os = "macos")]
    use std::time::Duration;
    use tempfile::tempdir;

    #[cfg(target_os = "macos")]
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn candidate() -> CandidateProcess {
        CandidateProcess {
            id: "macos-test-candidate".to_string(),
            pid: 1234,
            display_name: "WeApp · 测试".to_string(),
            process_role: "WeApp".to_string(),
            game: "世界 Online".to_string(),
            wechat_version: "4.1.13".to_string(),
            architecture: "arm64".to_string(),
            modules: vec![
                "WeChatAppEx Framework".to_string(),
                "XWeb.framework".to_string(),
            ],
            backend: "macos".to_string(),
        }
    }

    #[test]
    fn launch_spec_uses_macos_mode_and_both_boundaries() {
        let spec = MacosLaunchSpec {
            python: PathBuf::from("python3"),
            script: PathBuf::from("capture_miniapp_protocol.py"),
            output_dir: PathBuf::from("session/macos-frida"),
            stop_file: PathBuf::from("session/macos-frida/stop.requested"),
            pid: 1234,
            module: MACOS_MODULE_NAME.to_string(),
            http_write_symbol: "HttpWrite".to_string(),
            http_read_symbol: "HttpRead".to_string(),
            websocket_write_symbol: "WebSocketWrite".to_string(),
            websocket_read_symbol: "WebSocketRead".to_string(),
            max_bytes: 1024,
        };
        let args = spec.command_args();
        assert!(args.windows(2).any(|pair| pair == ["--transport", "macos"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--macos-http-write-symbol", "HttpWrite"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--macos-websocket-read-symbol", "WebSocketRead"]));
        assert!(!args.iter().any(|value| value == "flue"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn real_runner_restarts_collector_in_a_new_segment() {
        let _environment = ENV_LOCK.lock().expect("锁定测试环境变量");
        let workspace = tempdir().expect("创建测试工作区");
        let fixture = workspace.path().join("collector_fixture.py");
        fs::write(
            &fixture,
            r#"
import argparse, hashlib, json, time
from pathlib import Path

parser = argparse.ArgumentParser(add_help=False)
parser.add_argument('--output-dir', required=True)
parser.add_argument('--stop-file', required=True)
parser.add_argument('--pid', required=True)
args, _ = parser.parse_known_args()
root = Path(args.output_dir)
root.mkdir(parents=True, exist_ok=True)
(root / 'observations').mkdir(exist_ok=True)
payload = f'segment:{root.name}'.encode()
raw = root / 'observations' / 'payload.bin'
raw.write_bytes(payload)
(root / 'ready.json').write_text(json.dumps({
    'coverage': {'http': True, 'websocket': True, 'request': True, 'response': True}
}), encoding='utf-8')
entry = {
    'kind': 'raw_observation', 'direction': 'request', 'connection': 'fixture',
    'transport': 'http', 'observedAtMs': int(time.time() * 1000),
    'observationKey': root.name, 'classification': 'unparsed_payload',
    'length': len(payload), 'sha256': hashlib.sha256(payload).hexdigest(),
    'rawFile': 'observations/payload.bin'
}
(root / 'timeline.jsonl').write_text(json.dumps(entry) + '\n', encoding='utf-8')
deadline = time.monotonic() + 5
while not Path(args.stop_file).is_file() and time.monotonic() < deadline:
    time.sleep(.02)
(root / 'summary.json').write_text(json.dumps({'fatalError': None}), encoding='utf-8')
"#,
        )
        .expect("写入采集器 fixture");
        std::env::set_var("MINITRACE_CAPTURE_SCRIPT", &fixture);
        std::env::set_var("MINITRACE_PYTHON", "python3");

        let mut first = candidate();
        first.pid = std::process::id();
        first.id = "macos-fixture-first".to_string();
        let writer = SessionWriter::create(workspace.path(), "session-real-recovery", 1, &first)
            .expect("创建会话 writer");
        let (stop_sender, stop_receiver) = mpsc::channel();
        let (recovery_sender, recovery_receiver) = mpsc::channel();
        let sink: Arc<dyn EventSink> = Arc::new(NullEventSink);
        let first_for_runner = first.clone();
        let runner = thread::spawn(move || {
            run_macos_capture(
                writer,
                "session-real-recovery".to_string(),
                1,
                first_for_runner,
                stop_receiver,
                recovery_receiver,
                sink,
            )
        });

        let first_ready = workspace
            .path()
            .join("sessions/session-real-recovery/macos-frida/session-real-recovery-segment-001/ready.json");
        assert!(wait_for_file(&first_ready), "第一个分段未 ready");
        let mut second = first.clone();
        second.id = "macos-fixture-second".to_string();
        recovery_sender
            .send(RecoveryCommand::TargetExited {
                reason: "fixture 目标重启".to_string(),
                candidates: vec![second],
            })
            .expect("发送目标退出");
        let second_ready = workspace
            .path()
            .join("sessions/session-real-recovery/macos-frida/session-real-recovery-segment-002/ready.json");
        if !wait_for_file(&second_ready) {
            let _ = stop_sender.send(());
            let result = runner.join().expect("runner 线程结束");
            let timeline = fs::read_to_string(
                workspace
                    .path()
                    .join("sessions/session-real-recovery/timeline.jsonl"),
            )
            .unwrap_or_default();
            panic!("第二个分段未 ready；runner={result:?}；timeline={timeline}");
        }
        stop_sender.send(()).expect("请求停止");
        let summary = runner
            .join()
            .expect("runner 线程结束")
            .expect("真实 runner 正常收尾");

        std::env::remove_var("MINITRACE_CAPTURE_SCRIPT");
        std::env::remove_var("MINITRACE_PYTHON");
        assert_eq!(summary.status, SessionStatus::Completed);
        assert_eq!(summary.segments.len(), 2);
        assert_eq!(summary.interruptions.len(), 1);
        assert_eq!(summary.observation_count, 2);
        assert_eq!(
            summary.segments[0].status,
            crate::model::SegmentStatus::Interrupted
        );
        assert_eq!(
            summary.segments[1].status,
            crate::model::SegmentStatus::Completed
        );
    }

    #[cfg(target_os = "macos")]
    fn wait_for_file(path: &Path) -> bool {
        for _ in 0..260 {
            if path.is_file() {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        false
    }

    #[test]
    fn timeline_preserves_http_and_websocket_observations_and_frames() {
        let workspace = tempdir().expect("创建测试工作区");
        let staging = tempdir().expect("创建 Python staging");
        let http_bytes = b"HTTP/2 200\r\ncontent-type: application/json\r\n\r\n{}";
        let websocket_frame = [0x00, 0x06, 0x27, 0x12, b'o', b'k'];
        fs::create_dir_all(staging.path().join("observations")).expect("创建 observations");
        fs::create_dir_all(staging.path().join("frames")).expect("创建 frames");
        fs::write(staging.path().join("observations/http.bin"), http_bytes).expect("写 HTTP");
        fs::write(staging.path().join("observations/ws.bin"), b"\x82\x06").expect("写 WS 边界");
        fs::write(staging.path().join("frames/ws-frame.bin"), websocket_frame).expect("写帧");

        let http_sha = sha256_hex(http_bytes);
        let ws_sha = sha256_hex(b"\x82\x06");
        let frame_sha = sha256_hex(&websocket_frame);
        let candidate = candidate();
        let mut writer =
            SessionWriter::create(workspace.path(), "session-macos-test", 1, &candidate)
                .expect("创建会话 writer");
        let sink: Arc<dyn EventSink> = Arc::new(NullEventSink);
        let mut raw_sequence = 0;
        let mut observation_ids = HashMap::new();
        append_python_entry(
            &mut writer,
            staging.path(),
            "session-macos-test",
            "session-macos-test-segment-001",
            &mut raw_sequence,
            &mut observation_ids,
            &json!({
                "kind": "raw_observation", "direction": "response", "connection": "http-1",
                "transport": "http", "observedAtMs": 2, "observationKey": "http-key",
                "length": http_bytes.len(), "sha256": http_sha, "rawFile": "observations/http.bin"
            }),
            &sink,
        )
        .expect("归档 HTTP 观察");
        append_python_entry(
            &mut writer,
            staging.path(),
            "session-macos-test",
            "session-macos-test-segment-001",
            &mut raw_sequence,
            &mut observation_ids,
            &json!({
                "kind": "raw_observation", "direction": "request", "connection": "ws-1",
                "transport": "websocket", "observedAtMs": 3, "observationKey": "ws-key",
                "length": 2, "sha256": ws_sha, "rawFile": "observations/ws.bin"
            }),
            &sink,
        )
        .expect("归档 WebSocket 观察");
        append_python_entry(
            &mut writer,
            staging.path(),
            "session-macos-test",
            "session-macos-test-segment-001",
            &mut raw_sequence,
            &mut observation_ids,
            &json!({
                "kind": "protocol_frame", "direction": "request", "connection": "ws-1",
                "transport": "websocket", "observedAtMs": 3, "observationKey": "ws-key",
                "messageType": 10002, "protocolName": "CG_LOGIN", "actualLength": 6,
                "sha256": frame_sha, "rawFile": "frames/ws-frame.bin"
            }),
            &sink,
        )
        .expect("归档协议帧");
        let summary = writer
            .finish("session-macos-test", 1, 4, &candidate)
            .expect("发布会话");
        assert_eq!(summary.status, SessionStatus::Completed);
        assert_eq!(summary.observation_count, 2);
        assert_eq!(summary.frame_count, 1);
        assert_eq!(observation_ids.len(), 2);
    }

    #[test]
    fn evidence_path_rejects_escape() {
        let staging = tempdir().expect("创建 staging");
        let entry = json!({"rawFile": "../outside.bin"});
        let error = evidence_bytes(staging.path(), &entry).expect_err("应拒绝越界路径");
        assert!(error.contains("路径越界"));
    }

    #[test]
    fn capture_event_sink_type_remains_object_safe() {
        let _event: Option<CaptureEvent> = None;
    }
}
