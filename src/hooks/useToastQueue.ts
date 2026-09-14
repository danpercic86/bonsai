// P3e §5.5 / P70: the single global toast stack. Extracted verbatim from App so
// the container only wires the `Toasts` render and the ToastContext provider —
// nothing else in the app owns toast identity, timers, or the dedupe key.
import { useCallback, useRef, useState } from 'react';
import { applyToastPush } from '../components/toastQueue';
import type { Toast, ToastTone } from '../components/Toasts';
import { useSettingsOpenSignal, type SettingsOpenSignal } from './useSettingsOpenSignal';
import type { PushToast } from '../ToastContext';

export interface UseToastQueue {
  toasts: Toast[];
  pushToast: PushToast;
  dismissToast: (id: number) => void;
  /** P113 §13.1/§17.3 — the ONE settings-open signal, published so the other
   *  consumer (`useUiSettings`'s save-failure routing) reads the SAME ref by
   *  construction rather than a second copy that could drift from this one. */
  settingsOpen: SettingsOpenSignal;
}

/** P113 §13.1 — the DEV reachability assertion's text. Exported so the test can
 *  assert the guard fires on the message the contract specifies rather than on a
 *  paraphrase that could drift away from it. */
export const SETTINGS_TOAST_GUARD_MESSAGE =
  'pushToast called while Settings is open — this message renders behind .dialog-overlay and is unclickable. Use SettingsOutcomeNote (ui-reference §12.14).';

/**
 * @param settingsOpenNow is the Settings overlay mounted right now (App's
 * `settings.open`). P113 §13.1 guard part 1 — the PRIMARY guard, because the
 * path lint cannot express "reaches the Settings surface".
 * `no-restricted-imports` is import-graph-based, and the five call sites it
 * missed receive `pushToast` as a PARAMETER from App; there is no import to ban.
 * Asserting here instead checks the condition that actually matters, catches a
 * caller anywhere in the tree (including one reached through three layers of
 * props), and covers §13.3's background-toast case, which no lint rule can see
 * either.
 *
 * It is taken as a BOOLEAN and mirrored into a ref here, rather than App
 * building the ref: "wire it once" then holds by construction, and `pushToast`'s
 * identity — which `useUiSettings` documents a render-time ref assignment
 * against — cannot churn on an open/close.
 */
export function useToastQueue(settingsOpenNow: boolean): UseToastQueue {
  const settingsOpen = useSettingsOpenSignal(settingsOpenNow);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);

  const dismissToast = useCallback((id: number) => {
    setToasts((cur) => cur.filter((t) => t.id !== id));
  }, []);

  // `key` (P70, UI §10.1) coalesces a repeatable failure into ONE toast:
  //   same key + same text -> no-op (no remount, no flicker, no re-announce)
  //   same key + new  text -> replace IN PLACE, same slot, new id + timer
  //   no key               -> the pre-P70 behaviour, byte for byte.
  // Error toasts are sticky, so without this three failed presses would leave
  // three permanent identical toasts — the exact symptom P70 exists to kill.
  const pushToast = useCallback(
    (tone: ToastTone, text: string, key?: string) => {
      // DEV only, and deliberately not a throw: the message still needs to reach
      // SOMEWHERE while the developer fixes the call site. It is a smoke alarm,
      // not a type system — it fires only if someone runs the path — which is
      // still strictly more than the lint can do.
      if (import.meta.env.DEV && settingsOpen.current) {
        console.error(`${SETTINGS_TOAST_GUARD_MESSAGE} Message: ${text}`);
      }
      const id = ++toastId.current;
      const sticky = tone === 'error';
      // The updater stays PURE — nothing is read back out of it (React may run
      // it at render time, and StrictMode runs it twice). The timer decision is
      // therefore made from the arguments alone: a same-key/same-text push is a
      // no-op inside `applyToastPush`, and arming a timer for its unrendered id
      // is harmless — `dismissToast` finds nothing to remove, exactly as it
      // already does for a keyed toast that was replaced in place.
      setToasts((cur) => applyToastPush(cur, { id, tone, text, sticky, key }));
      if (!sticky) window.setTimeout(() => dismissToast(id), 5000);
    },
    [dismissToast, settingsOpen],
  );

  return { toasts, pushToast, dismissToast, settingsOpen };
}
