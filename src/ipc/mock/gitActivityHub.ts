/**
 * P123 — the mock IPC layer's git-activity fan-out hub. MIRRORS
 * `src-tauri/src/activity_hub.rs` exactly:
 *
 * - each subscription listens to ONE repo (`repoId`), or — test-only, like the
 *   Rust wildcard — to every run (`repoId: null`);
 * - a run routed to repo `r` reaches the subs of `r` (+ wildcards); an
 *   unscoped run (`route: null`, clone/init — P123 D3) reaches every sub;
 * - ids are minted here (`gas-mock-<n>`) and `unsubscribeGitActivity` is
 *   idempotent (an unknown id is a no-op).
 */
import type { GitActivityEvent, GitActivityListener } from '../types';

interface Sub {
  repoId: string | null;
  cb: GitActivityListener;
}

/** Insertion order = fan-out order (as the Rust `Vec`). */
const subs = new Map<string, Sub>();
let nextId = 0;

function matches(sub: Sub, route: string | null): boolean {
  return route === null || sub.repoId === null || sub.repoId === route;
}

/** The mock's `git_activity_subscribe`: register `onEvent` for `repoId`'s runs. */
export function subscribeGitActivity(repoId: string | null, onEvent: GitActivityListener): string {
  const id = `gas-mock-${nextId++}`;
  subs.set(id, { repoId, cb: onEvent });
  return id;
}

/** The mock's `git_activity_unsubscribe`. IDEMPOTENT. */
export function unsubscribeGitActivity(id: string): void {
  subs.delete(id);
}

/** Fan `event` out to the subs matching `route` (mirrors `GitActivityHub::emit_to`). */
export function emitGitActivity(route: string | null, event: GitActivityEvent): void {
  // Snapshot: a callback may unsubscribe mid-fan-out.
  for (const sub of [...subs.values()]) if (matches(sub, route)) sub.cb(event);
}

/** True while ≥1 sub matches `route` (mirrors `GitActivityHub::is_active_for`). */
export function gitActivityActive(route: string | null): boolean {
  for (const sub of subs.values()) if (matches(sub, route)) return true;
  return false;
}

/** Test/diagnostic count of live subs. */
export function gitActivitySubscriberCount(): number {
  return subs.size;
}
