/**
 * P91 §2.1/§3 — the frontend record model.
 *
 * This file NARROWS the wire types (`src/ipc/types/obs.ts`, increment 1) into the
 * per-`kind` discriminated union the emitters use. The wire types are re-exported
 * rather than re-declared: there is exactly one definition of `LogRecord`,
 * `LogKind` and `OBS_SCHEMA_VERSION` in the codebase.
 */
import type { LogKind, LogLevel, LogRecordBase, SpanId, TraceId } from '../ipc/types/obs';

export type {
  LogKind,
  LogLevel,
  LogRecord,
  LogRecordBase,
  LogSessionInfo,
  LogSource,
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
  /** Only ever populated when redaction === 'raw' (§7.1). */
  args?: Record<string, unknown>;
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

/** The subset of payloads increment 2 emits. Later increments extend the union
 *  additively (`render`, `refresh`, `span`, `anomaly`, ...). */
export type UiPayload =
  | ({ kind: 'gesture' } & GesturePayload)
  | ({ kind: 'ipc.call' } & IpcCallPayload)
  | ({ kind: 'ipc.result' } & IpcResultPayload)
  | ({ kind: 'event' } & EventPayload)
  | ({ kind: 'error' } & ErrorPayload)
  | ({ kind: 'drop' } & DropPayload)
  | ({ kind: Exclude<LogKind, 'gesture' | 'ipc.call' | 'ipc.result' | 'event' | 'error' | 'drop'> } & Record<
      string,
      unknown
    >);

/** What an emitter hands to `logRecord()`: the payload plus the optional
 *  correlation fields. `seq`/`ts`/`mono`/`src` are filled in by `log.ts`, and
 *  `seq` is finally overwritten by the Rust sink (it owns ordering). */
export type UiRecordInput = UiPayload & {
  lvl?: LogLevel;
  trace?: TraceId;
  span?: SpanId;
  causedBy?: TraceId;
};

export type UiRecord = LogRecordBase & Record<string, unknown>;
