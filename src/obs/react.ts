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
import { isFreeTextParam, isSensitiveParam } from './rawArgPolicy';
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
 * uniform, dev-only, and `render-storm` (§5) thresholds are set against it.
 *
 * `aggregate` does NOT collapse the doubling — it collapses the RECORD count, not
 * the render count. `renderTally.ts` adds 1 to `renders` per render body, while
 * `instanceId` below is minted once per instance, so under StrictMode `renders`
 * doubles and `instances` does not: the `renders / instances` ratio in a
 * `render.tally` is **2x inflated in dev**. The `render-storm` rule is
 * `renders > 3 * instances` (`src-tauri/src/obs/anomaly.rs`), so with the
 * inflation any window in which a component genuinely commits twice per instance
 * already reads as 4 renders per instance and trips the rule. Read dev
 * `render-storm` warnings on aggregate surfaces with that factor in mind (and any
 * renders-per-instance budget asserted in a StrictMode test likewise).
 *
 * OMITTING `props` means NOT TRACKED, and the emitted record omits
 * `changedProps` entirely to say so. Passing a bag that saw no change reports
 * `changedProps: []` — "tracked, nothing changed". Those are different claims
 * and the wire keeps them apart, so do not pass `{}` to silence the type.
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
  // `undefined` all the way through: a call site that names no props is NOT
  // TRACKED, and coercing to `{}` here would make every record claim "tracked,
  // nothing changed" — a diagnostic with exactly one possible value.
  let changed: string[] | undefined;
  if (props !== undefined) {
    changed = changedNames(prevProps.current, props);
    prevProps.current = { ...props };
  }
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
    ...(changed !== undefined && changed.length > 0 ? { changedProps: changed } : {}),
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
 *
 * `raw` widens **identifier** fidelity only — never content fidelity, the same
 * one rule A26 applies to `args`. So the raw branch is gated on the FIELD NAME
 * through the shared vocabulary (`isFreeTextParam`/`isSensitiveParam`): a field
 * called `message`, `query`, `note` or `token` takes the strict ordinal path in
 * BOTH modes, so wiring this hook to a store holding a commit-message draft or
 * the search input cannot write user prose to disk. `state` records ride outside
 * the writer's `raw_args` backstop, which guards `args` only — this is the sole
 * gate, hence a name gate rather than a value heuristic.
 *
 * The 48-char cap applies either way, so a transition record can never carry a
 * large repo blob.
 */
function briefString(field: string, s: string): string {
  if (s === '') return '';
  const isContentField = isFreeTextParam(field) || isSensitiveParam(field);
  if (obsRedaction() === 'raw' && !isContentField) {
    return s.length > 48 ? `${s.slice(0, 48)}…` : s;
  }
  // Over-classification is the deliberate failure direction: `origin/main`
  // becoming `ui:path#2` costs nothing, the reverse would leak a ref name.
  const tagged = looksLikePath(s) ? tagPath(s) : tagValue('other', s);
  // No salt installed yet (§7.2) ⇒ no redactor ⇒ we cannot even mint an ordinal.
  // Report the SHAPE; never fall back to the value.
  return tagged ?? `str:${s.length}`;
}

/** Cap/redact a stringified state value so a transition record can never carry a
 *  large repo blob — or, in `strict`, any repo NAME (§7.1). `field` selects the
 *  raw-mode policy in [`briefString`]. Numbers, booleans and container sizes are
 *  structural, not content, and are reported as-is. */
function briefValue(field: string, v: unknown): string {
  if (v === null) return 'null';
  if (v === undefined) return 'undefined';
  const t = typeof v;
  if (t === 'string') return briefString(field, v as string);
  if (t === 'number' || t === 'boolean') return String(v);
  if (Array.isArray(v)) return `arr:${v.length}`;
  return `obj:${Object.keys(v as object).length}`;
}

/**
 * §9.1 — log store-field transitions. Reserved API: it is required by the
 * contract but wired to no target in v1 (§9.2 assigns it none), so it is
 * implemented and unit-tested but instrumented nowhere.
 *
 * PRIVACY: `from`/`to` route through [`briefString`], so `strict` turns a
 * repo-derived value (branch name, path) into an ordinal, never the name — and
 * `raw` logs a value verbatim only for a field whose NAME is neither free-text
 * nor credential-shaped, capped at 48 chars. A `message`/`query`/`token` field
 * is an ordinal in BOTH modes. That is what makes this hook safe to wire to a
 * repo store BY CONSTRUCTION, rather than merely "safe because nothing calls it".
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
          from: briefValue(field, before[field]),
          to: briefValue(field, values[field]),
        });
      }
    }
  }
  prev.current = { ...values };
}
