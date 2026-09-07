//! 采集会话的分段与重新附加状态机。
//!
//! 一个用户操作周期始终保持同一个 `session_id`。承载小游戏的目标进程
//! 消失时，当前分段结束并记录中断；重新发现唯一且兼容的候选后才创建新
//! 分段。多候选或零候选都停留在等待状态，禁止把未知进程自动混入会话。

use crate::disk;
use crate::model::{
    CandidateProcess, CaptureRecoverySnapshot, CaptureSegment, DiskSpaceStatus, InterruptionInfo,
    ReattachState, SegmentStatus, SessionLifecycle,
};
use crate::storage::{now_ms, SessionWriter};
use serde_json::{json, Value};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 真实平台 runner 与恢复状态机之间的控制动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryAction {
    None,
    /// 当前采集子进程/附加实例必须停止，等待候选选择或下一次扫描。
    DetachChild,
    /// 当前候选已唯一确认，runner 应以新 segment 启动附加实例。
    RelaunchSegment,
}

const DISK_PROBE_INTERVAL: Duration = Duration::from_secs(5);

/// 真实 macOS/Windows runner 共用的恢复事件桥。
///
/// runner 仍负责平台子进程、时间线和真实 raw evidence；桥只负责把
/// 进程退出、候选发现、人工选择、ready 和磁盘水位映射到同一个会话状态机。
pub struct RecoveryBridge {
    machine: RecoveryMachine,
    requested: Receiver<RecoveryCommand>,
    last_disk_probe: Instant,
}

impl RecoveryBridge {
    pub fn new(
        session_id: impl Into<String>,
        started_at_ms: u64,
        candidate: CandidateProcess,
        requested: Receiver<RecoveryCommand>,
    ) -> Self {
        Self {
            machine: RecoveryMachine::new(session_id, started_at_ms, candidate),
            requested,
            last_disk_probe: Instant::now() - DISK_PROBE_INTERVAL,
        }
    }

    pub fn machine(&self) -> &RecoveryMachine {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut RecoveryMachine {
        &mut self.machine
    }

    pub fn current_candidate(&self) -> &CandidateProcess {
        self.machine.current_candidate()
    }

    pub fn current_segment_id(&self) -> &str {
        self.machine.current_segment_id()
    }

    pub fn state(&self) -> ReattachState {
        self.machine.state()
    }

    pub fn record_observation(&mut self) {
        self.machine.record_observation();
    }

    pub fn record_frame(&mut self) {
        self.machine.record_frame();
    }

    pub fn has_frames(&self) -> bool {
        self.machine.has_frames()
    }

    /// 处理一轮公开恢复命令。`discover` 必须是只读平台诊断，不得附加
    /// 未确认进程；唯一候选才返回 `RelaunchSegment`。
    pub fn process_commands<F>(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
        mut discover: F,
    ) -> Result<RecoveryAction, String>
    where
        F: FnMut() -> Vec<CandidateProcess>,
    {
        let mut action = RecoveryAction::None;
        while let Ok(command) = self.requested.try_recv() {
            let transition = match command {
                RecoveryCommand::TargetExited { reason, candidates } => self.target_exited(
                    writer,
                    sink,
                    &reason,
                    if candidates.is_empty() {
                        discover()
                    } else {
                        candidates
                    },
                )?,
                RecoveryCommand::CandidatesDiscovered(candidates) => {
                    self.discover(writer, sink, candidates)?
                }
                RecoveryCommand::SelectCandidate(candidate_id) => {
                    match self.machine.select_candidate(now_ms(), &candidate_id) {
                        Ok(transition) => {
                            self.machine
                                .append_transition(writer, &transition, now_ms(), sink)?;
                            transition
                        }
                        Err(error) => {
                            let transition = self.machine.attach_failed(error);
                            self.machine
                                .append_transition(writer, &transition, now_ms(), sink)?;
                            transition
                        }
                    }
                }
                RecoveryCommand::AttachFailed(reason) => {
                    let transition = self.machine.attach_failed(reason);
                    self.machine
                        .append_transition(writer, &transition, now_ms(), sink)?;
                    transition
                }
                RecoveryCommand::DiskStatus(status) => {
                    let transition = self.machine.record_disk_status(status);
                    self.machine
                        .append_transition(writer, &transition, now_ms(), sink)?;
                    transition
                }
            };
            action = merge_action(action, action_for_transition(&transition));
        }
        Ok(action)
    }

    /// 由平台 PID 监视器调用。候选列表来自同一轮只读诊断。
    pub fn target_process_exited(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
        reason: &str,
        candidates: Vec<CandidateProcess>,
    ) -> Result<RecoveryAction, String> {
        let transition = self.target_exited(writer, sink, reason, candidates)?;
        Ok(action_for_transition(&transition))
    }

    /// runner 在没有活动附加实例时定期调用。零候选继续等待，唯一兼容
    /// 候选开始新分段，多候选等待用户明确选择。
    pub fn discover_candidates(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
        candidates: Vec<CandidateProcess>,
    ) -> Result<RecoveryAction, String> {
        let transition = self.discover(writer, sink, candidates)?;
        Ok(action_for_transition(&transition))
    }

    /// 平台 ready 文件确认双边界后调用。只有此时才把 `reattaching`
    /// 转为 `attached`，避免在真实附加失败前提前发布 ready。
    pub fn mark_ready(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
        backend: &str,
        extra: Value,
    ) -> Result<(), String> {
        let timestamp = now_ms();
        let reattached = self.machine.state() == ReattachState::Reattaching;
        if reattached {
            self.machine.mark_reattach_complete(timestamp);
        }
        self.machine.mark_ready();
        writer.update_lifecycle(self.machine.current_candidate(), &self.machine.lifecycle())?;
        let mut data = json!({
            "backend": backend,
            "segmentId": self.machine.current_segment_id(),
            "pid": self.machine.current_candidate().pid,
            "candidate": self.machine.current_candidate(),
            "reattached": reattached
        });
        if let (Some(base), Some(extra)) = (data.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                base.insert(key.clone(), value.clone());
            }
        }
        writer.append_event(timestamp, "ready", data.clone())?;
        let _ = sink.emit(crate::model::CaptureEvent {
            kind: crate::model::CaptureEventKind::Ready,
            session_id: self.machine.session_id().to_string(),
            timestamp_ms: timestamp,
            data,
        });
        Ok(())
    }

    /// 以固定周期探测工作区文件系统。低磁盘只追加 warning，不改变
    /// runner 的停止标志，也不删除任何历史文件。
    pub fn probe_disk_if_due(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
    ) -> Result<(), String> {
        if self.last_disk_probe.elapsed() < DISK_PROBE_INTERVAL {
            return Ok(());
        }
        let workspace = writer
            .root_path()
            .parent()
            .and_then(|path| path.parent())
            .ok_or_else(|| "无法解析采集工作区路径".to_string())?
            .to_path_buf();
        probe_disk(&mut self.machine, writer, sink, &workspace)?;
        self.last_disk_probe = Instant::now();
        Ok(())
    }

    fn target_exited(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
        reason: &str,
        candidates: Vec<CandidateProcess>,
    ) -> Result<RecoveryTransition, String> {
        let Some(_) = self.machine.target_exited(now_ms(), reason) else {
            return Ok(RecoveryTransition::Ignored);
        };
        self.machine
            .append_transition(writer, &RecoveryTransition::Interrupted, now_ms(), sink)?;
        self.discover(writer, sink, candidates)
    }

    fn discover(
        &mut self,
        writer: &mut SessionWriter,
        sink: &Arc<dyn crate::backend::EventSink>,
        candidates: Vec<CandidateProcess>,
    ) -> Result<RecoveryTransition, String> {
        let transition = self.machine.discover(now_ms(), candidates);
        self.machine
            .append_transition(writer, &transition, now_ms(), sink)?;
        Ok(transition)
    }
}

fn action_for_transition(transition: &RecoveryTransition) -> RecoveryAction {
    match transition {
        RecoveryTransition::Reattached => RecoveryAction::RelaunchSegment,
        RecoveryTransition::Interrupted
        | RecoveryTransition::WaitingForCandidate
        | RecoveryTransition::AwaitingCandidateSelection
        | RecoveryTransition::AttachFailed => RecoveryAction::DetachChild,
        RecoveryTransition::Ignored | RecoveryTransition::DiskWarning => RecoveryAction::None,
    }
}

fn merge_action(left: RecoveryAction, right: RecoveryAction) -> RecoveryAction {
    match (left, right) {
        (RecoveryAction::RelaunchSegment, _) | (_, RecoveryAction::RelaunchSegment) => {
            RecoveryAction::RelaunchSegment
        }
        (RecoveryAction::DetachChild, _) | (_, RecoveryAction::DetachChild) => {
            RecoveryAction::DetachChild
        }
        _ => RecoveryAction::None,
    }
}

/// 供 worker 和公开命令之间传递的恢复控制消息。
#[derive(Clone, Debug)]
pub enum RecoveryCommand {
    /// 当前承载进程已经消失。`candidates` 是同一时刻的只读诊断快照；
    /// 恢复控制器会自行判断唯一候选或进入人工选择。
    TargetExited {
        reason: String,
        candidates: Vec<CandidateProcess>,
    },
    /// 重新扫描候选，不改变当前分段直到状态机确认可以附加。
    CandidatesDiscovered(Vec<CandidateProcess>),
    /// 用户在多个候选中明确选择一个候选 ID。
    SelectCandidate(String),
    /// 平台后端报告重新附加失败；保留已有证据并继续等待下一次扫描。
    AttachFailed(String),
    /// 测试和平台适配器可注入固定的低磁盘状态；生产路径使用探测器。
    DiskStatus(DiskSpaceStatus),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryTransition {
    Ignored,
    Interrupted,
    AwaitingCandidateSelection,
    WaitingForCandidate,
    Reattached,
    AttachFailed,
    DiskWarning,
}

/// 会话恢复控制器。它不直接拥有线程或进程，只管理可审计的生命周期。
#[derive(Clone, Debug)]
pub struct RecoveryMachine {
    session_id: String,
    current_candidate: CandidateProcess,
    current_segment: CaptureSegment,
    segments: Vec<CaptureSegment>,
    interruptions: Vec<InterruptionInfo>,
    disk_warnings: Vec<DiskSpaceStatus>,
    pending_candidates: Vec<CandidateProcess>,
    state: ReattachState,
    interruption_started_at_ms: Option<u64>,
    last_error: Option<String>,
    frame_count: usize,
    observation_count: usize,
    next_segment_number: u32,
}

impl RecoveryMachine {
    pub fn new(
        session_id: impl Into<String>,
        started_at_ms: u64,
        candidate: CandidateProcess,
    ) -> Self {
        let session_id = session_id.into();
        let segment = CaptureSegment {
            segment_id: format!("{session_id}-segment-001"),
            candidate: candidate.clone(),
            started_at_ms,
            ended_at_ms: None,
            status: SegmentStatus::Starting,
        };
        Self {
            session_id,
            current_candidate: candidate,
            current_segment: segment.clone(),
            segments: vec![segment],
            interruptions: Vec::new(),
            disk_warnings: Vec::new(),
            pending_candidates: Vec::new(),
            state: ReattachState::Attaching,
            interruption_started_at_ms: None,
            last_error: None,
            frame_count: 0,
            observation_count: 0,
            next_segment_number: 1,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn current_segment_id(&self) -> &str {
        &self.current_segment.segment_id
    }

    pub fn current_candidate(&self) -> &CandidateProcess {
        &self.current_candidate
    }

    pub fn state(&self) -> ReattachState {
        self.state.clone()
    }

    pub fn lifecycle(&self) -> SessionLifecycle {
        SessionLifecycle {
            segments: self.segments.clone(),
            interruptions: self.interruptions.clone(),
            disk_warnings: self.disk_warnings.clone(),
            zero_frame: self.frame_count == 0,
        }
    }

    pub fn snapshot(&self) -> CaptureRecoverySnapshot {
        CaptureRecoverySnapshot {
            state: Some(self.state.clone()),
            session_id: Some(self.session_id.clone()),
            current_segment_id: Some(self.current_segment.segment_id.clone()),
            current_pid: Some(self.current_candidate.pid),
            segments: self.segments.clone(),
            interruptions: self.interruptions.clone(),
            candidates: self.pending_candidates.clone(),
            interruption_started_at_ms: self.interruption_started_at_ms,
            last_error: self.last_error.clone(),
            zero_frame: self.frame_count == 0,
            disk: self.disk_warnings.last().cloned(),
        }
    }

    pub fn mark_ready(&mut self) {
        self.current_segment.status = SegmentStatus::Attached;
        if let Some(segment) = self
            .segments
            .iter_mut()
            .find(|segment| segment.segment_id == self.current_segment.segment_id)
        {
            segment.status = SegmentStatus::Attached;
        }
        self.state = ReattachState::Attached;
    }

    pub fn record_observation(&mut self) {
        self.observation_count = self.observation_count.saturating_add(1);
    }

    pub fn record_frame(&mut self) {
        self.frame_count = self.frame_count.saturating_add(1);
    }

    pub fn has_frames(&self) -> bool {
        self.frame_count > 0
    }

    pub fn has_observations(&self) -> bool {
        self.observation_count > 0
    }

    /// 记录承载进程消失。只有当前确实处于附加状态时才会生成中断。
    pub fn target_exited(
        &mut self,
        at_ms: u64,
        reason: impl Into<String>,
    ) -> Option<InterruptionInfo> {
        if !matches!(
            self.state,
            ReattachState::Attached | ReattachState::Attaching | ReattachState::Reattaching
        ) {
            return None;
        }
        let reason = reason.into();
        let interruption = InterruptionInfo {
            interruption_id: format!(
                "{}-interruption-{:04}",
                self.session_id,
                self.interruptions.len() + 1
            ),
            segment_id: self.current_segment.segment_id.clone(),
            started_at_ms: at_ms,
            ended_at_ms: None,
            reason: reason.clone(),
            previous_pid: self.current_candidate.pid,
        };
        self.current_segment.ended_at_ms = Some(at_ms);
        self.current_segment.status = SegmentStatus::Interrupted;
        if let Some(segment) = self
            .segments
            .iter_mut()
            .find(|segment| segment.segment_id == self.current_segment.segment_id)
        {
            segment.ended_at_ms = Some(at_ms);
            segment.status = SegmentStatus::Interrupted;
        }
        self.interruptions.push(interruption.clone());
        self.interruption_started_at_ms = Some(at_ms);
        self.pending_candidates.clear();
        self.last_error = Some(reason);
        self.state = ReattachState::Interrupted;
        Some(interruption)
    }

    /// 对新候选执行严格身份匹配。候选 ID 和 PID 可以变化，但游戏、角色、
    /// 微信版本、架构、后端及关键模块必须与原运行链相容。
    pub fn compatible_candidate(&self, candidate: &CandidateProcess) -> bool {
        candidate.game == self.current_candidate.game
            && compatible_process_role(
                &candidate.process_role,
                &self.current_candidate.process_role,
            )
            && candidate.wechat_version == self.current_candidate.wechat_version
            && candidate.architecture == self.current_candidate.architecture
            && candidate.backend == self.current_candidate.backend
            && self
                .current_candidate
                .modules
                .iter()
                .all(|module| candidate.modules.iter().any(|item| item == module))
    }

    pub fn discover(
        &mut self,
        at_ms: u64,
        candidates: Vec<CandidateProcess>,
    ) -> RecoveryTransition {
        if !matches!(
            self.state,
            ReattachState::Interrupted
                | ReattachState::AwaitingCandidate
                | ReattachState::AttachFailed
        ) {
            return RecoveryTransition::Ignored;
        }
        self.pending_candidates = candidates
            .into_iter()
            .filter(|candidate| self.compatible_candidate(candidate))
            .collect();
        match self.pending_candidates.len() {
            0 => {
                self.state = ReattachState::Interrupted;
                RecoveryTransition::WaitingForCandidate
            }
            1 => {
                let candidate = self.pending_candidates[0].clone();
                self.begin_reattach(at_ms, candidate);
                RecoveryTransition::Reattached
            }
            _ => {
                self.state = ReattachState::AwaitingCandidate;
                RecoveryTransition::AwaitingCandidateSelection
            }
        }
    }

    pub fn select_candidate(
        &mut self,
        at_ms: u64,
        candidate_id: &str,
    ) -> Result<RecoveryTransition, String> {
        if self.state != ReattachState::AwaitingCandidate {
            return Err("当前没有等待用户选择的重新附加候选".to_string());
        }
        let candidate = self
            .pending_candidates
            .iter()
            .find(|candidate| candidate.id == candidate_id)
            .cloned()
            .ok_or_else(|| format!("候选进程不在当前重新附加列表中：{candidate_id}"))?;
        self.begin_reattach(at_ms, candidate);
        Ok(RecoveryTransition::Reattached)
    }

    pub fn attach_failed(&mut self, reason: impl Into<String>) -> RecoveryTransition {
        let reason = reason.into();
        let ended_at_ms = now_ms();
        if self.current_segment.ended_at_ms.is_none() {
            self.current_segment.ended_at_ms = Some(ended_at_ms);
            self.current_segment.status = SegmentStatus::Failed;
            if let Some(segment) = self
                .segments
                .iter_mut()
                .find(|segment| segment.segment_id == self.current_segment.segment_id)
            {
                segment.ended_at_ms = Some(ended_at_ms);
                segment.status = SegmentStatus::Failed;
            }
        }
        self.last_error = Some(reason);
        self.state = ReattachState::AttachFailed;
        RecoveryTransition::AttachFailed
    }

    pub fn record_disk_status(&mut self, status: DiskSpaceStatus) -> RecoveryTransition {
        if !status.warning {
            return RecoveryTransition::Ignored;
        }
        let changed = self
            .disk_warnings
            .last()
            .map(|previous| previous.free_bytes != status.free_bytes)
            .unwrap_or(true);
        self.disk_warnings.push(status);
        if changed {
            RecoveryTransition::DiskWarning
        } else {
            RecoveryTransition::Ignored
        }
    }

    pub fn stop(&mut self, at_ms: u64) {
        if self.current_segment.ended_at_ms.is_none() {
            self.current_segment.ended_at_ms = Some(at_ms);
            self.current_segment.status = SegmentStatus::Completed;
            if let Some(segment) = self
                .segments
                .iter_mut()
                .find(|segment| segment.segment_id == self.current_segment.segment_id)
            {
                segment.ended_at_ms = Some(at_ms);
                segment.status = SegmentStatus::Completed;
            }
        }
        if let Some(interruption) = self.interruptions.last_mut() {
            if interruption.ended_at_ms.is_none() {
                interruption.ended_at_ms = Some(at_ms);
            }
        }
        self.state = ReattachState::Stopped;
    }

    /// 将生命周期变更同时写入权威时间线、manifest 和事件流。
    pub fn append_transition(
        &self,
        writer: &mut SessionWriter,
        transition: &RecoveryTransition,
        at_ms: u64,
        sink: &Arc<dyn crate::backend::EventSink>,
    ) -> Result<(), String> {
        writer.update_lifecycle(&self.current_candidate, &self.lifecycle())?;
        match transition {
            RecoveryTransition::Interrupted => {
                let interruption = self
                    .interruptions
                    .last()
                    .ok_or_else(|| "恢复状态缺少中断记录".to_string())?;
                writer.append_event(
                    at_ms,
                    "segment_interrupted",
                    serde_json::to_value(interruption)
                        .map_err(|error| format!("序列化中断记录失败：{error}"))?,
                )?;
                emit_recoverable(
                    sink,
                    &self.session_id,
                    at_ms,
                    json!({
                        "code": "target_process_disappeared",
                        "state": "interrupted",
                        "recoverable": true,
                        "segmentId": interruption.segment_id,
                        "pid": interruption.previous_pid,
                        "reason": interruption.reason,
                        "interruptionStartedAtMs": interruption.started_at_ms,
                        "interruptionCount": self.interruptions.len(),
                        "interruption": interruption
                    }),
                );
            }
            RecoveryTransition::AwaitingCandidateSelection => {
                writer.append_event(
                    at_ms,
                    "reattach_waiting",
                    json!({
                        "state": "awaiting_candidate",
                        "candidates": self.pending_candidates,
                        "interruptionStartedAtMs": self.interruption_started_at_ms
                    }),
                )?;
                emit_recoverable(
                    sink,
                    &self.session_id,
                    at_ms,
                    json!({
                        "code": "multiple_candidates",
                        "state": "awaiting_candidate",
                        "recoverable": true,
                        "candidates": self.pending_candidates,
                        "message": "重新附加发现多个候选，请明确选择运行链"
                    }),
                );
            }
            RecoveryTransition::WaitingForCandidate => {
                writer.append_event(
                    at_ms,
                    "reattach_waiting",
                    json!({
                        "state": "interrupted",
                        "candidates": [],
                        "interruptionStartedAtMs": self.interruption_started_at_ms
                    }),
                )?;
                emit_recoverable(
                    sink,
                    &self.session_id,
                    at_ms,
                    json!({
                        "code": "candidate_not_found",
                        "state": "interrupted",
                        "recoverable": true,
                        "message": "暂未发现可验证的新承载进程，采集已暂停"
                    }),
                );
            }
            RecoveryTransition::Reattached => {
                writer.append_event(
                    at_ms,
                    "segment_started",
                    json!({
                        "segmentId": self.current_segment.segment_id,
                        "candidate": self.current_candidate,
                        "reattached": true,
                        "interruptionStartedAtMs": self.interruption_started_at_ms,
                        "interruptionEndedAtMs": at_ms
                    }),
                )?;
                emit_recoverable(
                    sink,
                    &self.session_id,
                    at_ms,
                    json!({
                        "code": "candidate_selected",
                        "state": "reattaching",
                        "recoverable": true,
                        "segmentId": self.current_segment.segment_id,
                        "candidate": self.current_candidate,
                        "interruptionStartedAtMs": self.interruption_started_at_ms,
                        "message": "已选择重新附加候选，正在等待采集边界就绪"
                    }),
                );
            }
            RecoveryTransition::AttachFailed => {
                writer.append_event(
                    at_ms,
                    "reattach_failed",
                    json!({
                        "state": "attach_failed",
                        "recoverable": true,
                        "error": self.last_error
                    }),
                )?;
                emit_recoverable(
                    sink,
                    &self.session_id,
                    at_ms,
                    json!({
                        "code": "reattach_failed",
                        "state": "attach_failed",
                        "recoverable": true,
                        "message": self.last_error
                    }),
                );
            }
            RecoveryTransition::DiskWarning => {
                if let Some(status) = self.disk_warnings.last() {
                    writer.append_event(
                        at_ms,
                        "disk_warning",
                        serde_json::to_value(status)
                            .map_err(|error| format!("序列化磁盘水位警告失败：{error}"))?,
                    )?;
                    emit_recoverable(
                        sink,
                        &self.session_id,
                        at_ms,
                        json!({
                            "code": "disk_space_warning",
                            "state": "attached",
                            "recoverable": true,
                            "disk": status,
                            "message": "采集工作区剩余空间低于安全水位；采集不会自动停止"
                        }),
                    );
                }
            }
            RecoveryTransition::Ignored => {}
        }
        Ok(())
    }

    /// 完成一次重新附加后关闭上一条中断区间。
    pub fn mark_reattach_complete(&mut self, at_ms: u64) {
        if let Some(interruption) = self.interruptions.last_mut() {
            if interruption.ended_at_ms.is_none() {
                interruption.ended_at_ms = Some(at_ms);
            }
        }
        self.interruption_started_at_ms = None;
        self.pending_candidates.clear();
        self.last_error = None;
    }

    fn begin_reattach(&mut self, at_ms: u64, candidate: CandidateProcess) {
        self.next_segment_number = self.next_segment_number.saturating_add(1);
        self.current_candidate = candidate.clone();
        let segment = CaptureSegment {
            segment_id: format!(
                "{}-segment-{number:03}",
                self.session_id,
                number = self.next_segment_number
            ),
            candidate,
            started_at_ms: at_ms,
            ended_at_ms: None,
            status: SegmentStatus::Starting,
        };
        self.current_segment = segment.clone();
        self.segments.push(segment);
        self.state = ReattachState::Reattaching;
    }
}

fn emit_recoverable(
    sink: &Arc<dyn crate::backend::EventSink>,
    session_id: &str,
    timestamp_ms: u64,
    data: Value,
) {
    let _ = sink.emit(crate::model::CaptureEvent {
        kind: crate::model::CaptureEventKind::Error,
        session_id: session_id.to_string(),
        timestamp_ms,
        data,
    });
}

/// macOS/Windows 的微信承载链可能在重启时从 WeApp 切换到
/// WeChatAppEx（或反向切换）。二者都属于诊断层允许的目标角色；除此之外
/// 不自动扩大角色边界，避免把 helper 或主进程混入同一会话。
fn compatible_process_role(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    matches!(left, "WeApp" | "WeChatAppEx") && matches!(right, "WeApp" | "WeChatAppEx")
}

/// 生产 worker 使用的磁盘探测辅助函数。探测失败不会终止采集，因为磁盘
/// 水位只是风险提示；失败本身通过 recoverable warning 保留在时间线中。
pub fn probe_disk(
    machine: &mut RecoveryMachine,
    writer: &mut SessionWriter,
    sink: &Arc<dyn crate::backend::EventSink>,
    workspace_path: &std::path::Path,
) -> Result<(), String> {
    match disk::inspect(workspace_path, now_ms()) {
        Ok(status) if status.warning => {
            let transition = machine.record_disk_status(status);
            machine.append_transition(writer, &transition, now_ms(), sink)
        }
        Ok(_) => Ok(()),
        Err(reason) => {
            let _ = sink.emit(crate::model::CaptureEvent {
                kind: crate::model::CaptureEventKind::Error,
                session_id: machine.session_id().to_string(),
                timestamp_ms: now_ms(),
                data: json!({
                    "code": "disk_space_probe_failed",
                    "state": machine.state(),
                    "recoverable": true,
                    "message": reason
                }),
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CandidateProcess, ReattachState, SegmentStatus};

    fn candidate(id: &str, pid: u32) -> CandidateProcess {
        CandidateProcess {
            id: id.to_string(),
            pid,
            display_name: format!("WeApp {pid}"),
            process_role: "WeApp".to_string(),
            game: "世界 Online".to_string(),
            wechat_version: "4.1.13".to_string(),
            architecture: "arm64".to_string(),
            modules: vec![
                "WeChatAppEx Framework".to_string(),
                "flue.dylib".to_string(),
            ],
            backend: "fake".to_string(),
        }
    }

    #[test]
    fn process_restart_keeps_session_and_creates_new_segment_for_unique_candidate() {
        let first = candidate("first", 1);
        let mut machine = RecoveryMachine::new("session-1", 10, first);
        machine.mark_ready();
        let interruption = machine
            .target_exited(20, "承载进程退出")
            .expect("应记录中断");
        assert_eq!(interruption.previous_pid, 1);
        assert_eq!(machine.state(), ReattachState::Interrupted);
        assert_eq!(machine.session_id(), "session-1");
        assert_eq!(
            machine.discover(21, vec![candidate("second", 2)]),
            RecoveryTransition::Reattached
        );
        machine.mark_reattach_complete(22);
        machine.mark_ready();
        let snapshot = machine.snapshot();
        assert_eq!(snapshot.segments.len(), 2);
        assert_eq!(snapshot.segments[0].status, SegmentStatus::Interrupted);
        assert_eq!(snapshot.segments[1].candidate.pid, 2);
        assert_eq!(snapshot.interruptions.len(), 1);
        assert_eq!(snapshot.session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn multiple_candidates_require_explicit_selection() {
        let first = candidate("first", 1);
        let mut machine = RecoveryMachine::new("session-1", 10, first);
        machine.mark_ready();
        machine.target_exited(20, "退出");
        let candidates = vec![candidate("second-a", 2), candidate("second-b", 3)];
        assert_eq!(
            machine.discover(21, candidates),
            RecoveryTransition::AwaitingCandidateSelection
        );
        assert_eq!(machine.snapshot().segments.len(), 1);
        assert!(machine.select_candidate(22, "unknown").is_err());
        assert_eq!(
            machine.select_candidate(23, "second-b").expect("选择候选"),
            RecoveryTransition::Reattached
        );
        assert_eq!(machine.snapshot().segments.len(), 2);
    }

    #[test]
    fn incompatible_candidate_is_not_auto_attached() {
        let first = candidate("first", 1);
        let mut machine = RecoveryMachine::new("session-1", 10, first);
        machine.mark_ready();
        machine.target_exited(20, "退出");
        let mut wrong = candidate("wrong", 2);
        wrong.game = "其他小游戏".to_string();
        assert_eq!(
            machine.discover(21, vec![wrong]),
            RecoveryTransition::WaitingForCandidate
        );
        assert_eq!(machine.snapshot().segments.len(), 1);
    }

    #[test]
    fn low_disk_warning_is_recorded_without_stopping_capture() {
        let mut machine = RecoveryMachine::new("session-1", 10, candidate("first", 1));
        machine.mark_ready();
        let status = DiskSpaceStatus {
            path: "/tmp/minitrace".to_string(),
            free_bytes: 10,
            total_bytes: 100,
            warning: true,
            warning_ratio_percent: 10,
            warning_free_bytes: 512 * 1024 * 1024,
            checked_at_ms: 20,
        };
        assert_eq!(
            machine.record_disk_status(status.clone()),
            RecoveryTransition::DiskWarning
        );
        assert_eq!(machine.state(), ReattachState::Attached);
        assert_eq!(machine.snapshot().disk, Some(status.clone()));
        assert_eq!(
            machine.record_disk_status(status),
            RecoveryTransition::Ignored,
            "重复水位只持续保留警告，不应改变采集状态"
        );
        assert_eq!(machine.state(), ReattachState::Attached);
    }
}
