import { describe, expect, it } from 'vitest';
import {
  CAPTURE_EVENT_KINDS,
  CAPTURE_EVENT_NAME,
  isCaptureTerminalEvent
} from './contract';

describe('公开采集事件契约', () => {
  it('使用单一事件名称并覆盖最小闭环事件', () => {
    expect(CAPTURE_EVENT_NAME).toBe('capture://event');
    expect(CAPTURE_EVENT_KINDS).toEqual([
      'started',
      'ready',
      'raw_observation',
      'protocol_frame',
      'stopped',
      'summary',
      'error'
    ]);
  });

  it('只有摘要和错误表示一次事件流结束', () => {
    expect(isCaptureTerminalEvent('summary')).toBe(true);
    expect(isCaptureTerminalEvent('error')).toBe(true);
    expect(isCaptureTerminalEvent('stopped')).toBe(false);
    expect(isCaptureTerminalEvent('protocol_frame')).toBe(false);
  });
});
