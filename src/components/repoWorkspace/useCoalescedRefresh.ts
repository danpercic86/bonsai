import { useCallback, useEffect, useRef } from 'react';
import { createRefreshCoalescer, type RefreshCoalescer } from './refreshCoalescer';
import { armEcho, clearEchoSuppression, disarmEcho, isEchoSuppressed } from './echoSuppression';

/** P85 measurement: bump a test-visible tally once per EXECUTED refresh round
 *  (leading or trailing). Lets vitest / the e2e harness assert
 *  exactly-one-round-per-mutation via `window.__bonsaiRefreshRounds` without
 *  touching RepoWorkspace. DEV/test only — stripped from production builds. */
function countRefreshRound(): void {
  if (import.meta.env.DEV || import.meta.env.MODE === 'test') {
    const g = globalThis as { __bonsaiRefreshRounds?: number };
    g.__bonsaiRefreshRounds = (g.__bonsaiRefreshRounds ?? 0) + 1;
  }
}

// P81 §6 — React hook binding one coalescer instance + the shared echo registry
// to a single canonical refresh round for `repoId`. All refresh entry points
// (mutation / manual / activation / focus / watcher) funnel through `refresh`.

export type RefreshOrigin =
  | 'mutation' // a git write op just completed → arms echo suppression
  | 'manual' // Refresh button — always runs
  | 'activation' // tab flip to active — always runs
  | 'focus' // window focus — always runs
  | 'watcher'; // repo-changed event — gated by echo suppression

export interface UseCoalescedRefresh {
  /** Run a coalesced refresh round for `origin`. 'watcher' resolves immediately
   *  (no round) while the self-echo window is active; all other origins always
   *  enqueue. Resolves when the serving round settles. */
  refresh(origin: RefreshOrigin): Promise<void>;
}

/** Binds one coalescer instance + the shared echo registry to `run` for `repoId`.
 *  `run` is the canonical refresh round (today's refreshAll body). The coalescer
 *  is created once; `run` is kept current via a ref so identity churn never
 *  rebuilds it. */
export function useCoalescedRefresh(
  repoId: string,
  run: () => Promise<void>,
): UseCoalescedRefresh {
  const runRef = useRef(run);
  runRef.current = run;

  const coalescerRef = useRef<RefreshCoalescer | null>(null);
  const coalescer: RefreshCoalescer = (coalescerRef.current ??= createRefreshCoalescer(() => {
    // The coalescer invokes this exactly once per executed round, so counting
    // here counts rounds (leading + at-most-one-trailing), not requests.
    countRefreshRound();
    return runRef.current();
  }));

  // Drop this repoId's suppression window when the repo changes or the container
  // unmounts (tab close), so the module-level registry cannot grow unbounded.
  useEffect(() => {
    return () => clearEchoSuppression(repoId);
  }, [repoId]);

  const refresh = useCallback(
    (origin: RefreshOrigin): Promise<void> => {
      if (origin === 'watcher' && isEchoSuppressed(repoId)) return Promise.resolve();
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
