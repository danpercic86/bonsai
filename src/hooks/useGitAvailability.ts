// P70 §4.4: the git preflight state machine. Owns the one-shot startup probe,
// the user-driven Re-check, and the "a gitNotFound error was actually observed"
// latch. App only wires the surface (same shape as `useUpdateController`).
//
// Deliberately NOT a render gate: the probe runs from an effect (i.e. after
// first paint) and nothing awaits it, so a healthy launch — the overwhelming
// majority — never waits on, or shifts for, this hook.

import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { GitAvailability } from '../ipc';
import {
  clearGitNotFoundLatch,
  gitNotFoundLatched,
  noteGitNotFound as noteGitNotFoundGlobal,
  subscribeGitNotFound,
} from '../ipc/gitNotFound';

/** UI §7: the pending state must be perceptible on a fast machine, so
 *  `checking` is held for at least this long after a re-check starts. */
export const MIN_CHECKING_MS = 400;

export interface GitAvailabilityState {
  /** `null` = not probed yet (or the probe threw) ⇒ the banner renders nothing. */
  status: GitAvailability | null;
  checking: boolean;
  /** True once any `gitNotFound` error has been observed this session. Forces
   *  the banner visible even if the probe raced or has not landed yet. */
  latched: boolean;
  /** Re-invokes the preflight. Resolves with the fresh status (or `null` when
   *  the invoke itself threw) so the caller can fire the success toast. */
  recheck: () => Promise<GitAvailability | null>;
  /** Latch setter for any observed `gitNotFound` error. */
  noteGitNotFound: () => void;
}

export function useGitAvailability(): GitAvailabilityState {
  const [status, setStatus] = useState<GitAvailability | null>(null);
  const [checking, setChecking] = useState(false);
  const [latched, setLatched] = useState(gitNotFoundLatched);
  // Guards every post-await setState: this hook outlives no unmount quietly.
  const mounted = useRef(true);
  // Monotonic token so a slow earlier probe can never overwrite a later one.
  const runId = useRef(0);
  // Aborts the pending minimum-`checking` window (see the cleanup below).
  const cancelMinWindow = useRef<(() => void) | null>(null);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      // Never leave a timer (or the promise waiting on it) alive past unmount.
      cancelMinWindow.current?.();
    };
  }, []);

  // Mirror the module-level latch (it is set from deep inside the workspace
  // hooks, which have no path to this hook's state).
  useEffect(() => subscribeGitNotFound(() => setLatched(gitNotFoundLatched())), []);

  const probe = useCallback(async (minMs: number): Promise<GitAvailability | null> => {
    const id = ++runId.current;
    if (mounted.current) setChecking(true);
    const started = Date.now();
    let next: GitAvailability | null = null;
    try {
      next = await ipc.checkGitAvailability();
    } catch {
      // A failed probe must not itself produce chrome (§6 "Error"): leave the
      // status as-is and stay silent.
      next = null;
    }
    const remaining = minMs - (Date.now() - started);
    if (remaining > 0) {
      await new Promise<void>((resolve) => {
        const timer = window.setTimeout(() => {
          cancelMinWindow.current = null;
          resolve();
        }, remaining);
        // Resolve (not hang) on cancel: the `mounted` guard below then drops the
        // result without touching state.
        cancelMinWindow.current = () => {
          window.clearTimeout(timer);
          cancelMinWindow.current = null;
          resolve();
        };
      });
    }
    if (!mounted.current || id !== runId.current) return next;
    setChecking(false);
    if (next !== null) {
      setStatus(next);
      // Git is back: the latch has served its purpose and must not keep the
      // banner pinned open.
      if (next.found) clearGitNotFoundLatch();
    }
    return next;
  }, []);

  // One-shot startup probe (no minimum window — nothing is watching it yet).
  useEffect(() => {
    void probe(0);
  }, [probe]);

  // Ratified decision 4: an observed `gitNotFound` error kicks a fresh probe on
  // the latch's RISING EDGE — regardless of the status we currently hold.
  //
  // Deliberately not gated on `status === null`: git can be moved, uninstalled
  // or quarantined MID-SESSION, in which case a healthy startup status is stale
  // and would keep the banner hidden while remote ops fail with nothing but a
  // toast. Re-probing writes the truthful `found: false`, and the banner then
  // renders from real data — which also keeps the variant copy honest (pinning
  // the bar open off the latch alone would show Variant A over a stale
  // `path`-bearing status). A probe that comes back healthy clears the latch,
  // so a transient failure self-heals instead of pinning the bar.
  //
  // `probedForLatch` makes this an edge, not a level: `probe` calls `setStatus`,
  // and a `status` dependency would re-arm the effect on the new object
  // identity, looping.
  const probedForLatch = useRef(false);
  useEffect(() => {
    if (!latched) {
      probedForLatch.current = false;
      return;
    }
    if (probedForLatch.current) return;
    probedForLatch.current = true;
    void probe(0);
  }, [latched, probe]);

  const recheck = useCallback(() => probe(MIN_CHECKING_MS), [probe]);

  return {
    status,
    checking,
    latched,
    recheck,
    noteGitNotFound: noteGitNotFoundGlobal,
  };
}
