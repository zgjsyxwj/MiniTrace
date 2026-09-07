//! 采集工作区和会话文件存储。
//!
//! 原始字节与追加式时间线是权威数据。`index.sqlite` 只保存从时间线提取的查询
//! 结构，删除后可通过 `rebuild_index` 重新生成。

use crate::model::{
    CandidateProcess, CaptureSegment, ProtocolFrame, RawObservation, SegmentStatus, SessionFiles,
    SessionIndex, SessionLifecycle, SessionManifest, SessionStatus, SessionSummary,
    TrafficDirection, TransportKind,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SESSIONS_DIR: &str = "sessions";
pub const TIMELINE_FILE: &str = "timeline.jsonl";
pub const TRANSPORT_EVENTS_FILE: &str = "transport-events.jsonl";
pub const INDEX_FILE: &str = "index.sqlite";
pub const SUMMARY_FILE: &str = "summary.json";
pub const MANIFEST_FILE: &str = "session.json";
pub const RAW_DIR: &str = "raw";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub workspace_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineEntry {
    sequence: u64,
    timestamp_ms: u64,
    kind: String,
    data: Value,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn prepare_workspace(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("采集工作区必须使用绝对路径".to_string());
    }
    if path.exists() && !path.is_dir() {
        return Err(format!("采集工作区不是目录：{}", path.display()));
    }
    fs::create_dir_all(path).map_err(|error| format!("创建采集工作区失败：{error}"))?;
    fs::create_dir_all(path.join(SESSIONS_DIR))
        .map_err(|error| format!("创建会话目录失败：{error}"))
}

pub fn workspace_info(path: &Path) -> crate::model::WorkspaceInfo {
    crate::model::WorkspaceInfo {
        path: path.to_string_lossy().into_owned(),
        sessions_path: path.join(SESSIONS_DIR).to_string_lossy().into_owned(),
    }
}

pub fn session_dir(workspace: &Path, session_id: &str) -> Result<PathBuf, String> {
    if !workspace.is_absolute() {
        return Err("采集工作区必须使用绝对路径".to_string());
    }
    if session_id.is_empty()
        || session_id.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
    {
        return Err("非法采集会话编号".to_string());
    }

    Ok(workspace.join(SESSIONS_DIR).join(session_id))
}

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("文件没有父目录：{}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("创建文件目录失败：{error}"))?;

    let temporary = path.with_extension(format!(
        "{}tmp",
        path.extension().and_then(|ext| ext.to_str()).unwrap_or("")
    ));
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("序列化 JSON 失败：{error}"))?;
    {
        let mut file =
            File::create(&temporary).map_err(|error| format!("创建临时文件失败：{error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("写入临时文件失败：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步临时文件失败：{error}"))?;
    }
    fs::rename(&temporary, path).map_err(|error| format!("替换 JSON 文件失败：{error}"))
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("解析 {} 失败：{error}", path.display()))
}

fn write_index_sqlite(path: &Path, index: &SessionIndex) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("索引没有父目录：{}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("创建索引目录失败：{error}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("索引文件名无效：{}", path.display()))?;
    let temporary = parent.join(format!(".{file_name}.tmp"));
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|error| format!("清理索引临时文件失败：{error}"))?;
    }

    {
        let mut connection = Connection::open(&temporary)
            .map_err(|error| format!("创建 SQLite 索引失败：{error}"))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
                 PRAGMA synchronous = FULL;
                 PRAGMA user_version = 1;
                 CREATE TABLE metadata (
                   key TEXT PRIMARY KEY NOT NULL,
                   value TEXT NOT NULL
                 );
                 CREATE TABLE observations (
                   observation_id TEXT PRIMARY KEY NOT NULL,
                   segment_id TEXT NOT NULL,
                   connection_id TEXT NOT NULL,
                   direction TEXT NOT NULL,
                   transport TEXT NOT NULL,
                   observed_at_ms INTEGER NOT NULL,
                   length INTEGER NOT NULL,
                   sha256 TEXT NOT NULL,
                   raw_file TEXT NOT NULL
                 );
                 CREATE TABLE frames (
                   frame_id TEXT PRIMARY KEY NOT NULL,
                   observation_id TEXT NOT NULL,
                   segment_id TEXT NOT NULL,
                   connection_id TEXT NOT NULL,
                   direction TEXT NOT NULL,
                   transport TEXT NOT NULL,
                   observed_at_ms INTEGER NOT NULL,
                   message_id INTEGER NOT NULL,
                   protocol_name TEXT NOT NULL,
                   length INTEGER NOT NULL,
                   sha256 TEXT NOT NULL,
                   raw_file TEXT NOT NULL
                 );
                 CREATE INDEX observations_by_time ON observations(observed_at_ms);
                 CREATE INDEX frames_by_time ON frames(observed_at_ms);
                 CREATE INDEX frames_by_message_id ON frames(message_id);",
            )
            .map_err(|error| format!("初始化 SQLite 索引失败：{error}"))?;

        let transaction = connection
            .transaction()
            .map_err(|error| format!("开启索引事务失败：{error}"))?;
        transaction
            .execute(
                "INSERT INTO metadata(key, value) VALUES ('sessionId', ?1), ('generatedAtMs', ?2)",
                params![index.session_id, index.generated_at_ms.to_string()],
            )
            .map_err(|error| format!("写入索引元数据失败：{error}"))?;
        for observation in &index.observations {
            transaction
                .execute(
                    "INSERT INTO observations(
                       observation_id, segment_id, connection_id, direction, transport,
                       observed_at_ms, length, sha256, raw_file
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        observation.observation_id,
                        observation.segment_id,
                        observation.connection_id,
                        direction_name(&observation.direction),
                        transport_name(&observation.transport),
                        observation.observed_at_ms,
                        observation.length as i64,
                        observation.sha256,
                        observation.raw_file,
                    ],
                )
                .map_err(|error| format!("写入原始观察索引失败：{error}"))?;
        }
        for frame in &index.frames {
            transaction
                .execute(
                    "INSERT INTO frames(
                       frame_id, observation_id, segment_id, connection_id, direction, transport,
                       observed_at_ms, message_id, protocol_name, length, sha256, raw_file
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        frame.frame_id,
                        frame.observation_id,
                        frame.segment_id,
                        frame.connection_id,
                        direction_name(&frame.direction),
                        transport_name(&frame.transport),
                        frame.observed_at_ms,
                        frame.message_id,
                        frame.protocol_name,
                        frame.length as i64,
                        frame.sha256,
                        frame.raw_file,
                    ],
                )
                .map_err(|error| format!("写入协议帧索引失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交索引事务失败：{error}"))?;
    }

    File::open(&temporary)
        .map_err(|error| format!("打开最终索引失败：{error}"))?
        .sync_all()
        .map_err(|error| format!("同步最终索引失败：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("替换 SQLite 索引失败：{error}"))
}

fn read_index_sqlite(path: &Path) -> Result<SessionIndex, String> {
    let connection =
        Connection::open(path).map_err(|error| format!("打开 SQLite 索引失败：{error}"))?;
    let session_id: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'sessionId'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取索引会话编号失败：{error}"))?;
    let generated_at_ms = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'generatedAtMs'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("读取索引生成时间失败：{error}"))?
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(now_ms);

    let mut observation_statement = connection
        .prepare(
            "SELECT observation_id, segment_id, connection_id, direction, transport,
                    observed_at_ms, length, sha256, raw_file
             FROM observations ORDER BY observed_at_ms, rowid",
        )
        .map_err(|error| format!("准备原始观察查询失败：{error}"))?;
    let observation_rows = observation_statement
        .query_map([], |row| {
            let direction: String = row.get(3)?;
            let transport: String = row.get(4)?;
            Ok(RawObservation {
                observation_id: row.get(0)?,
                segment_id: row.get(1)?,
                connection_id: row.get(2)?,
                direction: parse_direction(&direction)?,
                transport: parse_transport(&transport)?,
                observed_at_ms: row.get(5)?,
                length: read_usize(row.get::<_, i64>(6)?)?,
                sha256: row.get(7)?,
                raw_file: row.get(8)?,
            })
        })
        .map_err(|error| format!("查询原始观察索引失败：{error}"))?;
    let observations = observation_rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("读取原始观察索引失败：{error}"))?;
    drop(observation_statement);

    let mut frame_statement = connection
        .prepare(
            "SELECT frame_id, observation_id, segment_id, connection_id, direction, transport,
                    observed_at_ms, message_id, protocol_name, length, sha256, raw_file
             FROM frames ORDER BY observed_at_ms, rowid",
        )
        .map_err(|error| format!("准备协议帧查询失败：{error}"))?;
    let frame_rows = frame_statement
        .query_map([], |row| {
            let direction: String = row.get(4)?;
            let transport: String = row.get(5)?;
            Ok(ProtocolFrame {
                frame_id: row.get(0)?,
                observation_id: row.get(1)?,
                segment_id: row.get(2)?,
                connection_id: row.get(3)?,
                direction: parse_direction(&direction)?,
                transport: parse_transport(&transport)?,
                observed_at_ms: row.get(6)?,
                message_id: row.get(7)?,
                protocol_name: row.get(8)?,
                length: read_usize(row.get::<_, i64>(9)?)?,
                sha256: row.get(10)?,
                raw_file: row.get(11)?,
            })
        })
        .map_err(|error| format!("查询协议帧索引失败：{error}"))?;
    let frames = frame_rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("读取协议帧索引失败：{error}"))?;

    Ok(SessionIndex {
        session_id,
        observations,
        frames,
        generated_at_ms,
    })
}

fn direction_name(direction: &TrafficDirection) -> &'static str {
    match direction {
        TrafficDirection::Request => "request",
        TrafficDirection::Response => "response",
    }
}

fn parse_direction(direction: &str) -> rusqlite::Result<TrafficDirection> {
    match direction {
        "request" => Ok(TrafficDirection::Request),
        "response" => Ok(TrafficDirection::Response),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn transport_name(transport: &TransportKind) -> &'static str {
    match transport {
        TransportKind::WebSocket => "websocket",
        TransportKind::Http => "http",
    }
}

fn parse_transport(transport: &str) -> rusqlite::Result<TransportKind> {
    match transport {
        "websocket" => Ok(TransportKind::WebSocket),
        "http" => Ok(TransportKind::Http),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn read_usize(value: i64) -> rusqlite::Result<usize> {
    usize::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

pub struct SessionWriter {
    root: PathBuf,
    timeline: File,
    transport_events: File,
    next_timeline_sequence: u64,
    observations: Vec<RawObservation>,
    frames: Vec<ProtocolFrame>,
    started_at_ms: u64,
    candidate: CandidateProcess,
    lifecycle: SessionLifecycle,
}

impl SessionWriter {
    pub fn create(
        workspace: &Path,
        session_id: &str,
        started_at_ms: u64,
        candidate: &CandidateProcess,
    ) -> Result<Self, String> {
        prepare_workspace(workspace)?;
        let root = session_dir(workspace, session_id)?;
        fs::create_dir_all(root.join(RAW_DIR))
            .map_err(|error| format!("创建原始文件目录失败：{error}"))?;

        let timeline = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(TIMELINE_FILE))
            .map_err(|error| format!("打开时间线失败：{error}"))?;
        let transport_events = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(TRANSPORT_EVENTS_FILE))
            .map_err(|error| format!("打开传输事件文件失败：{error}"))?;

        let files = session_files(&root);
        let lifecycle = SessionLifecycle {
            segments: vec![CaptureSegment {
                segment_id: format!("{session_id}-segment-001"),
                candidate: candidate.clone(),
                started_at_ms,
                ended_at_ms: None,
                status: SegmentStatus::Starting,
            }],
            ..SessionLifecycle::default()
        };
        let manifest = SessionManifest {
            session_id: session_id.to_string(),
            status: SessionStatus::Running,
            started_at_ms,
            ended_at_ms: None,
            candidate: candidate.clone(),
            files,
            lifecycle: lifecycle.clone(),
        };
        write_json_atomic(&root.join(MANIFEST_FILE), &manifest)?;

        let mut writer = Self {
            root,
            timeline,
            transport_events,
            next_timeline_sequence: 0,
            observations: Vec::new(),
            frames: Vec::new(),
            started_at_ms,
            candidate: candidate.clone(),
            lifecycle,
        };
        writer.append_event(
            started_at_ms,
            "started",
            json!({
                "sessionId": session_id,
                "candidateId": candidate.id,
                "segmentId": format!("{session_id}-segment-001")
            }),
        )?;
        Ok(writer)
    }

    /// 从仍处于 `running` 状态的会话重新打开追加文件。
    ///
    /// 该入口主要用于故障诊断和平台后端重新附加。应用启动时不会继续
    /// 一个未知 worker，而是由 [`recover_incomplete_sessions`] 将它安全地
    /// 收尾为 `incomplete`；只有明确的后端恢复流程才应调用本方法。
    pub fn resume(workspace: &Path, session_id: &str) -> Result<Self, String> {
        prepare_workspace(workspace)?;
        let root = session_dir(workspace, session_id)?;
        if !root.is_dir() {
            return Err(format!("找不到采集会话目录：{}", root.display()));
        }
        let manifest: SessionManifest = read_json(&root.join(MANIFEST_FILE))?;
        if manifest.status != SessionStatus::Running {
            return Err(format!(
                "会话 {} 当前不是 running 状态，不能继续追加",
                session_id
            ));
        }
        let (next_timeline_sequence, timeline_observations, timeline_frames) =
            read_timeline_records(&root.join(TIMELINE_FILE))?;
        let timeline = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(TIMELINE_FILE))
            .map_err(|error| format!("重新打开时间线失败：{error}"))?;
        let transport_events = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(TRANSPORT_EVENTS_FILE))
            .map_err(|error| format!("重新打开传输事件文件失败：{error}"))?;
        let writer = Self {
            root,
            timeline,
            transport_events,
            next_timeline_sequence,
            observations: timeline_observations,
            frames: timeline_frames,
            started_at_ms: manifest.started_at_ms,
            candidate: manifest.candidate.clone(),
            lifecycle: manifest.lifecycle,
        };
        writer.persist_manifest(SessionStatus::Running, None)?;
        Ok(writer)
    }

    pub fn append_event(
        &mut self,
        timestamp_ms: u64,
        kind: &str,
        data: Value,
    ) -> Result<(), String> {
        self.next_timeline_sequence += 1;
        let entry = TimelineEntry {
            sequence: self.next_timeline_sequence,
            timestamp_ms,
            kind: kind.to_string(),
            data,
        };
        write_json_line(&mut self.timeline, &entry)?;
        self.timeline
            .sync_data()
            .map_err(|error| format!("同步时间线失败：{error}"))
    }

    /// 把最新生命周期快照写入 manifest。时间线中的生命周期事件先写权威
    /// 记录，再通过原子 manifest 更新，让异常退出时至少有一份可恢复状态。
    pub fn update_lifecycle(
        &mut self,
        candidate: &CandidateProcess,
        lifecycle: &SessionLifecycle,
    ) -> Result<(), String> {
        self.candidate = candidate.clone();
        self.lifecycle = lifecycle.clone();
        self.persist_manifest(SessionStatus::Running, None)
    }

    pub fn lifecycle(&self) -> &SessionLifecycle {
        &self.lifecycle
    }

    pub fn root_path(&self) -> &Path {
        &self.root
    }

    pub fn append_transport_event(&mut self, timestamp_ms: u64, data: Value) -> Result<(), String> {
        self.next_timeline_sequence += 1;
        let entry = TimelineEntry {
            sequence: self.next_timeline_sequence,
            timestamp_ms,
            kind: "transport_event".to_string(),
            data,
        };
        write_json_line(&mut self.transport_events, &entry)?;
        self.transport_events
            .sync_data()
            .map_err(|error| format!("同步传输事件失败：{error}"))
    }

    pub fn write_raw(&mut self, sequence: u64, bytes: &[u8]) -> Result<String, String> {
        let relative = format!("{RAW_DIR}/{sequence:06}-observation.bin");
        let path = self.root.join(&relative);
        let mut file = File::create(&path).map_err(|error| format!("创建原始文件失败：{error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("写入原始文件失败：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步原始文件失败：{error}"))?;
        Ok(relative)
    }

    pub fn finish(
        self,
        session_id: &str,
        started_at_ms: u64,
        ended_at_ms: u64,
        candidate: &CandidateProcess,
    ) -> Result<SessionSummary, String> {
        self.finish_with_status(
            session_id,
            started_at_ms,
            ended_at_ms,
            candidate,
            SessionStatus::Completed,
        )
    }

    /// 将异常结束的会话以 `incomplete` 状态发布。
    ///
    /// 原始文件和时间线在错误发生前已经是有效证据；写出不完整摘要后，
    /// 历史列表仍可以浏览这些证据并重建索引。
    pub fn finish_incomplete(
        self,
        session_id: &str,
        started_at_ms: u64,
        ended_at_ms: u64,
        candidate: &CandidateProcess,
    ) -> Result<SessionSummary, String> {
        self.finish_with_status(
            session_id,
            started_at_ms,
            ended_at_ms,
            candidate,
            SessionStatus::Incomplete,
        )
    }

    fn finish_with_status(
        mut self,
        session_id: &str,
        started_at_ms: u64,
        ended_at_ms: u64,
        candidate: &CandidateProcess,
        status: SessionStatus,
    ) -> Result<SessionSummary, String> {
        self.finalize_lifecycle(ended_at_ms, &status);
        let status_name = match &status {
            SessionStatus::Running => "running",
            SessionStatus::Completed => "completed",
            SessionStatus::Incomplete => "incomplete",
        };
        self.append_event(
            ended_at_ms,
            "stopped",
            json!({
                "status": status_name,
                "observationCount": self.observations.len(),
                "frameCount": self.frames.len()
            }),
        )?;

        let index = SessionIndex {
            session_id: session_id.to_string(),
            observations: self.observations.clone(),
            frames: self.frames.clone(),
            generated_at_ms: ended_at_ms,
        };
        write_index_sqlite(&self.root.join(INDEX_FILE), &index)?;

        let summary = SessionSummary {
            session_id: session_id.to_string(),
            status: status.clone(),
            started_at_ms,
            ended_at_ms: Some(ended_at_ms),
            candidate: candidate.clone(),
            observation_count: self.observations.len(),
            frame_count: self.frames.len(),
            session_path: self.root.to_string_lossy().into_owned(),
            timeline_path: self.root.join(TIMELINE_FILE).to_string_lossy().into_owned(),
            index_path: self.root.join(INDEX_FILE).to_string_lossy().into_owned(),
            segments: self.lifecycle.segments.clone(),
            interruptions: self.lifecycle.interruptions.clone(),
            disk_warnings: self.lifecycle.disk_warnings.clone(),
            zero_frame: self.lifecycle.zero_frame,
        };
        self.append_event(
            ended_at_ms,
            "summary",
            serde_json::to_value(&summary).map_err(|error| format!("序列化摘要失败：{error}"))?,
        )?;
        write_json_atomic(&self.root.join(SUMMARY_FILE), &summary)?;

        let manifest = SessionManifest {
            session_id: session_id.to_string(),
            status,
            started_at_ms,
            ended_at_ms: Some(ended_at_ms),
            candidate: candidate.clone(),
            files: session_files(&self.root),
            lifecycle: self.lifecycle.clone(),
        };
        write_json_atomic(&self.root.join(MANIFEST_FILE), &manifest)?;
        self.timeline
            .sync_all()
            .map_err(|error| format!("同步最终时间线失败：{error}"))?;
        self.transport_events
            .sync_all()
            .map_err(|error| format!("同步最终传输事件失败：{error}"))?;
        Ok(summary)
    }

    pub fn add_observation(&mut self, observation: RawObservation) {
        self.observations.push(observation);
    }

    pub fn add_frame(&mut self, frame: ProtocolFrame) {
        self.frames.push(frame);
    }

    fn finalize_lifecycle(&mut self, ended_at_ms: u64, status: &SessionStatus) {
        self.lifecycle.zero_frame = self.frames.is_empty();
        for segment in &mut self.lifecycle.segments {
            if segment.ended_at_ms.is_none() {
                segment.ended_at_ms = Some(ended_at_ms);
                segment.status = if *status == SessionStatus::Completed {
                    SegmentStatus::Completed
                } else {
                    SegmentStatus::Failed
                };
            }
        }
        for interruption in &mut self.lifecycle.interruptions {
            if interruption.ended_at_ms.is_none() {
                interruption.ended_at_ms = Some(ended_at_ms);
            }
        }
    }

    fn persist_manifest(
        &self,
        status: SessionStatus,
        ended_at_ms: Option<u64>,
    ) -> Result<(), String> {
        let manifest = SessionManifest {
            session_id: self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            status,
            started_at_ms: self.started_at_ms,
            ended_at_ms,
            candidate: self.candidate.clone(),
            files: session_files(&self.root),
            lifecycle: self.lifecycle.clone(),
        };
        write_json_atomic(&self.root.join(MANIFEST_FILE), &manifest)
    }
}

fn write_json_line<T: Serialize>(file: &mut File, value: &T) -> Result<(), String> {
    serde_json::to_writer(&mut *file, value)
        .map_err(|error| format!("写入时间线 JSON 失败：{error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("写入时间线换行失败：{error}"))
}

fn session_files(root: &Path) -> SessionFiles {
    SessionFiles {
        timeline: root.join(TIMELINE_FILE).to_string_lossy().into_owned(),
        transport_events: root
            .join(TRANSPORT_EVENTS_FILE)
            .to_string_lossy()
            .into_owned(),
        index: root.join(INDEX_FILE).to_string_lossy().into_owned(),
        summary: root.join(SUMMARY_FILE).to_string_lossy().into_owned(),
        raw_directory: root.join(RAW_DIR).to_string_lossy().into_owned(),
    }
}

/// 读取时间线并返回最后序号、原始观察和协议帧。
///
/// 进程异常退出时最后一条 JSON 可能只写出了一部分。追加式时间线的前缀
/// 仍然是权威证据，因此只忽略最后一条没有换行且无法解析的残缺记录；
/// 中间损坏则返回错误，避免静默改变历史证据。
fn read_timeline_records(
    path: &Path,
) -> Result<(u64, Vec<RawObservation>, Vec<ProtocolFrame>), String> {
    let entries = read_timeline_entries(path)?;
    let mut next_sequence = 0_u64;
    let mut observations = Vec::new();
    let mut frames = Vec::new();
    for entry in entries {
        next_sequence = next_sequence.max(entry.sequence);
        match entry.kind.as_str() {
            "raw_observation" => observations.push(
                serde_json::from_value(entry.data)
                    .map_err(|error| format!("解析原始观察索引失败：{error}"))?,
            ),
            "protocol_frame" => frames.push(
                serde_json::from_value(entry.data)
                    .map_err(|error| format!("解析协议帧索引失败：{error}"))?,
            ),
            _ => {}
        }
    }
    Ok((next_sequence, observations, frames))
}

fn read_timeline_entries(path: &Path) -> Result<Vec<TimelineEntry>, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取时间线失败：{error}"))?;
    let has_trailing_newline = bytes.last().copied() == Some(b'\n');
    let chunks = bytes.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    let mut entries = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        if chunk.is_empty() {
            continue;
        }
        let is_last = index == chunks.len().saturating_sub(1);
        let line = match std::str::from_utf8(chunk) {
            Ok(line) => line,
            Err(error) if is_last && !has_trailing_newline => {
                let _ = error;
                break;
            }
            Err(error) => return Err(format!("时间线不是 UTF-8：{error}")),
        };
        let entry = match serde_json::from_str::<TimelineEntry>(line) {
            Ok(entry) => entry,
            Err(error) if is_last && !has_trailing_newline => {
                let _ = error;
                break;
            }
            Err(error) => return Err(format!("解析时间线条目失败：{error}")),
        };
        entries.push(entry);
    }
    Ok(entries)
}

/// 返回一个已经通过会话编号校验的历史会话目录。
///
/// 目录打开命令只应使用该函数的结果，禁止把前端传入的任意路径交给
/// `open`/`explorer.exe`。规范化后再次确认目录仍位于工作区的 sessions/ 下。
pub fn session_directory(workspace: &Path, session_id: &str) -> Result<PathBuf, String> {
    let root = session_dir(workspace, session_id)?;
    if !root.is_dir() {
        return Err(format!("找不到采集会话目录：{}", root.display()));
    }
    let sessions_root = fs::canonicalize(workspace.join(SESSIONS_DIR))
        .map_err(|error| format!("解析会话根目录失败：{error}"))?;
    let canonical_root =
        fs::canonicalize(&root).map_err(|error| format!("解析采集会话目录失败：{error}"))?;
    if !canonical_root.starts_with(&sessions_root) || canonical_root == sessions_root {
        return Err("采集会话目录超出工作区 sessions/ 范围".to_string());
    }
    Ok(canonical_root)
}

/// 应用启动和历史列表读取前调用。仍标记为 `running` 的会话说明承载应用
/// 可能异常退出；已有原始文件和时间线继续保留，并原子发布为 `incomplete`。
pub fn recover_incomplete_sessions(workspace: &Path) -> Result<Vec<SessionSummary>, String> {
    prepare_workspace(workspace)?;
    let entries = fs::read_dir(workspace.join(SESSIONS_DIR))
        .map_err(|error| format!("读取会话目录失败：{error}"))?;
    let mut recovered = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取会话目录项失败：{error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("读取会话目录项类型失败：{error}"))?
            .is_dir()
        {
            continue;
        }
        let root = entry.path();
        let manifest_path = root.join(MANIFEST_FILE);
        if !manifest_path.is_file() {
            continue;
        }
        let manifest: SessionManifest = read_json(&manifest_path)?;
        if manifest.status != SessionStatus::Running {
            continue;
        }
        recovered.push(recover_one_session(root, manifest)?);
    }
    Ok(recovered)
}

fn recover_one_session(root: PathBuf, manifest: SessionManifest) -> Result<SessionSummary, String> {
    let session_id = manifest.session_id.clone();
    let timeline_path = root.join(TIMELINE_FILE);
    let (next_sequence, observations, frames) = if timeline_path.is_file() {
        read_timeline_records(&timeline_path)?
    } else {
        (0, Vec::new(), Vec::new())
    };
    let ended_at_ms = now_ms();
    let mut lifecycle = manifest.lifecycle;
    if lifecycle.segments.is_empty() {
        lifecycle.segments.push(CaptureSegment {
            segment_id: format!("{session_id}-segment-001"),
            candidate: manifest.candidate.clone(),
            started_at_ms: manifest.started_at_ms,
            ended_at_ms: Some(ended_at_ms),
            status: SegmentStatus::Failed,
        });
    } else {
        for segment in &mut lifecycle.segments {
            if segment.ended_at_ms.is_none() {
                segment.ended_at_ms = Some(ended_at_ms);
                segment.status = SegmentStatus::Failed;
            }
        }
    }
    for interruption in &mut lifecycle.interruptions {
        if interruption.ended_at_ms.is_none() {
            interruption.ended_at_ms = Some(ended_at_ms);
        }
    }
    lifecycle.zero_frame = frames.is_empty();

    let index = SessionIndex {
        session_id: session_id.clone(),
        observations: observations.clone(),
        frames: frames.clone(),
        generated_at_ms: ended_at_ms,
    };
    write_index_sqlite(&root.join(INDEX_FILE), &index)?;

    let summary = SessionSummary {
        session_id: session_id.clone(),
        status: SessionStatus::Incomplete,
        started_at_ms: manifest.started_at_ms,
        ended_at_ms: Some(ended_at_ms),
        candidate: manifest.candidate.clone(),
        observation_count: observations.len(),
        frame_count: frames.len(),
        session_path: root.to_string_lossy().into_owned(),
        timeline_path: root.join(TIMELINE_FILE).to_string_lossy().into_owned(),
        index_path: root.join(INDEX_FILE).to_string_lossy().into_owned(),
        segments: lifecycle.segments.clone(),
        interruptions: lifecycle.interruptions.clone(),
        disk_warnings: lifecycle.disk_warnings.clone(),
        zero_frame: lifecycle.zero_frame,
    };

    if timeline_path.is_file() {
        let mut timeline = OpenOptions::new()
            .append(true)
            .open(&timeline_path)
            .map_err(|error| format!("打开恢复时间线失败：{error}"))?;
        let recovery_entry = TimelineEntry {
            sequence: next_sequence.saturating_add(1),
            timestamp_ms: ended_at_ms,
            kind: "recovered_incomplete".to_string(),
            data: json!({
                "reason": "承载应用异常退出或未完成正常停止",
                "status": "incomplete",
                "observationCount": observations.len(),
                "frameCount": frames.len()
            }),
        };
        write_json_line(&mut timeline, &recovery_entry)?;
        timeline
            .sync_all()
            .map_err(|error| format!("同步恢复时间线失败：{error}"))?;
    }

    let final_manifest = SessionManifest {
        session_id,
        status: SessionStatus::Incomplete,
        started_at_ms: manifest.started_at_ms,
        ended_at_ms: Some(ended_at_ms),
        candidate: manifest.candidate,
        files: session_files(&root),
        lifecycle,
    };
    write_json_atomic(&root.join(SUMMARY_FILE), &summary)?;
    write_json_atomic(&root.join(MANIFEST_FILE), &final_manifest)?;
    Ok(summary)
}

pub fn list_sessions(workspace: &Path) -> Result<Vec<SessionSummary>, String> {
    prepare_workspace(workspace)?;
    // 应用重启后，manifest 仍为 running 的会话必须先恢复为不完整；否则
    // 历史列表会把未正常收尾的证据误显示为“仍在采集”。
    let _ = recover_incomplete_sessions(workspace)?;
    let mut sessions = Vec::new();
    let entries = fs::read_dir(workspace.join(SESSIONS_DIR))
        .map_err(|error| format!("读取会话目录失败：{error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取会话目录项失败：{error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("读取会话目录项类型失败：{error}"))?
            .is_dir()
        {
            continue;
        }
        let summary_path = entry.path().join(SUMMARY_FILE);
        if !summary_path.is_file() {
            continue;
        }
        sessions.push(read_json::<SessionSummary>(&summary_path)?);
    }
    sessions.sort_by_key(|session| std::cmp::Reverse(session.started_at_ms));
    Ok(sessions)
}

pub fn read_session(
    workspace: &Path,
    session_id: &str,
) -> Result<crate::model::SessionDetail, String> {
    let _ = recover_incomplete_sessions(workspace)?;
    let root = session_dir(workspace, session_id)?;
    let summary: SessionSummary = read_json(&root.join(SUMMARY_FILE))?;
    let index: SessionIndex = if root.join(INDEX_FILE).is_file() {
        read_index_sqlite(&root.join(INDEX_FILE))
            .or_else(|_| rebuild_index(workspace, session_id))?
    } else {
        rebuild_index(workspace, session_id)?
    };
    Ok(crate::model::SessionDetail {
        summary,
        observations: index.observations,
        frames: index.frames,
    })
}

/// 读取会话 `raw/` 目录下的一份原始观察。
///
/// 工作台只需要字节本身，分段、连接和协议元数据继续来自事件或会话索引。
/// 这里严格拒绝绝对路径、父目录和 raw 目录之外的文件，避免把会话读取命令
/// 变成任意文件读取入口；同时校验 SHA-256，使展示结果不会悄悄偏离证据。
pub fn read_raw_bytes(
    workspace: &Path,
    session_id: &str,
    raw_file: &str,
) -> Result<crate::model::RawBytes, String> {
    let root = session_dir(workspace, session_id)?;
    let relative = Path::new(raw_file);
    let mut components = relative.components();
    match components.next() {
        Some(Component::Normal(name)) if name.to_str() == Some(RAW_DIR) => {}
        _ => return Err("原始文件必须位于当前会话的 raw/ 目录".to_string()),
    }
    if components.any(|component| !matches!(component, Component::Normal(_))) {
        return Err("原始文件路径包含非法目录组件".to_string());
    }

    let raw_root = root.join(RAW_DIR);
    let requested = root.join(relative);
    let canonical_raw_root =
        fs::canonicalize(&raw_root).map_err(|error| format!("解析原始文件目录失败：{error}"))?;
    let canonical_file =
        fs::canonicalize(&requested).map_err(|error| format!("读取原始文件失败：{error}"))?;
    if !canonical_file.starts_with(&canonical_raw_root) {
        return Err("原始文件路径超出当前会话 raw/ 目录".to_string());
    }
    let timeline = File::open(root.join(TIMELINE_FILE))
        .map_err(|error| format!("打开会话时间线失败：{error}"))?;
    let mut expected_length = None;
    let mut expected_sha256 = None;
    for line in BufReader::new(timeline).lines() {
        let line = line.map_err(|error| format!("读取会话时间线失败：{error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: TimelineEntry =
            serde_json::from_str(&line).map_err(|error| format!("解析会话时间线失败：{error}"))?;
        if entry.kind != "raw_observation"
            || entry.data.get("rawFile").and_then(Value::as_str) != Some(raw_file)
        {
            continue;
        }
        expected_length = entry
            .data
            .get("length")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        expected_sha256 = entry
            .data
            .get("sha256")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        break;
    }
    if expected_length.is_none() || expected_sha256.is_none() {
        return Err("原始文件未在当前会话时间线登记".to_string());
    }

    let bytes = fs::read(&canonical_file).map_err(|error| format!("读取原始文件失败：{error}"))?;
    let sha256 = sha256_hex(&bytes);
    if expected_length != Some(bytes.len()) {
        return Err(format!(
            "原始文件长度与时间线不一致：登记 {:?}，实际 {}",
            expected_length,
            bytes.len()
        ));
    }
    if expected_sha256.as_deref() != Some(sha256.as_str()) {
        return Err(format!(
            "原始文件 SHA-256 与时间线不一致：登记 {:?}，实际 {sha256}",
            expected_sha256
        ));
    }
    let bytes_hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(crate::model::RawBytes {
        session_id: session_id.to_string(),
        raw_file: raw_file.to_string(),
        length: bytes.len(),
        sha256,
        bytes_hex,
    })
}

pub fn rebuild_index(workspace: &Path, session_id: &str) -> Result<SessionIndex, String> {
    let root = session_dir(workspace, session_id)?;
    let timeline_path = root.join(TIMELINE_FILE);
    let (next_sequence, observations, frames) = read_timeline_records(&timeline_path)?;
    let generated_at_ms = if next_sequence == 0 {
        now_ms()
    } else {
        read_timeline_entries(&timeline_path)?
            .iter()
            .map(|entry| entry.timestamp_ms)
            .max()
            .unwrap_or_else(now_ms)
    };
    let index = SessionIndex {
        session_id: session_id.to_string(),
        observations,
        frames,
        generated_at_ms,
    };
    write_index_sqlite(&root.join(INDEX_FILE), &index)?;
    Ok(index)
}
