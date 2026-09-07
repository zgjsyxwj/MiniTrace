//! 可控的假采集后端。
//!
//! 这个后端只用于建立桌面端命令/事件/会话闭环。它不会接触真实微信进程，
//! 但产生与正式后端相同的公开事件顺序和磁盘证据结构。

use crate::model::{
    CandidateProcess, CaptureEvent, CaptureEventKind, ProtocolFrame, RawObservation, SessionStatus,
    SessionSummary, TrafficDirection, TransportKind,
};
use crate::recovery::{probe_disk, RecoveryCommand, RecoveryMachine, RecoveryTransition};
use crate::storage::{now_ms, sha256_hex, SessionWriter};
use serde_json::json;
use std::sync::{mpsc::Receiver, Arc};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub trait EventSink: Send + Sync {
    fn emit(&self, event: CaptureEvent) -> Result<(), String>;
}

#[derive(Default)]
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit(&self, _event: CaptureEvent) -> Result<(), String> {
        Ok(())
    }
}

pub struct TauriEventSink {
    pub app: AppHandle,
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: CaptureEvent) -> Result<(), String> {
        self.app
            .emit(crate::model::CAPTURE_EVENT_NAME, event)
            .map_err(|error| format!("发送采集事件失败：{error}"))
    }
}

pub fn fake_candidates() -> Vec<CandidateProcess> {
    vec![CandidateProcess {
        id: "fake-world-online-weapp".to_string(),
        pid: 42_013,
        display_name: "WeApp · 世界 Online（假后端）".to_string(),
        process_role: "WeApp".to_string(),
        game: "世界 Online".to_string(),
        wechat_version: "4.1.13-fake".to_string(),
        architecture: "arm64".to_string(),
        modules: vec![
            "WeChatAppEx Framework".to_string(),
            "flue.dylib (fake)".to_string(),
        ],
        backend: "fake".to_string(),
    }]
}

struct FakeEventSpec {
    bytes: Vec<u8>,
    connection_id: &'static str,
    direction: TrafficDirection,
    transport: TransportKind,
    message_id: Option<u32>,
    protocol_name: Option<&'static str>,
    classification: Option<&'static str>,
}

fn fake_frame_bytes(message_id: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x4d, 0x54, 0x52, 0x43, 0x00, 0x01];
    bytes.extend_from_slice(&(message_id as u16).to_le_bytes());
    bytes.extend_from_slice(&(body.len() as u16).to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}

/// issue 04 的确定性工作台 fixture：四条有消息号和协议名的双向帧，
/// 加上 HTTP 请求/响应两条未分类明文。它只用于 fake backend，不代表真实
/// 微信流量语义。
fn fake_workbench_events() -> Vec<FakeEventSpec> {
    vec![
        FakeEventSpec {
            bytes: fake_frame_bytes(10_504, br#"{"map":16}"#),
            connection_id: "fake-ws-001",
            direction: TrafficDirection::Request,
            transport: TransportKind::WebSocket,
            message_id: Some(10_504),
            protocol_name: Some("CG_ENTER_MAP_REQUEST"),
            classification: None,
        },
        FakeEventSpec {
            bytes: fake_frame_bytes(10_504, br#"{"result":0}"#),
            connection_id: "fake-ws-001",
            direction: TrafficDirection::Response,
            transport: TransportKind::WebSocket,
            message_id: Some(10_504),
            protocol_name: Some("CG_ENTER_MAP_RESPONSE"),
            classification: None,
        },
        FakeEventSpec {
            bytes: fake_frame_bytes(16_001, br#"{"heartbeat":1}"#),
            connection_id: "fake-ws-001",
            direction: TrafficDirection::Request,
            transport: TransportKind::WebSocket,
            message_id: Some(16_001),
            protocol_name: Some("CG_HEARTBEAT_REQUEST"),
            classification: None,
        },
        FakeEventSpec {
            bytes: fake_frame_bytes(16_001, br#"{"heartbeat":1}"#),
            connection_id: "fake-ws-001",
            direction: TrafficDirection::Response,
            transport: TransportKind::WebSocket,
            message_id: Some(16_001),
            protocol_name: Some("CG_HEARTBEAT_RESPONSE"),
            classification: None,
        },
        FakeEventSpec {
            bytes: b"GET /world/catalog HTTP/1.1\r\n\r\n".to_vec(),
            connection_id: "fake-http-001",
            direction: TrafficDirection::Request,
            transport: TransportKind::Http,
            message_id: None,
            protocol_name: None,
            classification: Some("unparsed_payload"),
        },
        FakeEventSpec {
            bytes: b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n".to_vec(),
            connection_id: "fake-http-001",
            direction: TrafficDirection::Response,
            transport: TransportKind::Http,
            message_id: None,
            protocol_name: None,
            classification: Some("unparsed_payload"),
        },
    ]
}

pub fn run_fake_capture(
    mut writer: SessionWriter,
    session_id: String,
    started_at_ms: u64,
    candidate: CandidateProcess,
    stop_requested: Receiver<()>,
    sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    let segment_id = format!("{session_id}-segment-001");
    let ready_at_ms = wait_or_stop(&stop_requested, Duration::from_millis(35));
    if ready_at_ms {
        return finish_capture(writer, &session_id, started_at_ms, &candidate, sink);
    }

    let ready_timestamp = now_ms();
    writer.append_event(
        ready_timestamp,
        "ready",
        json!({"backend": "fake", "segmentId": segment_id, "candidateId": candidate.id}),
    )?;
    emit(
        &sink,
        CaptureEvent {
            kind: CaptureEventKind::Ready,
            session_id: session_id.clone(),
            timestamp_ms: ready_timestamp,
            data: json!({"backend": "fake", "segmentId": segment_id}),
        },
    );

    let observation_stop = wait_or_stop(&stop_requested, Duration::from_millis(35));
    if observation_stop {
        return finish_capture(writer, &session_id, started_at_ms, &candidate, sink);
    }

    // 假后端固定产生两组请求/响应协议帧和一组 HTTP 未分类明文。事件先写
    // raw_observation 再写 protocol_frame，工作台可据此覆盖消息号、协议名、
    // 方向和未分类筛选；所有原始观察都继续进入会话索引。
    for (index, spec) in fake_workbench_events().into_iter().enumerate() {
        let raw_sequence = index as u64 + 1;
        let observed_at_ms = now_ms();
        let observation_id = format!("{session_id}-observation-{raw_sequence:06}");
        let raw_file = writer.write_raw(raw_sequence, &spec.bytes)?;
        let sha256 = sha256_hex(&spec.bytes);
        let observation = RawObservation {
            observation_id: observation_id.clone(),
            segment_id: segment_id.clone(),
            connection_id: spec.connection_id.to_string(),
            direction: spec.direction.clone(),
            transport: spec.transport.clone(),
            observed_at_ms,
            length: spec.bytes.len(),
            sha256: sha256.clone(),
            raw_file: raw_file.clone(),
        };
        let mut observation_data = serde_json::to_value(&observation)
            .map_err(|error| format!("序列化原始观察失败：{error}"))?;
        if let Some(classification) = spec.classification {
            observation_data["classification"] = json!(classification);
        }
        writer.append_event(observed_at_ms, "raw_observation", observation_data.clone())?;
        writer.append_transport_event(
            observed_at_ms,
            json!({
                "observationId": observation_id,
                "segmentId": segment_id,
                "connectionId": spec.connection_id,
                "direction": spec.direction,
                "transport": spec.transport,
                "length": spec.bytes.len(),
                "sha256": sha256,
                "classification": spec.classification
            }),
        )?;
        writer.add_observation(observation.clone());
        emit(
            &sink,
            CaptureEvent {
                kind: CaptureEventKind::RawObservation,
                session_id: session_id.clone(),
                timestamp_ms: observed_at_ms,
                data: observation_data,
            },
        );

        if let (Some(message_id), Some(protocol_name)) = (spec.message_id, spec.protocol_name) {
            let frame = ProtocolFrame {
                frame_id: format!("{session_id}-frame-{raw_sequence:06}"),
                observation_id,
                segment_id: segment_id.clone(),
                connection_id: spec.connection_id.to_string(),
                direction: spec.direction,
                transport: spec.transport,
                observed_at_ms,
                message_id,
                protocol_name: protocol_name.to_string(),
                length: spec.bytes.len(),
                sha256,
                raw_file,
            };
            let frame_data = serde_json::to_value(&frame)
                .map_err(|error| format!("序列化协议帧失败：{error}"))?;
            writer.append_event(observed_at_ms, "protocol_frame", frame_data.clone())?;
            writer.add_frame(frame);
            emit(
                &sink,
                CaptureEvent {
                    kind: CaptureEventKind::ProtocolFrame,
                    session_id: session_id.clone(),
                    timestamp_ms: observed_at_ms,
                    data: frame_data,
                },
            );
        }
    }

    // 事件已经形成后保持 worker 存活，直到显式 stop_capture。这样停止命令
    // 必须走“通知 → worker 收尾 → summary”路径，而不是依赖强杀进程。
    while !wait_or_stop(&stop_requested, Duration::from_millis(100)) {}
    finish_capture(writer, &session_id, started_at_ms, &candidate, sink)
}

/// 带分段恢复能力的确定性 fake worker。
///
/// 生产后端会把同一组 `RecoveryCommand` 接到目标 PID 监视器；fake worker
/// 通过公开测试/命令注入承载进程退出和候选快照，验证会话编号不变、分段
/// 增长、中断可见以及多候选必须人工选择。它仍然只是测试夹具，不代表真实
/// 微信流量或 issue 03 实机验收。
pub fn run_resilient_fake_capture(
    mut writer: SessionWriter,
    session_id: String,
    started_at_ms: u64,
    candidate: CandidateProcess,
    stop_requested: Receiver<()>,
    recovery_requested: Receiver<RecoveryCommand>,
    sink: Arc<dyn EventSink>,
) -> Result<crate::model::SessionSummary, String> {
    let workspace_root = writer
        .root_path()
        .parent()
        .and_then(|path| path.parent())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| writer.root_path().to_path_buf());
    let mut machine = RecoveryMachine::new(session_id.clone(), started_at_ms, candidate.clone());
    let mut sequence = 0_u64;
    let mut ready = true;
    let mut emitted_segment = String::new();
    let mut last_disk_probe = Instant::now() - Duration::from_secs(10);

    // 保持与 issue 01 fake 后端相同的附加等待语义。
    if wait_or_stop(&stop_requested, Duration::from_millis(35)) {
        machine.stop(now_ms());
        return finish_fake_capture(
            writer,
            &session_id,
            started_at_ms,
            &candidate,
            &machine,
            sink,
            SessionStatus::Completed,
        );
    }
    mark_fake_ready(&mut writer, &mut machine, &session_id, &sink, false)?;

    loop {
        let mut reattached = false;
        while let Ok(command) = recovery_requested.try_recv() {
            match command {
                RecoveryCommand::TargetExited { reason, candidates } => {
                    if let Some(_interruption) = machine.target_exited(now_ms(), reason) {
                        let interrupted_at = now_ms();
                        machine.append_transition(
                            &mut writer,
                            &RecoveryTransition::Interrupted,
                            interrupted_at,
                            &sink,
                        )?;
                        let transition = machine.discover(now_ms(), candidates);
                        machine.append_transition(&mut writer, &transition, now_ms(), &sink)?;
                        if transition == RecoveryTransition::Reattached {
                            machine.mark_reattach_complete(now_ms());
                            machine.mark_ready();
                            mark_fake_ready(&mut writer, &mut machine, &session_id, &sink, true)?;
                            reattached = true;
                        }
                    }
                }
                RecoveryCommand::CandidatesDiscovered(candidates) => {
                    let transition = machine.discover(now_ms(), candidates);
                    machine.append_transition(&mut writer, &transition, now_ms(), &sink)?;
                    if transition == RecoveryTransition::Reattached {
                        machine.mark_reattach_complete(now_ms());
                        machine.mark_ready();
                        mark_fake_ready(&mut writer, &mut machine, &session_id, &sink, true)?;
                        reattached = true;
                    }
                }
                RecoveryCommand::SelectCandidate(candidate_id) => {
                    match machine.select_candidate(now_ms(), &candidate_id) {
                        Ok(transition) => {
                            machine.append_transition(&mut writer, &transition, now_ms(), &sink)?;
                            machine.mark_reattach_complete(now_ms());
                            machine.mark_ready();
                            mark_fake_ready(&mut writer, &mut machine, &session_id, &sink, true)?;
                            reattached = true;
                        }
                        Err(error) => {
                            let transition = machine.attach_failed(error);
                            machine.append_transition(&mut writer, &transition, now_ms(), &sink)?;
                        }
                    }
                }
                RecoveryCommand::AttachFailed(reason) => {
                    let transition = machine.attach_failed(reason);
                    machine.append_transition(&mut writer, &transition, now_ms(), &sink)?;
                }
                RecoveryCommand::DiskStatus(status) => {
                    let transition = machine.record_disk_status(status);
                    machine.append_transition(&mut writer, &transition, now_ms(), &sink)?;
                }
            }
        }

        if last_disk_probe.elapsed() >= Duration::from_secs(5) {
            // 探测失败本身只生成可恢复 warning；不停止、不清理历史会话。
            probe_disk(&mut machine, &mut writer, &sink, &workspace_root)?;
            last_disk_probe = Instant::now();
        }

        if reattached {
            ready = false;
            emitted_segment.clear();
        }
        if ready && emitted_segment != machine.current_segment_id() {
            emit_fake_segment_events(&mut writer, &mut machine, &session_id, &sink, &mut sequence)?;
            emitted_segment = machine.current_segment_id().to_string();
        }
        if !ready && machine.state() == crate::model::ReattachState::Attached {
            ready = true;
        }

        match stop_requested.try_recv() {
            Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                let ended_at = now_ms();
                let interrupted = matches!(
                    machine.state(),
                    crate::model::ReattachState::Interrupted
                        | crate::model::ReattachState::AwaitingCandidate
                        | crate::model::ReattachState::AttachFailed
                );
                if !machine.has_frames() {
                    writer.append_event(
                        ended_at,
                        "zero_frame",
                        json!({
                            "recoverable": true,
                            "message": "会话在停止前没有收到任何协议帧"
                        }),
                    )?;
                    emit(
                        &sink,
                        CaptureEvent {
                            kind: CaptureEventKind::Error,
                            session_id: session_id.clone(),
                            timestamp_ms: ended_at,
                            data: json!({
                                "code": "zero_frames",
                                "state": if interrupted { "interrupted" } else { "attached" },
                                "recoverable": true,
                                "message": "会话在停止前没有收到任何协议帧"
                            }),
                        },
                    );
                }
                machine.stop(ended_at);
                writer
                    .update_lifecycle(&machine.current_candidate().clone(), &machine.lifecycle())?;
                return finish_fake_capture(
                    writer,
                    &session_id,
                    started_at_ms,
                    machine.current_candidate(),
                    &machine,
                    sink,
                    if interrupted {
                        SessionStatus::Incomplete
                    } else {
                        SessionStatus::Completed
                    },
                );
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn mark_fake_ready(
    writer: &mut SessionWriter,
    machine: &mut RecoveryMachine,
    session_id: &str,
    sink: &Arc<dyn EventSink>,
    reattached: bool,
) -> Result<(), String> {
    let timestamp = now_ms();
    machine.mark_ready();
    writer.update_lifecycle(machine.current_candidate(), &machine.lifecycle())?;
    writer.append_event(
        timestamp,
        "ready",
        json!({
            "backend": "fake",
            "segmentId": machine.current_segment_id(),
            "candidateId": machine.current_candidate().id,
            "pid": machine.current_candidate().pid,
            "reattached": reattached
        }),
    )?;
    emit(
        sink,
        CaptureEvent {
            kind: CaptureEventKind::Ready,
            session_id: session_id.to_string(),
            timestamp_ms: timestamp,
            data: json!({
                "backend": "fake",
                "segmentId": machine.current_segment_id(),
                "pid": machine.current_candidate().pid,
                "reattached": reattached,
                "candidate": machine.current_candidate()
            }),
        },
    );
    Ok(())
}

fn emit_fake_segment_events(
    writer: &mut SessionWriter,
    machine: &mut RecoveryMachine,
    session_id: &str,
    sink: &Arc<dyn EventSink>,
    sequence: &mut u64,
) -> Result<(), String> {
    for spec in fake_workbench_events() {
        *sequence = sequence.saturating_add(1);
        let observed_at_ms = now_ms();
        let observation_id = format!("{session_id}-observation-{sequence:06}");
        let raw_file = writer.write_raw(*sequence, &spec.bytes)?;
        let sha256 = sha256_hex(&spec.bytes);
        let observation = RawObservation {
            observation_id: observation_id.clone(),
            segment_id: machine.current_segment_id().to_string(),
            connection_id: spec.connection_id.to_string(),
            direction: spec.direction.clone(),
            transport: spec.transport.clone(),
            observed_at_ms,
            length: spec.bytes.len(),
            sha256: sha256.clone(),
            raw_file: raw_file.clone(),
        };
        let mut observation_data = serde_json::to_value(&observation)
            .map_err(|error| format!("序列化恢复原始观察失败：{error}"))?;
        if let Some(classification) = spec.classification {
            observation_data["classification"] = json!(classification);
        }
        writer.append_event(observed_at_ms, "raw_observation", observation_data.clone())?;
        writer.append_transport_event(
            observed_at_ms,
            json!({
                "backend": "fake",
                "observationId": observation_id,
                "segmentId": machine.current_segment_id(),
                "connectionId": spec.connection_id,
                "direction": spec.direction,
                "transport": spec.transport,
                "length": spec.bytes.len(),
                "sha256": sha256,
                "classification": spec.classification
            }),
        )?;
        writer.add_observation(observation.clone());
        machine.record_observation();
        emit(
            sink,
            CaptureEvent {
                kind: CaptureEventKind::RawObservation,
                session_id: session_id.to_string(),
                timestamp_ms: observed_at_ms,
                data: observation_data,
            },
        );
        if let (Some(message_id), Some(protocol_name)) = (spec.message_id, spec.protocol_name) {
            let frame = ProtocolFrame {
                frame_id: format!("{session_id}-frame-{sequence:06}"),
                observation_id,
                segment_id: machine.current_segment_id().to_string(),
                connection_id: spec.connection_id.to_string(),
                direction: spec.direction,
                transport: spec.transport,
                observed_at_ms,
                message_id,
                protocol_name: protocol_name.to_string(),
                length: spec.bytes.len(),
                sha256,
                raw_file,
            };
            let frame_data = serde_json::to_value(&frame)
                .map_err(|error| format!("序列化恢复协议帧失败：{error}"))?;
            writer.append_event(observed_at_ms, "protocol_frame", frame_data.clone())?;
            writer.add_frame(frame);
            machine.record_frame();
            emit(
                sink,
                CaptureEvent {
                    kind: CaptureEventKind::ProtocolFrame,
                    session_id: session_id.to_string(),
                    timestamp_ms: observed_at_ms,
                    data: frame_data,
                },
            );
        }
    }
    writer.update_lifecycle(machine.current_candidate(), &machine.lifecycle())?;
    Ok(())
}

fn finish_fake_capture(
    writer: SessionWriter,
    session_id: &str,
    started_at_ms: u64,
    candidate: &CandidateProcess,
    machine: &RecoveryMachine,
    sink: Arc<dyn EventSink>,
    status: SessionStatus,
) -> Result<crate::model::SessionSummary, String> {
    let ended_at_ms = now_ms();
    let summary = match status {
        SessionStatus::Completed => {
            writer.finish(session_id, started_at_ms, ended_at_ms, candidate)?
        }
        SessionStatus::Incomplete => {
            writer.finish_incomplete(session_id, started_at_ms, ended_at_ms, candidate)?
        }
        SessionStatus::Running => return Err("fake worker 不允许以 running 状态收尾".to_string()),
    };
    let status_name = match summary.status {
        SessionStatus::Completed => "completed",
        SessionStatus::Incomplete => "incomplete",
        SessionStatus::Running => "running",
    };
    emit(
        &sink,
        CaptureEvent {
            kind: CaptureEventKind::Stopped,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: json!({
                "status": status_name,
                "segmentCount": summary.segments.len(),
                "interruptionCount": summary.interruptions.len(),
                "zeroFrame": summary.zero_frame,
                "state": machine.state()
            }),
        },
    );
    emit(
        &sink,
        CaptureEvent {
            kind: CaptureEventKind::Summary,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化恢复摘要事件失败：{error}"))?,
        },
    );
    Ok(summary)
}

fn finish_capture(
    writer: SessionWriter,
    session_id: &str,
    started_at_ms: u64,
    candidate: &CandidateProcess,
    sink: Arc<dyn EventSink>,
) -> Result<SessionSummary, String> {
    let ended_at_ms = now_ms();
    let summary = writer.finish(session_id, started_at_ms, ended_at_ms, candidate)?;
    emit(
        &sink,
        CaptureEvent {
            kind: CaptureEventKind::Stopped,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化停止事件失败：{error}"))?,
        },
    );
    emit(
        &sink,
        CaptureEvent {
            kind: CaptureEventKind::Summary,
            session_id: session_id.to_string(),
            timestamp_ms: ended_at_ms,
            data: serde_json::to_value(&summary)
                .map_err(|error| format!("序列化摘要事件失败：{error}"))?,
        },
    );
    Ok(summary)
}

fn wait_or_stop(stop_requested: &Receiver<()>, duration: Duration) -> bool {
    match stop_requested.recv_timeout(duration) {
        Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => true,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => false,
    }
}

fn emit(sink: &Arc<dyn EventSink>, event: CaptureEvent) {
    // 磁盘写入是权威证据；窗口销毁后事件发送失败不能阻止 worker 收尾。
    let _ = sink.emit(event);
}
