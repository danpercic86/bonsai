/**
 * P91 §2.1/§3 — the frontend record model.
 *
 * This file NARROWS the wire types (`src/ipc/types/obs.ts`, increment 1) into the
 * per-`kind` discriminated union the emitters use. The wire types are re-exported
 * rather than re-declared: there is exactly one FRONTEND definition of
 * `LogRecord`, `LogKind` and `OBS_SCHEMA_VERSION`.
 *
 * "Frontend" is load-bearing for `OBS_SCHEMA_VERSION`: it is a MIRROR of Rust's
 * `obs::record::OBS_SCHEMA_VERSION`, which is the value `writer.rs` actually
 * stamps into every `session` header. Reading the sentence above as "one
 * definition anywhere" is what let the two sides sit at 2 and 1 for six days
 * after the v1 → v2 bump; `src-tauri/src/obs/tests_schema_parity.rs` now pins
 * them together.
 */
import type { LogKind, LogLevel, LogRecordBase, SpanId, TraceId } from '../ipc/types/obs';

export type {
  DayBucket,
  Histogram,
  LogKind,
  LogLevel,
  LogRecord,
  LogRecordBase,
  LogSessionInfo,
  LogSource,
  MetricsSnapshot,
  MetricTotals,
  RedactionMode,
  SpanId,
  TraceId,
} from '../ipc/types/obs';
export { OBS_SCHEMA_VERSION } from '../ipc/types/obs';

/** §2.1 — where a trace was born. Never free text. */
export type TraceOrigin =
  | 'click'
  | 'keybinding'
  | 'palette'
  | 'menu'
  | 'route'
  | 'timer'
  | 'watcher'
  | 'forge-poll'
  | 'boot'
  | 'backend';

export interface TraceRoot {
  trace: TraceId;
  origin: TraceOrigin;
  /** Stable label of the gesture, NOT free text: 'commit.submit'. */
  gesture: string;
  startedAt: number;
}

/** §3 — key names + type + length only, **never values**. */
export type ArgShapeValue = 'str' | 'num' | 'bool' | 'null' | 'fn' | 'undef' | string;
export type ArgShape = Record<string, ArgShapeValue>;

export interface GesturePayload {
  origin: TraceOrigin;
  gesture: string;
}

export interface IpcCallPayload {
  cmd: string;
  argsHash: string;
  argsShape?: ArgShape;
  /** Raw mode only; keyed by PARAM NAME, allow-listed IDENTIFIER scalars only
   *  (A26, `src/obs/rawArgPolicy.json`). Never free text, never a credential. */
  args?: Record<string, unknown>;
  /** Raw mode only, emitted only when > 0: positions the policy elided. */
  argsOmitted?: number;
  /** WRITER-SET ONLY. A producer must never emit it; Rust's serde struct has no
   *  such field, so a forged one is dropped at deserialisation (A26 §D). */
  argsPolicyViolation?: boolean;
}

export type IpcOutcome = 'ok' | 'err' | 'aborted' | 'superseded';

export interface IpcResultPayload {
  cmd: string;
  argsHash: string;
  ms: number;
  outcome: IpcOutcome;
  errCode?: string;
  resultShape?: ArgShape;
}

export interface EventPayload {
  name: string;
  reason?: string;
  delivered: boolean;
  listeners: number;
}

/** §2.4 — a `notify`/refresh signal. The frontend only ever emits the
 *  SUPPRESSED-echo variant (all count fields 0); real watcher batches come from
 *  Rust. `paths`/`relevant`/`debounceMs`/`fired` are required by the Rust mirror
 *  (no serde default), so they must always be present. */
export interface WatcherPayload {
  paths: number;
  relevant: number;
  debounceMs: number;
  fired: boolean;
  suppressed: boolean;
  suppressReason?: string;
  /** P110 — the debounced burst's path class. Rust sets it on `fired`
   *  records only; absent everywhere else. */
  burstClass?: 'worktree' | 'refs';
}

/** §2.5 — one executed coalesced refresh round. >1 contributing trace IS the
 *  collapse evidence. */
export interface RefreshPayload {
  round: number;
  scope: string;
  origins: string[];
  contributingTraces: TraceId[];
  collapsed: number;
  ms: number;
}

/** §9.1 `each` mode — one record per render. */
export interface RenderPayload {
  component: string;
  count: number;
  sinceMs: number;
  changedProps?: string[];
}

/** §9.2 aggregate mode — ONE record per component per 500 ms window. */
export interface RenderTallyPayload {
  component: string;
  windowMs: number;
  renders: number;
  instances: number;
  /** Absent ⇒ the call site tracks no props. `[]` ⇒ tracked, nothing changed
   *  this window. Names ⇒ tracked, these changed. Never coalesce the first two:
   *  a tally that can only ever say `[]` reports nothing. */
  changedProps?: string[];
  traces: TraceId[];
}

export interface EffectPayload {
  component: string;
  effect: string;
  run: number;
  /** `[]` ⇒ ran with no semantic change (the double-fire signal). */
  changedDeps: string[];
  depCount: number;
}

export interface StatePayload {
  store: string;
  field: string;
  from: string;
  to: string;
}

/** §9.3 — which quantity a `frame` window measured. The paint and gap recorders
 *  are separate (§4.7) and must never be averaged together. */
export type FrameDim = 'paint' | 'gap';

export interface FramePayload {
  /** REQUIRED discriminator: exactly one of `paintMs`/`gapMs` is a measurement
   *  and the other is a filler `0`. Without it, `gapMs: 0` on a paint record
   *  reads as "zero gap measured", which is a fabricated datum. */
  dim: FrameDim;
  /** Mean paint duration over the window. Meaningful only when `dim === 'paint'`. */
  paintMs: number;
  /** Mean scroll inter-frame gap over the window. Meaningful only when `dim === 'gap'`. */
  gapMs: number;
  over33: number;
  over100: number;
  /** The window's worst sample, in the dimension named by `dim`. */
  worstMs: number;
}

export interface ErrorPayload {
  where: string;
  code?: string;
  message: string;
  stackHash?: string;
}

export interface DropPayload {
  dropped: number;
  sinceSeq: number;
}

/** §6.3 — on-disk loss: an earlier part of the session was deleted at the part
 *  cap. Distinct from {@link DropPayload} (in-memory backpressure): `truncate` is
 *  loss of records ALREADY WRITTEN to disk. Rust-only (`LogPayload::Truncate`),
 *  emitted into the surviving newest part; the UI never mints it. */
export interface TruncatePayload {
  reason: 'max-parts';
  /** How many parts have now been deleted for this session. */
  droppedParts: number;
  /** Redacted part label, e.g. `part#0` — never a path. */
  droppedPart: string;
  bytes: number;
  /** Lowest `seq` still present on disk, when known. */
  firstRetainedSeq?: number;
}

/** §5 — a derived cross-record anomaly. Mirrors the Rust `LogPayload::Anomaly`
 *  wire form (`src-tauri/src/obs/record.rs:293`): `severity` is lowercase, `refs`
 *  are the `seq` numbers of the implicated records, `traces` are their trace ids.
 *  Emitted authoritatively by the Rust detector; in mock mode a harness-only
 *  batch analyzer (`src/ipc/mock/obsAnomaly.ts`) synthesises the same shape. */
export type AnomalySeverity = 'info' | 'warn' | 'error';
export interface AnomalyPayload {
  rule: string;
  severity: AnomalySeverity;
  detail: string;
  refs: number[];
  traces: TraceId[];
}

/** The subset of payloads increment 2 emits. Later increments extend the union
 *  additively (`render`, `refresh`, `span`, `anomaly`, ...). */
export type UiPayload =
  | ({ kind: 'gesture' } & GesturePayload)
  | ({ kind: 'ipc.call' } & IpcCallPayload)
  | ({ kind: 'ipc.result' } & IpcResultPayload)
  | ({ kind: 'event' } & EventPayload)
  | ({ kind: 'watcher' } & WatcherPayload)
  | ({ kind: 'refresh' } & RefreshPayload)
  | ({ kind: 'render' } & RenderPayload)
  | ({ kind: 'render.tally' } & RenderTallyPayload)
  | ({ kind: 'effect' } & EffectPayload)
  | ({ kind: 'state' } & StatePayload)
  | ({ kind: 'frame' } & FramePayload)
  | ({ kind: 'error' } & ErrorPayload)
  | ({ kind: 'anomaly' } & AnomalyPayload)
  | ({ kind: 'drop' } & DropPayload)
  | ({
      kind: Exclude<
        LogKind,
        | 'gesture'
        | 'ipc.call'
        | 'ipc.result'
        | 'event'
        | 'watcher'
        | 'refresh'
        | 'render'
        | 'render.tally'
        | 'effect'
        | 'state'
        | 'frame'
        | 'error'
        | 'anomaly'
        | 'drop'
      >;
    } & Record<string, unknown>);

/** What an emitter hands to `logRecord()`: the payload plus the optional
 *  correlation fields. `seq`/`ts`/`mono`/`src` are filled in by `log.ts`, and
 *  `seq` is finally overwritten by the Rust sink (it owns ordering). */
export type UiRecordInput = UiPayload & {
  lvl?: LogLevel;
  trace?: TraceId;
  span?: SpanId;
  causedBy?: TraceId;
  /** P117 §2.2 — RAW canonical `repoId`; see {@link LogRecordBase.repo}. It
   *  reaches the wire through `log.ts`'s spread of this input, so there is no
   *  per-field pick to update — and that spread must not become one. */
  repo?: string;
};

export type UiRecord = LogRecordBase & Record<string, unknown>;
