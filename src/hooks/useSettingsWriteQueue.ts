// P11c §3.2 / P69b / P113 §17.3 — the settings WRITE machine, extracted verbatim
// from `useUiSettings` (which sat at 494 of the 500-line ratchet, with no room
// left, and had two concerns in it). The move itself was verbatim; what has been
// ADDED here since is the failure's two halves — A2's diagnostic `console.warn`
// of the cause, and the `noteFailure`/`clearFailure` mirror listed below.
//
// What it owns, and nothing else does:
//   * the ~300 ms coalescing window, so a burst of knob changes is ONE write;
//   * the single-writer invariant and the failed-patch merge-back (P69b defect 2);
//   * the bounded 300/600/1200 ms retry budget;
//   * the teardown flush (P69b defect 3) — `pagehide`/`beforeunload` plus the
//     React cleanup, because App is the root and never unmounts in production;
//   * the failure's user-visible half (P113 §17.3), via `useSettingsSaveFailure`.
//
// `useUiSettings` keeps the per-field STATE and the live preview, and calls
// `queueSettingsWrite`. Nothing else in the app may call `ipc.setUiSettings`.

import { useCallback, useEffect, useRef } from 'react';

import { ipc } from '../ipc';
import type { UiSettingsPatch } from '../ipc';
import { errorMessage } from '../utils/errors';
import type { PushToast } from '../ToastContext';
import { useSettingsSaveFailure } from './useSettingsSaveFailure';
import type { SettingsOpenSignal } from './useSettingsOpenSignal';

/** Coalescing window for the settings write (§3.2; mirrors the session write). */
const SETTINGS_SAVE_DEBOUNCE_MS = 300;
/** P69b: automatic attempts after a failed write, then the patch waits for the
 *  next user change or teardown. Backoff 300 / 600 / 1200 ms — long enough to
 *  ride out a transient lock, bounded so a dead disk cannot spin or toast-storm. */
const SETTINGS_SAVE_MAX_RETRIES = 3;

export interface SettingsWriteQueue {
  /** Merge `patch` into the pending write and re-arm the single 300 ms window. */
  queueSettingsWrite(patch: UiSettingsPatch): void;
  /** Send the pending patch NOW (the save banner's Retry). */
  retrySettingsSave(): void;
  /** True while the write is failing — the Settings card's save banner. */
  settingsSaveFailed: boolean;
}

export function useSettingsWriteQueue(
  pushToast: PushToast,
  settingsOpen: SettingsOpenSignal,
): SettingsWriteQueue {
  // P11c §3.2: debounced settings persist — accumulates partial patches so a
  // burst of knob changes within the window all reach disk in one write.
  const settingsSaveTimerRef = useRef<number | null>(null);
  const pendingSettingsPatchRef = useRef<UiSettingsPatch>({});

  // P69b: at most ONE write may be outstanding. A second concurrent write makes
  // the failure merge-back below unsound: the newer values would already have
  // left `pendingSettingsPatchRef` inside that other write, so restoring the
  // failed patch could resurrect a value the UI has moved past. With a single
  // writer, everything newer is provably still pending.
  const settingsWriteInFlightRef = useRef(false);
  // Consecutive failed writes — bounds the automatic retry and keeps a dead disk
  // to ONE toast. Reset by a success and by any new user change.
  const settingsFailureStreakRef = useRef(0);
  // Set by the effect cleanup: after teardown nothing may arm a new timer. A
  // forced (teardown) flush that then REJECTS would otherwise leave a retry
  // timer outliving the component — harmless in production, but in tests it can
  // fire into a later test's spy.
  const disposedRef = useRef(false);
  // Late-bound so `armSettingsSave` can schedule the flush that is defined after
  // it (the timer only ever fires once the ref holds the real function).
  const flushRef = useRef<(force?: boolean) => void>(() => {});
  // P113 §17.3: the streak is a REF and cannot re-render, so its user-visible
  // half is mirrored into state here. `noteFailure`/`clearFailure` are stable,
  // which is what keeps `flushSettingsWrite` stable (see `flushRef` below).
  const { settingsSaveFailed, noteFailure, clearFailure } = useSettingsSaveFailure(
    pushToast,
    settingsOpen,
  );

  const armSettingsSave = useCallback((delayMs: number) => {
    if (disposedRef.current) return;
    if (settingsSaveTimerRef.current !== null) {
      window.clearTimeout(settingsSaveTimerRef.current);
    }
    settingsSaveTimerRef.current = window.setTimeout(() => {
      settingsSaveTimerRef.current = null;
      flushRef.current();
    }, delayMs);
  }, []);

  // P69b: send the accumulated patch now. Called by the debounce timer, by the
  // bounded retry, and by teardown (`force`) — unmount, `pagehide`,
  // `beforeunload` — where a patch still inside the window would otherwise die
  // with the JS context.
  const flushSettingsWrite = useCallback(
    (force = false) => {
      if (settingsSaveTimerRef.current !== null) {
        window.clearTimeout(settingsSaveTimerRef.current);
        settingsSaveTimerRef.current = null;
      }
      // A write is already out: leave the patch pending and let that write's
      // settle handler pump it, so only one write is ever in flight. Teardown
      // forces the send anyway — a possible reorder beats losing the patch.
      if (settingsWriteInFlightRef.current && !force) return;
      const merged = pendingSettingsPatchRef.current;
      // Nothing pending — also the StrictMode double-mount case, where the first
      // cleanup must not fire a write.
      if (Object.keys(merged).length === 0) return;
      pendingSettingsPatchRef.current = {};
      settingsWriteInFlightRef.current = true;
      void ipc.setUiSettings(merged).then(
        () => {
          settingsWriteInFlightRef.current = false;
          settingsFailureStreakRef.current = 0;
          clearFailure();
          // A change made while this write was out is still unsent. Re-arm the
          // FULL window rather than writing immediately: the burst it belongs to
          // may still be in progress, and coalescing it is the whole point.
          if (Object.keys(pendingSettingsPatchRef.current).length > 0) {
            armSettingsSave(SETTINGS_SAVE_DEBOUNCE_MS);
          }
        },
        (e: unknown) => {
          settingsWriteInFlightRef.current = false;
          const streak = settingsFailureStreakRef.current;
          // P113 A2 — the CAUSE, on a diagnostic path only. The approved A4 copy
          // (one string for banner and toast) deliberately names no OS error,
          // but it tells the user to "check that Bonsai can write to its config
          // folder" — a guess that disk-full, permission-denied, path-too-long
          // and a locked file all produce identically. The obs sink cannot stand
          // in: `ipcProxy` records `errCode: errCodeOf(err)`, which is the
          // `AppError.kind` (`'io'`) and never the message, and only with Dev
          // capture on. So `write C:\…\settings.json.tmp: Access is denied.
          // (os error 5)` reaches NOTHING without this line. Console, not the
          // sink, for the same reason: it is unconditional.
          //
          // The attempt index is part of the line because one streak logs up to
          // four times (initial + 3 bounded retries) with an identical message,
          // and a reader otherwise cannot tell four attempts on one patch from
          // four separate user changes each failing once.
          console.warn(
            `bonsai: settings write failed (attempt ${streak + 1} of ${SETTINGS_SAVE_MAX_RETRIES + 1}):`,
            errorMessage(e),
          );
          // P69b defect 2: the write failed, so put the patch back rather than
          // drop it. Spread `merged` FIRST so anything changed since (which,
          // single-writer, is still pending) wins — a retry must never resurrect
          // a value the UI has moved past.
          pendingSettingsPatchRef.current = { ...merged, ...pendingSettingsPatchRef.current };
          // P113 §17.3: banner when Settings is open, toast (one per streak, not
          // per retry) when it is not. The raw `e` is deliberately kept OFF
          // SCREEN — see `SETTINGS_SAVE_FAILURE_TEXT` (amendment A4) — and is
          // logged above instead.
          noteFailure(streak);
          settingsFailureStreakRef.current = streak + 1;
          // Bounded backoff (300 / 600 / 1200 ms), then wait for the next change
          // or teardown: a permanently failing disk must not spin forever.
          if (streak < SETTINGS_SAVE_MAX_RETRIES) {
            armSettingsSave(SETTINGS_SAVE_DEBOUNCE_MS * 2 ** streak);
          }
        },
      );
    },
    [armSettingsSave, clearFailure, noteFailure],
  );
  // Render-time ref mutation, deliberately — NOT the bug deleted from App.tsx's
  // `paneWidthsRef`. All three deps are stable, so
  // every candidate closure here is behaviourally identical and a re-assignment
  // from a discarded render cannot install a stale one. Do not "fix" by symmetry.
  flushRef.current = flushSettingsWrite;

  // P11c §3.2 / P69b: merge into the pending patch and re-arm the single 300 ms
  // window. Every persisted setting rides this — the ones this hook owns state
  // for (via `handleSettingsChange`) and App's four (theme, listView,
  // paneWidths, onboardingSeen) — so one burst is one write, whatever moved.
  const queueSettingsWrite = useCallback(
    (patch: UiSettingsPatch) => {
      pendingSettingsPatchRef.current = { ...pendingSettingsPatchRef.current, ...patch };
      // A fresh user action earns a fresh retry budget (and, if it fails again, a
      // fresh toast); the streak only silences the automatic retries.
      settingsFailureStreakRef.current = 0;
      armSettingsSave(SETTINGS_SAVE_DEBOUNCE_MS);
    },
    [armSettingsSave],
  );

  // P113 §17.3: the save banner's Retry. `0` rather than the debounce window —
  // the user just asked for it, and the patch has been sitting unsent since the
  // bounded backoff gave up.
  const retrySettingsSave = useCallback(() => armSettingsSave(0), [armSettingsSave]);

  // P69b defect 3: flush a patch that is still inside the debounce window when
  // the page goes away. React cleanup does NOT run on window close, app quit or
  // reload, and `App` is the root (`src/main.tsx`) so it never unmounts in
  // production — `pagehide`/`beforeunload` are what actually cover quit and
  // reload; the cleanup covers HMR and tests. Synchronous fire-and-forget: the
  // IPC call is dispatched, never awaited (nothing may await during teardown).
  useEffect(() => {
    // Cleared on every (re)mount: StrictMode's dev double-mount runs the cleanup
    // once on the SAME instance, and a permanently-disposed hook would then
    // never persist another setting.
    disposedRef.current = false;
    const flushNow = () => flushRef.current(true);
    window.addEventListener('pagehide', flushNow);
    window.addEventListener('beforeunload', flushNow);
    return () => {
      window.removeEventListener('pagehide', flushNow);
      window.removeEventListener('beforeunload', flushNow);
      flushNow();
      // After this point a rejection may still land, but it must not schedule.
      disposedRef.current = true;
    };
  }, []);
  return { queueSettingsWrite, retrySettingsSave, settingsSaveFailed };
}
