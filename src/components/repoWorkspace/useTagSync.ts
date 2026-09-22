// P77 §6: owns the live tag-sync reconciliation for one open repo — the report,
// its ls-remote lifecycle, the last-wins guard and the ~10s in-memory cache.
// Extracted from RepoWorkspace so the container only wires it (keeps the git/IPC
// concern out of the already-oversized container body). Best-effort: a rejection
// degrades to `unavailable` (no toast — a routine offline check is not an error).
import { useCallback, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { RemoteInfo, TagSyncReport } from '../../ipc';
import type { TagSyncState } from '../sidebar/TagsSection';

/** P113b — the in-flight duplicate floor.
 *
 *  Measured: `listTagSync` ran 138 times in one session, 86 of them superseded,
 *  with 12 `dup-ipc` anomalies that were PAIRS with an identical argsHash. The
 *  pairs come from two triggers landing in the same tick (e.g. an auto-fetch
 *  completing fires both the refresh round's tagSync slice and
 *  `afterAutoFetch`) — and the ~10 s cache window could not stop them, because
 *  it lives inside the `!force` branch AND reads React state that has not
 *  re-rendered yet within a tick.
 *
 *  So the floor keys off the synchronous `lastFetch`/`inFlight` refs instead,
 *  and applies to FORCED calls too. It is deliberately tiny and gated on a
 *  request still being in flight: a forced check issued after the previous one
 *  SETTLED always runs, so no freshness is lost — only the redundant twin of a
 *  request that is already on the wire is dropped. */
const IN_FLIGHT_FLOOR_MS = 2_000;

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
  /** P77 — the auto-fetch trigger, as a stable zero-arg callback. It is
   *  `refetch({ force: true })`: `force` so the ~10 s cache guard cannot swallow a
   *  check that a just-completed network cycle made worth re-running, and still a
   *  no-op while the state is `idle` (the Tags section has never been opened), so
   *  it adds NO network call of its own — it only rides one the user opted into.
   *
   *  A local compare would not do: a fetched tag and a local-only tag both land in
   *  `refs/tags/*`, so classifying them needs the `ls-remote` this runs. */
  afterAutoFetch: () => void;
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
  // P113b: true while a check is on the wire. Synchronous (a ref, not state),
  // because the duplicate arrives in the SAME tick as the original.
  const inFlight = useRef(false);
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
      // P113b: drop the redundant twin of a check that is ALREADY on the wire —
      // the identical `listTagSync(repoId, null)` would only supersede it. This
      // runs before the `!force` cache guard because the duplicates it kills are
      // mostly forced ones (see IN_FLIGHT_FLOOR_MS).
      if (inFlight.current && now - lastFetch.current < IN_FLIGHT_FLOOR_MS) return;
      if (!force && stateRef.current === 'ready' && now - lastFetch.current < 10_000) {
        return; // within the cache window — reuse the last verdict
      }
      const id = ++reqId.current;
      // Stamp the cache clock at initiation (the guard above only suppresses when
      // the last attempt reached `ready`, so a failed check never self-suppresses).
      lastFetch.current = now;
      inFlight.current = true;
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
        // P113b: a FAILED check must never self-suppress, so reset the clock —
        // the next trigger (retry, manual refresh) goes straight to the remote.
        lastFetch.current = 0;
      } finally {
        // Only the latest request owns the flag; a superseded one must not
        // unlock a newer check that is still running.
        if (id === reqId.current) inFlight.current = false;
      }
    },
    [repoId, remoteCount],
  );

  const afterAutoFetch = useCallback(() => {
    void refetch({ force: true });
  }, [refetch]);

  const clear = useCallback(() => {
    reqId.current += 1;
    lastFetch.current = 0;
    inFlight.current = false;
    setReport(null);
    setState('idle');
    setCheckedAt(null);
  }, []);

  // Prefer the report's echoed remote (authoritative once a check succeeded);
  // fall back to the resolved default so the offline line is never nameless.
  const remote = report?.remote ?? defaultRemote;

  return { report, state, remote, checkedAt, refetch, afterAutoFetch, clear };
}
