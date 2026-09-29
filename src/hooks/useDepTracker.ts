// P125 §8 — the dependency tracker's one fetch source. Mounted ONCE (App, with
// the active repo — P125-ui §2 plumbing) and shared by the toolbar button and
// the dialog, so there is a single subscription.
//
// Fetches the view on mount, on repo change, on window focus and on every
// `deps-changed` other than `progress`. It does NOT fetch on `repo-changed`
// (the backend decides staleness via `probe_stale`) and never polls on a timer
// — the schedule lives in Rust. Enable/disable is a settings write through
// `useUiSettings`, never through this hook.
import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { Unsubscribe } from '../ipc';
import type { DepsChangedPayload, DepTrackerView } from '../ipc/types/depTracker';

/** Work units (deps.dev lookups + OSV batches), NOT packages [UI-b]. */
export interface DepProgress {
  done: number;
  total: number;
}

export interface UseDepTracker {
  /** null until the first view for the CURRENT repo lands (or with no repo). */
  view: DepTrackerView | null;
  /** Live progress of the running check, from `deps-changed` (reset by
   *  started / finished / failed / cancelled / cleared and on repo change).
   *  Show it only while `view.running` is set. */
  progress: DepProgress | null;
  /** "Check now": starts (or joins) a check. Never rejects — a refusal
   *  (`noRepo`, or `other("dependency tracker is disabled")` after a racing
   *  disable) re-syncs the view instead, which is where the state is shown. */
  refresh: () => Promise<void>;
}

/** Kinds that begin or end a check, so the live numbers reset. `stale` is not
 *  one: a stale probe can land while a check is running. */
const RESETS_PROGRESS: ReadonlySet<DepsChangedPayload['kind']> = new Set([
  'started',
  'finished',
  'failed',
  'cancelled',
  'cleared',
]);

export function useDepTracker(repoId: string | null): UseDepTracker {
  const [view, setView] = useState<DepTrackerView | null>(null);
  const [progress, setProgress] = useState<DepProgress | null>(null);
  // The repo this hook currently serves; every async result is checked against
  // it so a slow answer for a previous tab is dropped (§6.3).
  const repoRef = useRef(repoId);
  repoRef.current = repoId;
  // Monotonic fetch token: only the latest request may land.
  const seq = useRef(0);
  const mounted = useRef(true);

  const fetchView = useCallback((forRepo: string) => {
    seq.current += 1;
    const mine = seq.current;
    ipc
      .getDepTracker(forRepo)
      .then((v) => {
        if (!mounted.current || mine !== seq.current) return;
        if (repoRef.current !== forRepo || v.repoId !== forRepo) return;
        // Progress is event-driven only: a fetch that was in flight when the
        // check started would otherwise wipe the live numbers.
        setView(v);
      })
      .catch((e: unknown) => {
        // noRepo (the tab just closed) is the only expected rejection; the view
        // simply stays as it was. Anything else is logged, never thrown.
        console.warn('deps: get view failed', e);
      });
  }, []);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  // Mount + repo change: never show one repo's report under another.
  useEffect(() => {
    setView(null);
    setProgress(null);
    if (repoId !== null) fetchView(repoId);
  }, [repoId, fetchView]);

  // One subscription pair for the hook's lifetime, reading the repo via ref.
  useEffect(() => {
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    const keep = (off: Unsubscribe) => {
      if (cancelled) off();
      else unsubs.push(off);
    };
    void ipc
      .onDepsChanged((p) => {
        const current = repoRef.current;
        if (current === null || p.repoId !== current) return;
        if (p.kind === 'progress') {
          if (p.done !== null && p.total !== null) setProgress({ done: p.done, total: p.total });
          return;
        }
        if (RESETS_PROGRESS.has(p.kind)) setProgress(null);
        // Every non-progress kind changes the view (§8 Event).
        fetchView(current);
      })
      .then(keep)
      .catch((e: unknown) => console.error('deps-changed subscription failed', e));
    void ipc
      .onWindowFocus(() => {
        const current = repoRef.current;
        if (current !== null) fetchView(current);
      })
      .then(keep)
      .catch((e: unknown) => console.error('deps focus subscription failed', e));
    return () => {
      cancelled = true;
      for (const off of unsubs) off();
    };
  }, [fetchView]);

  const refresh = useCallback(async () => {
    const forRepo = repoRef.current;
    if (forRepo === null) return;
    try {
      await ipc.depsCheckNow(forRepo);
    } catch (e: unknown) {
      console.warn('deps: check now refused', e);
    }
    // The `started` event also refetches; this covers a joined run (no new
    // `started`) and a refusal, so the view is never left behind.
    if (repoRef.current === forRepo) fetchView(forRepo);
  }, [fetchView]);

  return { view, progress, refresh };
}
