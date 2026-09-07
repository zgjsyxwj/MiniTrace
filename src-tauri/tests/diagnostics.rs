use minitrace_lib::backend::EventSink;
use minitrace_lib::commands::{
    diagnose_environment, list_candidates, list_sessions, set_workspace, start_capture,
    stop_capture, AppState,
};
use minitrace_lib::diagnostics::FakeDiagnosticScenario;
use minitrace_lib::model::{CaptureEvent, CaptureEventKind, DiagnosticMode, SessionStatus};
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

#[test]
fn unknown_version_is_visible_but_cannot_create_session() {
    let (root, app, _sink) = configured_app(FakeDiagnosticScenario::UnknownVersion);
    let state = app.state::<AppState>();
    let report = diagnose_environment(state).expect("读取未知版本诊断");
    assert_eq!(report.mode, DiagnosticMode::CompatibilityDiagnostic);
    assert_eq!(report.wechat_version, "unknown");
    assert!(!report.capture_allowed);
    assert!(report.candidates.is_empty());

    let workspace = root.path().join("workspace");
    set_workspace(workspace.to_string_lossy().into_owned(), app.state()).expect("设置工作区");
    let error = start_capture("fake-world-online-weapp".to_string(), app.state())
        .expect_err("未知版本不能建立正式会话");
    assert!(error.contains("兼容性诊断"));
    assert!(std::fs::read_dir(workspace.join("sessions"))
        .expect("读取会话目录")
        .next()
        .is_none());
}

#[test]
fn no_candidate_is_diagnostic_only() {
    let (root, app, _sink) = configured_app(FakeDiagnosticScenario::NoCandidate);
    let state = app.state::<AppState>();
    let report = diagnose_environment(state).expect("读取无候选诊断");
    assert!(report.supported);
    assert!(!report.capture_allowed);
    assert!(report.candidates.is_empty());
    assert!(list_candidates(app.state()).expect("读取候选").is_empty());

    let workspace = root.path().join("workspace");
    set_workspace(workspace.to_string_lossy().into_owned(), app.state()).expect("设置工作区");
    assert!(start_capture("missing".to_string(), app.state()).is_err());
    assert!(list_sessions(app.state()).expect("读取历史会话").is_empty());
}

#[test]
fn multiple_candidates_require_selected_id_and_capture_selected_process() {
    let (root, app, sink) = configured_app(FakeDiagnosticScenario::MultipleCandidates);
    let state = app.state::<AppState>();
    let report = diagnose_environment(state).expect("读取多候选诊断");
    assert_eq!(report.mode, DiagnosticMode::Supported);
    assert!(report.capture_allowed);
    assert!(report.requires_candidate_confirmation);
    assert_eq!(report.candidates.len(), 2);

    let workspace = root.path().join("workspace");
    set_workspace(workspace.to_string_lossy().into_owned(), app.state()).expect("设置工作区");
    let selected = report.candidates[1].clone();
    let session = start_capture(selected.id.clone(), app.state()).expect("选择候选并开始");
    for _ in 0..100 {
        if sink
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.kind == CaptureEventKind::ProtocolFrame)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let summary = stop_capture(app.state()).expect("正常停止");
    assert_eq!(summary.status, SessionStatus::Completed);
    assert_eq!(summary.candidate.id, selected.id);
    assert_eq!(session.candidate.id, selected.id);
}
