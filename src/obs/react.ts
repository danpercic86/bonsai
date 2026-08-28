/**
 * P91 §9.1 — React causality hooks (dev-mode only, zero-cost off).
 *
 * All three hooks keep **unconditional hook order**: every `useRef`/`useEffect`
 * they need is created on every render, and the `obsEnabled()` gate only decides
 * whether work is done — never whether a hook runs. This matters because the flag
 * flips live (§10): a hook that was skipped while off must not appear when on.
 *
 * `changedProps`/`changedDeps` are computed by `Object.is` and reported as the
 * caller-supplied NAMES only — never the values, which are frequently repo
 * content (§7 privacy property).
 */
import { useEffect, useRef } from 'react';
import { obsEnabled } from './enabled';
import { logRecord } from './log';
import { currentTrace } from './trace';
import { tallyRender } from './renderTally';

/** §9.1 — `each` emits one record per render; `aggregate` collapses to one
 *  `render.tally` per component per window (§9.2). */
export type RenderLogMode = 'each' | 'aggregate';

let instanceCounter = 0;

/** Names of props whose value changed vs the previous render (`Object.is`). */
function changedNames(
  prev: Record<string, unknown> | undefined,
  next: Record<string, unknown>,
): string[] {
  if (prev === undefined) return [];
  const out: string[] = [];
  for (const key of Object.keys(next)) {
    if (!Object.is(prev[key], next[key])) out.push(key);
  }
  return out;
}

/**
 * §9.1 — count renders of `component`. `each` emits one `render` record per
 * render; `aggregate` routes through `renderTally.ts` (§9.2) — one record per
 * component per 500 ms window, keyed by `component` so every row instance of a
 * list shares one tally key (never one record per row).
 */
export function useRenderCount(
  component: string,
  props?: Record<string, unknown>,
  mode: RenderLogMode = 'each',
): void {
  const count = useRef(0);
  const lastTs = useRef(0);
  const prevProps = useRef<Record<string, unknown> | undefined>(undefined);
  const instanceId = useRef<string | undefined>(undefined);
  if (!obsEnabled()) return;
  if (instanceId.current === undefined) {
    instanceCounter += 1;
    instanceId.current = `${component}#${instanceCounter}`;
  }
  const now = Date.now();
  const next = props ?? {};
  const changed = changedNames(prevProps.current, next);
  prevProps.current = { ...next };
  count.current += 1;
  const sinceMs = lastTs.current === 0 ? 0 : now - lastTs.current;
  lastTs.current = now;
  if (mode === 'aggregate') {
    tallyRender(component, instanceId.current, changed, currentTrace()?.trace);
    return;
  }
  logRecord({
    kind: 'render',
    component,
    count: count.current,
    sinceMs,
    ...(changed.length > 0 ? { changedProps: changed } : {}),
  });
}

/**
 * §9.1 — an instrumented `useEffect`. OFF path is a plain `useEffect(fn, deps)`
 * with no prev-deps copy and no extra allocation. ON, it records which of the
 * named deps changed on each run: `changedDeps.length === 0` on a re-run is the
 * double-fire signal (`effect-no-change`, §5).
 */
export function useTracedEffect(
  component: string,
  effect: string,
  fn: React.EffectCallback,
  deps: React.DependencyList,
  depNames: string[],
): void {
  const prevDeps = useRef<React.DependencyList | undefined>(undefined);
  const run = useRef(0);
  useEffect(() => {
    if (!obsEnabled()) return fn();
    const prev = prevDeps.current;
    const changed: string[] = [];
    if (prev !== undefined) {
      for (let i = 0; i < deps.length; i += 1) {
        if (!Object.is(prev[i], deps[i])) changed.push(depNames[i] ?? `#${i}`);
      }
    }
    prevDeps.current = deps;
    run.current += 1;
    logRecord({
      kind: 'effect',
      component,
      effect,
      run: run.current,
      changedDeps: changed,
      depCount: deps.length,
    });
    return fn();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}

/** Cap a stringified state value so a transition record can never carry a large
 *  repo blob. Reports type + a short prefix, not the full value. */
function briefValue(v: unknown): string {
  if (v === null) return 'null';
  if (v === undefined) return 'undefined';
  const t = typeof v;
  if (t === 'string' || t === 'number' || t === 'boolean') {
    const s = String(v);
    return s.length > 48 ? `${s.slice(0, 48)}…` : s;
  }
  if (Array.isArray(v)) return `arr:${v.length}`;
  return `obj:${Object.keys(v as object).length}`;
}

/**
 * §9.1 — log store-field transitions. Reserved API: it is required by the
 * contract but wired to no target in v1 (§9.2 assigns it none), so it is
 * implemented and unit-tested but instrumented nowhere. Its value strings are
 * brief-formatted (type + short prefix), never a full repo value.
 *
 * KNOWN LIMITATION (must fix before wiring): `briefValue` caps length but does
 * NOT run the §7 redactor, so a raw string field (e.g. a branch name) would land
 * in the log verbatim. Before this is wired to any repo-derived store, its
 * `from`/`to` must route through `obs/redact.ts` (or be restricted to
 * non-content fields). It is safe today only because it is called from nowhere.
 */
export function useStateTransitionLog(store: string, values: Record<string, unknown>): void {
  const prev = useRef<Record<string, unknown> | undefined>(undefined);
  if (!obsEnabled()) return;
  const before = prev.current;
  if (before !== undefined) {
    for (const field of Object.keys(values)) {
      if (!Object.is(before[field], values[field])) {
        logRecord({
          kind: 'state',
          store,
          field,
          from: briefValue(before[field]),
          to: briefValue(values[field]),
        });
      }
    }
  }
  prev.current = { ...values };
}
