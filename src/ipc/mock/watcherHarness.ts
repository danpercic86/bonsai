// Browser-harness trigger for synthetic `repo-changed` watcher bursts.
//
// There is no `notify` watcher in the browser harness, so without this the two
// filesystem reasons the Rust watcher emits (P110) cannot be exercised at all:
// `"fs"` (a burst that touched `.git/HEAD` / `.git/refs/**` / `.git/packed-refs`
// → full refresh) and `"fsWorktree"` (working-tree content and/or `.git/index`
// only → narrow status+opState refresh). `window.__bonsaiEmitWatcher(reason)`
// dispatches one through the same listener registry the real subscription uses.

import { repoChangedListeners } from './events';
import { repos } from './repoState';
import type { RepoChangedPayload } from '../types';

/** The reasons a filesystem burst can carry (mirrors `commands/repo.rs`). */
export type MockWatcherReason = 'fs' | 'fsWorktree';

declare global {
  interface Window {
    /** Dispatch a synthetic watcher burst. `repoId` defaults to the first open
     *  mock repo. Returns how many listeners received it. */
    __bonsaiEmitWatcher?: (reason?: MockWatcherReason, repoId?: string) => number;
  }
}

export function emitMockWatcher(reason: MockWatcherReason, repoId: string): number {
  const payload: RepoChangedPayload = { repoId, reason };
  let n = 0;
  for (const cb of repoChangedListeners) {
    cb(payload);
    n += 1;
  }
  return n;
}

export function installWatcherHarness(): void {
  if (typeof window === 'undefined') return;
  window.__bonsaiEmitWatcher = (reason = 'fs', repoId) => {
    const id = repoId ?? [...repos.keys()][0];
    if (id === undefined) return 0; // no repo open — nothing to refresh
    return emitMockWatcher(reason, id);
  };
}
