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

/**
 * Record-schema version written into every `session` header (§3).
 *
 * MIRROR, not source: Rust's `obs::record::OBS_SCHEMA_VERSION` owns this value
 * — `src-tauri/src/obs/writer.rs` is what actually stamps it into the header —
 * and `src-tauri/src/obs/tests_schema_parity.rs` pins this copy to it.
 */
export const OBS_SCHEMA_VERSION = 3;

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
  | 'drop'
  | 'truncate';

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
  /**
   * P117 §2.2 — the canonical `repoId` this record is about. Absent ⇒ not
   * repo-scoped, or unattributed. Never rendered in the UI; it is a correlation
   * key that makes `redundant-refresh` and `cache-collapse` repo-aware.
   *
   * **NORMATIVE: the RAW string, never redacted on this side.** `tagPath` /
   * `tagValue` must never touch it. A `ui:path#3` here would never equal the
   * Rust span's raw path, so `cache-collapse` would fire right after a real
   * mutation — the exact inversion of the suppression argument. The Rust writer
   * is the only redaction point for this field (strict ⇒ `repo#N`).
   */
  repo?: string;
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
  /** §6.3 — parts of THIS session already evicted at the 128 MB cap. `> 0` means
   *  the session is truncated (its earliest records are gone) and the Dev page
   *  shows a warning line. 0 while Dev mode is off. */
  droppedParts: number;
  /** §8.4/§16.9 — the log is currently NOT reaching disk (disk full, permission
   *  loss on the log dir). Sticky-until-next-success: `true` while the most
   *  recent write/flush failed, cleared once one succeeds. `false` while Dev mode
   *  is off. Drives the "Not writing" status row + the header pill danger variant.
   *
   *  PRIVACY: a BOOL by design — the underlying `io::Error` (whose text embeds the
   *  log path) never crosses IPC. The UI shows generic copy only. */
  writeFailed: boolean;
  /** §6.2 — Bonsai-created export zips, which are inside the delete scope, so
   *  the confirm copy can name them. Absent when the exports directory does not
   *  exist yet (distinct from an existing but empty one, which reports 0). */
  exportFiles?: number;
  exportBytes?: number;
}

/** §6.1 — the honest result of "delete all log files". */
export interface LogsDeleteResult {
  /** Files actually removed (log parts AND export zips — see `deletedExports`). */
  deletedFiles: number;
  /** Bytes reclaimed (each size measured immediately before removal). */
  deletedBytes: number;
  /** Files that could not be removed (locked, permission denied, ...). */
  failedFiles: number;
  /** Present only when Dev mode was ON: the fresh, empty file logging continues
   *  into. NAME only, never a path. */
  activeFile: string | null;
  /** True when the writer was rolled to a new file as part of this operation. */
  rolled: boolean;
  /** §6.2 — how many of `deletedFiles` were export zips. */
  deletedExports?: number;
  /** §F6 — usage-statistics files removed from the `metrics` folder
   *  (`usage.json`, `.bak`, `.tmp`). ALSO counted in `deletedFiles`/
   *  `deletedBytes`, like export zips. */
  deletedMetrics: number;
  /** §F6 — the in-memory aggregate was cleared AND no usage file remains.
   *
   *  **True even when `deletedMetrics === 0`**: on a launch younger than the
   *  first 60 s flush there is nothing on disk yet while the aggregate is very
   *  much live. The "usage counts cleared" copy is justified by THIS flag, never
   *  by the file count. Required, not optional — an absent flag would fall back
   *  to a misleading default. */
  metricsCleared: boolean;
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

/** P91 §8 / §F6 — the durable metrics root returned by `metricsSnapshot()`.
 *  Retains up to 90 day buckets AND no bucket older than 90 calendar days; days
 *  outside the window fold into `lifetime`, which is a lifetime figure the window
 *  never ages out. */
export interface MetricsSnapshot {
  schema: number;
  firstSeen: string;
  sessions: number;
  days: DayBucket[];
  lifetime: MetricTotals;
}
