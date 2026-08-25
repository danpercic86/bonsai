import { useCallback, useEffect, useReducer, useRef } from 'react';
import { ipc } from '../../ipc';
import type { FileDiff } from '../../ipc';
import { errorMessage } from '../../utils/errors';

// P89: per-file PR diff fetch orchestration (mirrors DiffBrowser's bounded
// queue + component-local cache, §10). Unlike DiffBrowser this is LAZY — a file
// is only enqueued when its row expands (PR diffs can be large; never fetch all
// at once). Keyed `${mergeBaseOid}:${headOid}:${path}` so a head advance (new
// oids) never serves a stale payload. No IPC leaks into the presentational row.

/** At most 4 per-file hunk fetches in flight (matches DiffBrowser §6.4). */
const MAX_CONCURRENCY = 4;

/** Per-file fetch state. `idle` = queued but not yet fetched. */
export type PrFileState =
  | { state: 'idle' }
  | { state: 'loading' }
  | { state: 'ready'; diff: FileDiff }
  | { state: 'error'; error: string };

export interface UsePrFileDiffs {
  /** Current cache entry for a path (undefined = never requested / binary). */
  getEntry(path: string): PrFileState | undefined;
  /** Enqueue a file for fetch (idempotent per cache key). No-op for binaries. */
  requestFile(path: string, origPath: string | null): void;
  /** Drop the cache entry and refetch (per-file error Retry). */
  retryFile(path: string, origPath: string | null): void;
}

export function usePrFileDiffs(
  repoId: string,
  mergeBaseOid: string,
  headOid: string,
): UsePrFileDiffs {
  const cacheRef = useRef<Map<string, PrFileState>>(new Map());
  const [, bump] = useReducer((n: number) => n + 1, 0);
  const queueRef = useRef<Array<{ path: string; origPath: string | null }>>([]);
  const inFlightRef = useRef(0);
  const cancelledRef = useRef(false);

  // Read the latest oids/repoId inside the stable pump without churning it.
  const ctxRef = useRef({ repoId, mergeBaseOid, headOid });
  ctxRef.current = { repoId, mergeBaseOid, headOid };

  const keyOf = useCallback(
    (path: string) => `${ctxRef.current.mergeBaseOid}:${ctxRef.current.headOid}:${path}`,
    [],
  );

  const pump = useCallback(() => {
    if (cancelledRef.current) return;
    while (inFlightRef.current < MAX_CONCURRENCY && queueRef.current.length > 0) {
      const next = queueRef.current.shift();
      if (next === undefined) break;
      const { repoId: rid, mergeBaseOid: base, headOid: head } = ctxRef.current;
      const key = `${base}:${head}:${next.path}`;
      const entry = cacheRef.current.get(key);
      if (entry === undefined || entry.state !== 'idle') continue; // superseded
      cacheRef.current.set(key, { state: 'loading' });
      inFlightRef.current += 1;
      bump();
      void ipc
        .forgePrFileDiff(rid, base, head, next.path, next.origPath, false, false)
        .then(
          (diff) => {
            cacheRef.current.set(key, { state: 'ready', diff });
          },
          (e: unknown) => {
            cacheRef.current.set(key, { state: 'error', error: errorMessage(e) });
          },
        )
        .finally(() => {
          inFlightRef.current -= 1;
          bump();
          pump();
        });
    }
  }, []);

  const requestFile = useCallback(
    (path: string, origPath: string | null) => {
      if (cancelledRef.current) return;
      const key = keyOf(path);
      if (cacheRef.current.has(key)) return; // already queued/loading/ready/error
      cacheRef.current.set(key, { state: 'idle' });
      queueRef.current.push({ path, origPath });
      pump();
    },
    [keyOf, pump],
  );

  const retryFile = useCallback(
    (path: string, origPath: string | null) => {
      cacheRef.current.delete(keyOf(path));
      requestFile(path, origPath);
    },
    [keyOf, requestFile],
  );

  const getEntry = useCallback((path: string) => cacheRef.current.get(keyOf(path)), [keyOf]);

  // Cancel in-flight re-pumps on unmount; reset on remount (StrictMode-safe).
  useEffect(() => {
    cancelledRef.current = false;
    return () => {
      cancelledRef.current = true;
    };
  }, []);

  return { getEntry, requestFile, retryFile };
}
