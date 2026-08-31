// P77 §6: owns the live tag-sync reconciliation for one open repo — the report,
// its ls-remote lifecycle, the last-wins guard and the ~10s in-memory cache.
// Extracted from RepoWorkspace so the container only wires it (keeps the git/IPC
// concern out of the already-oversized container body). Best-effort: a rejection
// degrades to `unavailable` (no toast — a routine offline check is not an error).
import { useCallback, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { RemoteInfo, TagSyncReport } from '../../ipc';
import type { TagSyncState } from '../sidebar/TagsSection';

export interface UseTagSync {
  report: TagSyncReport | null;
  state: TagSyncState;
  /** The remote name the check targets (resolved the same way Rust does: origin,
   *  else the first configured remote), or null when none is configured. Exposed
   *  independently of a successful report so the §2.3 offline line can still name
   *  the remote on the cold-start-offline case (report is null then). */
  remote: string | null;
  /** Unix secs of the last successful check (for the "last checked" tooltip). */
  checkedAt: number | null;
  /** Run a reconciliation. `force` (manual refresh / focus rescan) bypasses the
   *  cache but is a no-op until the Tags section has been opened once. */
  refetch: (opts?: { force?: boolean }) => Promise<void>;
  /** Reset to the pristine (no-check) state on repo switch / close. */
  clear: () => void;
}

export function useTagSync(repoId: string, remotes: RemoteInfo[]): UseTagSync {
  const [report, setReport] = useState<TagSyncReport | null>(null);
  const [state, setState] = useState<TagSyncState>('idle');
  const [checkedAt, setCheckedAt] = useState<number | null>(null);
  const remoteCount = remotes.length;
  // Mirror Rust's default-remote resolution so the offline line can name the
  // remote even before/without a successful report (origin, else first).
  const defaultRemote = useMemo(() => {
    const origin = remotes.find((r) => r.name === 'origin');
    if (origin !== undefined) return origin.name;
    return remotes[0]?.name ?? null;
  }, [remotes]);
  const reqId = useRef(0);
  const lastFetch = useRef(0);
  // Latest-state mirror so the force path can read the current state without
  // widening the callback's deps (would re-create it on every check).
  const stateRef = useRef<TagSyncState>('idle');
  stateRef.current = state;

  const refetch = useCallback(
    async (opts?: { force?: boolean }) => {
      const force = opts?.force ?? false;
      if (remoteCount === 0) {
        // §2.4: no remote → feature absent. Keep the tags list, drop any report.
        reqId.current += 1;
        setState('idle');
        setReport(null);
        return;
      }
      if (force && stateRef.current === 'idle') return;
      const now = Date.now();
      if (!force && stateRef.current === 'ready' && now - lastFetch.current < 10_000) {
        return; // within the cache window — reuse the last verdict
      }
      const id = ++reqId.current;
      // Stamp the cache clock at initiation (the guard above only suppresses when
      // the last attempt reached `ready`, so a failed check never self-suppresses).
      lastFetch.current = now;
      setState('checking');
      try {
        // Pass null → Rust resolves the default remote (origin, else first); the
        // report echoes which remote it queried for every label/tooltip.
        const r = await ipc.listTagSync(repoId, null);
        if (id !== reqId.current) return;
        setReport(r);
        setState('ready');
        setCheckedAt(Math.floor(Date.now() / 1000));
      } catch {
        if (id !== reqId.current) return;
        // §2.3: degrade quietly — no error banner, no toast. Keep the last
        // `checkedAt` for the "last checked" tooltip.
        setState('unavailable');
      }
    },
    [repoId, remoteCount],
  );

  const clear = useCallback(() => {
    reqId.current += 1;
    lastFetch.current = 0;
    setReport(null);
    setState('idle');
    setCheckedAt(null);
  }, []);

  // Prefer the report's echoed remote (authoritative once a check succeeded);
  // fall back to the resolved default so the offline line is never nameless.
  const remote = report?.remote ?? defaultRemote;

  return { report, state, remote, checkedAt, refetch, clear };
}
