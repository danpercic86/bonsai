/** P38 final batch — the Ask-Bonsai leak, driven through the REAL
 *  `useAskBonsai.tearDownAsk` (not a spy), the way the AI-panel / commit-search /
 *  composer cases in `unusableRepoTeardown.test.tsx` drive theirs.
 *
 *  Both surfaces here are rendered purely off hook-local state — `askOpen` for the
 *  one-line NL input, `pendingProposedOp` for the ProposedOpDialog — so no slice
 *  clear touched them: before this teardown field, a repo whose `.git` vanished
 *  left a resolved git op sitting there one Confirm away from dispatch. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useAskBonsai } from './useAskBonsai';
import type { OperationPlan, ProposedOperation } from '../../ipc';

const REPO = '/mock/repo';

/** A destructive proposal — the worst thing to leave armed over a dead repo. */
const PROPOSAL: ProposedOperation = {
  op: { kind: 'reset', targetOid: 'a'.repeat(40), targetShort: 'aaaaaaa', mode: 'hard' },
  preview: {
    title: 'Reset main to aaaaaaa (hard)',
    summary: 'Moves main back one commit and discards uncommitted changes.',
    danger: 'destructive',
    refChanges: [{ name: 'main', fromShort: 'bbbbbbb', toShort: 'aaaaaaa' }],
    droppedCommits: [{ short: 'bbbbbbb', summary: 'feat: oops' }],
    addedCommits: 0,
    worktreeWarning: null,
    confirmLabel: 'Reset',
  },
  rationale: 'You asked to undo the last commit.',
  costUsd: null,
};

const PLAN: OperationPlan = { kind: 'proposed', operation: PROPOSAL };

// `src/test/setup.ts` restores nothing, and the never-settling `resetBranch` spy
// in the last case would otherwise outlive its test and make this file (and any
// suite sharing the module registry) order-sensitive.
beforeEach(() => vi.restoreAllMocks());

function deps() {
  return { repoId: REPO, pushToast: vi.fn(), refreshAll: vi.fn(async () => {}), setMutating: vi.fn() };
}

describe('the Ask-Bonsai leak (real useAskBonsai.tearDownAsk)', () => {
  it('drops an armed proposal dialog — a git op one Confirm from dispatch', async () => {
    vi.spyOn(mockIpc, 'aiPlanOperation').mockResolvedValue(PLAN);
    const { result } = renderHook(() => useAskBonsai(deps()));

    await act(async () => {
      result.current.runPlanOperation('undo the last commit');
      await Promise.resolve();
    });
    // Reachable: WorkspaceOverlays renders ProposedOpDialog off this state.
    expect(result.current.pendingProposedOp).toEqual(PROPOSAL);

    act(() => result.current.tearDownAsk());
    expect(result.current.pendingProposedOp).toBeNull();
    expect(result.current.askOpen).toBe(false);
  });

  it('closes the NL input and drops the in-flight plan reply', async () => {
    let settle: ((p: OperationPlan) => void) | null = null;
    vi.spyOn(mockIpc, 'aiPlanOperation').mockReturnValue(
      new Promise<OperationPlan>((res) => {
        settle = res;
      }),
    );
    const { result } = renderHook(() => useAskBonsai(deps()));

    act(() => result.current.openAskBonsai());
    act(() => result.current.runPlanOperation('undo the last commit'));
    expect(result.current.askOpen).toBe(true);
    expect(result.current.askBusy).toBe(true);

    act(() => result.current.tearDownAsk());
    expect(result.current.askOpen).toBe(false);
    expect(result.current.askBusy).toBe(false);

    // The req-id bump inside `cancelAskBonsai` drops the late reply, so a plan
    // that lands after the teardown cannot arm the dialog over a dead repo.
    await act(async () => {
      settle?.(PLAN);
      await Promise.resolve();
    });
    expect(result.current.pendingProposedOp).toBeNull();
    expect(result.current.askBusy).toBe(false);
  });

  it('keeps the dialog up while a CONFIRMED op is dispatching (no force-close mid-write)', async () => {
    vi.spyOn(mockIpc, 'aiPlanOperation').mockResolvedValue(PLAN);
    // The dispatch parks forever, so `opDispatching` stays true across the teardown.
    vi.spyOn(mockIpc, 'resetBranch').mockReturnValue(new Promise<never>(() => {}));
    const { result } = renderHook(() => useAskBonsai(deps()));

    await act(async () => {
      result.current.runPlanOperation('undo the last commit');
      await Promise.resolve();
    });
    act(() => void result.current.confirmProposedOp());
    expect(result.current.opDispatching).toBe(true);

    act(() => result.current.tearDownAsk());
    // Deliberate: a teardown must not force-close mid-write (same rule as the
    // commit composer). `confirmProposedOp`'s own `finally` clears it, and the
    // NL input is closed regardless.
    expect(result.current.pendingProposedOp).toEqual(PROPOSAL);
    expect(result.current.askOpen).toBe(false);
  });
});
