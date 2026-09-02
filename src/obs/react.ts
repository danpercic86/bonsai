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
import { obsEnabled, obsRedaction } from './enabled';
import { logRecord } from './log';
import { tagPath, tagValue } from './redact';
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
 *
 * STRICTMODE, DELIBERATE — do NOT "fix" this by moving the emit into an effect.
 * `each` counts and logs in the RENDER BODY, so React's development-only double
 * invoke makes a StrictMode container report two renders where the user saw one.
 * That is the correct trade: the whole point of §9.1 is to make a render that
 * should not have happened visible, and an effect-based emit would miss renders
 * that bail out before commit — exactly the ones worth catching. The doubling is
 * uniform, dev-only, and `render-storm` (§5) thresholds are set against it;
 * `aggregate` mode collapses it away for the list-heavy surfaces anyway.
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

/** Forward slash or backslash — either makes a string path-shaped. */
const PATH_SEPARATOR = /[/\\]/;

/** Path-shaped by the same conservative rule the Rust strict pass uses: any
 *  separator run is treated as a path rather than inspected further. */
function looksLikePath(s: string): boolean {
  return PATH_SEPARATOR.test(s);
}

/**
 * §7.1 — a state STRING is repo content until proven otherwise (branch name,
 * path, remote), so in `strict` it never reaches the record verbatim: it is
 * replaced by a salt-seeded ordinal (`ui:path#3.ts` / `ui:other#5`), which still
 * makes "this field flipped back and forth" visible without carrying the name.
 * `raw` is the one mode allowed to log values (§7.1), and even there the length
 * cap applies so a transition record can never carry a large repo blob.
 */
function briefString(s: string): string {
  const capped = s.length > 48 ? `${s.slice(0, 48)}…` : s;
  if (obsRedaction() === 'raw') return capped;
  if (s === '') return '';
  // Over-classification is the deliberate failure direction: `origin/main`
  // becoming `ui:path#2` costs nothing, the reverse would leak a ref name.
  const tagged = looksLikePath(s) ? tagPath(s) : tagValue('other', s);
  // No salt installed yet (§7.2) ⇒ no redactor ⇒ we cannot even mint an ordinal.
  // Report the SHAPE; never fall back to the value.
  return tagged ?? `str:${s.length}`;
}

/** Cap/redact a stringified state value so a transition record can never carry a
 *  large repo blob — or, in `strict`, any repo NAME (§7.1). Numbers, booleans and
 *  container sizes are structural, not content, and are reported as-is. */
function briefValue(v: unknown): string {
  if (v === null) return 'null';
  if (v === undefined) return 'undefined';
  const t = typeof v;
  if (t === 'string') return briefString(v as string);
  if (t === 'number' || t === 'boolean') return String(v);
  if (Array.isArray(v)) return `arr:${v.length}`;
  return `obj:${Object.keys(v as object).length}`;
}

/**
 * §9.1 — log store-field transitions. Reserved API: it is required by the
 * contract but wired to no target in v1 (§9.2 assigns it none), so it is
 * implemented and unit-tested but instrumented nowhere.
 *
 * PRIVACY: `from`/`to` route through [`briefString`] → `obs/redact.ts`, so in
 * `strict` a repo-derived value (branch name, path) is an ordinal, never the
 * name. This hook is safe to wire to a repo store BY CONSTRUCTION — it is
 * deliberately not merely "safe because nothing calls it".
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
