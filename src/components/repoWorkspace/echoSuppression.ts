// P85 A2 §A2 — Round-anchored self-event (watcher echo) suppression registry.
// Module-level, shared singleton keyed by repoId (OD-P81-1 → Option B). A
// mutation-origin refresh opens a suppression SPAN for the entire time it is
// writing/refreshing (nesting-counted), then a fixed tail after the round
// settles. Watcher-origin refreshes for the same repoId while a span is open OR
// inside the tail are dropped. Arm-and-check (NOT consume-on-read), so all
// subscribers of the repoId (RepoWorkspace + the two sibling panels) honor a
// single span. Safe because there is at most one tab per repoId (App.tsx dedupes
// by repoId), so a repoId-keyed span can never swallow another tab's genuine
// refresh.
//
// Why round-anchored (supersedes P81's wall-clock arm→now+600): the P81 window
// was `now + 600` from ARM time, so on a large repo a round (re-open + full
// walk) plus the 300 ms debounce could exceed 600 ms and the self-echo landed
// AFTER the window → a double refresh. Anchoring the tail to round completion
// (span open while count>0, tail begins only at settle) makes suppression
// duration-INDEPENDENT: the echo (≤ write+300+dispatch) always lands inside the
// open span or its tail, whatever the round took.

/** Tail after the last self-caused write + round settle: 300 ms watcher debounce
 *  (watcher.rs) + 300 ms dispatch/render slack. Named tunable. (Renames P81's
 *  ECHO_TTL_MS; semantics changed from arm-relative to settle-relative.) */
export const ECHO_TAIL_MS = 600;

/** Open spans per repoId (mutation count currently writing/refreshing). */
const armedCount = new Map<string, number>();
/** epoch-ms at which the post-settle tail expires per repoId (set on the
 *  transition to count 0). */
const disarmUntil = new Map<string, number>();

/** Begin a self-caused-write span for `repoId` (call BEFORE enqueuing the round).
 *  Nesting-counted: overlapping mutations each arm once. While the count is > 0
 *  every watcher event for the repo is suppressed with NO expiry, so a slow
 *  round can never outlive its own window. Clears any pending tail. */
export function armEcho(repoId: string): void {
  armedCount.set(repoId, (armedCount.get(repoId) ?? 0) + 1);
  disarmUntil.delete(repoId);
}

/** End a span (call in the serving round's `finally`). Decrements the nesting
 *  count (floored at 0); when it reaches 0, start the tail: suppress until
 *  `now + ECHO_TAIL_MS`. `now` is injectable for tests. */
export function disarmEcho(repoId: string, now: number = Date.now()): void {
  const next = (armedCount.get(repoId) ?? 0) - 1;
  if (next > 0) {
    armedCount.set(repoId, next);
    return;
  }
  armedCount.delete(repoId);
  disarmUntil.set(repoId, now + ECHO_TAIL_MS);
}

/** True iff a span is open (count > 0) OR `now` is inside the post-settle tail. */
export function isEchoSuppressed(repoId: string, now: number = Date.now()): boolean {
  if ((armedCount.get(repoId) ?? 0) > 0) return true;
  return now < (disarmUntil.get(repoId) ?? 0);
}

/** Drop `repoId`'s entries (call on tab close / RepoWorkspace unmount) so the
 *  maps cannot grow unbounded across the app's lifetime. */
export function clearEchoSuppression(repoId: string): void {
  armedCount.delete(repoId);
  disarmUntil.delete(repoId);
}

/** Test-only: wipe the registry between vitest cases. */
export function __resetEchoSuppression(): void {
  armedCount.clear();
  disarmUntil.clear();
}
