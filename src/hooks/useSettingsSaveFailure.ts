// P113 §17.3 — the routing and the standing state for ONE failed settings write.
//
// Split out of `useUiSettings` rather than added to it: that file is at 494 of
// the 500-line ratchet and is not baselined, so it has no room, and this is a
// genuinely separate concern — `useUiSettings` owns the debounce/retry machine,
// this owns where its failure is SHOWN.
//
// The routing rule (§17.3): **banner when Settings is open, toast when it is
// not.** `useUiSettings` serves the whole app — density, sidebar and
// graph-filter writes all ride the same patch path — so this call site fires
// with Settings CLOSED, where the toast is already correct and visible. It is
// the one place in the P113 sweep where `pushToast` must survive.
//
// This is NOT the tone-routing §3.1.2 rejected. There, the location varied by
// what the BACKEND returned, which the user cannot see. Here it varies by where
// the user is looking, which the user determines and can see.

import { useCallback, useState } from 'react';

import type { PushToast } from '../ToastContext';
import type { SettingsOpenSignal } from './useSettingsOpenSignal';

/**
 * P113 §17.3 amendment A4 (APPROVED by the orchestrator, 2026-09-14) — replaces
 * `Could not save settings: {raw}`.
 *
 * Every clause was verified against `useUiSettings`'s flush handler before it
 * was approved:
 *  * *"They still apply"* — the values are already live in the UI; only
 *    persistence failed.
 *  * *"until Bonsai closes"* — nothing else persists them, so an unsaved patch
 *    dies with the process.
 *  * *"it will try again on your next change"* — the failure branch puts the
 *    patch BACK (`{ ...merged, ...pending }`, P69b defect 2) rather than
 *    dropping it, and the bounded 300/600/1200 ms backoff then waits for the
 *    next change or teardown. This is the literal resume condition.
 *  * *"check that Bonsai can write to its config folder"* — the only
 *    user-actionable cause, replacing a raw OS error that named no action.
 *
 * ONE string for both channels. A raw OS error was tolerable in a transient
 * toast and is not in a permanent banner, and keeping two texts for one event
 * would be worse than either.
 */
export const SETTINGS_SAVE_FAILURE_TEXT =
  "Your settings couldn't be saved. They still apply until Bonsai closes, and it will try again on your next change. If this keeps happening, check that Bonsai can write to its config folder.";

export interface SettingsSaveFailure {
  /** True while the write is failing. Mirrors `settingsFailureStreakRef`, which
   *  is a ref and therefore cannot drive a render on its own. */
  settingsSaveFailed: boolean;
  /** Called from the write's failure branch with the streak count BEFORE the
   *  bump, so the one-toast-per-streak rule is preserved exactly. */
  noteFailure(streak: number): void;
  /** Called from the write's success branch. */
  clearFailure(): void;
}

export function useSettingsSaveFailure(
  pushToast: PushToast,
  settingsOpen: SettingsOpenSignal,
): SettingsSaveFailure {
  const [settingsSaveFailed, setSettingsSaveFailed] = useState(false);

  const noteFailure = useCallback(
    (streak: number) => {
      // Set unconditionally, including while Settings is closed: the condition
      // outlives the toast, so opening Settings later must still show it.
      setSettingsSaveFailed(true);
      // One toast per failure STREAK, not per retry (pre-existing rule), and
      // only when the user is not looking at the surface that renders it inline.
      if (!settingsOpen.current && streak === 0) {
        pushToast('error', SETTINGS_SAVE_FAILURE_TEXT);
      }
    },
    [pushToast, settingsOpen],
  );

  // Cleared on the SUCCESS branch only — deliberately not when
  // `queueSettingsWrite` resets the streak for a fresh user action. The values
  // are still not on disk at that moment, and clearing there would blink the
  // banner off for the ~300 ms debounce window and back on if the write fails
  // again. The banner tracks the CONDITION, which only a successful write ends.
  const clearFailure = useCallback(() => setSettingsSaveFailed(false), []);

  return { settingsSaveFailed, noteFailure, clearFailure };
}
