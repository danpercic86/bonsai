import { useCallback, useEffect, useRef } from 'react';
import { createRefreshCoalescer, type RefreshCoalescer } from './refreshCoalescer';
import { armEcho, clearEchoSuppression, disarmEcho, isEchoSuppressed } from './echoSuppression';
import { type RefreshScope, unionScopes } from './refreshScope';

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
   *  origins always enqueue. Resolves when the serving round settles. */
  refresh(origin: RefreshOrigin, scope: RefreshScope): Promise<void>;
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

  // P86a: pending-scope accumulator — the union of every scope requested since
  // the last executed round started. Read+cleared when a round begins, so the
  // coalescer's trailing round widens to the union of what collapsed into it.
  const pendingScopesRef = useRef<Set<RefreshScope>>(new Set());

  const coalescerRef = useRef<RefreshCoalescer | null>(null);
  const coalescer: RefreshCoalescer = (coalescerRef.current ??= createRefreshCoalescer(() => {
    // The coalescer invokes this exactly once per executed round, so draining the
    // pending scopes here yields the ONE scope this round runs (leading, or the
    // trailing it collapsed into) — and counts rounds, not requests.
    const scope = unionScopes(pendingScopesRef.current);
    pendingScopesRef.current.clear();
    countRefreshRound(scope);
    return runRef.current(scope);
  }));

  // Drop this repoId's suppression window when the repo changes or the container
  // unmounts (tab close), so the module-level registry cannot grow unbounded.
  useEffect(() => {
    return () => clearEchoSuppression(repoId);
  }, [repoId]);

  const refresh = useCallback(
    (origin: RefreshOrigin, scope: RefreshScope): Promise<void> => {
      // Only the raw notify watcher is echo-gated. 'external' (backend-confirmed
      // genuine change) bypasses suppression by design — CI-1: a fetch's async
      // tag-sync must refresh even inside the fetch's own armed window.
      if (origin === 'watcher' && isEchoSuppressed(repoId)) return Promise.resolve();
      pendingScopesRef.current.add(scope);
      if (origin !== 'mutation') return coalescer.request();
      // P85 A2: open the span BEFORE enqueuing, close it when THIS caller's
      // serving round (leading, or the trailing it collapsed into) settles. The
      // nesting count keeps overlapping mutations suppressed until all settle;
      // the tail then applies once. Round-duration-independent by construction.
      armEcho(repoId);
      return coalescer.request().finally(() => disarmEcho(repoId));
    },
    [repoId, coalescer],
  );

  return { refresh };
}
