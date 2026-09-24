/**
 * P119 §4.3 — the store's new terminal status and fields: `finished{success,
 * outcome:'conflicts'}` → status `conflicts` (terminal, clearable, prunable);
 * `started.targetCount` and `finished.outcome` are stored as-is.
 *
 * Driven through the REAL mock seam (`emitGitActivity`), like the P87b suite.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useGitActivity } from './useGitActivity';
import { isTerminalGitStatus, newGitRun, pruneGitRuns, GIT_ACTIVITY_RUNS_MAX } from './gitActivityState';
import type { GitActivityRun } from './gitActivityState';
import { emitGitActivity } from '../../ipc/mock/gitActivity';
import type { GitActivityEvent, GitActivityKind } from '../../ipc';

let idc = 0;
const newId = (): string => `git-p119-${(idc += 1)}`;

function ev(
  id: string,
  seq: number,
  kind: GitActivityKind,
  extra: Partial<GitActivityEvent> = {},
): GitActivityEvent {
  return { id, seq, kind, elapsedMs: seq * 10, ...extra };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useGitActivity — P119 conflicts / outcome / targetCount', () => {
  it('success + outcome conflicts → status conflicts, terminal and clearable', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();
    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'merge', target: 'feature/x' })));
    act(() => emitGitActivity(ev(id, 1, 'finished', { success: true, outcome: 'conflicts' })));
    const run = result.current.runs[0];
    expect(run?.status).toBe('conflicts');
    expect(run?.outcome).toBe('conflicts');
    expect(result.current.activeRun).toBeNull();
    expect(result.current.hasTerminalRuns).toBe(true);
    act(() => result.current.clear());
    expect(result.current.runs).toHaveLength(0);
  });

  it('stores a fast-forward outcome on a successful run', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();
    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'merge', target: 'feature/x' })));
    act(() => emitGitActivity(ev(id, 1, 'finished', { success: true, outcome: 'fastForwarded' })));
    expect(result.current.runs[0]?.status).toBe('success');
    expect(result.current.runs[0]?.outcome).toBe('fastForwarded');
  });

  it('a failed run ignores any outcome for its status', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();
    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'rebase', target: 'main' })));
    act(() => emitGitActivity(ev(id, 1, 'finished', { success: false })));
    expect(result.current.runs[0]?.status).toBe('failed');
    expect(result.current.runs[0]?.outcome).toBeNull();
  });

  it('stores started.targetCount (null when absent)', () => {
    const { result } = renderHook(() => useGitActivity());
    const many = newId();
    const one = newId();
    act(() => emitGitActivity(ev(many, 0, 'started', { category: 'deleteBranches', targetCount: 3 })));
    act(() =>
      emitGitActivity(ev(one, 0, 'started', { category: 'deleteBranches', target: 'feature/x' })),
    );
    const byId = new Map(result.current.runs.map((r) => [r.id, r]));
    expect(byId.get(many)?.targetCount).toBe(3);
    expect(byId.get(many)?.target).toBeNull();
    expect(byId.get(one)?.targetCount).toBeNull();
    expect(byId.get(one)?.target).toBe('feature/x');
  });
});

describe('gitActivityState — conflicts is terminal', () => {
  it('isTerminalGitStatus covers conflicts', () => {
    expect(isTerminalGitStatus('conflicts')).toBe(true);
    expect(isTerminalGitStatus('running')).toBe(false);
  });

  it('pruneGitRuns evicts a conflicts run like any terminal run', () => {
    const runs = new Map<string, GitActivityRun>();
    const order: string[] = [];
    for (let i = 0; i <= GIT_ACTIVITY_RUNS_MAX; i += 1) {
      const id = `r${i}`;
      const r = newGitRun(id, 'merge', { kind: 'preparing' }, 0, i, null);
      runs.set(id, i === 0 ? { ...r, status: 'conflicts' } : r);
      order.push(id);
    }
    expect(pruneGitRuns(order, runs)?.dropped).toEqual(['r0']);
  });
});
