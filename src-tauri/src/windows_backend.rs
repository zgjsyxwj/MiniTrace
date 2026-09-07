//! Windows `flue.dll` 采集后端适配。
//!
//! Windows 端不重新实现 Frida hook 或游戏帧拆分。真实采集时由本模块启动
//! 项目已有的 `capture_miniapp_protocol.py --transport flue`，再把它产生的
//! ready、帧和未分类明文证据规范化到 MiniTrace 的统一事件与会话模型。
//!
//! 本机没有 Windows 微信实例，因此这里的“接入”只表示命令/事件边界已经
//! 接好；候选诊断和事件归一化均可用 fixture 测试，不能把它们当成真实平台
//! 采集能力。

use crate::backend::EventSink;
use crate::model::{CandidateProcess, SessionSummary};
#[cfg(any(windows, test))]
use crate::model::{
    CaptureEvent, CaptureEventKind, ProtocolFrame, RawObservation, TrafficDirection, TransportKind,
};
use crate::recovery::RecoveryCommand;
use crate::storage::SessionWriter;
#[cfg(any(windows, test))]
use crate::storage::{now_ms, sha256_hex};
use serde::{Deserialize, Serialize};
#[cfg(any(windows, test))]
use serde_json::{json, Value};
use std::env;
#[cfg(any(windows, test))]
use std::fs::{self, File};
#[cfg(any(windows, test))]
use std::io::{Read, Seek, SeekFrom};
#[cfg(any(windows, test))]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::{mpsc::Receiver, Arc};
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::Duration;
#[cfg(windows)]
use std::time::Instant;

pub const WINDOWS_BACKEND_ID: &str = "windows-flue";
pub const WINDOWS_PLATFORM_STATUS: &str = "平台接入、未验证";
pub const FLUE_MODULE_NAME: &str = "flue.dll";
pub const CAPTURE_SCRIPT_NAME: &str = "capture_miniapp_protocol.py";
pub const DEFAULT_FLUE_WRITE_RVA: u64 = 0x573550;
pub const DEFAULT_FLUE_READ_RVA: u64 = 0x4C06990;
const STAGING_DIRECTORY: &str = "windows-flue";
const STOP_FILE_NAME: &str = "stop.requested";
#[cfg(windows)]
const READY_FILE_NAME: &str = "ready.json";
#[cfg(windows)]
const SUMMARY_FILE_NAME: &str = "summary.json";
#[cfg(windows)]
const TIMELINE_FILE_NAME: &str = "timeline.jsonl";
#[cfg(windows)]
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(15);

/// 从 Windows 进程清单中提取的最小诊断输入。
///
/// 生产实现使用 `tasklist /m flue.dll` 填充它；测试直接构造 fixture，因而
/// 不需要 Windows、微信或 Frida。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowsProcessSnapshot {
    pub pid: u32,
    pub image_name: String,
    pub modules: Vec<String>,
    pub wechat_version: Option<String>,
    pub architecture: Option<String>,
}

impl WindowsProcessSnapshot {
    pub fn new(
        pid: u32,
        image_name: impl Into<String>,
        modules: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            pid,
            image_name: image_name.into(),
            modules: modules.into_iter().map(Into::into).collect(),
            wechat_version: None,
            architecture: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsDiagnosticStatus {
    PlatformIntegratedUnverified,
    NoCandidate,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowsDiagnostic {
    pub platform: String,
    pub architecture: String,
    pub backend: String,
    pub status: WindowsDiagnosticStatus,
    pub status_label: String,
    pub candidates: Vec<CandidateProcess>,
    pub message: String,
}

/// 只接受可能承载小游戏流量、且已加载 `flue.dll` 的 Windows 子进程。
///
/// `WeChat.exe` 主进程即使存在也不会成为候选；候选必须来自 `WeChatAppEx`
/// 或 `WeApp` 角色并带有 flue 模块。微信版本未知时仍只能是诊断候选，正式
/// 真实流量验收由后续工作完成。
pub fn candidate_from_process(process: &WindowsProcessSnapshot) -> Option<CandidateProcess> {
    if process.pid == 0 || !is_supported_image(&process.image_name) {
        return None;
    }
    if !has_module(&process.modules, FLUE_MODULE_NAME) {
        return None;
    }

    let process_role = process_role(&process.image_name);
    let mut modules = process.modules.clone();
    if !has_module(&modules, FLUE_MODULE_NAME) {
        modules.push(FLUE_MODULE_NAME.to_string());
    }
    modules.sort_by_key(|module| module.to_ascii_lowercase());
    modules.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    Some(CandidateProcess {
        id: format!("{WINDOWS_BACKEND_ID}-pid-{}", process.pid),
        pid: process.pid,
        display_name: format!("{process_role} · 世界 Online（Windows flue.dll）"),
        process_role: process_role.to_string(),
        game: "世界 Online".to_string(),
        wechat_version: process
            .wechat_version
            .clone()
            .filter(|version| !version.trim().is_empty())
            .unwrap_or_else(|| "未知（待诊断）".to_string()),
        architecture: process
            .architecture
            .clone()
            .filter(|architecture| !architecture.trim().is_empty())
            .unwrap_or_else(detect_windows_architecture),
        modules,
        backend: WINDOWS_BACKEND_ID.to_string(),
    })
}

pub fn diagnose_processes(processes: &[WindowsProcessSnapshot]) -> WindowsDiagnostic {
    let candidates = processes
        .iter()
        .filter_map(candidate_from_process)
        .collect::<Vec<_>>();
    let architecture = candidates
        .first()
        .map(|candidate| candidate.architecture.clone())
        .unwrap_or_else(detect_windows_architecture);
    let status = if candidates.is_empty() {
        WindowsDiagnosticStatus::NoCandidate
    } else {
        WindowsDiagnosticStatus::PlatformIntegratedUnverified
    };
    let message = if candidates.is_empty() {
        "未发现同时满足进程角色和 flue.dll 模块条件的候选；当前仅提供兼容性诊断。".to_string()
    } else {
        "已接入 Windows flue.dll 后端，但尚未通过当前微信版本的真实双向流量验收。".to_string()
    };
    WindowsDiagnostic {
        platform: "windows".to_string(),
        architecture,
        backend: WINDOWS_BACKEND_ID.to_string(),
        status,
        status_label: WINDOWS_PLATFORM_STATUS.to_string(),
        candidates,
        message,
    }
}

fn is_supported_image(image_name: &str) -> bool {
    matches!(
        image_name.to_ascii_lowercase().as_str(),
        "wechatappex.exe" | "weapp.exe"
    )
}

fn process_role(image_name: &str) -> &'static str {
    if image_name.eq_ignore_ascii_case("weapp.exe") {
        "WeApp"
    } else {
        "WeChatAppEx"
    }
}

fn has_module(modules: &[String], expected: &str) -> bool {
    modules
        .iter()
        .any(|module| module.eq_ignore_ascii_case(expected))
}

fn detect_windows_architecture() -> String {
    env::var("PROCESSOR_ARCHITECTURE")
        .ok()
        .map(
            |architecture| match architecture.to_ascii_uppercase().as_str() {
                "AMD64" => "x64".to_string(),
                "ARM64" => "arm64".to_string(),
                "X86" => "x86".to_string(),
                _ => architecture,
            },
        )
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(windows)]
pub fn list_candidates() -> Result<Vec<CandidateProcess>, String> {
    let processes = discover_processes()?;
    Ok(diagnose_processes(&processes).candidates)
}

#[cfg(not(windows))]
pub fn list_candidates() -> Result<Vec<CandidateProcess>, String> {
    // 非 Windows 开发机不伪造 Windows 进程；issue 01 的 fake candidate 由
    // commands 在当前平台显式选择，避免把“平台接入”冒充成 Windows 能力。
    Ok(Vec::new())
}

#[cfg(windows)]
fn discover_processes() -> Result<Vec<WindowsProcessSnapshot>, String> {
    let output = std::process::Command::new("tasklist")
        .args(["/m", FLUE_MODULE_NAME, "/fo", "csv", "/nh"])
        .output()
        .map_err(|error| format!("运行 tasklist 诊断 Windows 进程失败：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "tasklist 诊断 Windows 进程失败，退出码 {:?}",
            output.status.code()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();
    for line in stdout.lines() {
        let fields = parse_csv_line(line);
        if fields.len() < 2 {
            continue;
        }
        let Ok(pid) = fields[1].trim().parse::<u32>() else {
            continue;
        };
        if pid == 0 {
            continue;
        }
        let modules = fields
            .get(2)
            .map(|value| {
                value
                    .split_whitespace()
                    .filter(|module| !module.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        processes.push(WindowsProcessSnapshot {
            pid,
            image_name: fields[0].clone(),
            modules,
            wechat_version: None,
            architecture: Some(detect_windows_architecture()),
        });
    }
    Ok(processes)
}

#[cfg(windows)]
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                field.push('"');
                characters.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(character),
        }
    }
    if !field.is_empty() || line.ends_with(',') {
        fields.push(field.trim().to_string());
    }
    fields
}

/// Python 采集器的启动配置。生产 Windows 运行时允许通过环境变量替换
/// Python 和脚本路径，正式自包含打包留给后续 Ticket。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlueLaunchSpec {
    pub python: PathBuf,
    pub script: PathBuf,
    pub output_dir: PathBuf,
    pub stop_file: PathBuf,
    pub pid: u32,
    pub flue_module: String,
    pub flue_write_rva: u64,
    pub flue_read_rva: u64,
    pub max_bytes: u64,
}

impl FlueLaunchSpec {
    pub fn for_session(session_root: &Path, pid: u32) -> Result<Self, String> {
        let output_dir = session_root.join(STAGING_DIRECTORY);
        let stop_file = output_dir.join(STOP_FILE_NAME);
        let python = env::var_os("MINITRACE_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python"));
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
            flue_module: FLUE_MODULE_NAME.to_string(),
            flue_write_rva: DEFAULT_FLUE_WRITE_RVA,
            flue_read_rva: DEFAULT_FLUE_READ_RVA,
            max_bytes: 16 * 1024 * 1024,
        })
    }

    /// 暴露给契约测试的参数序列，确保 Windows 端不会意外退回 WinHTTP。
    pub fn command_args(&self) -> Vec<String> {
        vec![
            self.script.to_string_lossy().into_owned(),
            "--transport".to_string(),
            "flue".to_string(),
            "--pid".to_string(),
            self.pid.to_string(),
            "--output-dir".to_string(),
            self.output_dir.to_string_lossy().into_owned(),
            "--allow-external-output".to_string(),
            "--stop-file".to_string(),
            self.stop_file.to_string_lossy().into_owned(),
            "--flue-module".to_string(),
            self.flue_module.clone(),
            "--flue-write-rva".to_string(),
            format!("0x{:x}", self.flue_write_rva),
            "--flue-read-rva".to_string(),
            format!("0x{:x}", self.flue_read_rva),
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

#[cfg(any(windows, test))]
#[derive(Default)]
struct JsonlTail {
    offset: u64,
    pending: String,
}

#[cfg(any(windows, test))]
impl JsonlTail {
    fn read(&mut self, path: &Path) -> Result<Vec<Value>, String> {
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let mut file =
            File::open(path).map_err(|error| format!("打开 Python 时间线失败：{error}"))?;
        let file_length = file
            .metadata()
            .map_err(|error| format!("读取 Python 时间线大小失败：{error}"))?
            .len();
        if file_length < self.offset {
            self.offset = 0;
            self.pending.clear();
        }
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|error| format!("定位 Python 时间线失败：{error}"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("读取 Python 时间线失败：{error}"))?;
        self.offset += bytes.len() as u64;
        self.pending.push_str(
            std::str::from_utf8(&bytes)
                .map_err(|error| format!("Python 时间线不是 UTF-8：{error}"))?,
        );

        let mut values = Vec::new();
        let complete_length = self.pending.rfind('\n').map(|index| index + 1).unwrap_or(0);
        let complete = self.pending[..complete_length].to_string();
        self.pending = self.pending[complete_length..].to_string();
        for line in complete.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            values.push(
                serde_json::from_str::<Value>(line)
                    .map_err(|error| format!("解析 Python 时间线条目失败：{error}"))?,
            );
        }
        Ok(values)
    }
}

#[cfg(any(windows, test))]
fn value_string(value: &Value, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("Python 时间线缺少字符串字段：{field}"))
}

#[cfg(any(windows, test))]
fn value_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("Python timeline missing integer field: {field}"))
}

#[cfg(any(windows, test))]
fn parse_direction(value: &str) -> Result<TrafficDirection, String> {
    match value {
        "request" => Ok(TrafficDirection::Request),
        "response" => Ok(TrafficDirection::Response),
        _ => Err(format!("Python 时间线方向无效：{value}")),
    }
}

#[cfg(any(windows, test))]
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
        return Err(format!("Python 原始文件路径越界：{raw_file}"));
    }
    Ok(staging.join(relative))
}

#[cfg(any(windows, test))]
fn evidence_bytes(staging: &Path, entry: &Value) -> Result<(Vec<u8>, PathBuf), String> {
    let raw_file = value_string(entry, "rawFile")?;
    let path = safe_evidence_path(staging, &raw_file)?;
    let bytes = fs::read(&path).map_err(|error| format!("读取 Python 原始证据失败：{error}"))?;
    if let Some(length) = entry.get("actualLength").and_then(Value::as_u64) {
        if length != bytes.len() as u64 {
            return Err(format!(
                "Python 原始证据长度不一致：声明 {length}，实际 {}",
                bytes.len()
            ));
        }
    }
    if let Some(length) = entry.get("unparsedLength").and_then(Value::as_u64) {
        if length != bytes.len() as u64 {
            return Err(format!(
                "Python 未分类明文长度不一致：声明 {length}，实际 {}",
                bytes.len()
            ));
        }
    }
    if let Some(expected) = entry.get("sha256").and_then(Value::as_str) {
        let actual = sha256_hex(&bytes);
        if expected != actual {
            return Err(format!(
                "Python 原始证据 SHA-256 不一致：声明 {expected}，实际 {actual}"
            ));
        }
    }
    Ok((bytes, path))
}

/// 将一个已有 Python `timeline.jsonl` 帧或未分类明文条目转换为统一事件。
///
/// 这是 Windows 后端的核心复用边界：Python 继续负责 flue hook、WebSocket
/// 重组、`split_game_frames` 和协议号解析；Rust 只负责会话归档及公开契约。
#[cfg(any(windows, test))]
fn append_python_entry(
    writer: &mut SessionWriter,
    staging: &Path,
    session_id: &str,
    segment_id: &str,
    raw_sequence: &mut u64,
    entry: &Value,
    sink: &Arc<dyn EventSink>,
) -> Result<(), String> {
    let (bytes, _) = evidence_bytes(staging, entry)?;
    let direction = parse_direction(&value_string(entry, "direction")?)?;
    let connection_id = value_string(entry, "connection")?;
    let observed_at_ms = now_ms();
    *raw_sequence += 1;
    let observation_id = format!("{session_id}-observation-{raw_sequence:06}");
    let raw_file = writer.write_raw(*raw_sequence, &bytes)?;
    let sha256 = sha256_hex(&bytes);
    let observation = RawObservation {
        observation_id: observation_id.clone(),
        segment_id: segment_id.to_string(),
        connection_id: connection_id.clone(),
        direction: direction.clone(),
        transport: TransportKind::WebSocket,
        observed_at_ms,
        length: bytes.len(),
        sha256: sha256.clone(),
        raw_file: raw_file.clone(),
    };
    let unparsed = entry.get("unparsedLength").is_some();
    let mut observation_data = serde_json::to_value(&observation)
        .map_err(|error| format!("序列化 Windows 原始观察失败：{error}"))?;
    if unparsed {
        observation_data["classification"] = json!("unparsed_payload");
    }
    writer.append_event(observed_at_ms, "raw_observation", observation_data.clone())?;
    writer.append_transport_event(
        observed_at_ms,
        json!({
            "backend": WINDOWS_BACKEND_ID,
            "source": "capture_miniapp_protocol.py",
            "observationId": observation_id,
            "segmentId": segment_id,
            "connectionId": connection_id,
            "direction": direction,
            "transport": "websocket",
            "length": bytes.len(),
            "sha256": sha256,
            "sourceRawFile": entry.get("rawFile")
        }),
    )?;
    writer.add_observation(observation.clone());
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::RawObservation,
            session_id: session_id.to_string(),
            timestamp_ms: observed_at_ms,
            data: observation_data,
        },
    );

    if unparsed {
        return Ok(());
    }

    let message_id = value_u64(entry, "messageType")?;
    let message_id = u32::try_from(message_id)
        .map_err(|_| format!("Python 协议号超出 u32 范围：{message_id}"))?;
    let protocol_name = entry
        .get("protocolName")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("UNKNOWN_{message_id}"));
    let frame = ProtocolFrame {
        frame_id: format!("{session_id}-frame-{raw_sequence:06}"),
        observation_id,
        segment_id: segment_id.to_string(),
        connection_id,
        direction,
        transport: TransportKind::WebSocket,
        observed_at_ms,
        message_id,
        protocol_name,
        length: bytes.len(),
        sha256,
        raw_file,
    };
    let frame_data = serde_json::to_value(&frame)
        .map_err(|error| format!("序列化 Windows 协议帧失败：{error}"))?;
    writer.append_event(observed_at_ms, "protocol_frame", frame_data.clone())?;
    writer.add_frame(frame);
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::ProtocolFrame,
            session_id: session_id.to_string(),
            timestamp_ms: observed_at_ms,
            data: frame_data,
        },
    );
    Ok(())
}

#[cfg(windows)]
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

#[cfg(any(windows, test))]
fn emit(sink: &Arc<dyn EventSink>, event: CaptureEvent) {
    // 会话文件是权威证据；窗口关闭或事件订阅端断开不能阻止收尾。
    let _ = sink.emit(event);
}

#[cfg(windows)]
fn emit_ready(sink: &Arc<dyn EventSink>, session_id: &str, segment_id: &str) {
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Ready,
            session_id: session_id.to_string(),
            timestamp_ms: now_ms(),
            data: json!({
                "backend": WINDOWS_BACKEND_ID,
                "transport": "flue",
                "module": FLUE_MODULE_NAME,
                "segmentId": segment_id
            }),
        },
    );
}

#[cfg(windows)]
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
                .map_err(|error| format!("序列化 Windows 停止事件失败：{error}"))?,
        },
    );
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Summary,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化 Windows 摘要事件失败：{error}"))?,
        },
    );
    Ok(summary)
}

#[cfg(windows)]
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
                .map_err(|error| format!("序列化 Windows 不完整摘要失败：{error}"))?,
        },
    );
    Err(reason)
}

#[cfg(windows)]
pub fn run_windows_capture(
    mut writer: SessionWriter,
    session_id: String,
    started_at_ms: u64,
    candidate: CandidateProcess,
    stop_requested: Receiver<()>,
    _recovery_requested: Receiver<RecoveryCommand>,
    sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    if candidate.backend != WINDOWS_BACKEND_ID {
        return Err(format!("Windows 后端不能接管候选：{}", candidate.backend));
    }
    let session_root = writer.root_path().to_path_buf();
    let spec = FlueLaunchSpec::for_session(&session_root, candidate.pid)?;
    fs::create_dir_all(&spec.output_dir)
        .map_err(|error| format!("创建 Windows flue 临时目录失败：{error}"))?;
    if spec.stop_file.exists() {
        fs::remove_file(&spec.stop_file)
            .map_err(|error| format!("清理上次 Windows 停止标记失败：{error}"))?;
    }

    let mut command = std::process::Command::new(&spec.python);
    let args = spec.command_args();
    command.args(args.iter());
    command
        .current_dir(&session_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 Windows flue Python 采集器失败：{error}"))?;

    let segment_id = format!("{session_id}-segment-001");
    let ready_path = spec.output_dir.join(READY_FILE_NAME);
    let timeline_path = spec.output_dir.join(TIMELINE_FILE_NAME);
    let mut timeline = JsonlTail::default();
    let mut ready_seen = false;
    let mut stop_seen = false;
    let mut raw_sequence = 0_u64;
    let stop_deadline = Instant::now() + STOP_GRACE_PERIOD;

    loop {
        if !ready_seen && ready_path.is_file() {
            let ready_timestamp = now_ms();
            writer.append_event(
                ready_timestamp,
                "ready",
                json!({
                    "backend": WINDOWS_BACKEND_ID,
                    "module": FLUE_MODULE_NAME,
                    "segmentId": segment_id,
                    "source": "capture_miniapp_protocol.py"
                }),
            )?;
            emit_ready(&sink, &session_id, &segment_id);
            ready_seen = true;
        }

        for entry in timeline.read(&timeline_path)? {
            append_python_entry(
                &mut writer,
                &spec.output_dir,
                &session_id,
                &segment_id,
                &mut raw_sequence,
                &entry,
                &sink,
            )?;
        }

        if !stop_seen {
            match stop_requested.try_recv() {
                Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    fs::write(&spec.stop_file, b"stop\n")
                        .map_err(|error| format!("写入 Windows 停止标记失败：{error}"))?;
                    stop_seen = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("读取 Windows flue 采集器状态失败：{error}"))?
        {
            // 采集器退出后再读一次，确保最后一帧已被归档。
            for entry in timeline.read(&timeline_path)? {
                append_python_entry(
                    &mut writer,
                    &spec.output_dir,
                    &session_id,
                    &segment_id,
                    &mut raw_sequence,
                    &entry,
                    &sink,
                )?;
            }
            if let Some(reason) = read_python_failure(&spec.output_dir) {
                return finish_incomplete_capture(
                    writer,
                    &session_id,
                    started_at_ms,
                    &candidate,
                    &sink,
                    format!("Python flue 采集器失败：{reason}"),
                );
            }
            if stop_seen && status.success() {
                return finish_capture(writer, &session_id, started_at_ms, &candidate, &sink);
            }
            return finish_incomplete_capture(
                writer,
                &session_id,
                started_at_ms,
                &candidate,
                &sink,
                format!(
                    "Windows flue 采集器异常结束（退出码 {:?}，未收到正常停止：{}）",
                    status.code(),
                    !stop_seen
                ),
            );
        }

        if stop_seen && Instant::now() >= stop_deadline {
            let _ = child.kill();
            let _ = child.wait();
            return finish_incomplete_capture(
                writer,
                &session_id,
                started_at_ms,
                &candidate,
                &sink,
                "Windows flue 采集器未在正常停止期限内退出".to_string(),
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(not(windows))]
pub fn run_windows_capture(
    _writer: SessionWriter,
    _session_id: String,
    _started_at_ms: u64,
    _candidate: CandidateProcess,
    _stop_requested: Receiver<()>,
    _recovery_requested: Receiver<RecoveryCommand>,
    _sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    Err("Windows flue 后端仅在 Windows 上运行；当前环境未验证平台采集能力".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wechat_network_roles_with_flue_are_candidates() {
        let valid =
            WindowsProcessSnapshot::new(120, "WeChatAppEx.exe", ["kernel32.dll", "flue.dll"]);
        let valid_weapp = WindowsProcessSnapshot::new(121, "weapp.exe", ["FLUE.DLL"]);
        let wrong_process = WindowsProcessSnapshot::new(122, "WeChat.exe", ["flue.dll"]);
        let missing_module = WindowsProcessSnapshot::new(123, "WeChatAppEx.exe", ["kernel32.dll"]);

        let candidate = candidate_from_process(&valid).expect("WeChatAppEx + flue 应为候选");
        assert_eq!(candidate.pid, 120);
        assert_eq!(candidate.process_role, "WeChatAppEx");
        assert_eq!(candidate.backend, WINDOWS_BACKEND_ID);
        assert!(candidate.modules.iter().any(|module| module == "flue.dll"));
        assert_eq!(
            candidate_from_process(&valid_weapp)
                .expect("WeApp + flue 应为候选")
                .process_role,
            "WeApp"
        );
        assert!(candidate_from_process(&wrong_process).is_none());
        assert!(candidate_from_process(&missing_module).is_none());
    }

    #[test]
    fn diagnostic_never_calls_unverified_windows_platform_supported() {
        let process = WindowsProcessSnapshot::new(120, "WeChatAppEx.exe", ["flue.dll"]);
        let diagnostic = diagnose_processes(&[process]);
        assert_eq!(
            diagnostic.status,
            WindowsDiagnosticStatus::PlatformIntegratedUnverified
        );
        assert_eq!(diagnostic.status_label, WINDOWS_PLATFORM_STATUS);
        assert_eq!(diagnostic.candidates.len(), 1);
        assert!(diagnostic.message.contains("尚未通过"));
    }

    #[test]
    fn empty_diagnostic_is_compatibility_only() {
        let diagnostic = diagnose_processes(&[]);
        assert_eq!(diagnostic.status, WindowsDiagnosticStatus::NoCandidate);
        assert!(diagnostic.message.contains("兼容性诊断"));
        assert!(diagnostic.candidates.is_empty());
    }

    #[test]
    fn launch_spec_reuses_flue_python_path_and_rvas() {
        let spec = FlueLaunchSpec {
            python: PathBuf::from("python"),
            script: PathBuf::from("capture_miniapp_protocol.py"),
            output_dir: PathBuf::from("session/windows-flue"),
            stop_file: PathBuf::from("session/windows-flue/stop.requested"),
            pid: 120,
            flue_module: FLUE_MODULE_NAME.to_string(),
            flue_write_rva: DEFAULT_FLUE_WRITE_RVA,
            flue_read_rva: DEFAULT_FLUE_READ_RVA,
            max_bytes: 1024,
        };
        let args = spec.command_args();
        assert!(args.windows(2).any(|pair| pair == ["--transport", "flue"]));
        assert!(!args.iter().any(|arg| arg == "winhttp"));
        assert!(args.iter().any(|arg| arg == "--stop-file"));
        assert!(args.iter().any(|arg| arg == "0x573550"));
        assert!(args.iter().any(|arg| arg == "0x4c06990"));
    }

    #[test]
    fn python_timeline_tail_handles_partial_lines_without_duplicates() {
        let root = tempfile::tempdir().expect("创建测试目录");
        let path = root.path().join("timeline.jsonl");
        fs::write(&path, b"{\"direction\":\"request\"}\n{\"direction\":").expect("写入首段");
        let mut tail = JsonlTail::default();
        let first = tail.read(&path).expect("读取首段");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0]["direction"], "request");

        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("打开时间线");
        file.write_all(b"\"response\"}\n").expect("补齐时间线");
        let second = tail.read(&path).expect("读取补齐时间线");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0]["direction"], "response");
    }

    #[test]
    fn evidence_paths_cannot_escape_staging_directory() {
        let root = PathBuf::from("C:/capture/session/windows-flue");
        assert!(safe_evidence_path(&root, "frames/000001.bin").is_ok());
        assert!(safe_evidence_path(&root, "../outside.bin").is_err());
        assert!(safe_evidence_path(&root, "/outside.bin").is_err());
    }

    #[test]
    fn python_frame_and_unparsed_payloads_share_the_unified_event_boundary() {
        #[derive(Clone, Default)]
        struct RecordingSink(std::sync::Arc<std::sync::Mutex<Vec<CaptureEventKind>>>);

        impl EventSink for RecordingSink {
            fn emit(&self, event: CaptureEvent) -> Result<(), String> {
                self.0.lock().unwrap().push(event.kind);
                Ok(())
            }
        }

        let root = tempfile::tempdir().expect("创建会话目录");
        let workspace = root.path().join("workspace");
        let candidate = candidate_from_process(&WindowsProcessSnapshot::new(
            120,
            "WeChatAppEx.exe",
            [FLUE_MODULE_NAME],
        ))
        .expect("构造 Windows 候选");
        let session_id = "session-windows-fixture";
        let mut writer =
            SessionWriter::create(&workspace, session_id, 1, &candidate).expect("创建会话 writer");
        let staging = workspace
            .join("sessions")
            .join(session_id)
            .join(STAGING_DIRECTORY);
        fs::create_dir_all(staging.join("frames")).expect("创建 Python fixture 目录");
        fs::write(
            staging.join("frames/000001-response-16001.bin"),
            [0, 4, 62, 129],
        )
        .expect("写入 Python 帧");
        fs::create_dir_all(staging.join("unparsed-payloads")).expect("创建未分类明文 fixture 目录");
        fs::write(
            staging.join("unparsed-payloads/000001-request-unparsed.bin"),
            [0xde, 0xad, 0xbe, 0xef],
        )
        .expect("写入未分类明文");
        let sink = RecordingSink::default();
        let sink_events = sink.0.clone();
        let sink: Arc<dyn EventSink> = Arc::new(sink);
        let mut sequence = 0;
        append_python_entry(
            &mut writer,
            &staging,
            session_id,
            "session-windows-fixture-segment-001",
            &mut sequence,
            &json!({
                "direction": "response",
                "connection": "0xabc",
                "messageType": 16001,
                "protocolName": "CG_HEARTBEAT_RESPONSE",
                "actualLength": 4,
                "sha256": sha256_hex(&[0, 4, 62, 129]),
                "rawFile": "frames/000001-response-16001.bin"
            }),
            &sink,
        )
        .expect("归一化 Python 协议帧");
        append_python_entry(
            &mut writer,
            &staging,
            session_id,
            "session-windows-fixture-segment-001",
            &mut sequence,
            &json!({
                "direction": "request",
                "connection": "0xdef",
                "unparsedLength": 4,
                "sha256": sha256_hex(&[0xde, 0xad, 0xbe, 0xef]),
                "rawFile": "unparsed-payloads/000001-request-unparsed.bin"
            }),
            &sink,
        )
        .expect("归一化未分类明文");
        let summary = writer
            .finish(session_id, 1, 2, &candidate)
            .expect("完成 fixture 会话");
        assert_eq!(summary.observation_count, 2);
        assert_eq!(summary.frame_count, 1);
        assert_eq!(
            sink_events.lock().unwrap().as_slice(),
            &[
                CaptureEventKind::RawObservation,
                CaptureEventKind::ProtocolFrame,
                CaptureEventKind::RawObservation,
            ]
        );
    }
}
