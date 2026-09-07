use minitrace_lib::backend::EventSink;
use minitrace_lib::commands::{
    get_app_state, get_session, list_candidates, list_sessions, read_raw_bytes,
    rebuild_session_index, set_workspace, start_capture, stop_capture, AppState,
};
use minitrace_lib::model::{CaptureEvent, CaptureEventKind, CapturePhase, SessionStatus};
use std::path::PathBuf;
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

#[test]
fn public_command_surface_runs_complete_fake_capture_flow() {
    let root = tempfile::tempdir().expect("创建测试目录");
    let workspace = root.path().join("capture-workspace");
    let config_path = root.path().join("application").join("workspace.json");
    let sink = RecordingSink::default();
    let app = tauri::test::mock_app();
    app.manage(AppState::new());
    let state = app.state::<AppState>();
    state
        .configure_for_test(config_path.clone(), Arc::new(sink.clone()))
        .expect("配置测试事件发送器");

    let workspace_info = set_workspace(workspace.to_string_lossy().into_owned(), app.state())
        .expect("设置采集工作区");
    assert_eq!(workspace_info.path, workspace.to_string_lossy());
    assert!(config_path.is_file());

    let candidates = list_candidates(app.state()).expect("读取候选进程");
    assert_eq!(candidates.len(), 1);
    let session = start_capture(candidates[0].id.clone(), app.state()).expect("开始采集");

    for _ in 0..100 {
        let events = sink.events.lock().unwrap();
        let frame_count = events
            .iter()
            .filter(|event| event.kind == CaptureEventKind::ProtocolFrame)
            .count();
        let observation_count = events
            .iter()
            .filter(|event| event.kind == CaptureEventKind::RawObservation)
            .count();
        if frame_count == 4 && observation_count == 6 {
            break;
        }
        drop(events);
        thread::sleep(Duration::from_millis(10));
    }
    let recorded_events = sink.events.lock().unwrap().clone();
    let kinds: Vec<CaptureEventKind> = recorded_events
        .iter()
        .map(|event| event.kind.clone())
        .collect();
    assert!(kinds.contains(&CaptureEventKind::Started));
    assert!(kinds.contains(&CaptureEventKind::Ready));
    assert!(kinds.contains(&CaptureEventKind::RawObservation));
    assert!(kinds.contains(&CaptureEventKind::ProtocolFrame));
    let frames: Vec<&CaptureEvent> = recorded_events
        .iter()
        .filter(|event| event.kind == CaptureEventKind::ProtocolFrame)
        .collect();
    assert_eq!(frames.len(), 4);
    assert_eq!(
        frames
            .iter()
            .map(|event| event.data["messageId"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![10_504, 10_504, 16_001, 16_001]
    );
    assert_eq!(
        frames
            .iter()
            .map(|event| event.data["protocolName"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "CG_ENTER_MAP_REQUEST",
            "CG_ENTER_MAP_RESPONSE",
            "CG_HEARTBEAT_REQUEST",
            "CG_HEARTBEAT_RESPONSE"
        ]
    );
    let unparsed: Vec<&CaptureEvent> = recorded_events
        .iter()
        .filter(|event| {
            event.kind == CaptureEventKind::RawObservation
                && event.data["classification"] == "unparsed_payload"
        })
        .collect();
    assert_eq!(unparsed.len(), 2);
    assert_eq!(unparsed[0].data["direction"], "request");
    assert_eq!(unparsed[1].data["direction"], "response");

    let summary = stop_capture(app.state()).expect("正常停止采集");
    assert_eq!(summary.session_id, session.session_id);
    assert_eq!(summary.status, SessionStatus::Completed);
    assert_eq!(summary.observation_count, 6);
    assert_eq!(summary.frame_count, 4);

    let final_kinds: Vec<CaptureEventKind> = sink
        .events
        .lock()
        .unwrap()
        .iter()
        .map(|event| event.kind.clone())
        .collect();
    assert!(final_kinds.ends_with(&[CaptureEventKind::Stopped, CaptureEventKind::Summary]));

    let snapshot = get_app_state(app.state()).expect("读取应用状态");
    assert_eq!(snapshot.capture.phase, CapturePhase::Completed);
    assert_eq!(snapshot.capture.frame_count, 4);

    let sessions = list_sessions(app.state()).expect("读取历史会话");
    assert_eq!(sessions.len(), 1);
    let detail = get_session(session.session_id.clone(), app.state()).expect("打开历史会话");
    assert_eq!(detail.frames.len(), 4);
    assert_eq!(detail.frames[0].message_id, 10_504);
    assert_eq!(
        detail.frames[0].direction,
        minitrace_lib::model::TrafficDirection::Request
    );
    assert_eq!(
        detail.frames[1].direction,
        minitrace_lib::model::TrafficDirection::Response
    );
    assert_eq!(detail.observations.len(), 6);

    let session_path = PathBuf::from(&summary.session_path);
    assert!(session_path.join("timeline.jsonl").is_file());
    assert!(session_path.join("transport-events.jsonl").is_file());
    assert!(session_path.join("summary.json").is_file());
    assert!(session_path.join("index.sqlite").is_file());
    let raw_file = session_path.join(&detail.frames[0].raw_file);
    assert_eq!(
        std::fs::read(raw_file).expect("读取原始字节").len(),
        detail.frames[0].length
    );
    let raw_bytes = read_raw_bytes(
        session.session_id.clone(),
        detail.frames[0].raw_file.clone(),
        app.state(),
    )
    .expect("通过公开命令读取原始字节");
    assert_eq!(raw_bytes.length, detail.frames[0].length);
    assert_eq!(raw_bytes.bytes_hex.len(), raw_bytes.length * 2);
    assert!(read_raw_bytes(
        session.session_id.clone(),
        "../summary.json".to_string(),
        app.state(),
    )
    .is_err());

    std::fs::remove_file(session_path.join("index.sqlite")).expect("删除可重建索引");
    let rebuilt = rebuild_session_index(session.session_id, app.state()).expect("重建索引");
    assert_eq!(rebuilt.frames.len(), 4);
    assert_eq!(rebuilt.observations.len(), 6);
    assert!(session_path.join("index.sqlite").is_file());

    let restarted_app = tauri::test::mock_app();
    restarted_app.manage(AppState::new());
    let restarted_state = restarted_app.state::<AppState>();
    restarted_state
        .configure_for_test(config_path, Arc::new(RecordingSink::default()))
        .expect("重新载入应用配置");
    let restarted = get_app_state(restarted_app.state()).expect("读取重启后的状态");
    assert_eq!(
        restarted.workspace.unwrap().path,
        workspace.to_string_lossy()
    );
}
