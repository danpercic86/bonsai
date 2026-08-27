/**
 * P91 §3 — the single emit entry point.
 *
 * `logRecord()` is called from render paths, so its DISABLED path must be one
 * boolean read and a return: no object literal, no timestamp, no allocation
 * (§11). Everything expensive happens after the gate.
 */
import { obsEnabled, obsLevelAllows } from './enabled';
import { enqueue, sessionMono } from './batcher';
import { currentTrace } from './trace';
import type { LogLevel, LogRecord, TraceId, TraceOrigin, UiRecordInput } from './types';

/** Default level per kind, so call sites do not repeat themselves. */
const DEFAULT_LEVEL: Record<string, LogLevel> = {
  error: 'error',
  drop: 'error',
  anomaly: 'warn',
  gesture: 'info',
  'ipc.call': 'debug',
  'ipc.result': 'debug',
  event: 'debug',
  frame: 'trace',
  render: 'trace',
  'render.tally': 'debug',
  effect: 'debug',
  state: 'debug',
};

/**
 * Stamps and enqueues one record. The ambient trace (§2.5) is filled in when the
 * caller did not supply one; an unbound async continuation therefore logs
 * `trace: undefined` rather than a wrong trace.
 */
export function logRecord(input: UiRecordInput): void {
  if (!obsEnabled()) return;
  const lvl = input.lvl ?? DEFAULT_LEVEL[input.kind] ?? 'debug';
  if (!obsLevelAllows(lvl)) return;
  const trace = input.trace ?? currentTrace()?.trace;
  const record: LogRecord = {
    ...(input as Record<string, unknown>),
    // `seq` is assigned authoritatively by the Rust sink in write order (§3).
    seq: 0,
    ts: Date.now(),
    mono: sessionMono(),
    src: 'ui',
    lvl,
    kind: input.kind,
    ...(trace ? { trace } : {}),
  };
  enqueue(record);
}

/** §3 `gesture` — the t0 of a user-initiated trace. Emitted by `withTrace` call
 *  sites (increment 4 wires the six surfaces); available here so increment 2's
 *  harness scenario can produce a complete trace. */
export function logGesture(origin: TraceOrigin, gesture: string, trace?: TraceId): void {
  logRecord({
    kind: 'gesture',
    origin,
    gesture,
    ...(trace ? { trace } : {}),
  });
}
