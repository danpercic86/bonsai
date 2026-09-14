// P113 §13.1 / §17.3 — the ONE "is the Settings overlay mounted" signal.
//
// Two unrelated consumers need the same fact, and the contract is explicit that
// it is wired once:
//   * `useToastQueue`'s DEV reachability assertion (§13.1 guard part 1) — a
//     toast raised while the overlay is up renders behind `.dialog-overlay`
//     (z-index 100 vs the stack's 90) and is unclickable, not merely dim;
//   * `useUiSettings`'s save-failure routing (§17.3) — banner when Settings is
//     open, toast when it is not. That call site is the one place in the sweep
//     where `pushToast` legitimately SURVIVES, so it may not be linted away.
//
// A REF, not a boolean: both consumers read it from inside `useCallback`s whose
// identity is load-bearing (`pushToast` and `flushSettingsWrite` are documented
// as stable, and `useUiSettings` mutates `flushRef` at render time on that
// assumption). A boolean dependency would rebuild them on every open/close.
//
// WHERE IT IS WIRED: App passes the plain boolean `settings.open` to
// `useToastQueue`, which calls this hook and RETURNS the ref (`App.tsx:90`);
// `useUiSettings` then takes that same ref. "Wired once" therefore holds by
// construction — there is no second mirror of the same fact that could drift —
// and App gains no new statement, which matters because it sits on the file-size
// ratchet at exactly 590 lines.
//
// Why not a module-level global: there is exactly one overlay, but a global
// leaks between vitest cases and hides the wiring. Passing it means a test can
// hand `useToastQueue` a boolean or `useUiSettings` a `{ current: true }` and
// exercise either branch directly — which is how the guard and its
// `useUiSettings` exemption are both proved (`settingsToastGuard.test.tsx`).

import { useEffect, useRef } from 'react';

/** Read-at-call-time view of the Settings overlay's mounted state. */
export interface SettingsOpenSignal {
  readonly current: boolean;
}

/** Mirrors `open` into a stable ref. The write is an EFFECT, not a render-time
 *  mutation: the overlay paints in the same commit, and every reader of this
 *  signal runs later than that (a toast push, a settled IPC write), so an
 *  effect-synced value is never stale by the time it is read. */
export function useSettingsOpenSignal(open: boolean): SettingsOpenSignal {
  const ref = useRef(open);
  useEffect(() => {
    ref.current = open;
  }, [open]);
  return ref;
}
