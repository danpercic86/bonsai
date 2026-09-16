/**
 * P91 §9.2 — aggregate render accumulator.
 *
 * A surface that re-renders once per row per ref change (the sidebar) would emit
 * hundreds of `render` records per interaction in `each` mode, blowing both the
 * 100-records-per-batch and the ≤1-`log_append`-per-500 ms budgets (§11). This
 * module collapses that to **one `render.tally` record per component per 500 ms
 * window**: renders accumulate in a module-level map keyed by `component`, and a
 * single `setTimeout` flush drains the window.
 *
 * Zero-render windows emit nothing (the timer is only armed by the first render
 * of a window), so there is no idle work when a surface is quiet or Dev mode is
 * off — `tallyRender` is only ever reached through the `obsEnabled()` gate in
 * `obs/react.ts`.
 */
import { logRecord } from './log';
import type { TraceId } from './types';

/** Flush cadence. Exported so tests can drive fake timers precisely. */
export const RENDER_TALLY_WINDOW_MS = 500;

interface Bucket {
  renders: number;
  /** Distinct mounted instances seen this window (§9.2 `instances`). */
  instances: Set<string>;
  /** Union of changed prop NAMES over the window — never values (§7/§9.1).
   *  `null` while no render in this window supplied a props bag at all: the
   *  emitted record then OMITS `changedProps`, which is how "not tracked" stays
   *  distinguishable from the empty set ("tracked, nothing changed"). */
  changedProps: Set<string> | null;
  traces: Set<TraceId>;
}

let buckets = new Map<string, Bucket>();
let timer: ReturnType<typeof setTimeout> | null = null;
let windowStart = 0;

/** Accumulate one render of `component` from `instanceId`. Called only from the
 *  enabled path of `useRenderCount(mode: 'aggregate')`. */
export function tallyRender(
  component: string,
  instanceId: string,
  changedProps: string[] | undefined,
  trace: TraceId | undefined,
): void {
  let b = buckets.get(component);
  if (b === undefined) {
    b = { renders: 0, instances: new Set(), changedProps: null, traces: new Set() };
    buckets.set(component, b);
  }
  b.renders += 1;
  b.instances.add(instanceId);
  // An EMPTY array still tracks — it promotes the bucket out of `null`, so a
  // window of no-change renders reports `[]` rather than nothing.
  if (changedProps !== undefined) {
    const seen = b.changedProps ?? new Set<string>();
    for (const p of changedProps) seen.add(p);
    b.changedProps = seen;
  }
  if (trace !== undefined) b.traces.add(trace);
  if (timer === null) {
    windowStart = Date.now();
    timer = setTimeout(flushRenderTally, RENDER_TALLY_WINDOW_MS);
  }
}

/** Emit one `render.tally` per component with a non-empty window and reset. A
 *  window with zero renders never reaches here (no timer was armed). */
export function flushRenderTally(): void {
  const windowMs = Date.now() - windowStart;
  timer = null;
  const drained = buckets;
  buckets = new Map();
  for (const [component, b] of drained) {
    logRecord({
      kind: 'render.tally',
      component,
      windowMs,
      renders: b.renders,
      instances: b.instances.size,
      ...(b.changedProps === null ? {} : { changedProps: [...b.changedProps] }),
      traces: [...b.traces],
    });
  }
}

/** TEST ONLY — drop all pending tallies between cases. */
export function __resetRenderTally(): void {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  buckets = new Map();
  windowStart = 0;
}
