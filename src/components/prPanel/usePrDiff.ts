import { useCallback, useEffect, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { AppError, PrDiffStats } from '../../ipc';

// P89: owns the `forgePrDiff` auto-fetch for the open PR + the re-open cache
// (contract §5). Keyed `${repoId}:${number}` at module scope so the stats
// survive the detail unmount/remount (going back to the list and reopening the
// same PR shows the files section INSTANTLY in `ready`, no skeleton flash).
// When head advanced (summary.headSha changed) we refetch but keep the previous
// rows underneath, dimmed via `.diff-stale` (the `stale` flag drives that).

export type PrDiffStatus = 'loading' | 'ready' | 'empty' | 'error';

/** Error cause, mapped from the AppError kind, so the section shows the right
 *  human copy (contract §4) without leaking raw libgit2/forge text. */
export type PrDiffErrorCause =
  | 'network'
  | 'auth'
  | 'unresolved'
  | 'rateLimited'
  | 'generic';

export interface UsePrDiff {
  status: PrDiffStatus;
  /** Non-null once loaded (or a stale prior value during a head-advance refetch). */
  stats: PrDiffStats | null;
  /** Prior rows are shown dimmed while a head-advance refetch is in flight. */
  stale: boolean;
  errorCause: PrDiffErrorCause;
  retry(): void;
}

interface CacheEntry {
  headSha: string;
  stats: PrDiffStats;
}

const statsCache = new Map<string, CacheEntry>();

function causeFrom(e: unknown): PrDiffErrorCause {
  const kind = (e as Partial<AppError> | null)?.kind;
  switch (kind) {
    case 'networkError':
      return 'network';
    case 'authFailed':
    case 'forgeAuthRequired':
      return 'auth';
    case 'forgeRateLimited':
      return 'rateLimited';
    case 'git':
      return 'unresolved';
    default:
      return 'generic';
  }
}

export function usePrDiff(repoId: string, number: number, headSha: string): UsePrDiff {
  const cacheKey = `${repoId}:${number}`;
  const cached = statsCache.get(cacheKey);
  const freshHit = cached !== undefined && cached.headSha === headSha;

  const [status, setStatus] = useState<PrDiffStatus>(() =>
    freshHit ? (cached.stats.files.length > 0 ? 'ready' : 'empty') : 'loading',
  );
  const [stats, setStats] = useState<PrDiffStats | null>(() => cached?.stats ?? null);
  const [errorCause, setErrorCause] = useState<PrDiffErrorCause>('generic');
  // Stale = a prior (different-head) result is showing while we recompute.
  const [stale, setStale] = useState<boolean>(() => cached !== undefined && !freshHit);

  const cancelledRef = useRef(false);
  const [reloadTick, setReloadTick] = useState(0);

  const retry = useCallback(() => setReloadTick((n) => n + 1), []);

  useEffect(() => {
    cancelledRef.current = false;
    const entry = statsCache.get(cacheKey);
    const isFresh = entry !== undefined && entry.headSha === headSha && reloadTick === 0;
    if (isFresh) {
      setStats(entry.stats);
      setStatus(entry.stats.files.length > 0 ? 'ready' : 'empty');
      setStale(false);
      return () => {
        cancelledRef.current = true;
      };
    }
    // Head advanced (or first load / explicit retry): fetch. Keep any prior
    // rows underneath, dimmed, so the panel doesn't jump.
    if (entry !== undefined) {
      setStats(entry.stats);
      setStale(true);
    } else {
      setStale(false);
    }
    setStatus('loading');
    void ipc.forgePrDiff(repoId, number).then(
      (result) => {
        if (cancelledRef.current) return;
        statsCache.set(cacheKey, { headSha, stats: result });
        setStats(result);
        setStale(false);
        setStatus(result.files.length > 0 ? 'ready' : 'empty');
      },
      (e: unknown) => {
        if (cancelledRef.current) return;
        setErrorCause(causeFrom(e));
        setStale(false);
        setStatus('error');
        // Keep `stats` untouched so the header can still show prior local counts
        // if we had them; the section body shows the error banner.
      },
    );
    return () => {
      cancelledRef.current = true;
    };
  }, [cacheKey, repoId, number, headSha, reloadTick]);

  return { status, stats, stale, errorCause, retry };
}
