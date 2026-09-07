//! Tauri 命令实现。
//!
//! 命令是前端和后续平台后端唯一依赖的边界；fake backend 的线程、文件 writer
//! 和事件转发器都隐藏在命令实现之后。

use crate::backend::{EventSink, NullEventSink};
use crate::diagnostics::{diagnose, diagnose_host, fake_input, FakeDiagnosticScenario};
use crate::model::{
    AppSnapshot, CandidateProcess, CaptureEvent, CaptureEventKind, CapturePhase,
    CaptureSessionInfo, CaptureSnapshot, EnvironmentDiagnosis, RawBytes, ReattachState,
    SessionDetail, SessionIndex, SessionSummary, WorkspaceInfo,
};
use crate::recovery::RecoveryCommand;
use crate::storage::{
    list_sessions as read_session_summaries, now_ms, prepare_workspace, read_json, rebuild_index,
    recover_incomplete_sessions, session_directory, workspace_info, SessionWriter, WorkspaceConfig,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

struct WorkerHandle {
    session_id: String,
    stop_sender: mpsc::Sender<()>,
    recovery_sender: mpsc::Sender<RecoveryCommand>,
    join: JoinHandle<Result<SessionSummary, String>>,
}

#[derive(Clone)]
enum DiagnosticSource {
    Host,
    Fake(FakeDiagnosticScenario),
}

pub struct AppState {
    workspace: Mutex<Option<PathBuf>>,
    capture: Arc<Mutex<CaptureSnapshot>>,
    worker: Mutex<Option<WorkerHandle>>,
    config_path: Mutex<Option<PathBuf>>,
    event_sink: Mutex<Arc<dyn EventSink>>,
    diagnostic_source: Mutex<DiagnosticSource>,
    session_counter: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: Mutex::new(None),
            capture: Arc::new(Mutex::new(CaptureSnapshot::default())),
            worker: Mutex::new(None),
            config_path: Mutex::new(None),
            event_sink: Mutex::new(Arc::new(NullEventSink)),
            // 安全默认值是主机只读诊断。公开测试通过 configure_for_test 显式
            // 注入假清单，避免未配置的状态意外建立正式采集会话。
            diagnostic_source: Mutex::new(DiagnosticSource::Host),
            session_counter: AtomicU64::new(0),
        }
    }

    pub fn configure(
        &self,
        config_path: PathBuf,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<(), String> {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建应用配置目录失败：{error}"))?;
        }
        *self
            .config_path
            .lock()
            .map_err(|_| "锁定配置路径失败".to_string())? = Some(config_path.clone());
        *self
            .event_sink
            .lock()
            .map_err(|_| "锁定事件发送器失败".to_string())? = event_sink;
        *self
            .diagnostic_source
            .lock()
            .map_err(|_| "锁定环境诊断来源失败".to_string())? = DiagnosticSource::Host;

        let configured: Option<WorkspaceConfig> = if config_path.is_file() {
            read_json(&config_path).ok()
        } else {
            None
        };
        if let Some(configured) = configured {
            let path = PathBuf::from(configured.workspace_path);
            if path.is_absolute() && prepare_workspace(&path).is_ok() {
                // 应用重启后把上次仍为 running 的会话安全收尾为 incomplete；
                // 任何已经写入的 raw/和时间线都不会被删除。
                let _ = recover_incomplete_sessions(&path);
                *self
                    .workspace
                    .lock()
                    .map_err(|_| "锁定采集工作区失败".to_string())? = Some(path);
            }
        }
        Ok(())
    }

    pub fn configure_for_test(
        &self,
        config_path: PathBuf,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<(), String> {
        self.configure(config_path, event_sink)?;
        self.set_diagnostic_fixture_for_test(FakeDiagnosticScenario::Supported);
        Ok(())
    }

    /// 为 Rust 集成测试注入固定进程清单，不影响正式运行时的主机诊断。
    pub fn set_diagnostic_fixture_for_test(&self, scenario: FakeDiagnosticScenario) {
        if let Ok(mut source) = self.diagnostic_source.lock() {
            *source = DiagnosticSource::Fake(scenario);
        }
    }

    pub fn configure_for_test_with_scenario(
        &self,
        config_path: PathBuf,
        event_sink: Arc<dyn EventSink>,
        scenario: FakeDiagnosticScenario,
    ) -> Result<(), String> {
        self.configure(config_path, event_sink)?;
        self.set_diagnostic_fixture_for_test(scenario);
        Ok(())
    }

    fn diagnosis(&self) -> Result<EnvironmentDiagnosis, String> {
        let source = self
            .diagnostic_source
            .lock()
            .map_err(|_| "锁定环境诊断来源失败".to_string())?
            .clone();
        Ok(match source {
            DiagnosticSource::Host => diagnose_host(),
            DiagnosticSource::Fake(scenario) => diagnose(fake_input(scenario)),
        })
    }

    fn workspace_path(&self) -> Result<PathBuf, String> {
        self.workspace
            .lock()
            .map_err(|_| "锁定采集工作区失败".to_string())?
            .clone()
            .ok_or_else(|| "请先选择采集工作区".to_string())
    }

    fn set_workspace_path(&self, path: PathBuf) -> Result<WorkspaceInfo, String> {
        if self
            .worker
            .lock()
            .map_err(|_| "锁定采集 worker 失败".to_string())?
            .is_some()
        {
            return Err("采集进行中不能切换工作区".to_string());
        }
        if matches!(
            self.capture
                .lock()
                .map_err(|_| "锁定采集状态失败".to_string())?
                .phase,
            CapturePhase::Starting
                | CapturePhase::Ready
                | CapturePhase::Capturing
                | CapturePhase::Stopping
        ) {
            return Err("采集进行中不能切换工作区".to_string());
        }
        prepare_workspace(&path)?;
        let _ = recover_incomplete_sessions(&path)?;

        if let Some(config_path) = self
            .config_path
            .lock()
            .map_err(|_| "锁定配置路径失败".to_string())?
            .clone()
        {
            let config = WorkspaceConfig {
                workspace_path: path.to_string_lossy().into_owned(),
            };
            crate::storage::write_json_atomic(&config_path, &config)?;
        }
        *self
            .workspace
            .lock()
            .map_err(|_| "锁定采集工作区失败".to_string())? = Some(path.clone());
        Ok(workspace_info(&path))
    }

    fn emit(&self, event: CaptureEvent) {
        update_snapshot(&self.capture, &event);
        let sink = self
            .event_sink
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| Arc::new(NullEventSink));
        let _ = sink.emit(event);
    }

    fn start(&self, candidate_id: String) -> Result<CaptureSessionInfo, String> {
        let diagnosis = self.diagnosis()?;
        if !diagnosis.capture_allowed {
            let detail = diagnosis
                .issues
                .iter()
                .filter(|issue| issue.blocking)
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("；");
            return Err(if detail.is_empty() {
                "当前环境仅支持兼容性诊断，不能建立正式采集会话".to_string()
            } else {
                format!("当前环境仅支持兼容性诊断，不能建立正式采集会话：{detail}")
            });
        }
        let candidate = diagnosis
            .candidates
            .into_iter()
            .find(|candidate| candidate.id == candidate_id)
            .ok_or_else(|| format!("找不到当前诊断中的候选进程：{candidate_id}"))?;
        if self
            .worker
            .lock()
            .map_err(|_| "锁定采集 worker 失败".to_string())?
            .is_some()
        {
            return Err("已有采集正在进行".to_string());
        }
        if matches!(
            self.capture
                .lock()
                .map_err(|_| "锁定采集状态失败".to_string())?
                .phase,
            CapturePhase::Starting
                | CapturePhase::Ready
                | CapturePhase::Capturing
                | CapturePhase::Stopping
        ) {
            return Err("已有采集正在进行".to_string());
        }
        let workspace = self.workspace_path()?;
        let started_at_ms = now_ms();
        let counter = self.session_counter.fetch_add(1, Ordering::Relaxed);
        let session_id = format!(
            "session-{started_at_ms:013}-{counter:04}-{}",
            Uuid::new_v4().simple()
        );
        let writer = SessionWriter::create(&workspace, &session_id, started_at_ms, &candidate)?;
        let recovery_snapshot = crate::recovery::RecoveryMachine::new(
            session_id.clone(),
            started_at_ms,
            candidate.clone(),
        )
        .snapshot();
        if let Ok(mut snapshot) = self.capture.lock() {
            *snapshot = CaptureSnapshot {
                phase: CapturePhase::Starting,
                session_id: Some(session_id.clone()),
                candidate: Some(candidate.clone()),
                observation_count: 0,
                frame_count: 0,
                recovery: recovery_snapshot,
            };
        }
        let (stop_sender, stop_receiver) = mpsc::channel();
        let (recovery_sender, recovery_receiver) = mpsc::channel();
        let (start_sender, start_receiver) = mpsc::channel();
        let sink = self
            .event_sink
            .lock()
            .map_err(|_| "锁定事件发送器失败".to_string())?
            .clone();
        let capture = Arc::clone(&self.capture);
        let worker_session_id = session_id.clone();
        let worker_candidate = candidate.clone();
        let worker = thread::Builder::new()
            .name(format!("minitrace-capture-{session_id}"))
            .spawn(move || {
                if start_receiver.recv().is_err() {
                    return Err("采集 worker 启动闸门已关闭".to_string());
                }
                let stateful_sink: Arc<dyn EventSink> = Arc::new(StatefulEventSink {
                    state: capture,
                    inner: sink,
                });
                run_capture_backend(
                    writer,
                    worker_session_id,
                    started_at_ms,
                    worker_candidate,
                    stop_receiver,
                    recovery_receiver,
                    stateful_sink,
                )
            })
            .map_err(|error| format!("启动采集 worker 失败：{error}"))?;

        *self
            .worker
            .lock()
            .map_err(|_| "锁定采集 worker 失败".to_string())? = Some(WorkerHandle {
            session_id: session_id.clone(),
            stop_sender,
            recovery_sender,
            join: worker,
        });
        self.emit(CaptureEvent {
            kind: CaptureEventKind::Started,
            session_id: session_id.clone(),
            timestamp_ms: started_at_ms,
            data: serde_json::json!({
                "backend": candidate.backend,
                "candidate": candidate,
                "segmentId": format!("{session_id}-segment-001"),
                "workspacePath": workspace.to_string_lossy()
            }),
        });
        start_sender
            .send(())
            .map_err(|error| format!("启动采集 worker 失败：{error}"))?;
        Ok(CaptureSessionInfo {
            session_id,
            started_at_ms,
            candidate,
            workspace_path: workspace.to_string_lossy().into_owned(),
        })
    }

    fn stop(&self) -> Result<SessionSummary, String> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| "锁定采集 worker 失败".to_string())?
            .take()
            .ok_or_else(|| "当前没有进行中的采集".to_string())?;
        if let Ok(mut snapshot) = self.capture.lock() {
            snapshot.phase = CapturePhase::Stopping;
        }
        let stop_error = worker.stop_sender.send(()).err();
        let session_id = worker.session_id.clone();
        let result = match worker.join.join() {
            Ok(result) => result,
            Err(_) => {
                if let Ok(mut snapshot) = self.capture.lock() {
                    snapshot.phase = CapturePhase::Error;
                }
                let message = format!("采集 worker 异常退出：{session_id}");
                self.emit(CaptureEvent {
                    kind: CaptureEventKind::Error,
                    session_id,
                    timestamp_ms: now_ms(),
                    data: serde_json::json!({"message": message}),
                });
                return Err(message);
            }
        };
        if let Some(error) = stop_error {
            let message = format!("通知采集 worker 停止失败：{error}");
            if let Ok(mut snapshot) = self.capture.lock() {
                snapshot.phase = CapturePhase::Error;
            }
            return Err(message);
        }
        match result {
            Ok(summary) => {
                if let Ok(mut snapshot) = self.capture.lock() {
                    snapshot.phase = if summary.status == crate::model::SessionStatus::Completed {
                        CapturePhase::Completed
                    } else {
                        CapturePhase::Error
                    };
                }
                Ok(summary)
            }
            Err(error) => {
                if let Ok(mut snapshot) = self.capture.lock() {
                    snapshot.phase = CapturePhase::Error;
                }
                self.emit(CaptureEvent {
                    kind: CaptureEventKind::Error,
                    session_id,
                    timestamp_ms: now_ms(),
                    data: serde_json::json!({"message": error}),
                });
                Err(error)
            }
        }
    }

    fn send_recovery_command(&self, command: RecoveryCommand) -> Result<(), String> {
        let workers = self
            .worker
            .lock()
            .map_err(|_| "锁定采集 worker 失败".to_string())?;
        let worker = workers
            .as_ref()
            .ok_or_else(|| "当前没有等待恢复的采集".to_string())?;
        worker
            .recovery_sender
            .send(command)
            .map_err(|error| format!("发送采集恢复命令失败：{error}"))
    }

    fn report_target_process_exit(&self, reason: String) -> Result<(), String> {
        let candidates = self.diagnosis()?.candidates;
        self.send_recovery_command(RecoveryCommand::TargetExited { reason, candidates })
    }

    fn refresh_recovery_candidates(&self) -> Result<(), String> {
        self.send_recovery_command(RecoveryCommand::CandidatesDiscovered(
            self.diagnosis()?.candidates,
        ))
    }

    fn select_recovery_candidate(&self, candidate_id: String) -> Result<(), String> {
        self.send_recovery_command(RecoveryCommand::SelectCandidate(candidate_id))
    }
}

fn run_capture_backend(
    writer: SessionWriter,
    session_id: String,
    started_at_ms: u64,
    candidate: CandidateProcess,
    stop_requested: mpsc::Receiver<()>,
    recovery_requested: mpsc::Receiver<RecoveryCommand>,
    sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    if candidate.backend == "fake" {
        return crate::backend::run_resilient_fake_capture(
            writer,
            session_id,
            started_at_ms,
            candidate,
            stop_requested,
            recovery_requested,
            sink,
        );
    }
    #[cfg(target_os = "windows")]
    {
        return crate::windows_backend::run_windows_capture(
            writer,
            session_id,
            started_at_ms,
            candidate,
            stop_requested,
            recovery_requested,
            sink,
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        {
            crate::macos_backend::run_macos_capture(
                writer,
                session_id,
                started_at_ms,
                candidate,
                stop_requested,
                recovery_requested,
                sink,
            )
        }

        #[cfg(not(target_os = "macos"))]
        {
            drop(recovery_requested);
            crate::backend::run_fake_capture(
                writer,
                session_id,
                started_at_ms,
                candidate,
                stop_requested,
                sink,
            )
        }
    }
}

struct StatefulEventSink {
    state: Arc<Mutex<CaptureSnapshot>>,
    inner: Arc<dyn EventSink>,
}

impl EventSink for StatefulEventSink {
    fn emit(&self, event: CaptureEvent) -> Result<(), String> {
        update_snapshot(&self.state, &event);
        self.inner.emit(event)
    }
}

fn update_snapshot(state: &Arc<Mutex<CaptureSnapshot>>, event: &CaptureEvent) {
    let Ok(mut snapshot) = state.lock() else {
        return;
    };
    snapshot.session_id = Some(event.session_id.clone());
    match event.kind {
        CaptureEventKind::Started => {
            snapshot.phase = CapturePhase::Starting;
            snapshot.recovery.session_id = Some(event.session_id.clone());
            snapshot.recovery.state = Some(ReattachState::Attaching);
            if let Some(segment_id) = event.data.get("segmentId").and_then(|value| value.as_str()) {
                snapshot.recovery.current_segment_id = Some(segment_id.to_string());
            }
            if let Some(candidate) = event.data.get("candidate") {
                if let Ok(candidate) = serde_json::from_value::<CandidateProcess>(candidate.clone())
                {
                    snapshot.candidate = Some(candidate.clone());
                    snapshot.recovery.current_pid = Some(candidate.pid);
                    snapshot.recovery.segments = vec![crate::model::CaptureSegment {
                        segment_id: snapshot
                            .recovery
                            .current_segment_id
                            .clone()
                            .unwrap_or_else(|| format!("{}-segment-001", event.session_id)),
                        candidate,
                        started_at_ms: event.timestamp_ms,
                        ended_at_ms: None,
                        status: crate::model::SegmentStatus::Starting,
                    }];
                }
            }
        }
        CaptureEventKind::Ready => {
            snapshot.phase = CapturePhase::Ready;
            snapshot.recovery.state = Some(ReattachState::Attached);
            let candidate_from_event = event
                .data
                .get("candidate")
                .and_then(|value| serde_json::from_value::<CandidateProcess>(value.clone()).ok());
            let pid_from_event = event
                .data
                .get("pid")
                .and_then(|value| value.as_u64())
                .and_then(|pid| u32::try_from(pid).ok());
            if let Some(segment_id) = event.data.get("segmentId").and_then(|value| value.as_str()) {
                snapshot.recovery.current_segment_id = Some(segment_id.to_string());
                if let Some(segment) = snapshot
                    .recovery
                    .segments
                    .iter_mut()
                    .find(|segment| segment.segment_id == segment_id)
                {
                    segment.status = crate::model::SegmentStatus::Attached;
                    if let Some(candidate) = candidate_from_event.clone() {
                        segment.candidate = candidate;
                    }
                } else if let Some(candidate) = candidate_from_event.clone().or_else(|| {
                    snapshot.candidate.clone().map(|mut candidate| {
                        if let Some(pid) = pid_from_event {
                            candidate.pid = pid;
                        }
                        candidate.id = segment_id.to_string();
                        candidate
                    })
                }) {
                    snapshot
                        .recovery
                        .segments
                        .push(crate::model::CaptureSegment {
                            segment_id: segment_id.to_string(),
                            candidate,
                            started_at_ms: event.timestamp_ms,
                            ended_at_ms: None,
                            status: crate::model::SegmentStatus::Attached,
                        });
                }
            }
            if let Some(candidate) = candidate_from_event {
                snapshot.candidate = Some(candidate.clone());
                snapshot.recovery.current_pid = Some(candidate.pid);
            } else if let Some(pid) = pid_from_event {
                snapshot.recovery.current_pid = Some(pid);
            }
        }
        CaptureEventKind::RawObservation => {
            snapshot.phase = CapturePhase::Capturing;
            snapshot.observation_count += 1;
            snapshot.recovery.zero_frame = snapshot.frame_count == 0;
        }
        CaptureEventKind::ProtocolFrame => {
            snapshot.phase = CapturePhase::Capturing;
            snapshot.frame_count += 1;
            snapshot.recovery.zero_frame = false;
        }
        CaptureEventKind::Stopped | CaptureEventKind::Summary => {
            if event.data.get("status").and_then(|value| value.as_str()) == Some("incomplete") {
                snapshot.phase = CapturePhase::Error;
                snapshot.recovery.state = Some(ReattachState::AttachFailed);
            } else {
                snapshot.phase = CapturePhase::Completed;
                snapshot.recovery.state = Some(ReattachState::Stopped);
            }
        }
        CaptureEventKind::Error => {
            let recoverable = event
                .data
                .get("recoverable")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if recoverable {
                if let Some(state) = event.data.get("state").and_then(|value| value.as_str()) {
                    snapshot.recovery.state = match state {
                        "interrupted" => Some(ReattachState::Interrupted),
                        "awaiting_candidate" => Some(ReattachState::AwaitingCandidate),
                        "reattaching" => Some(ReattachState::Reattaching),
                        "attach_failed" => Some(ReattachState::AttachFailed),
                        "attached" => Some(ReattachState::Attached),
                        _ => snapshot.recovery.state.clone(),
                    };
                }
                snapshot.recovery.last_error = event
                    .data
                    .get("message")
                    .or_else(|| event.data.get("reason"))
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
                if let Some(candidates) = event.data.get("candidates") {
                    if let Ok(candidates) = serde_json::from_value(candidates.clone()) {
                        snapshot.recovery.candidates = candidates;
                    }
                }
                if let Some(interruption_started_at_ms) = event
                    .data
                    .get("interruptionStartedAtMs")
                    .and_then(|value| value.as_u64())
                {
                    snapshot.recovery.interruption_started_at_ms = Some(interruption_started_at_ms);
                }
                if let Some(interruption) = event.data.get("interruption") {
                    if let Ok(interruption) = serde_json::from_value::<crate::model::InterruptionInfo>(
                        interruption.clone(),
                    ) {
                        if !snapshot
                            .recovery
                            .interruptions
                            .iter()
                            .any(|item| item.interruption_id == interruption.interruption_id)
                        {
                            snapshot.recovery.interruptions.push(interruption.clone());
                        }
                        if let Some(segment) = snapshot
                            .recovery
                            .segments
                            .iter_mut()
                            .find(|segment| segment.segment_id == interruption.segment_id)
                        {
                            segment.status = crate::model::SegmentStatus::Interrupted;
                            segment.ended_at_ms = Some(interruption.started_at_ms);
                        }
                    }
                }
                if event.data.get("code").and_then(|value| value.as_str())
                    == Some("disk_space_warning")
                {
                    if let Some(disk) = event.data.get("disk") {
                        snapshot.recovery.disk = serde_json::from_value(disk.clone()).ok();
                    }
                }
            } else {
                snapshot.phase = CapturePhase::Error;
                snapshot.recovery.state = Some(ReattachState::AttachFailed);
                snapshot.recovery.last_error = event
                    .data
                    .get("message")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
            }
        }
    }
}

#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let workspace = state
        .workspace
        .lock()
        .map_err(|_| "锁定采集工作区失败".to_string())?
        .clone()
        .map(|path| workspace_info(&path));
    let capture = state
        .capture
        .lock()
        .map_err(|_| "锁定采集状态失败".to_string())?
        .clone();
    Ok(AppSnapshot { workspace, capture })
}

#[tauri::command]
pub fn set_workspace(path: String, state: State<'_, AppState>) -> Result<WorkspaceInfo, String> {
    state.set_workspace_path(PathBuf::from(path))
}

#[tauri::command]
pub fn pick_workspace(app: AppHandle, state: State<'_, AppState>) -> Result<WorkspaceInfo, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("选择 MiniTrace 采集工作区")
        .blocking_pick_folder()
        .ok_or_else(|| "用户取消选择采集工作区".to_string())?;
    let path = selected
        .into_path()
        .map_err(|error| format!("目录选择结果不是本地路径：{error}"))?;
    state.set_workspace_path(path)
}

#[tauri::command]
pub fn diagnose_environment(state: State<'_, AppState>) -> Result<EnvironmentDiagnosis, String> {
    state.diagnosis()
}

#[tauri::command]
pub fn list_candidates(state: State<'_, AppState>) -> Result<Vec<CandidateProcess>, String> {
    Ok(state.diagnosis()?.candidates)
}

#[tauri::command]
pub fn start_capture(
    candidate_id: String,
    state: State<'_, AppState>,
) -> Result<CaptureSessionInfo, String> {
    state.start(candidate_id)
}

#[tauri::command]
pub fn stop_capture(state: State<'_, AppState>) -> Result<SessionSummary, String> {
    state.stop()
}

/// 平台进程监视器或自动化验收在确认承载进程消失后调用。命令会立即把
/// 当前区间标记为中断，再用只读诊断结果尝试唯一候选自动重新附加。
#[tauri::command]
pub fn report_target_process_exit(
    reason: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.report_target_process_exit(
        reason.unwrap_or_else(|| "承载进程消失，等待重新发现".to_string()),
    )
}

#[tauri::command]
pub fn refresh_recovery_candidates(state: State<'_, AppState>) -> Result<(), String> {
    state.refresh_recovery_candidates()
}

#[tauri::command]
pub fn select_recovery_candidate(
    candidate_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.select_recovery_candidate(candidate_id)
}

#[tauri::command]
pub fn get_capture_recovery(
    state: State<'_, AppState>,
) -> Result<crate::model::CaptureRecoverySnapshot, String> {
    Ok(state
        .capture
        .lock()
        .map_err(|_| "锁定采集状态失败".to_string())?
        .recovery
        .clone())
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    let workspace = state.workspace_path()?;
    read_session_summaries(&workspace)
}

#[tauri::command]
pub fn get_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionDetail, String> {
    let workspace = state.workspace_path()?;
    crate::storage::read_session(&workspace, &session_id)
}

#[tauri::command]
pub fn read_raw_bytes(
    session_id: String,
    raw_file: String,
    state: State<'_, AppState>,
) -> Result<RawBytes, String> {
    let workspace = state.workspace_path()?;
    crate::storage::read_raw_bytes(&workspace, &session_id, &raw_file)
}

#[tauri::command]
pub fn rebuild_session_index(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionIndex, String> {
    let workspace = state.workspace_path()?;
    rebuild_index(&workspace, &session_id)
}

/// 在不接受任意前端路径的前提下打开指定历史会话目录。
#[tauri::command]
pub fn open_session_directory(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let workspace = state.workspace_path()?;
    let directory = session_directory(&workspace, &session_id)?;
    let mut command = if cfg!(target_os = "macos") {
        let mut command = std::process::Command::new("open");
        command.arg(&directory);
        command
    } else if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("explorer.exe");
        command.arg(&directory);
        command
    } else if cfg!(target_os = "linux") {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(&directory);
        command
    } else {
        return Err("当前平台没有安全的会话目录打开器".to_string());
    };
    command
        .spawn()
        .map_err(|error| format!("打开采集会话目录失败：{error}"))?;
    Ok(directory.to_string_lossy().into_owned())
}
