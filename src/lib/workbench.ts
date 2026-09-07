import type {
  CaptureEvent,
  ProtocolFrame,
  RawObservation,
  SessionDetail,
  TrafficDirection,
  TransportKind
} from './types';

/**
 * 实时视图只保留最近的一小段记录。这个窗口是 UI 投影，不代表会话证据
 * 被删除；后端仍会把每一条原始观察追加到时间线和 raw/ 文件。
 */
export const LIVE_WINDOW_SIZE = 120;

export type WorkbenchRecordKind = 'protocol_frame' | 'unparsed_payload';
export type WorkbenchFilterMode = 'all' | TrafficDirection | 'unparsed';

export interface WorkbenchFilter {
  messageId: string;
  protocolName: string;
  mode: WorkbenchFilterMode;
}

export interface WorkbenchRecord {
  id: string;
  sessionId: string;
  kind: WorkbenchRecordKind;
  observedAtMs: number;
  direction: TrafficDirection;
  transport: TransportKind;
  messageId: number | null;
  protocolName: string;
  length: number;
  sha256: string;
  rawFile: string;
  observationId: string;
  segmentId: string;
  connectionId: string;
  frameId: string | null;
}

export interface WindowAppendResult {
  records: WorkbenchRecord[];
  evicted: WorkbenchRecord[];
}

/** 实时视图的可测试状态。`records` 是后台收到的有界投影，`visibleRecords`
 * 是用户当前看到的快照；冻结时两者刻意分离。 */
export interface LiveViewState {
  records: WorkbenchRecord[];
  visibleRecords: WorkbenchRecord[];
  frozen: boolean;
  pendingRecordCount: number;
  evictedRecordCount: number;
}

export function createLiveViewState(): LiveViewState {
  return {
    records: [],
    visibleRecords: [],
    frozen: false,
    pendingRecordCount: 0,
    evictedRecordCount: 0
  };
}

function stringValue(data: Record<string, unknown>, key: string): string | null {
  const value = data[key];
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function numberValue(data: Record<string, unknown>, key: string): number | null {
  const value = data[key];
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function directionValue(data: Record<string, unknown>): TrafficDirection | null {
  const direction = stringValue(data, 'direction');
  return direction === 'request' || direction === 'response' ? direction : null;
}

function transportValue(data: Record<string, unknown>): TransportKind | null {
  const transport = stringValue(data, 'transport');
  return transport === 'websocket' || transport === 'http' ? transport : null;
}

function commonRecordFields(
  event: CaptureEvent,
  data: Record<string, unknown>
): Omit<WorkbenchRecord, 'id' | 'kind' | 'messageId' | 'protocolName' | 'frameId'> | null {
  const observedAtMs = numberValue(data, 'observedAtMs') ?? event.timestampMs;
  const direction = directionValue(data);
  const transport = transportValue(data);
  const length = numberValue(data, 'length');
  const sha256 = stringValue(data, 'sha256');
  const rawFile = stringValue(data, 'rawFile');
  const observationId = stringValue(data, 'observationId');
  const segmentId = stringValue(data, 'segmentId');
  const connectionId = stringValue(data, 'connectionId');
  if (
    direction === null ||
    transport === null ||
    length === null ||
    sha256 === null ||
    rawFile === null ||
    observationId === null ||
    segmentId === null ||
    connectionId === null
  ) {
    return null;
  }
  return {
    sessionId: event.sessionId,
    observedAtMs,
    direction,
    transport,
    length,
    sha256,
    rawFile,
    observationId,
    segmentId,
    connectionId
  };
}

/**
 * 将公开事件转换成工作台记录。
 *
 * `raw_observation` 只有明确标记为 `unparsed_payload` 时才进入列表；已拆分
 * 的观察会紧随其后产生 `protocol_frame`，工作台展示后者避免同一字节重复一行。
 */
export function recordFromCaptureEvent(event: CaptureEvent): WorkbenchRecord | null {
  if (event.kind !== 'protocol_frame' && event.kind !== 'raw_observation') return null;
  const data = event.data ?? {};
  const common = commonRecordFields(event, data);
  if (!common) return null;

  if (event.kind === 'protocol_frame') {
    const frameId = stringValue(data, 'frameId');
    const messageId = numberValue(data, 'messageId');
    const protocolName = stringValue(data, 'protocolName');
    if (frameId === null || messageId === null || protocolName === null) return null;
    return {
      ...common,
      id: frameId,
      kind: 'protocol_frame',
      messageId,
      protocolName,
      frameId
    };
  }

  if (data.classification !== 'unparsed_payload') return null;
  return {
    ...common,
    id: common.observationId,
    kind: 'unparsed_payload',
    messageId: null,
    protocolName: '未分类明文载荷',
    frameId: null
  };
}

/**
 * 追加一条实时记录并裁剪显示窗口。返回被裁剪出的记录，调用方只能将其
 * 作为 UI 计数使用，不能据此删除任何会话文件。
 */
export function appendWorkbenchRecord(
  records: WorkbenchRecord[],
  record: WorkbenchRecord,
  windowSize = LIVE_WINDOW_SIZE
): WindowAppendResult {
  if (!Number.isInteger(windowSize) || windowSize < 1) {
    throw new Error('实时窗口大小必须是正整数');
  }
  const withoutDuplicate = records.filter((item) => item.id !== record.id);
  const next = [...withoutDuplicate, record];
  const evicted = next.length > windowSize ? next.slice(0, next.length - windowSize) : [];
  return {
    records: next.slice(-windowSize),
    evicted
  };
}

export function appendLiveViewRecord(
  state: LiveViewState,
  record: WorkbenchRecord,
  windowSize = LIVE_WINDOW_SIZE
): LiveViewState {
  const result = appendWorkbenchRecord(state.records, record, windowSize);
  return {
    ...state,
    records: result.records,
    visibleRecords: state.frozen ? state.visibleRecords : result.records,
    pendingRecordCount: state.frozen ? state.pendingRecordCount + 1 : state.pendingRecordCount,
    evictedRecordCount: state.evictedRecordCount + result.evicted.length
  };
}

export function freezeLiveViewState(state: LiveViewState): LiveViewState {
  return {
    ...state,
    visibleRecords: state.records,
    frozen: true,
    pendingRecordCount: 0
  };
}

export function resumeLiveViewState(state: LiveViewState): LiveViewState {
  return {
    ...state,
    visibleRecords: state.records,
    frozen: false,
    pendingRecordCount: 0
  };
}

export function filterWorkbenchRecords(
  records: WorkbenchRecord[],
  filter: WorkbenchFilter
): WorkbenchRecord[] {
  const messageQuery = filter.messageId.trim();
  const protocolQuery = filter.protocolName.trim().toLocaleLowerCase();
  return records.filter((record) => {
    if (messageQuery && !String(record.messageId ?? '').includes(messageQuery)) return false;
    if (protocolQuery && !record.protocolName.toLocaleLowerCase().includes(protocolQuery)) return false;
    if (filter.mode === 'request' || filter.mode === 'response') {
      return record.direction === filter.mode;
    }
    if (filter.mode === 'unparsed') return record.kind === 'unparsed_payload';
    return true;
  });
}

function frameRecord(sessionId: string, frame: ProtocolFrame): WorkbenchRecord {
  return {
    id: frame.frameId,
    sessionId,
    kind: 'protocol_frame',
    observedAtMs: frame.observedAtMs,
    direction: frame.direction,
    transport: frame.transport,
    messageId: frame.messageId,
    protocolName: frame.protocolName,
    length: frame.length,
    sha256: frame.sha256,
    rawFile: frame.rawFile,
    observationId: frame.observationId,
    segmentId: frame.segmentId,
    connectionId: frame.connectionId,
    frameId: frame.frameId
  };
}

function unparsedRecord(sessionId: string, observation: RawObservation): WorkbenchRecord {
  return {
    id: observation.observationId,
    sessionId,
    kind: 'unparsed_payload',
    observedAtMs: observation.observedAtMs,
    direction: observation.direction,
    transport: observation.transport,
    messageId: null,
    protocolName: '未分类明文载荷',
    length: observation.length,
    sha256: observation.sha256,
    rawFile: observation.rawFile,
    observationId: observation.observationId,
    segmentId: observation.segmentId,
    connectionId: observation.connectionId,
    frameId: null
  };
}

/**
 * 从历史索引重建工作台记录。没有对应协议帧的原始观察仍保留为未分类
 * 明文，保证停止后重新打开会话时不会把它隐藏掉。
 */
export function recordsFromSession(detail: SessionDetail): WorkbenchRecord[] {
  const framedObservationIds = new Set(detail.frames.map((frame) => frame.observationId));
  const records = detail.frames.map((frame) => frameRecord(detail.summary.sessionId, frame));
  for (const observation of detail.observations) {
    if (!framedObservationIds.has(observation.observationId)) {
      records.push(unparsedRecord(detail.summary.sessionId, observation));
    }
  }
  return records.sort((left, right) => left.observedAtMs - right.observedAtMs);
}
