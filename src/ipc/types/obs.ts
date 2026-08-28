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
  | 'span'
  | 'drop';

/** §3.1 — one phase of a backend operation span. `name` is allow-listed on the
 *  Rust producer side, so it is never user-derived. */
export interface PhaseTiming {
  /** Allow-listed, dotted for nesting: `revwalk`, `decorate`, `lane`, `serialize`. */
  name: string;
  ms: number;
  /** Optional unit count for the phase (commits walked, files scanned). */
  n?: number;
}

/** §3.1 — ONE `span` record per completed backend operation, carrying its phase
 *  breakdown. Carries NO `argsHash`/`argsShape` (§7.2). Every field beyond
 *  `op`/`ms` is optional and additive. */
export interface SpanPayload {
  /** Allow-listed `<domain>.<action>`: `graph.get` | `status.scan` | `diff.compute`. */
  op: string;
  /** Total wall time of the operation, measured at the src-tauri call site. */
  ms: number;
  /** Ordered, ≤16 entries. Sum may be < `ms`; the remainder is unattributed. */
  phases?: PhaseTiming[];
  /** Ms spent QUEUED before the `spawn_blocking` closure started (§3.1.2). */
  queuedMs?: number;
  /** Blocking-pool tasks in flight when this one started, and the pool cap. */
  poolInflight?: number;
  poolMax?: number;
  /** elapsed / git-timeout deadline, 0..1+ — watchdog pressure (§3.1.3). */
  deadlineFrac?: number;
  /** Graph-cache outcome, emitted only by `graph_cache.rs` (§5.1). */
  cache?: 'hit' | 'redecorate' | 'miss';
  /** Primary unit count for the whole op (commits, files). */
  items?: number;
  outcome?: 'ok' | 'err' | 'timeout';
}

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

/**
 * P91 §8.1 — the bounded duration summary. 8 frozen buckets (1,5,10,50,100,500,
 * 2000,+inf ms) plus running aggregates; no raw sample is ever retained, so its
 * size is independent of the observation count.
 *
 * `p50Ms`/`p95Ms` are DERIVED and present **only** on `metricsSnapshot()`
 * results — they are computed at snapshot time and never persisted to
 * `usage.json`, so they are absent from any stored histogram.
 */
export interface Histogram {
  count: number;
  sumMs: number;
  maxMs: number;
  /** Always length 8. */
  buckets: number[];
  p50Ms?: number;
  p95Ms?: number;
}

/** P91 §8 — the tallies inside a day bucket (or `lifetime`). Every key is from
 *  the fixed `<domain>.<action>` allow-list — never user-derived. */
export interface MetricTotals {
  counters: Record<string, number>;
  durations: Record<string, Histogram>;
  errors: Record<string, number>;
  sessionMs: number;
}

/** P91 §8 — one calendar day's aggregates. */
export interface DayBucket {
  date: string;
  totals: MetricTotals;
}

/** P91 §8 — the durable metrics root returned by `metricsSnapshot()`. Retains up
 *  to 400 day buckets; older days fold into `lifetime`. */
export interface MetricsSnapshot {
  schema: number;
  firstSeen: string;
  sessions: number;
  days: DayBucket[];
  lifetime: MetricTotals;
}
