/**
 * P68d §C — THE ITEM-5 FIX and the run lifecycle.
 *
 * The test that matters is `the item-5 regression`: start a run on file A, open an
 * unrelated diff (bumping `fileDiffReqId`) and switch to file B while the CLI is still
 * working, come back — the proposal must still be there and re-openable. Under the old
 * code that scenario silently discarded the finished proposal, and the negative control
 * below reproduces exactly that.
 *
 * Log batching, autonomy routing and store hygiene live in `useAiRuns.routing.test.tsx`.
 */
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useAiRuns } from './useAiRuns';
import { AI_MAX_CONCURRENT_RUNS } from '../../settings/ranges';
import { appErr } from '../../test/actionHookKit';
import {
  batch,
  CLEAN,
  deferred,
  ev,
  gatedStream,
  makeDeps,
  neverSettles,
  stubStream,
} from '../../test/aiRunsKit';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('useAiRuns — the item-5 fix', () => {
  /**
   * PART (b) OF THE REPORTED BUG. The old path was
   * `++fileDiffReqId` → `await ipc.aiResolveConflict(path)` → `if (id !==
   * fileDiffReqId.current) return;` on ONE shared counter, so opening any other file
   * during the run discarded the finished proposal: no toast, no cache, no retry.
   *
   * The store now owns the result, so nothing a diff open does can reach it. The
   * `fileDiffReqId` counter is simulated here because that is precisely what the
   * store must be immune to: it is bumped repeatedly mid-run and must change nothing.
   */
  it('the item-5 regression: a proposal survives switching files mid-run', async () => {
    const stream = stubStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));

    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));

    // The user goes and looks at another conflicted file while Claude works. In the
    // old code this is the exact keystroke that lost the result.
    const fileDiffReqId = { current: 0 };
    fileDiffReqId.current += 1; // opened b.ts's diff
    fileDiffReqId.current += 1; // opened a commit diff
    expect(result.current.runForPath('a.ts')?.status).toBe('running');

    // The CLI finishes AFTER the switch — the old guard's fatal window.
    await act(async () => {
      stream.gate.resolve(batch());
      await Promise.resolve();
    });

    const run = result.current.runForPath('a.ts');
    expect(run?.status).toBe('ready');
    expect(run?.proposal).toBe(CLEAN);
    expect(run?.files[0]).toMatchObject({ path: 'a.ts', status: 'ready', proposal: CLEAN });

    // And it is re-openable on demand — the row's `✓ review` affordance.
    (deps.openAiProposal as ReturnType<typeof vi.fn>).mockClear();
    act(() => result.current.reviewProposal('conflict:a.ts', 'a.ts'));
    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', CLEAN);
  });

  /**
   * NEGATIVE CONTROL for the test above — proof that the scenario really did fail
   * before P68d, expressed cheaply.
   *
   * The store's own code no longer contains the old algorithm, so the regression test
   * above cannot be run "against the old behaviour" directly. What CAN be pinned is
   * the algorithm itself: this reproduces the exact deleted sequence from
   * `useMergeActions.ts:149-162` — bump a SHARED counter, await the multi-second CLI
   * call, then compare the counter — and shows it drops a successful result the moment
   * anything else bumps that counter. That "anything else" was ordinary diff opening,
   * which bumped it from nine different call sites.
   */
  it('the OLD single-slot algorithm loses the same proposal (negative control)', async () => {
    const fileDiffReqId = { current: 0 }; // shared with ordinary diff opening
    let landed: string | null = null;
    const gate = deferred<string>();

    // Verbatim shape of the deleted code path.
    const oldHandleAiResolveConflict = async () => {
      const id = ++fileDiffReqId.current; // bumped BEFORE the CLI call — the bug
      const proposedText = await gate.promise; // ipc.aiResolveConflict(path)
      if (id !== fileDiffReqId.current) return; // silently discards the result
      landed = proposedText;
    };

    const inFlight = oldHandleAiResolveConflict();
    fileDiffReqId.current += 1; // the user opens b.ts's diff
    gate.resolve(CLEAN); // the CLI succeeds anyway
    await inFlight;

    expect(landed).toBeNull(); // <- the reported bug, reproduced
    // The new store, given the identical interleaving, keeps it (test above).
  });

  /**
   * PART (a). `runForPath` is what feeds the row affordance, and a run on a.ts must
   * be invisible to b.ts. The old scalar made `aiDisabled` true for EVERY row; the
   * only remaining gate is the concurrency cap, which is >= 2 by construction (see
   * `ranges.test.ts`), so one run can never disable another row.
   */
  it('a run on one path leaves other paths untouched and below capacity', () => {
    stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    expect(result.current.runForPath('a.ts')?.status).toBe('running');
    expect(result.current.runForPath('b.ts')).toBeNull();
    expect(result.current.rowStates['b.ts']).toBeUndefined();
    expect(result.current.atCapacity).toBe(false);
    expect(AI_MAX_CONCURRENT_RUNS).toBeGreaterThanOrEqual(2);
  });

  it('two paths run concurrently, each with its own state', () => {
    neverSettles();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => result.current.startConflictRun('b.ts'));
    expect(result.current.runningCount).toBe(2);
    expect(Object.keys(result.current.runs).sort()).toEqual(['conflict:a.ts', 'conflict:b.ts']);
  });

  it('refuses a new run at the concurrency cap, with the cap toast', () => {
    neverSettles();
    const deps = makeDeps({ conflictPaths: ['p0', 'p1', 'p2', 'p3'] });
    const { result } = renderHook(() => useAiRuns(deps));
    for (let i = 0; i < AI_MAX_CONCURRENT_RUNS; i++) {
      act(() => result.current.startConflictRun(`p${i}`));
    }
    expect(result.current.atCapacity).toBe(true);
    act(() => result.current.startConflictRun('p3'));
    expect(result.current.runs['conflict:p3']).toBeUndefined();
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      expect.stringContaining('Too many AI runs in progress'),
    );
  });
});

/**
 * P68e S1 — THE STRICTMODE TRAP, guarded directly.
 *
 * The `mounted` ref used to be latched `false` by the cleanup and never re-armed. Under
 * React 19 StrictMode the dev-mode mount → cleanup → mount cycle runs on the SAME
 * component instance (and therefore the same ref), so `commit()` returned early
 * FOREVER: no row status, no dock, no visible run at all in `pnpm dev` / `pnpm tauri
 * dev`. It shipped in P68d with 1440 green tests because jsdom `renderHook` is NOT
 * StrictMode-wrapped, so every other test in this file renders the non-double-invoked
 * path. This is the one that would have caught it.
 *
 * NEGATIVE CONTROL (run by hand): drop the `mounted.current = true` line from the mount
 * effect in `useAiRuns.ts` and this test fails with `status === undefined` while the
 * whole rest of the suite stays green.
 */
describe('useAiRuns — under StrictMode', () => {
  it('still commits state after the double mount (the P68d dev-only blackout)', () => {
    stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()), { wrapper: StrictMode });
    act(() => result.current.startConflictRun('a.ts'));
    expect(result.current.runForPath('a.ts')?.status).toBe('running');
    expect(result.current.rowStates['a.ts']?.status).toBe('running');
  });
});

describe('useAiRuns — lifecycle events', () => {
  it('records runId from the first event and fires a queued cancel (D8)', async () => {
    const cancel = vi.spyOn(mockIpc, 'aiCancelRun').mockResolvedValue(undefined);
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));

    // Cancel clicked BEFORE `started` — there is no id to cancel yet.
    act(() => result.current.cancelRun('conflict:a.ts'));
    expect(cancel).not.toHaveBeenCalled();
    expect(result.current.runForPath('a.ts')?.cancelRequested).toBe(true);

    act(() => stream.send(ev({ seq: 0, kind: 'started', runId: 'ai-xyz' })));
    expect(result.current.runForPath('a.ts')?.runId).toBe('ai-xyz');
    expect(cancel).toHaveBeenCalledWith('ai-xyz');
  });

  it('awaitingInput parks the question; replyRun resumes and clears it', () => {
    const reply = vi.spyOn(mockIpc, 'aiReplyRun').mockResolvedValue(undefined);
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started', runId: 'ai-1' })));
    act(() => stream.send(ev({ seq: 1, kind: 'awaitingInput', text: 'which plural?', turn: 1 })));

    const asking = result.current.runForPath('a.ts');
    expect(asking?.status).toBe('awaitingInput');
    expect(asking?.question).toBe('which plural?');
    expect(asking?.turn).toBe(1);

    act(() => result.current.replyRun('conflict:a.ts', 'Einträge'));
    expect(reply).toHaveBeenCalledWith('ai-1', 'Einträge');
    expect(result.current.runForPath('a.ts')?.status).toBe('running');
    expect(result.current.runForPath('a.ts')?.question).toBeNull();
  });

  it('turnEnd keeps the LAST cost, never a sum (A10)', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => stream.send(ev({ seq: 1, kind: 'turnEnd', costUsd: 0.018, turn: 1 })));
    act(() => stream.send(ev({ seq: 2, kind: 'turnEnd', costUsd: 0.0238, turn: 2 })));
    expect(result.current.runForPath('a.ts')?.costUsd).toBe(0.0238);
  });

  it('an aiCancelled rejection yields status cancelled and NO error toast', async () => {
    const gate = gatedStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.reject(appErr('aiCancelled', 'cancelled by user'));
      await Promise.resolve();
    });
    expect(result.current.runForPath('a.ts')?.status).toBe('cancelled');
    expect(deps.pushToast).not.toHaveBeenCalledWith('error', expect.anything());
  });

  it('any other rejection fails the run with one error toast', async () => {
    const gate = gatedStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.reject(appErr('aiFailed', 'Claude exited without a result'));
      await Promise.resolve();
    });
    expect(result.current.runForPath('a.ts')?.status).toBe('failed');
    expect(result.current.rowStates['a.ts']?.error).toBe('Claude exited without a result');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'Claude exited without a result');
  });

  it('a failed event keeps the partial text for display only (D2)', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() =>
      stream.send(
        ev({ seq: 1, kind: 'failed', text: 'watchdog', partialText: 'half a body' }),
      ),
    );
    const run = result.current.runForPath('a.ts');
    expect(run?.status).toBe('failed');
    expect(run?.partialText).toBe('half a body');
    // NEVER offered as a proposal.
    expect(run?.proposal).toBeNull();
  });

  it('a terminal status is never resurrected by a late event', () => {
    const stream = stubStream();
    const { result } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started' })));
    act(() => stream.send(ev({ seq: 1, kind: 'cancelled', partialText: '' })));
    act(() => stream.send(ev({ seq: 2, kind: 'awaitingInput', text: 'too late?' })));
    expect(result.current.runForPath('a.ts')?.status).toBe('cancelled');
    expect(result.current.runForPath('a.ts')?.question).toBeNull();
  });
});

