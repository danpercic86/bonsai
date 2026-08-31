/**
 * P87b — the git-activity store: the started→(phase/lines/progress)→finished
 * integration sequence (P87a review NIT #3), 50 ms line batching, seq de-dup, the
 * per-run 500-line ring, the 200-run eviction that never drops a running run, and
 * clear().
 *
 * Events are driven through the REAL mock seam (`emitGitActivity`): the hook
 * subscribes via `ipc.gitActivitySubscribe` → the mock registers the callback, and
 * `emitGitActivity` fans an event out to it — exactly the browser path.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useGitActivity } from './useGitActivity';
import { GIT_ACTIVITY_LINES_MAX } from './gitActivityLog';
import {
  GIT_ACTIVITY_RUNS_MAX,
  newGitRun,
  pruneGitRuns,
  type GitActivityRun,
} from './gitActivityState';
import { emitGitActivity } from '../../ipc/mock/gitActivity';
import type { GitActivityEvent, GitActivityKind } from '../../ipc';

let idc = 0;
const newId = (): string => `git-test-${(idc += 1)}`;

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

describe('useGitActivity — the integration sequence (NIT #3)', () => {
  it('flows started → phase → lines → progress → finished into activeRun then a row', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();

    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'fetch', phase: { kind: 'preparing' } })));
    expect(result.current.activeRun?.id).toBe(id);
    expect(result.current.runs).toHaveLength(1);

    act(() => emitGitActivity(ev(id, 1, 'phase', { phase: { kind: 'network' } })));
    expect(result.current.activeRun?.phase.kind).toBe('network');

    act(() => {
      emitGitActivity(ev(id, 2, 'stdoutLine', { line: 'remote: counting objects' }));
      vi.advanceTimersByTime(60);
    });
    expect(result.current.runs[0]?.lines.map((l) => l.text)).toContain('remote: counting objects');

    act(() => {
      emitGitActivity(
        ev(id, 3, 'progress', {
          progress: { receivedObjects: 5, totalObjects: 10, indexedObjects: 5, receivedBytes: 400 },
        }),
      );
      vi.advanceTimersByTime(60);
    });
    expect(result.current.activeRun?.progress?.totalObjects).toBe(10);

    act(() => emitGitActivity(ev(id, 4, 'finished', { code: 0, success: true })));
    expect(result.current.activeRun).toBeNull();
    expect(result.current.runs[0]?.status).toBe('success');
    expect(result.current.hasTerminalRuns).toBe(true);
  });

  it('records per-hook rows and a failed terminal', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();
    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'push', phase: { kind: 'preparing' } })));
    act(() => emitGitActivity(ev(id, 1, 'phase', { phase: { kind: 'runningHook', hook: 'pre-push' } })));
    act(() => {
      emitGitActivity(ev(id, 2, 'stderrLine', { line: 'gitleaks: Failed' }));
      vi.advanceTimersByTime(60);
    });
    act(() => emitGitActivity(ev(id, 3, 'hookDone', { hook: 'pre-push', code: 1, success: false })));
    act(() => emitGitActivity(ev(id, 4, 'finished', { code: 1, success: false })));

    const row = result.current.runs[0];
    expect(row?.status).toBe('failed');
    expect(row?.hooks).toEqual([
      expect.objectContaining({ hook: 'pre-push', code: 1, success: false }),
    ]);
    expect(row?.lines[0]).toMatchObject({ stream: 'stderr', text: 'gitleaks: Failed' });
  });
});

describe('useGitActivity — batching, de-dup, ring', () => {
  it('drops an out-of-order (seq <= last-seen) event', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();
    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'push', phase: { kind: 'preparing' } })));
    act(() => {
      emitGitActivity(ev(id, 2, 'stdoutLine', { line: 'fresh' }));
      vi.advanceTimersByTime(60);
    });
    act(() => {
      emitGitActivity(ev(id, 1, 'stdoutLine', { line: 'stale' }));
      vi.advanceTimersByTime(60);
    });
    const texts = result.current.runs[0]?.lines.map((l) => l.text) ?? [];
    expect(texts).toContain('fresh');
    expect(texts).not.toContain('stale');
  });

  it('caps per-run output at 500 lines and counts the overflow', () => {
    const { result } = renderHook(() => useGitActivity());
    const id = newId();
    act(() => emitGitActivity(ev(id, 0, 'started', { category: 'push', phase: { kind: 'network' } })));
    act(() => {
      for (let i = 1; i <= 600; i += 1) emitGitActivity(ev(id, i, 'stdoutLine', { line: `L${i}` }));
      vi.advanceTimersByTime(60);
    });
    const row = result.current.runs[0];
    expect(row?.lines).toHaveLength(GIT_ACTIVITY_LINES_MAX);
    expect(row?.linesDropped).toBe(100);
    // The oldest lines were dropped; the newest survive.
    expect(row?.lines.at(-1)?.text).toBe('L600');
  });

  it('clear() removes terminal runs but keeps a running one', () => {
    const { result } = renderHook(() => useGitActivity());
    const done = newId();
    const live = newId();
    act(() => emitGitActivity(ev(done, 0, 'started', { category: 'fetch', phase: { kind: 'network' } })));
    act(() => emitGitActivity(ev(done, 1, 'finished', { code: 0, success: true })));
    act(() => emitGitActivity(ev(live, 0, 'started', { category: 'push', phase: { kind: 'network' } })));
    expect(result.current.runs).toHaveLength(2);

    act(() => result.current.clear());
    expect(result.current.runs).toHaveLength(1);
    expect(result.current.runs[0]?.id).toBe(live);
    expect(result.current.hasTerminalRuns).toBe(false);
  });
});

describe('pruneGitRuns — 200-run eviction never drops a running run', () => {
  function makeMap(specs: Array<{ id: string; running: boolean }>): {
    order: string[];
    runs: Map<string, GitActivityRun>;
  } {
    const runs = new Map<string, GitActivityRun>();
    const order: string[] = [];
    for (const s of specs) {
      const r = newGitRun(s.id, 'push', { kind: 'network' }, 0, 0);
      runs.set(s.id, s.running ? r : { ...r, status: 'success', endedAt: 1 });
      order.push(s.id);
    }
    return { order, runs };
  }

  it('returns null under the cap', () => {
    const { order, runs } = makeMap([{ id: 'a', running: false }]);
    expect(pruneGitRuns(order, runs)).toBeNull();
  });

  it('evicts the oldest terminal, preserving a running run at the front', () => {
    const specs = [
      { id: 'oldest-running', running: true },
      ...Array.from({ length: GIT_ACTIVITY_RUNS_MAX }, (_, i) => ({ id: `t${i}`, running: false })),
    ];
    const { order, runs } = makeMap(specs);
    const pruned = pruneGitRuns(order, runs);
    expect(pruned).not.toBeNull();
    // Exactly one over the cap → one terminal dropped; the running one is NOT it.
    expect(pruned?.dropped).toEqual(['t0']);
    expect(pruned?.kept).toContain('oldest-running');
    expect(pruned?.kept).toHaveLength(GIT_ACTIVITY_RUNS_MAX);
  });
});
