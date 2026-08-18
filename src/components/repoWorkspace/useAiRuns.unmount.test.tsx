/**
 * Audit 2026-08-18 §3.10 — unmount hygiene for `useAiRuns` (split file per the
 * ~500-line rule; shared helpers in src/test/aiRunsKit.ts).
 *
 * Closing a tab used to leave the Claude CLI running (and spending) with no dock
 * or cancel affordance, and a batch that settled after the unmount could still
 * stage into the closed tab's repo. The cleanup now cancels every live run with
 * a recorded id, and `settle` goes log-only once unmounted.
 *
 * THE STRICTMODE TRAP, re-checked here: the dev-mode mount → cleanup → mount
 * cycle runs on the SAME instance, so the cleanup MUST NOT cancel spuriously.
 * It is naturally safe because the synthetic cleanup fires while `runsRef` is
 * still empty (runs only start from user callbacks) — the test pins that down.
 */
import { StrictMode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useAiRuns } from './useAiRuns';
import { batch, ev, gatedStream, makeDeps, stubStream } from '../../test/aiRunsKit';

// House idiom (useAiRuns.test.tsx): no global restore exists, so spy state —
// including call history — would otherwise bleed across tests in this file.
afterEach(() => vi.restoreAllMocks());

describe('useAiRuns — unmount cancels live runs (audit §3.10)', () => {
  it('fires aiCancelRun for every non-terminal run with a recorded id', () => {
    const cancel = vi.spyOn(mockIpc, 'aiCancelRun').mockResolvedValue(undefined);
    const stream = stubStream();
    const { result, unmount } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started', runId: 'ai-xyz' })));
    expect(cancel).not.toHaveBeenCalled();
    unmount();
    expect(cancel).toHaveBeenCalledExactlyOnceWith('ai-xyz');
  });

  it('does not cancel a run that already settled (terminal at unmount)', async () => {
    const cancel = vi.spyOn(mockIpc, 'aiCancelRun').mockResolvedValue(undefined);
    const gate = gatedStream();
    const { result, unmount } = renderHook(() => useAiRuns(makeDeps()));
    act(() => result.current.startConflictRun('a.ts'));
    await act(async () => gate.resolve(batch()));
    expect(result.current.runForPath('a.ts')?.status).toBe('ready');
    unmount();
    expect(cancel).not.toHaveBeenCalled();
  });

  it('StrictMode: the synthetic mount→cleanup→mount cycle cancels nothing', () => {
    const cancel = vi.spyOn(mockIpc, 'aiCancelRun').mockResolvedValue(undefined);
    const stream = stubStream();
    const { result, unmount } = renderHook(() => useAiRuns(makeDeps()), {
      wrapper: StrictMode,
    });
    // The double-invoked dev mount ran its cleanup with runsRef still empty.
    expect(cancel).not.toHaveBeenCalled();
    // A run started afterwards is still cancelled by the REAL unmount.
    act(() => result.current.startConflictRun('a.ts'));
    act(() => stream.send(ev({ seq: 0, kind: 'started', runId: 'ai-1' })));
    expect(cancel).not.toHaveBeenCalled();
    unmount();
    expect(cancel).toHaveBeenCalledExactlyOnceWith('ai-1');
  });
});

describe('useAiRuns — settle after unmount is log-only (audit §3.10)', () => {
  it('skips staging, toasts and the pane open when the batch lands post-unmount', async () => {
    vi.spyOn(mockIpc, 'aiCancelRun').mockResolvedValue(undefined);
    const gate = gatedStream();
    const deps = makeDeps({ aiConflictAutonomy: 'autoResolve' });
    const { result, unmount } = renderHook(() => useAiRuns(deps));
    act(() => result.current.startConflictRun('a.ts'));
    unmount();
    // The batch resolves AFTER the tab closed: state is recorded in the ref
    // store, but nothing may stage into (or toast over) the gone workspace.
    gate.resolve(batch());
    await act(async () => {});
    expect(deps.applyResolution).not.toHaveBeenCalled();
    expect(deps.refreshAll).not.toHaveBeenCalled();
    expect(deps.openAiProposal).not.toHaveBeenCalled();
    expect(deps.pushToast).not.toHaveBeenCalled();
  });
});
