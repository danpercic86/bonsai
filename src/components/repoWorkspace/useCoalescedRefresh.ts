import { useCallback, useEffect, useRef } from 'react';
import { createRefreshCoalescer, type RefreshCoalescer } from './refreshCoalescer';
import { armEcho, clearEchoSuppression, isEchoSuppressed } from './echoSuppression';

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
  const coalescer: RefreshCoalescer = (coalescerRef.current ??= createRefreshCoalescer(() =>
    runRef.current(),
  ));

  // Drop this repoId's suppression window when the repo changes or the container
  // unmounts (tab close), so the module-level registry cannot grow unbounded.
  useEffect(() => {
    return () => clearEchoSuppression(repoId);
  }, [repoId]);

  const refresh = useCallback(
    (origin: RefreshOrigin): Promise<void> => {
      if (origin === 'watcher' && isEchoSuppressed(repoId)) return Promise.resolve();
      if (origin === 'mutation') armEcho(repoId);
      return coalescer.request();
    },
    [repoId, coalescer],
  );

  return { refresh };
}
