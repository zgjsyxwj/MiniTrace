//! MiniTrace 的公开命令与事件模型。
//!
//! 这些类型同时被 Tauri 命令、事件载荷和磁盘上的 JSON 使用。字段名采用
//! camelCase，事件 kind 和状态值采用 snake_case，保证 Rust 与前端契约稳定。

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPTURE_EVENT_NAME: &str = "capture://event";

/// 当前已完成真实流量验收的 macOS 采集环境。
///
/// 这些值是采集能力的资格边界，不代表任意看起来相似的微信版本都可以
/// 复用旧的附加地址或采集配置。
pub const SUPPORTED_MACOS_WECHAT_VERSION: &str = "4.1.13";
pub const SUPPORTED_MACOS_ARCHITECTURE: &str = "arm64";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureEventKind {
    Started,
    Ready,
    RawObservation,
    ProtocolFrame,
    Stopped,
    Summary,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureEvent {
    pub kind: CaptureEventKind,
    pub session_id: String,
    pub timestamp_ms: u64,
    pub data: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticMode {
    Supported,
    CompatibilityDiagnostic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticIssueCode {
    UnsupportedPlatform,
    UnsupportedArchitecture,
    UnknownWechatVersion,
    MismatchedWechatVersion,
    MissingCandidate,
    MissingRequiredModule,
    MissingCaptureBoundary,
    CandidateRoleMismatch,
    MultipleCandidates,
    HardenedRuntimeBlocked,
    ProcessInspectionUnavailable,
    PlatformIntegrationUnverified,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticIssue {
    pub code: DiagnosticIssueCode,
    pub message: String,
    pub blocking: bool,
}

/// 当前主机和微信运行环境的只读诊断结果。
///
/// `candidates` 只包含已经通过所有资格检查的候选进程。被检查但未通过的
/// 进程不会被误当成可采集目标，具体原因保存在 `issues` 中。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentDiagnosis {
    pub platform: String,
    pub os_version: String,
    pub architecture: String,
    pub wechat_version: String,
    pub mode: DiagnosticMode,
    pub supported: bool,
    pub capture_allowed: bool,
    pub requires_candidate_confirmation: bool,
    pub candidates: Vec<CandidateProcess>,
    pub issues: Vec<DiagnosticIssue>,
    pub required_modules: Vec<String>,
    pub checked_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapturePhase {
    Idle,
    Starting,
    Ready,
    Capturing,
    Stopping,
    Completed,
    Error,
}

/// 承载进程重新附加状态。`Attached` 表示当前分段已成功附加；中断后
/// `AwaitingCandidate` 和 `Interrupted` 都表示采集暂停，不能自动混入未知
/// 进程。`AttachFailed` 可继续等待下一次诊断，不会丢弃已有证据。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReattachState {
    Attaching,
    Attached,
    Interrupted,
    AwaitingCandidate,
    Reattaching,
    AttachFailed,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentStatus {
    Starting,
    Attached,
    Interrupted,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
    Incomplete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrafficDirection {
    Request,
    Response,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    WebSocket,
    Http,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateProcess {
    pub id: String,
    pub pid: u32,
    pub display_name: String,
    pub process_role: String,
    pub game: String,
    pub wechat_version: String,
    pub architecture: String,
    pub modules: Vec<String>,
    pub backend: String,
}

/// 一次连续附加区间。进程 PID 变化只影响新的分段，不改变会话编号。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSegment {
    pub segment_id: String,
    pub candidate: CandidateProcess,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub status: SegmentStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InterruptionInfo {
    pub interruption_id: String,
    pub segment_id: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub reason: String,
    pub previous_pid: u32,
}

/// 磁盘容量快照。`warning` 只表示风险，不是自动停止条件。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceStatus {
    pub path: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub warning: bool,
    pub warning_ratio_percent: u64,
    pub warning_free_bytes: u64,
    pub checked_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionLifecycle {
    #[serde(default)]
    pub segments: Vec<CaptureSegment>,
    #[serde(default)]
    pub interruptions: Vec<InterruptionInfo>,
    #[serde(default)]
    pub disk_warnings: Vec<DiskSpaceStatus>,
    #[serde(default)]
    pub zero_frame: bool,
}

/// 前端用于展示重新附加和磁盘风险的有界快照。完整生命周期保存在会话
/// manifest/时间线；这里只用于当前应用状态和人工选择。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecoverySnapshot {
    pub state: Option<ReattachState>,
    pub session_id: Option<String>,
    pub current_segment_id: Option<String>,
    pub current_pid: Option<u32>,
    pub segments: Vec<CaptureSegment>,
    pub interruptions: Vec<InterruptionInfo>,
    pub candidates: Vec<CandidateProcess>,
    pub interruption_started_at_ms: Option<u64>,
    pub last_error: Option<String>,
    pub zero_frame: bool,
    pub disk: Option<DiskSpaceStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub path: String,
    pub sessions_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSessionInfo {
    pub session_id: String,
    pub started_at_ms: u64,
    pub candidate: CandidateProcess,
    pub workspace_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RawObservation {
    pub observation_id: String,
    pub segment_id: String,
    pub connection_id: String,
    pub direction: TrafficDirection,
    pub transport: TransportKind,
    pub observed_at_ms: u64,
    pub length: usize,
    pub sha256: String,
    pub raw_file: String,
}

/// 由公开命令读取的一份原始观察字节。
///
/// 工作台只允许读取当前会话目录下的 `raw/` 文件；`bytes_hex` 便于前端在
/// 不改变原始文件的前提下展示完整字节内容。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RawBytes {
    pub session_id: String,
    pub raw_file: String,
    pub length: usize,
    pub sha256: String,
    pub bytes_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolFrame {
    pub frame_id: String,
    pub observation_id: String,
    pub segment_id: String,
    pub connection_id: String,
    pub direction: TrafficDirection,
    pub transport: TransportKind,
    pub observed_at_ms: u64,
    pub message_id: u32,
    pub protocol_name: String,
    pub length: usize,
    pub sha256: String,
    pub raw_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub status: SessionStatus,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub candidate: CandidateProcess,
    pub observation_count: usize,
    pub frame_count: usize,
    pub session_path: String,
    pub timeline_path: String,
    pub index_path: String,
    #[serde(default)]
    pub segments: Vec<CaptureSegment>,
    #[serde(default)]
    pub interruptions: Vec<InterruptionInfo>,
    #[serde(default)]
    pub disk_warnings: Vec<DiskSpaceStatus>,
    #[serde(default)]
    pub zero_frame: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndex {
    pub session_id: String,
    pub observations: Vec<RawObservation>,
    pub frames: Vec<ProtocolFrame>,
    pub generated_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub observations: Vec<RawObservation>,
    pub frames: Vec<ProtocolFrame>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSnapshot {
    pub phase: CapturePhase,
    pub session_id: Option<String>,
    pub candidate: Option<CandidateProcess>,
    pub observation_count: usize,
    pub frame_count: usize,
    #[serde(default)]
    pub recovery: CaptureRecoverySnapshot,
}

impl Default for CaptureSnapshot {
    fn default() -> Self {
        Self {
            phase: CapturePhase::Idle,
            session_id: None,
            candidate: None,
            observation_count: 0,
            frame_count: 0,
            recovery: CaptureRecoverySnapshot::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub workspace: Option<WorkspaceInfo>,
    pub capture: CaptureSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionManifest {
    pub session_id: String,
    pub status: SessionStatus,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub candidate: CandidateProcess,
    pub files: SessionFiles,
    #[serde(default)]
    pub lifecycle: SessionLifecycle,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionFiles {
    pub timeline: String,
    pub transport_events: String,
    pub index: String,
    pub summary: String,
    pub raw_directory: String,
}
