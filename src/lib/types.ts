export type CaptureEventKind =
  | 'started'
  | 'ready'
  | 'raw_observation'
  | 'protocol_frame'
  | 'stopped'
  | 'summary'
  | 'error';

export type DiagnosticMode = 'supported' | 'compatibility_diagnostic';

export type DiagnosticIssueCode =
  | 'unsupported_platform'
  | 'unsupported_architecture'
  | 'unknown_wechat_version'
  | 'mismatched_wechat_version'
  | 'missing_candidate'
  | 'missing_required_module'
  | 'missing_capture_boundary'
  | 'candidate_role_mismatch'
  | 'multiple_candidates'
  | 'hardened_runtime_blocked'
  | 'process_inspection_unavailable'
  | 'platform_integration_unverified';

export type CapturePhase =
  | 'idle'
  | 'starting'
  | 'ready'
  | 'capturing'
  | 'stopping'
  | 'completed'
  | 'error';

export type ReattachState =
  | 'attaching'
  | 'attached'
  | 'interrupted'
  | 'awaiting_candidate'
  | 'reattaching'
  | 'attach_failed'
  | 'stopped';

export type SegmentStatus = 'starting' | 'attached' | 'interrupted' | 'completed' | 'failed';

export type SessionStatus = 'running' | 'completed' | 'incomplete';
export type TrafficDirection = 'request' | 'response';
export type TransportKind = 'websocket' | 'http';

export interface CandidateProcess {
  id: string;
  pid: number;
  displayName: string;
  processRole: string;
  game: string;
  wechatVersion: string;
  architecture: string;
  modules: string[];
  backend: string;
}

export interface CaptureSegment {
  segmentId: string;
  candidate: CandidateProcess;
  startedAtMs: number;
  endedAtMs: number | null;
  status: SegmentStatus;
}

export interface InterruptionInfo {
  interruptionId: string;
  segmentId: string;
  startedAtMs: number;
  endedAtMs: number | null;
  reason: string;
  previousPid: number;
}

export interface DiskSpaceStatus {
  path: string;
  freeBytes: number;
  totalBytes: number;
  warning: boolean;
  warningRatioPercent: number;
  warningFreeBytes: number;
  checkedAtMs: number;
}

export interface CaptureRecoverySnapshot {
  state: ReattachState | null;
  sessionId: string | null;
  currentSegmentId: string | null;
  currentPid: number | null;
  segments: CaptureSegment[];
  interruptions: InterruptionInfo[];
  candidates: CandidateProcess[];
  interruptionStartedAtMs: number | null;
  lastError: string | null;
  zeroFrame: boolean;
  disk: DiskSpaceStatus | null;
}

export interface DiagnosticIssue {
  code: DiagnosticIssueCode;
  message: string;
  blocking: boolean;
}

export interface EnvironmentDiagnosis {
  platform: string;
  osVersion: string;
  architecture: string;
  wechatVersion: string;
  mode: DiagnosticMode;
  supported: boolean;
  captureAllowed: boolean;
  requiresCandidateConfirmation: boolean;
  candidates: CandidateProcess[];
  issues: DiagnosticIssue[];
  requiredModules: string[];
  checkedAtMs: number;
}

export interface WorkspaceInfo {
  path: string;
  sessionsPath: string;
}

export interface CaptureSessionInfo {
  sessionId: string;
  startedAtMs: number;
  candidate: CandidateProcess;
  workspacePath: string;
}

export interface RawObservation {
  observationId: string;
  segmentId: string;
  connectionId: string;
  direction: TrafficDirection;
  transport: TransportKind;
  observedAtMs: number;
  length: number;
  sha256: string;
  rawFile: string;
}

/** 从会话 raw/ 目录读取的原始字节。元数据仍以工作台记录为准。 */
export interface RawBytes {
  sessionId: string;
  rawFile: string;
  length: number;
  sha256: string;
  bytesHex: string;
}

export interface ProtocolFrame {
  frameId: string;
  observationId: string;
  segmentId: string;
  connectionId: string;
  direction: TrafficDirection;
  transport: TransportKind;
  observedAtMs: number;
  messageId: number;
  protocolName: string;
  length: number;
  sha256: string;
  rawFile: string;
}

export interface SessionSummary {
  sessionId: string;
  status: SessionStatus;
  startedAtMs: number;
  endedAtMs: number | null;
  candidate: CandidateProcess;
  observationCount: number;
  frameCount: number;
  sessionPath: string;
  timelinePath: string;
  indexPath: string;
  segments?: CaptureSegment[];
  interruptions?: InterruptionInfo[];
  diskWarnings?: DiskSpaceStatus[];
  zeroFrame?: boolean;
}

export interface SessionIndex {
  sessionId: string;
  observations: RawObservation[];
  frames: ProtocolFrame[];
  generatedAtMs: number;
}

export interface SessionDetail {
  summary: SessionSummary;
  observations: RawObservation[];
  frames: ProtocolFrame[];
}

export interface CaptureSnapshot {
  phase: CapturePhase;
  sessionId: string | null;
  candidate: CandidateProcess | null;
  observationCount: number;
  frameCount: number;
  recovery: CaptureRecoverySnapshot;
}

export interface AppSnapshot {
  workspace: WorkspaceInfo | null;
  capture: CaptureSnapshot;
}

export interface CaptureEvent {
  kind: CaptureEventKind;
  sessionId: string;
  timestampMs: number;
  data: Record<string, unknown>;
}
