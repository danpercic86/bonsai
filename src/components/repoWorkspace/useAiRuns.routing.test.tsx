/**
 * P68d §C — `useAiRuns` log batching (D5), autonomy routing incl. THE SAFETY GATE, and
 * store hygiene. The item-5 regression itself lives in `useAiRuns.test.tsx`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { AI_LOG_MAX, useAiRuns, type AiRunsDeps } from './useAiRuns';
import { appErr, REPO } from '../../test/actionHookKit';
import {
  batch,
  CLEAN,
  gatedStream,
  MARKERFUL,
  makeDeps,
  ev,
  stubStream,
} from '../../test/aiRunsKit';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('useAiRuns — log batching (D5)', () => {
  it('300 log events commit ONCE per 50 ms flush, not once per line', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));

    act(() => {
      for (let i = 1; i <= 300; i++) stream.send(ev({ seq: i, kind: 'log', text: `line ${i}` }));
    });
    // Nothing committed yet: the buffer has not been flushed.
    expect(result.current.runForPath('a.ts')?.log).toHaveLength(0);

    act(() => void vi.advanceTimersByTime(50));
    expect(result.current.runForPath('a.ts')?.log).toHaveLength(300);
  });

  it('caps retained lines at AI_LOG_MAX and counts the drops', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));

    const total = AI_LOG_MAX + 120;
    act(() => {
      for (let i = 1; i <= total; i++) stream.send(ev({ seq: i, kind: 'log', text: `l${i}` }));
    });
    act(() => void vi.advanceTimersByTime(50));

    const run = result.current.runForPath('a.ts');
    expect(run?.log).toHaveLength(AI_LOG_MAX);
    expect(run?.logDropped).toBe(120);
    // The OLDEST lines are the ones dropped.
    expect(run?.log[0]?.text).toBe('l121');
    expect(run?.log[AI_LOG_MAX - 1]?.text).toBe(`l${total}`);
  });

  it('classifies each line kind once, at ingest', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => {
      stream.send(ev({ seq: 1, kind: 'log', text: 'the merged body' }));
      stream.send(ev({ seq: 2, kind: 'log', text: '⚙ Read(src/auth.ts)' }));
      stream.send(ev({ seq: 3, kind: 'log', text: 'stderr: boom' }));
      stream.send(ev({ seq: 4, kind: 'log', text: 'session s · model m · tools: none' }));
    });
    act(() => void vi.advanceTimersByTime(50));
    expect(result.current.runForPath('a.ts')?.log.map((l) => l.kind)).toEqual([
      'text',
      'tool',
      'stderr',
      'meta',
    ]);
  });

  it('ignores a stale or duplicate seq', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => {
      stream.send(ev({ seq: 5, kind: 'log', text: 'fresh' }));
      stream.send(ev({ seq: 5, kind: 'log', text: 'duplicate' }));
      stream.send(ev({ seq: 2, kind: 'log', text: 'stale' }));
    });
    act(() => void vi.advanceTimersByTime(50));
    expect(result.current.runForPath('a.ts')?.log.map((l) => l.text)).toEqual(['fresh']);
  });

  it('a metrics-only log event records thinkingTokens and adds NO log line', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => {
      stream.send(ev({ seq: 1, kind: 'log', text: null, thinkingTokens: 150 }));
      stream.send(ev({ seq: 2, kind: 'log', text: null, thinkingTokens: 350 }));
    });
    act(() => void vi.advanceTimersByTime(50));
    const run = result.current.runForPath('a.ts');
    expect(run?.thinkingTokens).toBe(350);
    expect(run?.log).toHaveLength(0);
  });
});

describe('useAiRuns — autonomy routing and THE SAFETY GATE', () => {
  it('proposeReview opens the proposal and toasts', async () => {
    const gate = gatedStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'AI proposal ready for a.ts — opened for review',
    );
    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', CLEAN);
    expect(deps.applyResolution).not.toHaveBeenCalled();
  });

  it('autoResolve stages a CLEAN body through the single writer', async () => {
    const gate = gatedStream();
    const deps = makeDeps({ aiConflictAutonomy: 'autoResolve' });
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    expect(deps.applyResolution).toHaveBeenCalledWith(
      'a.ts',
      CLEAN,
      'Resolved a.ts with AI — review the staged result',
    );
  });

  /**
   * THE SAFETY GATE — the blocking requirement from the P68b review: nothing
   * markerful may ever be presented as clean. Bulk marks a markerful body `failed`
   * on the Rust side, but a SINGLE-path stream returns the model's body verbatim
   * (P13 parity), so `hasUnresolvedMarkers` in this store is the only thing between
   * that body and a silent stage.
   */
  it('autoResolve + a MARKERFUL body never stages; falls back to review', async () => {
    const gate = gatedStream();
    const deps = makeDeps({ aiConflictAutonomy: 'autoResolve' });
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(
        batch({ proposals: [{ path: 'a.ts', proposedText: MARKERFUL, costUsd: null }] }),
      );
      await Promise.resolve();
    });
    expect(deps.applyResolution).not.toHaveBeenCalled();
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      'AI left unresolved markers in a.ts — opened for review',
    );
    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', MARKERFUL);
    // The row shows the failure, not a false "ready".
    expect(result.current.rowStates['a.ts']?.status).toBe('failed');
  });

  it('proposeReview does NOT stage a markerful body either (nothing is auto-written)', async () => {
    const gate = gatedStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(
        batch({ proposals: [{ path: 'a.ts', proposedText: MARKERFUL, costUsd: null }] }),
      );
      await Promise.resolve();
    });
    expect(deps.applyResolution).not.toHaveBeenCalled();
    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', MARKERFUL);
  });

  it('a per-file failure inside a ready batch marks only ITS row', async () => {
    const gate = gatedStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startBulkRun(['a.ts', 'b.ts']));
    await act(async () => {
      gate.resolve(
        batch({
          proposals: [{ path: 'a.ts', proposedText: CLEAN, costUsd: null }],
          failed: [{ path: 'b.ts', reason: 'no result block returned' }],
        }),
      );
      await Promise.resolve();
    });
    expect(result.current.rowStates['a.ts']?.status).toBe('ready');
    expect(result.current.rowStates['b.ts']?.status).toBe('failed');
    expect(result.current.rowStates['b.ts']?.error).toBe('no result block returned');
  });

  it('every path failing fails the run', async () => {
    const gate = gatedStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch({ proposals: [], failed: [{ path: 'a.ts', reason: 'binary file' }] }));
      await Promise.resolve();
    });
    expect(result.current.runForPath('a.ts')?.status).toBe('failed');
    expect(result.current.runForPath('a.ts')?.error).toBe('binary file');
  });
});

describe('useAiRuns — store hygiene', () => {
  it('refuses to start when AI is not eligible', () => {
    const spy = vi.spyOn(mockIpc, 'aiResolveConflictStream');
    const deps = makeDeps({ aiEligible: false });
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.runs).toEqual({});
    expect(deps.pushToast).toHaveBeenCalledWith('error', expect.stringContaining('AI features'));
  });

  it('a second click while running is a no-op (one run per path)', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => result.current.startConflictRun('a.ts'));
    expect(stream.spy).toHaveBeenCalledTimes(1);
  });

  it('a retry after failure replaces the entry and clears the old log', async () => {
    const gate = gatedStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.reject(appErr('aiFailed', 'nope'));
      await Promise.resolve();
    });
    expect(result.current.runForPath('a.ts')?.status).toBe('failed');

    stubStream();
    act(() => result.current.startConflictRun('a.ts'));
    const retried = result.current.runForPath('a.ts');
    expect(retried?.status).toBe('running');
    expect(retried?.error).toBeNull();
    expect(retried?.log).toHaveLength(0);
  });

  it('startBulkRun with one path degrades to a per-path conflict run', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startBulkRun(['a.ts']));
    expect(Object.keys(result.current.runs)).toEqual(['conflict:a.ts']);
    expect(stream.spy).toHaveBeenCalledWith(REPO, ['a.ts'], expect.any(Function));
  });

  it('dismissRun drops a terminal run but never a live one', async () => {
    const gate = gatedStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => result.current.dismissRun('conflict:a.ts'));
    expect(result.current.runs['conflict:a.ts']).toBeDefined();
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    act(() => result.current.dismissRun('conflict:a.ts'));
    expect(result.current.runs['conflict:a.ts']).toBeUndefined();
  });

  it('prunes a terminal run once its path is no longer conflicted (P68e §12-A4)', async () => {
    const gate = gatedStream();
    const { result, rerender } = renderHook((props: AiRunsDeps) => useAiRuns(props), {
      initialProps: makeDeps(),
    });
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    expect(result.current.runs['conflict:a.ts']).toBeDefined();
    // The user resolved a.ts: it leaves the conflicts list.
    act(() => rerender(makeDeps({ conflictPaths: ['b.ts'] })));
    expect(result.current.runs['conflict:a.ts']).toBeUndefined();
  });

  it('elapsed seconds tick while running and freeze once terminal', async () => {
    const gate = gatedStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    expect(result.current.rowStates['a.ts']?.elapsedSecs).toBe(0);
    act(() => void vi.advanceTimersByTime(3000));
    expect(result.current.rowStates['a.ts']?.elapsedSecs).toBe(3);
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    const frozen = result.current.rowStates['a.ts']?.elapsedSecs;
    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current.rowStates['a.ts']?.elapsedSecs).toBe(frozen);
  });
});
