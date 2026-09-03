/**
 * P110 — the `repo-changed` reason → (origin, scope) routing table.
 *
 * The watcher now classifies each debounced burst in Rust: a burst of
 * working-tree content and/or `.git/index` only cannot have moved HEAD, so it
 * arrives as `"fsWorktree"` and must refresh the NARROW `worktree` scope instead
 * of re-streaming the whole graph. Everything else — including any unknown
 * reason — must still be `full`, which is the always-safe fallback.
 */
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { RepoChangedPayload } from '../../ipc/types';
import { repoChangedListeners } from '../../ipc/mock/events';
import { useRepoChangeSubscription } from './useRepoChangeSubscription';

const REPO = 'C:\\mock\\bonsai-fixture';

afterEach(() => {
  repoChangedListeners.clear();
  vi.restoreAllMocks();
});

/** Mounts the hook and returns the recorded (origin, scope) pairs. */
async function mountHook() {
  const calls: [string, string][] = [];
  const refresh = vi.fn(async (origin: string, scope: string) => {
    calls.push([origin, scope]);
  });
  const view = renderHook(() =>
    useRepoChangeSubscription(
      REPO,
      refresh as unknown as Parameters<typeof useRepoChangeSubscription>[1],
      vi.fn(),
    ),
  );
  await waitFor(() => expect(repoChangedListeners.size).toBeGreaterThan(0));
  return { calls, view };
}

async function dispatch(payload: RepoChangedPayload) {
  await act(async () => {
    for (const cb of [...repoChangedListeners]) cb(payload);
    await Promise.resolve();
  });
}

describe('useRepoChangeSubscription — reason routing', () => {
  it('routes "fsWorktree" to the narrow watcher/worktree refresh', async () => {
    const { calls } = await mountHook();
    await dispatch({ repoId: REPO, reason: 'fsWorktree' });
    expect(calls).toEqual([['watcher', 'worktree']]);
  });

  it('routes "fs" to watcher/full (graph may have moved)', async () => {
    const { calls } = await mountHook();
    await dispatch({ repoId: REPO, reason: 'fs' });
    expect(calls).toEqual([['watcher', 'full']]);
  });

  it('routes an unknown or absent reason to watcher/full', async () => {
    const { calls } = await mountHook();
    await dispatch({ repoId: REPO, reason: 'somethingNew' });
    await dispatch({ repoId: REPO } as RepoChangedPayload);
    expect(calls).toEqual([
      ['watcher', 'full'],
      ['watcher', 'full'],
    ]);
  });

  it('ignores bursts for another repo', async () => {
    const { calls } = await mountHook();
    await dispatch({ repoId: 'C:\\mock\\other', reason: 'fsWorktree' });
    expect(calls).toEqual([]);
  });

  it('unsubscribes on unmount', async () => {
    const { calls, view } = await mountHook();
    view.unmount();
    await waitFor(() => expect(repoChangedListeners.size).toBe(0));
    await dispatch({ repoId: REPO, reason: 'fsWorktree' });
    expect(calls).toEqual([]);
  });
});
