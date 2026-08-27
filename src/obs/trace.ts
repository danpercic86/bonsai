/**
 * P91 §2.5 — the frontend ambient trace.
 *
 * JS has no async-context propagation, so the ambient is deliberately
 * SYNCHRONOUS: `withTrace` installs it for the synchronous extent of `fn` and
 * clears it on return. Work resumed after an `await` must be re-entered with
 * `bindTrace`. An unbound async continuation logs `trace: undefined` — the
 * contract's explicit choice: never guess a wrong trace.
 */
import type { SpanId, TraceId, TraceOrigin, TraceRoot } from './types';

/** §2.2 — reserved arg keys the IPC proxy injects. Exported so any consumer that
 *  persists or forwards an args object can strip them. */
export const TRACE_ARG_KEY = '__trace';
export const SPAN_ARG_KEY = '__span';

let counter = 0;
let ambient: TraceRoot | undefined;

function rand4(): string {
  // Not a security primitive: only per-session uniqueness is required, and the
  // monotonic prefix already orders. Math.random keeps this off the crypto path.
  return Math.floor(Math.random() * 36 ** 4)
    .toString(36)
    .padStart(4, '0');
}

/** 12-char base36, monotonic-prefixed (§2.1): `${t36}-${rand4}`. */
export function newTrace(): TraceId {
  const t36 = Date.now().toString(36); // 8 chars until year 4147
  return `${t36}-${rand4()}`;
}

/** 6-char base36, unique within a trace (§2.1). */
export function newSpan(): SpanId {
  counter = (counter + 1) % 36 ** 6;
  return counter.toString(36).padStart(6, '0');
}

/** Sets the ambient trace for the synchronous extent of `fn`. Re-entrant: a
 *  nested `withTrace` restores the outer trace on return, so an inner gesture
 *  cannot orphan its caller's trace. */
export function withTrace<T>(origin: TraceOrigin, gesture: string, fn: () => T): T {
  const prev = ambient;
  ambient = { trace: newTrace(), origin, gesture, startedAt: Date.now() };
  try {
    return fn();
  } finally {
    ambient = prev;
  }
}

/** Runs `fn` under an EXISTING trace root (used when re-entering a captured
 *  trace, e.g. from `bindTrace` or a coalescer replay in increment 4). */
export function withTraceRoot<T>(root: TraceRoot | undefined, fn: () => T): T {
  const prev = ambient;
  ambient = root;
  try {
    return fn();
  } finally {
    ambient = prev;
  }
}

export function currentTrace(): TraceRoot | undefined {
  return ambient;
}

/** Captures the ambient trace NOW and restores it whenever the returned function
 *  is called — the bridge across `await` boundaries and callback registration. */
export function bindTrace<F extends (...args: never[]) => unknown>(fn: F): F {
  const captured = ambient;
  const bound = (...args: never[]): unknown => withTraceRoot(captured, () => fn(...args));
  return bound as F;
}

/** TEST ONLY — clears the ambient trace between cases. */
export function resetTraceForTests(): void {
  ambient = undefined;
  ambientSpan = undefined;
  counter = 0;
}

/**
 * The span the IPC proxy minted for the call currently being dispatched.
 *
 * Why this exists: §2.2 requires `__trace` **and** `__span` in the invoke
 * payload, but the payload is built one layer BELOW the proxy — inside each
 * `src/ipc/tauri/*.ts` wrapper — so the transport needs the proxy's span id.
 * A module-level ambient is sound here because the wrapper calls `invoke`
 * synchronously, inside the proxy's own synchronous `fn.apply`.
 */
let ambientSpan: SpanId | undefined;

export function withIpcSpan<T>(span: SpanId, fn: () => T): T {
  const prev = ambientSpan;
  ambientSpan = span;
  try {
    return fn();
  } finally {
    ambientSpan = prev;
  }
}

export function currentIpcSpan(): SpanId | undefined {
  return ambientSpan;
}
