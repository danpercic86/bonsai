// P70 (UI §10.1): the pure toast-stack reducer, extracted from App so the
// coalescing rule is testable without rendering the whole app.
//
// Why a key at all: error toasts are STICKY, so without coalescing three failed
// presses of Fetch leave three permanent, identical toasts on screen — the exact
// symptom P70 exists to kill.
import type { Toast } from './Toasts';

/** Max toasts on screen; the oldest NON-sticky one is dropped past it. */
export const TOAST_CAP = 5;

/**
 * Apply one push to the stack.
 *
 * - `incoming.key === undefined` → append (pre-P70 behaviour, byte for byte).
 * - key matches a visible toast, SAME text → returns `cur` **by identity**, so
 *   the caller can detect the no-op (no remount, no flicker, no re-announcement,
 *   and no auto-dismiss timer armed for an id that was never rendered).
 * - key matches, DIFFERENT text → replaced in place at the same index with the
 *   new id/text, so the visible message always names the operation the user
 *   pressed last.
 *
 * Invariant: at most ONE toast per key exists at any moment.
 */
export function applyToastPush(cur: Toast[], incoming: Toast): Toast[] {
  if (incoming.key !== undefined) {
    const idx = cur.findIndex((t) => t.key === incoming.key);
    if (idx !== -1) {
      if (cur[idx].text === incoming.text) return cur; // identity => no-op
      const next = [...cur];
      next[idx] = incoming;
      return next;
    }
  }
  const next = [...cur, incoming];
  if (next.length <= TOAST_CAP) return next;
  const dropIdx = next.findIndex((t) => !t.sticky && t.id !== incoming.id);
  return next.filter((_unused, i) => i !== (dropIdx !== -1 ? dropIdx : 0));
}
