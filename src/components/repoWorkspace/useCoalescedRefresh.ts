import { useCallback, useRef } from 'react';
import { useTracedEffect } from '../../obs/react';
import { createRefreshCoalescer, type RefreshCoalescer } from './refreshCoalescer';
import {
  armEcho,
  clearEchoSuppression,
  disarmEcho,
  echoTraceFor,
  isEchoSuppressed,
} from './echoSuppression';
import { type RefreshScope, unionScopes } from './refreshScope';
import { logRecord } from '../../obs/log';
import { currentTrace } from '../../obs/trace';
import type { TraceId } from '../../obs/types';

/** P85/P86a measurement: bump test-visible tallies once per EXECUTED refresh
 *  round (leading or trailing) — total via `window.__bonsaiRefreshRounds` and a
 *  per-scope breakdown via `window.__bonsaiRefreshScopes`. Lets vitest / the e2e
 *  harness assert exactly-one-round-per-mutation AND scope-per-mutation without
 *  touching RepoWorkspace. DEV/test only — stripped from production builds. */
function countRefreshRound(scope: RefreshScope): void {
  if (import.meta.env.DEV || import.meta.env.MODE === 'test') {
    const g = globalThis as {
      __bonsaiRefreshRounds?: number;
      __bonsaiRefreshScopes?: Record<string, number>;
    };
    g.__bonsaiRefreshRounds = (g.__bonsaiRefreshRounds ?? 0) + 1;
    const scopes = (g.__bonsaiRefreshScopes ??= {});
    scopes[scope] = (scopes[scope] ?? 0) + 1;
  }
}

// P81/P85 §6 — React hook binding one coalescer instance + the shared echo
// registry to a single canonical refresh round for `repoId`. All refresh entry
// points funnel through `refresh`.

export type RefreshOrigin =
  | 'mutation' // a local git write op just completed → arms echo suppression
  | 'external' // P86a: a backend-CONFIRMED genuine change (repo-changed reason
  //   "fetch"/"tags", tag-auto-sync) — NOT our own fs echo, so it bypasses echo
  //   suppression and never arms it.
  | 'manual' // Refresh button — always runs
  | 'activation' // tab flip to active — always runs
  | 'focus' // window focus — always runs
  | 'watcher'; // raw notify repo-changed (reason "fs"/unknown) — echo-gated

export interface UseCoalescedRefresh {
  /** Run a coalesced refresh round for `origin` at `scope`. 'watcher' resolves
   *  immediately (no round) while the self-echo window is active; all other
   *  origins always enqueue. Resolves when the serving round settles.
   *
   *  P91 §2.4/§2.5: `trace` is the arming gesture's `TraceId`, captured at the
   *  caller's SYNCHRONOUS entry and threaded by value (the ambient is already
   *  cleared by the time a post-`await` handler calls this). A `mutation` records
   *  it as the echo cause; every origin contributes it to the round's collapse
   *  evidence. Omitted ⇒ falls back to a synchronous `currentTrace()` read (right
   *  for the manual-refresh button, which calls in its own gesture's sync
   *  extent) or `undefined` — never a guessed stale trace. */
  refresh(origin: RefreshOrigin, scope: RefreshScope, trace?: TraceId): Promise<void>;
}

/** Binds one coalescer instance + the shared echo registry to `run` for `repoId`.
 *  `run` is the canonical refresh round (RepoWorkspace's `runRefreshRound`); it
 *  receives the scope the round should execute. The coalescer is created once;
 *  `run` is kept current via a ref so identity churn never rebuilds it. */
export function useCoalescedRefresh(
  repoId: string,
  run: (scope: RefreshScope) => Promise<void>,
): UseCoalescedRefresh {
  const runRef = useRef(run);
  runRef.current = run;
  // P117 §2.2 — the coalescer closure below is built ONCE, so `repoId` has to be
  // read through a ref exactly like `run`: a captured value would attribute
  // every later round to whichever repo was open at mount, which is precisely
  // the cross-repo confusion the dimension exists to remove.
  const repoIdRef = useRef(repoId);
  repoIdRef.current = repoId;

  // P86a: pending-scope accumulator — the union of every scope requested since
  // the last executed round started. Read+cleared when a round begins, so the
  // coalescer's trailing round widens to the union of what collapsed into it.
  const pendingScopesRef = useRef<Set<RefreshScope>>(new Set());
  // P91 §2.5 — causality accumulators, drained in lockstep with pendingScopesRef
  // when a round executes: the distinct origins + contributing traces that
  // collapsed into this round, a request counter (to report `collapsed`), the
  // wall time the first request entered, and a monotonic round number.
  const pendingTracesRef = useRef<Set<TraceId>>(new Set());
  const pendingOriginsRef = useRef<Set<string>>(new Set());
  const pendingCountRef = useRef(0);
  const roundNoRef = useRef(0);

  const coalescerRef = useRef<RefreshCoalescer | null>(null);
  const coalescer: RefreshCoalescer = (coalescerRef.current ??= createRefreshCoalescer(() => {
    // The coalescer invokes this exactly once per executed round, so draining the
    // pending scopes here yields the ONE scope this round runs (leading, or the
    // trailing it collapsed into) — and counts rounds, not requests.
    const scope = unionScopes(pendingScopesRef.current);
    // Snapshot the collapse evidence BEFORE draining, then reset so the next round
    // accumulates cleanly. These are held by value for the settle-time emit below.
    const contributingTraces = [...pendingTracesRef.current];
    const origins = [...pendingOriginsRef.current];
    const collapsed = Math.max(0, pendingCountRef.current - 1);
    pendingScopesRef.current.clear();
    pendingTracesRef.current.clear();
    pendingOriginsRef.current.clear();
    pendingCountRef.current = 0;
    roundNoRef.current += 1;
    const round = roundNoRef.current;
    // Read at round START: this is the repo the round executes for, even if the
    // user switches repos while it settles.
    const repo = repoIdRef.current;
    countRefreshRound(scope);
    // §2.5 — ONE record per executed round, emitted when the round SETTLES so `ms`
    // is the round's execution duration (matching `ipc.result`/`span` semantics),
    // not the coalescing delay. >1 contributing trace IS the collapse evidence.
    // The record carries no `trace` (it is an aggregate); the continuation is
    // unbound, so logRecord stamps `trace: undefined` rather than a wrong ambient.
    const runStart = Date.now();
    return runRef.current(scope).finally(() => {
      logRecord({
        kind: 'refresh',
        round,
        scope,
        origins,
        contributingTraces,
        collapsed,
        ms: Date.now() - runStart,
        // P117 §2.2 — the repo dimension, RAW and unredacted. Without it
        // `redundant-refresh` keys on `scope` alone and every pair of repos
        // refreshing together reads as one repo refreshing twice (17 of 26
        // firings in the measured 5-repo session). Must stay byte-identical to
        // the `repoId` the Rust `graph.get` span reports, so no `tagPath` here.
        repo,
      });
    });
  }));

  // Drop this repoId's suppression window when the repo changes or the container
  // unmounts (tab close), so the module-level registry cannot grow unbounded.
  // P91 §9.2 row 1a — instrumented as effect 'coalescedRefresh'.
  useTracedEffect(
    'RepoWorkspace',
    'coalescedRefresh',
    () => {
      return () => clearEchoSuppression(repoId);
    },
    [repoId],
    ['repoId'],
  );

  const refresh = useCallback(
    (origin: RefreshOrigin, scope: RefreshScope, trace?: TraceId): Promise<void> => {
      // §2.5 — a `mutation` threads its trace explicitly (post-await, ambient is
      // gone); other origins may run inside their own gesture's synchronous
      // extent (e.g. the manual-refresh button), so fall back to a sync ambient
      // read — never a stale one.
      const effTrace = trace ?? currentTrace()?.trace;
      // Only the raw notify watcher is echo-gated. 'external' (backend-confirmed
      // genuine change) bypasses suppression by design — CI-1: a fetch's async
      // tag-sync must refresh even inside the fetch's own armed window.
      if (origin === 'watcher' && isEchoSuppressed(repoId)) {
        // §2.4 — the primary double-trigger evidence: a self-caused fs echo we
        // dropped, attributed to the mutation that armed the window. Frontend
        // count fields are 0 (the real notify batch is logged Rust-side).
        logRecord({
          kind: 'watcher',
          paths: 0,
          relevant: 0,
          debounceMs: 0,
          fired: false,
          suppressed: true,
          suppressReason: 'echo',
          ...(echoTraceFor(repoId) ? { causedBy: echoTraceFor(repoId) } : {}),
        });
        return Promise.resolve();
      }
      pendingScopesRef.current.add(scope);
      pendingOriginsRef.current.add(origin);
      if (effTrace !== undefined) pendingTracesRef.current.add(effTrace);
      pendingCountRef.current += 1;
      if (origin !== 'mutation') return coalescer.request();
      // P85 A2: open the span BEFORE enqueuing, close it when THIS caller's
      // serving round (leading, or the trailing it collapsed into) settles. The
      // nesting count keeps overlapping mutations suppressed until all settle;
      // the tail then applies once. Round-duration-independent by construction.
      // §2.4 — record the arming trace so a later echo can name its cause.
      armEcho(repoId, effTrace);
      return coalescer.request().finally(() => disarmEcho(repoId));
    },
    [repoId, coalescer],
  );

  return { refresh };
}
