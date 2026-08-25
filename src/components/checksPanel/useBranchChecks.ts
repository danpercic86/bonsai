// P90: container hook for the per-branch CI Checks tab. Owns the forge-context
// bootstrap (provider/auth detection) + a single-sha `forgeCommitStatuses` fetch
// for the selected branch tip. Last-wins reqId guard + 300 ms debounce mirror
// `useForgeSignals`. Errors are surfaced (this is a user action surface, unlike
// the silent badge cache). Only fetches while the tab is active.
import { useCallback, useEffect, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { CommitStatus, ForgeRepoContext } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { ChecksTarget } from './checksTarget';

export type ChecksState =
  | { kind: 'idle' } // no branch selected
  | { kind: 'loading'; target: ChecksTarget }
  | { kind: 'noForge'; target: ChecksTarget } // provider === 'unknown'
  | { kind: 'connect'; target: ChecksTarget } // provider known, not authenticated
  // fetched, no rows to show — `reason` splits §4.4/4.5/4.6:
  //  'no-upstream' → branch not pushed; 'waiting' → pushed, nothing reported yet;
  //  'configured'  → forge returned an empty set.
  | {
      kind: 'noChecks';
      target: ChecksTarget;
      reason: 'no-upstream' | 'waiting' | 'configured';
    }
  // `stale` carries the last-good status when a REFETCH fails over shown data
  // (§4.10 stale-while-error): the panel keeps the header + rows under the banner.
  | { kind: 'error'; target: ChecksTarget; message: string; stale: CommitStatus | null }
  | { kind: 'loaded'; target: ChecksTarget; status: CommitStatus };

export interface UseBranchChecksResult {
  state: ChecksState;
  /** The resolved forge context (for the account header); null until first load. */
  ctx: ForgeRepoContext | null;
  /** Epoch-ms of the last successful load; null before the first. */
  lastUpdated: number | null;
  /** Epoch-ms of the last FAILED refetch that still has stale data shown; null
   *  otherwise. Drives the "Couldn't refresh — tried HH:MM." freshness copy. */
  failedRefreshAt: number | null;
  /** True while a (re)fetch is in flight over already-shown data. */
  refreshing: boolean;
  /** Manual ⟳ / focus refresh — forces a refetch of the current target. */
  refresh(): void;
  /** After a successful connect, re-run the bootstrap. */
  reconnect(): void;
}

export function useBranchChecks(deps: {
  repoId: string;
  target: ChecksTarget | null;
  /** Bumped on fetch/pull to force a refetch. */
  refreshSeq: number;
  /** Only fetch while the Checks tab is active (avoid work for a hidden panel). */
  active: boolean;
}): UseBranchChecksResult {
  const { repoId, target, refreshSeq, active } = deps;

  const [state, setState] = useState<ChecksState>({ kind: 'idle' });
  const [ctx, setCtx] = useState<ForgeRepoContext | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  const [failedRefreshAt, setFailedRefreshAt] = useState<number | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [manualSeq, setManualSeq] = useState(0);
  const [connectSeq, setConnectSeq] = useState(0);

  const reqRef = useRef(0);
  // Last successfully-loaded status, keyed by tip — the source for stale-while-error
  // (read synchronously in the catch, unlike a setState updater's closure).
  const lastGoodRef = useRef<{ tip: string; status: CommitStatus } | null>(null);

  const tip = target?.tip ?? null;

  useEffect(() => {
    if (!active) return;
    if (target === null || tip === null) {
      setState({ kind: 'idle' });
      setRefreshing(false);
      return;
    }

    const id = ++reqRef.current;
    // Keep already-shown rows visible while refetching (stale-while-revalidate).
    setState((prev) =>
      prev.kind === 'loaded' && prev.target.tip === tip ? prev : { kind: 'loading', target },
    );
    setRefreshing(true);

    const timer = setTimeout(() => {
      void (async () => {
        try {
          const context = await ipc.forgeRepoContext(repoId);
          if (id !== reqRef.current) return;
          setCtx(context);
          if (context.provider === 'unknown') {
            setState({ kind: 'noForge', target });
            setRefreshing(false);
            return;
          }
          if (!context.authenticated) {
            setState({ kind: 'connect', target });
            setRefreshing(false);
            return;
          }
          const statuses = await ipc.forgeCommitStatuses(repoId, [tip]);
          if (id !== reqRef.current) return;
          const status = statuses.find((s) => s.sha === tip) ?? statuses[0] ?? null;
          if (status === null || status.contexts.length === 0) {
            const reason = !target.hasUpstream
              ? 'no-upstream'
              : status === null || status.state === 'none'
                ? 'waiting'
                : 'configured';
            setState({ kind: 'noChecks', target, reason });
            lastGoodRef.current = null;
          } else {
            setState({ kind: 'loaded', target, status });
            lastGoodRef.current = { tip, status };
          }
          setLastUpdated(Date.now());
          setFailedRefreshAt(null);
          setRefreshing(false);
        } catch (e: unknown) {
          if (id !== reqRef.current) return;
          // Stale-while-error (§4.10): if we have last-good rows for this same tip,
          // keep them under the banner instead of blanking the panel.
          const stale =
            lastGoodRef.current?.tip === tip ? lastGoodRef.current.status : null;
          setState({ kind: 'error', target, message: errorMessage(e), stale });
          setFailedRefreshAt(stale !== null ? Date.now() : null);
          setRefreshing(false);
        }
      })();
    }, 300);

    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoId, tip, refreshSeq, manualSeq, connectSeq, active]);

  const refresh = useCallback(() => setManualSeq((s) => s + 1), []);
  const reconnect = useCallback(() => setConnectSeq((s) => s + 1), []);

  return { state, ctx, lastUpdated, failedRefreshAt, refreshing, refresh, reconnect };
}
