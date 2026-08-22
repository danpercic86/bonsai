/**
 * P87a — the git-activity stream in the mock IPC layer (MINIMAL stub).
 *
 * `gitActivitySubscribe` stores the callback(s) in module state (like the events
 * bus), and `emitGitActivity` fans an event out to all of them. That is all P87a
 * needs to compile + let a subscriber attach in the browser harness.
 *
 * P87b will add `runMockActivity(category, opts, fn)` (wrapping the push/commit/
 * fetch handler bodies) and the `?prePushHook` / `?prePushFail` / `?fetchSlow` /
 * `?fetchNoCount` query seams that drive every event kind + terminal state.
 */
import type { GitActivityEvent } from '../types';

/** Every live `gitActivitySubscribe` callback. A reload re-subscribes; the mock
 *  keeps them all (the real backend prunes on send failure — harmless here). */
const subscribers: Array<(e: GitActivityEvent) => void> = [];

/** Register a long-lived git-activity listener (the mock's `git_activity_subscribe`). */
export function subscribeGitActivity(onEvent: (e: GitActivityEvent) => void): void {
  subscribers.push(onEvent);
}

/** Fan one event out to every subscriber. A no-op when nobody is listening
 *  (mirrors `GitActivityHub::emit`). Used by P87b's `runMockActivity`. */
export function emitGitActivity(event: GitActivityEvent): void {
  for (const cb of subscribers) cb(event);
}

/** True while ≥1 subscriber is attached (mirrors `GitActivityHub::is_active`). */
export function gitActivityActive(): boolean {
  return subscribers.length > 0;
}
