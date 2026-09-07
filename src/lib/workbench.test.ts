import { describe, expect, it } from 'vitest';
import {
  LIVE_WINDOW_SIZE,
  appendWorkbenchRecord,
  appendLiveViewRecord,
  createLiveViewState,
  filterWorkbenchRecords,
  freezeLiveViewState,
  recordFromCaptureEvent,
  recordsFromSession,
  resumeLiveViewState,
  type WorkbenchRecord
} from './workbench';
import type { CaptureEvent, SessionDetail } from './types';

function event(
  kind: CaptureEvent['kind'],
  data: Record<string, unknown>,
  timestampMs = 1
): CaptureEvent {
  return {
    kind,
    sessionId: 'session-fake-workbench',
    timestampMs,
    data
  };
}

function frameEvent(
  frameId: string,
  messageId: number,
  protocolName: string,
  direction: 'request' | 'response',
  timestampMs = 1
): CaptureEvent {
  return event('protocol_frame', {
    frameId,
    observationId: `observation-${frameId}`,
    segmentId: 'segment-001',
    connectionId: 'connection-001',
    direction,
    transport: 'websocket',
    observedAtMs: timestampMs,
    messageId,
    protocolName,
    length: 12,
    sha256: `${frameId}-hash`,
    rawFile: `raw/${frameId}.bin`
  }, timestampMs);
}

function unparsedEvent(observationId: string, direction: 'request' | 'response'): CaptureEvent {
  return event('raw_observation', {
    observationId,
    segmentId: 'segment-001',
    connectionId: 'http-001',
    direction,
    transport: 'http',
    observedAtMs: 2,
    length: 8,
    sha256: `${observationId}-hash`,
    rawFile: `raw/${observationId}.bin`,
    classification: 'unparsed_payload'
  }, 2);
}

function collect(...events: CaptureEvent[]): WorkbenchRecord[] {
  return events
    .map(recordFromCaptureEvent)
    .filter((record): record is WorkbenchRecord => record !== null);
}

describe('实时协议检查工作台', () => {
  it('覆盖消息号、协议名、请求/响应和未分类明文筛选', () => {
    const records = collect(
      frameEvent('frame-request', 10504, 'CG_ENTER_MAP_REQUEST', 'request'),
      frameEvent('frame-response', 10504, 'CG_ENTER_MAP_RESPONSE', 'response'),
      unparsedEvent('observation-http', 'request')
    );

    expect(records.map((record) => record.kind)).toEqual([
      'protocol_frame',
      'protocol_frame',
      'unparsed_payload'
    ]);
    expect(filterWorkbenchRecords(records, {
      messageId: '10504',
      protocolName: '',
      mode: 'all'
    }).map((record) => record.protocolName)).toEqual([
      'CG_ENTER_MAP_REQUEST',
      'CG_ENTER_MAP_RESPONSE'
    ]);
    expect(filterWorkbenchRecords(records, {
      messageId: '',
      protocolName: 'response',
      mode: 'all'
    })).toHaveLength(1);
    expect(filterWorkbenchRecords(records, {
      messageId: '',
      protocolName: '',
      mode: 'request'
    })).toHaveLength(2);
    expect(filterWorkbenchRecords(records, {
      messageId: '',
      protocolName: '',
      mode: 'response'
    })).toHaveLength(1);
    expect(filterWorkbenchRecords(records, {
      messageId: '',
      protocolName: '',
      mode: 'unparsed'
    })).toEqual([records[2]]);
  });

  it('窗口淘汰只影响实时投影，不修改输入记录', () => {
    const first = collect(frameEvent('first', 1, 'FIRST', 'request'))[0];
    const second = collect(frameEvent('second', 2, 'SECOND', 'response'))[0];
    const third = collect(frameEvent('third', 3, 'THIRD', 'request'))[0];
    const initial = [first, second];
    const result = appendWorkbenchRecord(initial, third, 2);

    expect(result.records.map((record) => record.id)).toEqual(['second', 'third']);
    expect(result.evicted.map((record) => record.id)).toEqual(['first']);
    expect(initial.map((record) => record.id)).toEqual(['first', 'second']);
  });

  it('冻结只固定可见快照，后台新记录仍进入窗口并累计待查看数量', () => {
    const first = collect(frameEvent('freeze-first', 1, 'FIRST', 'request'))[0];
    const second = collect(frameEvent('freeze-second', 2, 'SECOND', 'response'))[0];
    const third = collect(frameEvent('freeze-third', 3, 'THIRD', 'request'))[0];
    let state = appendLiveViewRecord(createLiveViewState(), first, 2);
    state = freezeLiveViewState(state);
    state = appendLiveViewRecord(state, second, 2);
    state = appendLiveViewRecord(state, third, 2);

    expect(state.frozen).toBe(true);
    expect(state.visibleRecords.map((record) => record.id)).toEqual(['freeze-first']);
    expect(state.records.map((record) => record.id)).toEqual(['freeze-second', 'freeze-third']);
    expect(state.pendingRecordCount).toBe(2);
    expect(state.evictedRecordCount).toBe(1);

    const resumed = resumeLiveViewState(state);
    expect(resumed.frozen).toBe(false);
    expect(resumed.visibleRecords.map((record) => record.id)).toEqual(['freeze-second', 'freeze-third']);
    expect(resumed.pendingRecordCount).toBe(0);
  });

  it('默认实时窗口有界，历史索引仍重建完整记录', () => {
    let records: WorkbenchRecord[] = [];
    for (let index = 0; index < LIVE_WINDOW_SIZE + 7; index += 1) {
      const record = collect(frameEvent(`frame-${index}`, 16001, 'CG_HEARTBEAT', 'request', index))[0];
      records = appendWorkbenchRecord(records, record).records;
    }
    expect(records).toHaveLength(LIVE_WINDOW_SIZE);
    expect(records[0].id).toBe('frame-7');

    const framedObservation: SessionDetail['observations'][number] = {
      observationId: 'observation-frame',
      segmentId: 'segment-002',
      connectionId: 'ws-002',
      direction: 'request',
      transport: 'websocket',
      observedAtMs: 20,
      length: 5,
      sha256: 'frame-hash',
      rawFile: 'raw/000001-observation.bin'
    };
    const raw: SessionDetail['observations'][number] = {
      observationId: 'observation-unparsed',
      segmentId: 'segment-002',
      connectionId: 'http-002',
      direction: 'response',
      transport: 'http',
      observedAtMs: 30,
      length: 4,
      sha256: 'raw-hash',
      rawFile: 'raw/000002-observation.bin'
    };
    const frame: SessionDetail['frames'][number] = {
      frameId: 'frame-history',
      observationId: 'observation-frame',
      segmentId: 'segment-002',
      connectionId: 'ws-002',
      direction: 'request',
      transport: 'websocket',
      observedAtMs: 20,
      messageId: 10521,
      protocolName: 'CG_WORLD_CATALOG',
      length: 5,
      sha256: 'frame-hash',
      rawFile: 'raw/000001-observation.bin'
    };
    const history = recordsFromSession({
      summary: {
        sessionId: 'session-history',
        status: 'completed',
        startedAtMs: 1,
        endedAtMs: 40,
        candidate: {} as SessionDetail['summary']['candidate'],
        observationCount: 2,
        frameCount: 1,
        sessionPath: '/tmp/session-history',
        timelinePath: '/tmp/session-history/timeline.jsonl',
        indexPath: '/tmp/session-history/index.sqlite'
      },
      observations: [framedObservation, raw],
      frames: [frame],
    });
    expect(history.map((record) => record.id)).toEqual(['frame-history', 'observation-unparsed']);
    expect(history[1].segmentId).toBe('segment-002');
    expect(history[1].rawFile).toBe('raw/000002-observation.bin');
  });
});
