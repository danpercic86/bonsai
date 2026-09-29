// P125 §8 — the dependency tracker's one fetch source. Mounted ONCE (App, with
// the active repo — P125-ui §2 plumbing) and shared by the toolbar button and
// the dialog, so there is a single subscription.
//
// ZERO IPC while the tracker is off (`enabled`, from App's `useUiSettings`
// state — no extra settings read): no fetch, no `deps-changed`, no focus
// listener, and a synthetic disabled view (what the backend serves when off).
// This keeps the cold-open IPC count flat for the default (off) install.
//
// While on: fetches the view on mount, on repo change, on window focus and on
// every `deps-changed` other than `progress`. It does NOT fetch on `repo-changed`
// (the backend decides staleness via `probe_stale`) and never polls on a timer
// — the schedule lives in Rust. The one timed re-read is bounded: a DISABLED
// answer while the local setting is on AND no view has landed yet (the window
// right after enabling, before the enable write is applied) is dropped and
// re-read a few times, so the UI never flashes "off" after enabling. The budget
// is per enable / repo switch, never per fetch; once a view is showing, a
// disabled answer is believed at once (never ~2 s of loading, never 9 IPC).
// Enable/disable is a settings write through `useUiSettings`, never this hook.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

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

/** The view the backend serves while disabled: nothing read, cached or scheduled. */
export function disabledDepView(repoId: string): DepTrackerView {
  return {
    repoId,
    enabled: false,
    report: null,
    manifestsChanged: false,
    running: null,
    lastError: null,
    nextCheckMs: null,
  };
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

/** Re-reads of a disabled answer while the local setting is on and the view is
 *  still loading (≈ 2 s total, per enable / repo switch) before it is
 *  believed — see `fetchView`. Exported for tests. */
export const OFF_RETRY_MS = 250;
export const OFF_RETRY_LIMIT = 8;

export function useDepTracker(repoId: string | null, enabled: boolean): UseDepTracker {
  const [view, setViewState] = useState<DepTrackerView | null>(null);
  // Mirrors `view` for async results: the off re-read runs only while null.
  const viewRef = useRef<DepTrackerView | null>(null);
  const setView = useCallback((v: DepTrackerView | null) => {
    viewRef.current = v;
    setViewState(v);
  }, []);
  // Off re-reads left in the current loading window (reset on enable / repo switch).
  const offBudget = useRef(0);
  // Read by async results: an answer that lands after a disable is dropped.
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const [progress, setProgress] = useState<DepProgress | null>(null);
  // The repo this hook currently serves; every async result is checked against
  // it so a slow answer for a previous tab is dropped (§6.3).
  const repoRef = useRef(repoId);
  repoRef.current = repoId;
  // Monotonic fetch token: only the latest request may land.
  const seq = useRef(0);
  const mounted = useRef(true);

  // The pending "backend has not applied the enable yet" re-read (see below).
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearRetry = useCallback(() => {
    if (retryTimer.current !== null) clearTimeout(retryTimer.current);
    retryTimer.current = null;
  }, []);

  const fetchView = useCallback((forRepo: string) => {
    clearRetry();
    seq.current += 1;
    const mine = seq.current;
    ipc
      .getDepTracker(forRepo)
      .then((v) => {
        if (!mounted.current || mine !== seq.current || !enabledRef.current) return;
        if (repoRef.current !== forRepo || v.repoId !== forRepo) return;
        // Enabling is an optimistic settings write: the first read can beat
        // `set_ui_settings` → `apply_config` and come back disabled. While
        // the local setting is on and NO view has landed yet, such an answer
        // is dropped (the view stays loading — never a flash of "off") and
        // re-read shortly; the `started` event usually lands first. Once the
        // window's budget is spent (e.g. the write failed), or whenever a view
        // is already showing, the backend's answer is shown as-is.
        if (!v.enabled && viewRef.current === null && offBudget.current > 0) {
          offBudget.current -= 1;
          retryTimer.current = setTimeout(() => {
            retryTimer.current = null;
            if (mounted.current && enabledRef.current && repoRef.current === forRepo) {
              fetchView(forRepo);
            }
          }, OFF_RETRY_MS);
          return;
        }
        // Progress is event-driven only: a fetch that was in flight when the
        // check started would otherwise wipe the live numbers.
        setView(v);
      })
      .catch((e: unknown) => {
        // noRepo (the tab just closed) is the only expected rejection; the view
        // simply stays as it was. Anything else is logged, never thrown.
        console.warn('deps: get view failed', e);
      });
  }, [clearRetry, setView]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      clearRetry();
    };
  }, [clearRetry]);

  // Mount + repo change + enable: never show one repo's report under another,
  // and never a stale report after a disable (the view flips to off at once,
  // without waiting for `cleared`).
  useEffect(() => {
    seq.current += 1; // drop any in-flight answer
    clearRetry(); // …and any pending off re-read (zero IPC once disabled)
    setView(null);
    setProgress(null);
    // A fresh loading window: this is the only place the off budget refills.
    offBudget.current = enabled ? OFF_RETRY_LIMIT : 0;
    if (enabled && repoId !== null) fetchView(repoId);
  }, [repoId, enabled, fetchView, clearRetry, setView]);

  // One subscription pair while enabled, reading the repo via ref.
  useEffect(() => {
    if (!enabled) return;
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
  }, [enabled, fetchView]);

  const refresh = useCallback(async () => {
    const forRepo = repoRef.current;
    if (forRepo === null || !enabledRef.current) return;
    try {
      await ipc.depsCheckNow(forRepo);
    } catch (e: unknown) {
      console.warn('deps: check now refused', e);
    }
    // The `started` event also refetches; this covers a joined run (no new
    // `started`) and a refusal, so the view is never left behind.
    if (repoRef.current === forRepo) fetchView(forRepo);
  }, [fetchView]);

  const offView = useMemo(() => (repoId === null ? null : disabledDepView(repoId)), [repoId]);
  return enabled ? { view, progress, refresh } : { view: offView, progress: null, refresh };
}
