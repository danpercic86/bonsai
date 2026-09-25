/**
 * P123 V4-V5 — two `useGitActivity` stores (tabs A and B) over the real mock
 * hub: a run routed to A lands only in A (B's listener is never invoked), and
 * the defensive `started.repoId` drop keeps a mis-routed broadcast out of the
 * wrong tab. This is the runtime proof for the P123-ui §2.2 surfaces a-f, all of
 * which read `runs` / `activeRun` / `hasTerminalRuns` from this store.
 */
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { emitGitActivity } from '../../ipc/mock/gitActivityHub';
import type { GitActivityEvent, GitActivityKind, GitActivityListener } from '../../ipc';
import { useGitActivity } from './useGitActivity';

const A = 'C:\\mock\\repo-a';
const B = 'C:\\mock\\repo-b';
let idc = 0;
const newId = (): string => `git-scope-${(idc += 1)}`;

function ev(id: string, seq: number, kind: GitActivityKind, extra: Partial<GitActivityEvent> = {}): GitActivityEvent {
  return { id, seq, kind, elapsedMs: 0, ...extra };
}

/** Mounts A and B, spying on the listener each one registers. */
async function mountBoth() {
  const listeners = new Map<string, ReturnType<typeof vi.fn<GitActivityListener>>>();
  const real = mockIpc.gitActivitySubscribe.bind(mockIpc);
  vi.spyOn(mockIpc, 'gitActivitySubscribe').mockImplementation((repoId, cb) => {
    const spy = vi.fn<GitActivityListener>(cb);
    listeners.set(repoId, spy);
    return real(repoId, spy);
  });
  const a = renderHook(() => useGitActivity(A));
  const b = renderHook(() => useGitActivity(B));
  await act(async () => {
    await Promise.resolve();
  });
  return { a, b, onA: listeners.get(A), onB: listeners.get(B) };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('useGitActivity — per-repo scope (P123)', () => {
  it('V4: a run routed to A shows only in A; B is never invoked', async () => {
    const { a, b, onA, onB } = await mountBoth();
    const id = newId();
    act(() => emitGitActivity(A, ev(id, 0, 'started', { category: 'push', repoId: A })));
    expect(a.result.current.activeRun?.id).toBe(id);
    expect(b.result.current.activeRun).toBeNull();
    act(() => emitGitActivity(A, ev(id, 1, 'finished', { code: 0, success: true })));
    expect(a.result.current.runs).toHaveLength(1);
    expect(a.result.current.hasTerminalRuns).toBe(true);
    expect(b.result.current.runs).toHaveLength(0);
    expect(b.result.current.hasTerminalRuns).toBe(false);
    expect(onA).toHaveBeenCalledTimes(2);
    expect(onB).not.toHaveBeenCalled();
    // Clear in B never touches A's runs.
    act(() => b.result.current.clear());
    expect(a.result.current.runs).toHaveLength(1);
  });

  it('V5: a broadcast `started` naming B is dropped by A and kept by B; no repoId = kept by both', async () => {
    const { a, b } = await mountBoth();
    const forB = newId();
    act(() => emitGitActivity(null, ev(forB, 0, 'started', { category: 'fetch', repoId: B })));
    act(() => emitGitActivity(null, ev(forB, 1, 'phase', { phase: { kind: 'network' } })));
    expect(a.result.current.runs).toHaveLength(0);
    expect(b.result.current.runs.map((r) => r.id)).toEqual([forB]);
    const unscoped = newId();
    act(() => emitGitActivity(null, ev(unscoped, 0, 'started', { category: 'cloneRepo' })));
    expect(a.result.current.runs.map((r) => r.id)).toEqual([unscoped]);
    expect(b.result.current.runs.map((r) => r.id)).toContain(unscoped);
  });
});
