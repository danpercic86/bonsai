/**
 * P91 §3/§6 — the observability IPC types.
 *
 * SCOPE: this file mirrors only what crosses the IPC boundary in increment 1.
 * The full per-`kind` payload union lives in `src/obs/types.ts` (increment 2),
 * which will narrow {@link LogRecord} from "base fields + payload" to a proper
 * discriminated union. Nothing here should grow into that union — keep the two
 * concerns apart.
 */

import type { LogLevel, RedactionMode } from './settings';

export type { LogLevel, RedactionMode } from './settings';

/** Record-schema version written into every `session` header (§3). */
export const OBS_SCHEMA_VERSION = 1;

/** 12-char base36, monotonic-prefixed (§2.1). */
export type TraceId = string;
/** 6-char base36, unique within a trace (§2.1). */
export type SpanId = string;

export type LogSource = 'ui' | 'rust';

/** §3. Extended additively — increment 3 adds `'span'`. */
export type LogKind =
  | 'session'
  | 'gesture'
  | 'ipc.call'
  | 'ipc.result'
  | 'ipc.recv'
  | 'event'
  | 'channel'
  | 'watcher'
  | 'refresh'
  | 'render'
  | 'render.tally'
  | 'effect'
  | 'state'
  | 'frame'
  | 'error'
  | 'anomaly'
  | 'drop';

/** §3 `LogRecordBase`. `seq` is assigned by the RUST sink, so a record sent over
 *  `logAppend` leaves it at 0 and has it overwritten in write order. */
export interface LogRecordBase {
  seq: number;
  /** Epoch ms, wall clock. */
  ts: number;
  /** Ms since session start (jitter-free ordering aid). */
  mono: number;
  src: LogSource;
  lvl: LogLevel;
  trace?: TraceId;
  span?: SpanId;
  causedBy?: TraceId;
  kind: LogKind;
}

/** One JSONL line: the base fields plus the kind's own payload keys. */
export type LogRecord = LogRecordBase & Record<string, unknown>;

/** §6 — what the Dev page shows about the current log session. */
export interface LogSessionInfo {
  /** Empty while Dev mode is off. */
  sessionId: string;
  /** Absolute path of the logs directory. */
  dir: string;
  /** File NAMES of the current session's parts. */
  files: string[];
  bytes: number;
  records: number;
  anomalies: number;
  dropped: number;
  redaction: RedactionMode;
  /** Hex of the 16-byte session salt (§7.2). In-process ONLY — it seeds the
   *  frontend's own ordinal counter and its `argsHash`, and is never written to
   *  a log file or an export. Empty while Dev mode is off.
   *
   *  Note (§7.2, amended): ordinals are side-LOCAL. Rust emits `ref#3`, the
   *  frontend emits `ui:ref#3`, and the two never denote the same value —
   *  records are joined on `trace`/`span`/`argsHash`, never on ordinal equality. */
  salt: string;
  /** Totals across ALL log files on disk, not just this session. */
  totalFiles: number;
  totalBytes: number;
  /** §6.2 — Bonsai-created export zips, which are inside the delete scope, so
   *  the confirm copy can name them. Absent when the exports directory does not
   *  exist yet (distinct from an existing but empty one, which reports 0). */
  exportFiles?: number;
  exportBytes?: number;
}
