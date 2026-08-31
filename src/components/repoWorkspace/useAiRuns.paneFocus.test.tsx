/**
 * P68e FOLD-IN 1 + FOLD-IN 2 — two things the P68d review left unproven.
 *
 * FOLD-IN 1 (the user's locked decision). Fixing item-5 removed an accidental side
 * benefit: the old reqId guard meant a finished run could never steal the center
 * pane, because a superseded open simply returned. With the guard gone, `settle`
 * opened `openAiProposal` unconditionally, so a user reading file B's diff could have
 * it replaced 40 s later by file A's proposal — repeatedly under a bulk run. The rule
 * is now "auto-open only if the user has NOT navigated away", and BOTH branches are
 * asserted here, because a guard that is only tested on its happy path is trivially
 * satisfiable.
 *
 * FOLD-IN 2 (P68d review SHOULD-FIX 1). The `?aiMarkers` guarantee was proved in two
 * halves — the mock seam returns a markerful body, and separately the frontend gate
 * stages nothing — but the gate half ran against a FABRICATED batch. P68b's
 * requirement was "provable end-to-end", so the last test here composes them: the
 * real `mockIpc` under the real seam, `autoResolve`, and the assertion that nothing
 * was ever staged.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { batch, CLEAN, gatedStream, makeDeps } from '../../test/aiRunsKit';
import { useAiRuns } from './useAiRuns';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  window.history.replaceState({}, '', '/');
});

describe('FOLD-IN 1 — a finished run never steals a pane the user navigated away from', () => {
  it('opens the proposal when the diff slot is UNCHANGED since the run started', async () => {
    const gate = gatedStream();
    // The user clicked ✨AI while looking at that file's conflict view.
    const slot: string | null = 'conflict:a.ts';
    const deps = makeDeps({ diffSlotKey: () => slot });
    const { result } = renderHook(() => useAiRuns(deps));

    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });

    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', CLEAN);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'AI proposal ready for a.ts — opened for review',
    );
    // M1: the outcome is RECORDED, which is what lets the dock say
    // `Proposal is open in the center pane.` in this branch and only in this branch.
    expect(result.current.runForPath('a.ts')?.openedInPane).toBe(true);
  });

  it('does NOT open it when the user moved to another file, and says where it is', async () => {
    const gate = gatedStream();
    let slot: string | null = 'conflict:a.ts';
    const deps = makeDeps({ diffSlotKey: () => slot });
    const { result } = renderHook(() => useAiRuns(deps));

    act(() => result.current.startConflictRun('a.ts'));
    // 40 seconds of reading file B while Claude works.
    slot = 'conflict:b.ts';
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });

    expect(deps.openAiProposal).not.toHaveBeenCalled();
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'AI proposal ready for a.ts — review it from the AI activity dock',
    );
    // The PROPOSAL itself is untouched — only the pane grab was suppressed, and the
    // row's `✓ review` affordance re-opens it on demand.
    const run = result.current.runForPath('a.ts');
    expect(run?.status).toBe('ready');
    expect(run?.proposal).toBe(CLEAN);
    // M1: and the store SAYS the pane was not taken, so the dock renders the sentence
    // that points at `Review proposal` instead of claiming the opposite of the toast.
    expect(run?.openedInPane).toBe(false);

    act(() => result.current.reviewProposal('conflict:a.ts', 'a.ts'));
    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', CLEAN);
    // Once the user opens it themselves, the pane really does show it.
    expect(result.current.runForPath('a.ts')?.openedInPane).toBe(true);
  });

  it('closing the diff entirely also counts as navigating away', async () => {
    const gate = gatedStream();
    let slot: string | null = 'conflict:a.ts';
    const deps = makeDeps({ diffSlotKey: () => slot });
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    slot = null;
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    expect(deps.openAiProposal).not.toHaveBeenCalled();
    expect(result.current.runForPath('a.ts')?.openedInPane).toBe(false);
  });

  it('with no diffSlotKey dep at all, behaviour is unchanged (the other six runners)', async () => {
    const gate = gatedStream();
    const deps = makeDeps();
    const { result } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => {
      gate.resolve(batch());
      await Promise.resolve();
    });
    expect(deps.openAiProposal).toHaveBeenCalledWith('a.ts', CLEAN);
  });
});

describe('FOLD-IN 2 — the markerful guarantee, composed end-to-end', () => {
  /**
   * The whole chain in one test: the `?aiMarkers` seam is read at module init, so the
   * graph is re-imported under it; `mockIpc` is NOT stubbed, so the body that reaches
   * the store is the one the mock actually produced from the conflict fixture; and
   * `autoResolve` is the only autonomy under which a stage is even attempted.
   *
   * `applyResolution` never being called is the load-bearing assertion — it is the
   * single writer (D4), so if it stayed untouched, nothing markerful was staged.
   */
  it('a markerful body from the real mock IPC is never staged under autoResolve', async () => {
    vi.useRealTimers();
    vi.resetModules();
    window.history.replaceState({}, '', '/?aiMarkers');

    const { repoHandlers } = await import('../../ipc/mock/handlers/repo');
    const { mergeHandlers } = await import('../../ipc/mock/handlers/merge');
    const { useAiRuns } = await import('./useAiRuns');
    const { hasUnresolvedMarkers } = await import('../../utils/conflictRegions');

    const { repoId } = await repoHandlers.openRepo('/tmp/bonsai-p68e-markers');
    await mergeHandlers.mergeBranch(repoId, 'demo-conflict');
    const path = 'src/auth.ts';

    const deps = makeDeps({
      repoId,
      aiConflictAutonomy: 'autoResolve',
      conflictPaths: [path],
      diffSlotKey: () => null,
    });
    const { result } = renderHook(() => useAiRuns(deps));

    await act(async () => {
      result.current.startConflictRun(path);
      // The mock's default script is 3 ticks x 200 ms plus small gaps.
      await new Promise((resolve) => {
        setTimeout(resolve, 1_500);
      });
    });

    // The seam really did hand back a markerful body (otherwise this test proves
    // nothing about the gate).
    const opened = (deps.openAiProposal as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(opened?.[0]).toBe(path);
    expect(hasUnresolvedMarkers(String(opened?.[1] ?? ''))).toBe(true);

    // ...and the gate refused it: nothing staged, the row reads failed.
    expect(deps.applyResolution).not.toHaveBeenCalled();
    expect(result.current.rowStates[path]?.status).toBe('failed');
  }, 20_000);
});
