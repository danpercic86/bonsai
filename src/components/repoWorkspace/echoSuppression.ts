// P81 §4 — Self-event (watcher echo) suppression registry. Module-level, shared
// singleton keyed by repoId (OD-P81-1 → Option B). A mutation-origin refresh
// ARMS a time window; watcher-origin refreshes for the same repoId inside that
// window are dropped. Arm-and-check (NOT consume-on-read), so multiple
// subscribers of the same repoId (RepoWorkspace + the two sibling panels) all
// honor a single arm. Safe because there is at most one tab per repoId
// (App.tsx dedupes by repoId), so a repoId-keyed window can never swallow
// another tab's genuine refresh.

/** Suppression window after a self-initiated (mutation) refresh, in ms.
 *  600 ms = 2× the 300 ms backend watcher debounce (watcher.rs) — budgets the
 *  debounce plus event-dispatch + render-contention slack. Named tunable. */
export const ECHO_TTL_MS = 600;

/** Module-level singleton: repoId → epoch-ms at which suppression expires. */
const suppressUntil = new Map<string, number>();

/** Arm the window for `repoId`. `now` defaults to Date.now() (injectable for tests). */
export function armEcho(repoId: string, now: number = Date.now()): void {
  suppressUntil.set(repoId, now + ECHO_TTL_MS);
}

/** True iff a watcher event for `repoId` at `now` falls inside the armed window. */
export function isEchoSuppressed(repoId: string, now: number = Date.now()): boolean {
  const until = suppressUntil.get(repoId) ?? 0;
  return now < until;
}

/** Drop `repoId`'s entry (call on tab close / RepoWorkspace unmount) to keep the
 *  map from growing unbounded across the app's lifetime. */
export function clearEchoSuppression(repoId: string): void {
  suppressUntil.delete(repoId);
}

/** Test-only: wipe the registry between vitest cases. */
export function __resetEchoSuppression(): void {
  suppressUntil.clear();
}
