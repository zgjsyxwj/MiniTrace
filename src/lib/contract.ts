import type { CaptureEventKind } from './types';

export const CAPTURE_EVENT_NAME = 'capture://event';

export const CAPTURE_EVENT_KINDS: readonly CaptureEventKind[] = [
  'started',
  'ready',
  'raw_observation',
  'protocol_frame',
  'stopped',
  'summary',
  'error'
];

export function isCaptureTerminalEvent(kind: CaptureEventKind): boolean {
  return kind === 'summary' || kind === 'error';
}
