use minitrace_lib::backend::{fake_candidates, EventSink};
use minitrace_lib::commands::{
    get_app_state, get_capture_recovery, list_candidates, list_sessions,
    report_target_process_exit, select_recovery_candidate, set_workspace, start_capture,
    stop_capture, AppState,
};
use minitrace_lib::diagnostics::FakeDiagnosticScenario;
use minitrace_lib::model::{
    CaptureEvent, CaptureEventKind, RawObservation, ReattachState, SessionStatus, TrafficDirection,
    TransportKind,
};
use minitrace_lib::storage::{
    now_ms, read_session, rebuild_index, recover_incomplete_sessions, sha256_hex, SessionWriter,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::Manager;

#[derive(Clone, Default)]
struct RecordingSink {
    events: Arc<Mutex<Vec<CaptureEvent>>>,
}

impl EventSink for RecordingSink {
    fn emit(&self, event: CaptureEvent) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

fn configured_app(
    scenario: FakeDiagnosticScenario,
) -> (
    tempfile::TempDir,
    tauri::App<tauri::test::MockRuntime>,
    RecordingSink,
) {
    let root = tempfile::tempdir().expect("创建测试目录");
    let config_path = root.path().join("application").join("workspace.json");
    let sink = RecordingSink::default();
    let app = tauri::test::mock_app();
    app.manage(AppState::new());
    app.state::<AppState>()
        .configure_for_test_with_scenario(config_path, Arc::new(sink.clone()), scenario)
        .expect("配置测试诊断来源");
    (root, app, sink)
}

fn wait_until<F>(mut predicate: F)
where
    F: FnMut() -> bool,
{
    for _ in 0..300 {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(predicate(), "等待恢复状态超时");
}

#[test]
fn target_restart_keeps_session_and_adds_segment() {
    let (root, app, sink) = configured_app(FakeDiagnosticScenario::Supported);
    let workspace = root.path().join("workspace");
    set_workspace(workspace.to_string_lossy().into_owned(), app.state()).expect("设置工作区");
    let candidate = list_candidates(app.state()).expect("读取候选").remove(0);
    let session = start_capture(candidate.id.clone(), app.state()).expect("开始采集");

    wait_until(|| {
        sink.events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.kind == CaptureEventKind::ProtocolFrame)
    });
    report_target_process_exit(Some("测试承载进程退出".to_string()), app.state())
        .expect("报告承载进程退出");
    wait_until(|| {
        let snapshot = get_capture_recovery(app.state()).expect("读取恢复快照");
        snapshot.state == Some(ReattachState::Attached)
            && snapshot.segments.len() == 2
            && snapshot.interruptions.len() == 1
    });

    let recovery = get_capture_recovery(app.state()).expect("读取恢复快照");
    assert_eq!(
        recovery.session_id.as_deref(),
        Some(session.session_id.as_str())
    );
    assert_ne!(
        recovery.segments[0].segment_id, recovery.segments[1].segment_id,
        "进程重新附加必须建立新分段"
    );
    assert!(recovery.segments[1].candidate.pid > 0);
    let events = sink.events.lock().unwrap().clone();
    assert!(events.iter().any(|event| {
        event.kind == CaptureEventKind::Error && event.data["code"] == "target_process_disappeared"
    }));
    assert!(events.iter().any(|event| {
        event.kind == CaptureEventKind::Ready
            && event.data["segmentId"] == recovery.segments[1].segment_id
    }));

    let summary = stop_capture(app.state()).expect("停止采集");
    assert_eq!(summary.status, SessionStatus::Completed);
    assert_eq!(summary.session_id, session.session_id);
    assert_eq!(summary.segments.len(), 2);
    assert_eq!(summary.interruptions.len(), 1);
    assert_eq!(list_sessions(app.state()).expect("读取历史会话").len(), 1);
}

#[test]
fn multiple_restart_candidates_pause_until_user_selection() {
    let (root, app, sink) = configured_app(FakeDiagnosticScenario::MultipleCandidates);
    let workspace = root.path().join("workspace");
    set_workspace(workspace.to_string_lossy().into_owned(), app.state()).expect("设置工作区");
    let candidates = list_candidates(app.state()).expect("读取候选");
    let selected = candidates[0].clone();
    start_capture(selected.id, app.state()).expect("开始采集");
    wait_until(|| {
        sink.events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.kind == CaptureEventKind::ProtocolFrame)
    });

    report_target_process_exit(Some("测试多候选".to_string()), app.state())
        .expect("报告承载进程退出");
    wait_until(|| {
        let snapshot = get_capture_recovery(app.state()).expect("读取恢复快照");
        snapshot.state == Some(ReattachState::AwaitingCandidate) && snapshot.candidates.len() == 2
    });
    let paused = get_app_state(app.state()).expect("读取应用状态");
    assert_eq!(
        paused.capture.recovery.state,
        Some(ReattachState::AwaitingCandidate)
    );
    assert_eq!(paused.capture.recovery.segments.len(), 1);
    assert!(paused.capture.recovery.interruption_started_at_ms.is_some());

    let next = paused.capture.recovery.candidates[1].id.clone();
    select_recovery_candidate(next.clone(), app.state()).expect("选择恢复候选");
    wait_until(|| {
        let snapshot = get_capture_recovery(app.state()).expect("读取恢复快照");
        snapshot.state == Some(ReattachState::Attached)
            && snapshot.segments.len() == 2
            && snapshot.current_pid == Some(snapshot.segments[1].candidate.pid)
    });
    let summary = stop_capture(app.state()).expect("停止采集");
    assert_eq!(summary.status, SessionStatus::Completed);
    assert_eq!(summary.segments.len(), 2);
    assert_eq!(summary.candidate.id, next);
}

#[test]
fn abnormal_exit_is_recovered_without_deleting_raw_evidence() {
    let root = tempfile::tempdir().expect("创建测试目录");
    let workspace = root.path().join("workspace");
    let candidate = fake_candidates().remove(0);
    let session_id = "session-abnormal-recovery";
    let mut writer =
        SessionWriter::create(&workspace, session_id, 100, &candidate).expect("创建会话");
    let bytes = b"unfinished raw observation";
    let raw_file = writer.write_raw(1, bytes).expect("写入原始证据");
    let observation = RawObservation {
        observation_id: format!("{session_id}-observation-1"),
        segment_id: format!("{session_id}-segment-001"),
        connection_id: "connection-1".to_string(),
        direction: TrafficDirection::Request,
        transport: TransportKind::Http,
        observed_at_ms: 120,
        length: bytes.len(),
        sha256: sha256_hex(bytes),
        raw_file: raw_file.clone(),
    };
    writer
        .append_event(
            120,
            "raw_observation",
            serde_json::to_value(&observation).expect("序列化观察"),
        )
        .expect("写入时间线");
    writer.add_observation(observation);
    let session_path = writer.root_path().to_path_buf();
    drop(writer);

    let recovered = recover_incomplete_sessions(&workspace).expect("恢复不完整会话");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, SessionStatus::Incomplete);
    assert_eq!(recovered[0].observation_count, 1);
    assert!(session_path.join(&raw_file).is_file());
    assert!(std::fs::read_to_string(session_path.join("timeline.jsonl"))
        .expect("读取时间线")
        .contains("recovered_incomplete"));

    let detail = read_session(&workspace, session_id).expect("读取恢复会话");
    assert_eq!(detail.summary.status, SessionStatus::Incomplete);
    assert_eq!(detail.observations.len(), 1);
    std::fs::remove_file(session_path.join("index.sqlite")).expect("删除索引");
    let rebuilt = rebuild_index(&workspace, session_id).expect("从时间线重建索引");
    assert_eq!(rebuilt.observations.len(), 1);
    assert!(session_path.join("index.sqlite").is_file());
}

#[test]
fn session_directory_is_confined_to_workspace() {
    let root = tempfile::tempdir().expect("创建测试目录");
    let workspace = root.path().join("workspace");
    let candidate = fake_candidates().remove(0);
    let writer = SessionWriter::create(&workspace, "session-safe-directory", now_ms(), &candidate)
        .expect("创建会话");
    let session_path = writer.root_path().to_path_buf();
    drop(writer);
    let resolved = minitrace_lib::storage::session_directory(&workspace, "session-safe-directory")
        .expect("解析会话目录");
    assert_eq!(
        resolved,
        std::fs::canonicalize(session_path).expect("规范化目录")
    );
    assert!(minitrace_lib::storage::session_directory(&workspace, "../outside").is_err());
}

#[test]
fn disk_probe_reads_real_workspace_capacity() {
    let root = tempfile::tempdir().expect("创建测试目录");
    let status = minitrace_lib::disk::inspect(root.path(), 123).expect("读取磁盘容量");
    assert!(status.total_bytes > 0);
    assert!(status.free_bytes <= status.total_bytes);
    assert_eq!(status.checked_at_ms, 123);
}
