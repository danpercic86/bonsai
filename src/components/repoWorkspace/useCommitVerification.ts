import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { CommitVerification, GraphLayout, VerifyStatus } from '../../ipc';
import type { PushToast } from '../../ToastContext';

/** Mirror of the Rust `MAX_VERIFY_BATCH` (argv sanity). The frontend already
 *  sends only the visible window, but chunk defensively so a very tall viewport
 *  (large overscan) can never exceed the backend cap. */
const MAX_VERIFY_BATCH = 512;

/** ~150 ms — coalesces a scroll flick into ONE verify request (contract §7.1). */
const DEBOUNCE_MS = 150;

export interface UseCommitVerification {
  /** oid → verdict for the badge draw (session cache). Empty while disabled. */
  verifyStatus: ReadonlyMap<string, VerifyStatus>;
  /** Full record (signer / key) for the commit-details signature line — reuses
   *  the same cache, so a selected commit needs no extra IPC. */
  detailsFor(oid: string): CommitVerification | undefined;
  /** From GraphCanvas: the visible (overscanned) row window changed. Maps rows
   *  → oids, collects the UNCACHED ones, and debounces one batched verify. */
  onVisibleRangeChange(first: number, last: number): void;
  /** Drop the cache + re-request the current window (Refresh action + after a
   *  successful commit, so the new HEAD verifies). */
  refresh(): void;
}

/** P58c: per-oid signature-verdict cache keyed on the graph's visible range
 *  (mirrors {@link useCommitSearch}). A commit oid is immutable, so its verdict
 *  is cache-stable for the session — invalidated only by {@link refresh}. When
 *  `enabled` is false NO request is made and the map is empty (the graph shows
 *  the faint P51 stub). A last-wins `reqId` guard drops superseded responses. */
export function useCommitVerification(deps: {
  repoId: string;
  /** Ref to the current layout — read at fetch time (not a reactive dep) so a
   *  post-commit re-request sees the freshest nodes without re-subscribing. */
  graphDataRef: { current: GraphLayout | null };
  /** graphPrefs.showSignatureBadge — gates BOTH the request and the lit draw. */
  enabled: boolean;
  pushToast: PushToast;
}): UseCommitVerification {
  const { repoId, graphDataRef, enabled, pushToast } = deps;

  const [cache, setCache] = useState<ReadonlyMap<string, CommitVerification>>(() => new Map());
  const cacheRef = useRef(cache);
  cacheRef.current = cache;

  // Latest scalars for the debounced async closures (avoid stale captures).
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const repoIdRef = useRef(repoId);
  repoIdRef.current = repoId;

  const reqIdRef = useRef(0);
  const debounceRef = useRef<number | null>(null);
  /** Latest visible window — recorded on EVERY range change (even while
   *  disabled) so a later enable re-verifies the CURRENT rows. */
  const pendingRef = useRef<{ first: number; last: number } | null>(null);

  const clearDebounce = useCallback(() => {
    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
  }, []);

  // Collect the UNCACHED visible oids and verify them in ≤ MAX_VERIFY_BATCH
  // chunks. A last-wins reqId guard drops stale / superseded responses.
  const runFetch = useCallback(async () => {
    clearDebounce();
    if (!enabledRef.current) return;
    const range = pendingRef.current;
    const layout = graphDataRef.current;
    if (range === null || layout === null) return;
    const nodes = layout.nodes;
    const first = Math.max(0, range.first);
    const last = Math.min(nodes.length - 1, range.last);
    const uncached: string[] = [];
    const seen = new Set<string>();
    for (let row = first; row <= last; row++) {
      const oid = nodes[row]?.id;
      if (oid === undefined || seen.has(oid) || cacheRef.current.has(oid)) continue;
      seen.add(oid);
      uncached.push(oid);
    }
    if (uncached.length === 0) return;
    const reqId = ++reqIdRef.current;
    const repo = repoIdRef.current;
    try {
      const collected: CommitVerification[] = [];
      for (let i = 0; i < uncached.length; i += MAX_VERIFY_BATCH) {
        const chunk = uncached.slice(i, i + MAX_VERIFY_BATCH);
        const res = await ipc.verifyCommits(repo, chunk);
        if (reqIdRef.current !== reqId) return; // superseded
        collected.push(...res.verifications);
      }
      if (collected.length === 0) return; // all omitted (unresolvable) — leave unchecked
      setCache((prev) => {
        const next = new Map(prev);
        for (const cv of collected) next.set(cv.oid, cv);
        return next;
      });
    } catch (e) {
      if (reqIdRef.current !== reqId) return;
      pushToast('error', errorMessage(e));
    }
  }, [clearDebounce, graphDataRef, pushToast]);

  const scheduleFetch = useCallback(() => {
    clearDebounce();
    debounceRef.current = window.setTimeout(() => {
      debounceRef.current = null;
      void runFetch();
    }, DEBOUNCE_MS);
  }, [clearDebounce, runFetch]);

  const onVisibleRangeChange = useCallback(
    (first: number, last: number) => {
      // Always record the latest window (so a re-enable re-fetches the CURRENT
      // rows); only actually request while enabled — "off" makes NO request.
      pendingRef.current = { first, last };
      if (!enabledRef.current) return;
      scheduleFetch();
    },
    [scheduleFetch],
  );

  const refresh = useCallback(() => {
    reqIdRef.current += 1; // drop any in-flight
    setCache((prev) => (prev.size === 0 ? prev : new Map()));
    if (enabledRef.current && pendingRef.current !== null) scheduleFetch();
    else clearDebounce();
  }, [scheduleFetch, clearDebounce]);

  // Enable / repo transitions: disabling (or switching repo) drops the cache
  // and stops fetching; enabling re-verifies the last known window. The initial
  // mount is handled by the first paint's onVisibleRangeChange (pendingRef is
  // null until then), so this does not double-fetch.
  useEffect(() => {
    reqIdRef.current += 1;
    clearDebounce();
    setCache((prev) => (prev.size === 0 ? prev : new Map()));
    if (enabled && pendingRef.current !== null) scheduleFetch();
    return clearDebounce;
  }, [enabled, repoId, scheduleFetch, clearDebounce]);

  const verifyStatus = useMemo(() => {
    const m = new Map<string, VerifyStatus>();
    for (const [oid, cv] of cache) m.set(oid, cv.status);
    return m;
  }, [cache]);

  const detailsFor = useCallback((oid: string) => cache.get(oid), [cache]);

  return { verifyStatus, detailsFor, onVisibleRangeChange, refresh };
}
