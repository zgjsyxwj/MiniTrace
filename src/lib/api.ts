import { invoke } from '@tauri-apps/api/core';
import type {
  AppSnapshot,
  CandidateProcess,
  CaptureSessionInfo,
  CaptureRecoverySnapshot,
  EnvironmentDiagnosis,
  RawBytes,
  SessionDetail,
  SessionIndex,
  SessionSummary,
  WorkspaceInfo
} from './types';

export const api = {
  getAppState: () => invoke<AppSnapshot>('get_app_state'),
  setWorkspace: (path: string) => invoke<WorkspaceInfo>('set_workspace', { path }),
  pickWorkspace: () => invoke<WorkspaceInfo>('pick_workspace'),
  diagnoseEnvironment: () => invoke<EnvironmentDiagnosis>('diagnose_environment'),
  listCandidates: () => invoke<CandidateProcess[]>('list_candidates'),
  startCapture: (candidateId: string) =>
    invoke<CaptureSessionInfo>('start_capture', { candidateId }),
  stopCapture: () => invoke<SessionSummary>('stop_capture'),
  reportTargetProcessExit: (reason?: string) =>
    invoke<void>('report_target_process_exit', { reason }),
  refreshRecoveryCandidates: () => invoke<void>('refresh_recovery_candidates'),
  selectRecoveryCandidate: (candidateId: string) =>
    invoke<void>('select_recovery_candidate', { candidateId }),
  getCaptureRecovery: () => invoke<CaptureRecoverySnapshot>('get_capture_recovery'),
  listSessions: () => invoke<SessionSummary[]>('list_sessions'),
  getSession: (sessionId: string) => invoke<SessionDetail>('get_session', { sessionId }),
  readRawBytes: (sessionId: string, rawFile: string) =>
    invoke<RawBytes>('read_raw_bytes', { sessionId, rawFile }),
  rebuildSessionIndex: (sessionId: string) =>
    invoke<SessionIndex>('rebuild_session_index', { sessionId }),
  openSessionDirectory: (sessionId: string) =>
    invoke<string>('open_session_directory', { sessionId })
};
