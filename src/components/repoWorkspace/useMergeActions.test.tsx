/** T3.2a — useMergeActions: merge, conflict resolution, merge commit, and P68d's
 *  `openAiProposal`.
 *
 *  The old `handleAiResolveConflict` tests moved to `useAiRuns.test.tsx` together with
 *  the logic (P68d §5.3). What is asserted here is the ONE thing that stayed: the
 *  `fileDiffReqId` guard now wraps only the fast local `getConflict`, so losing that
 *  race costs the diff SLOT and never a proposal. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useMergeActions } from './useMergeActions';
import { COMMIT_HOOK_CANCELED } from '../commitPushSignal';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  passthroughGate,
  REPO,
} from '../../test/actionHookKit';
import type { CommitResult, ConflictFile } from '../../ipc';

afterEach(() => vi.restoreAllMocks());

const CONFLICT_FILE: ConflictFile = {
  path: 'a.ts',
  kind: 'bothModified',
  binary: false,
  tooLarge: false,
  missing: false,
  text: '<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feat\n',
  ours: 'ours\n',
  theirs: 'theirs\n',
};
const MARKERFUL = CONFLICT_FILE.text;
const COMMIT_RES: CommitResult = { oid: 'c'.repeat(40), summary: 'merge', branch: 'main' };

type Deps = Parameters<typeof useMergeActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    setDiffSlot: vi.fn(),
    fileDiffReqId: { current: 0 },
    runWithHookGate: passthroughGate(),
    ...over,
  };
}

describe('handleMergeBranch', () => {
  it('merged → success toast + refreshAll', async () => {
    const merge = vi
      .spyOn(mockIpc, 'mergeBranch')
      .mockResolvedValue({ kind: 'merged', oid: 'm'.repeat(40), stashed: false });
    const deps = makeDeps();
    await useMergeActions(deps).handleMergeBranch('feat');
    expect(merge).toHaveBeenCalledWith(REPO, 'feat');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Merged feat');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('conflicts → info toast with count (+ stash hint when stashed)', async () => {
    vi.spyOn(mockIpc, 'mergeBranch').mockResolvedValue({
      kind: 'conflicts',
      paths: ['a.ts', 'b.ts'],
      stashed: true,
    });
    const deps = makeDeps();
    await useMergeActions(deps).handleMergeBranch('feat');
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      expect.stringContaining('2 conflict(s)'),
    );
    expect(deps.pushToast).toHaveBeenCalledWith('info', expect.stringContaining('stash@{0}'));
  });

  it('errors toast and never throw', async () => {
    vi.spyOn(mockIpc, 'mergeBranch').mockRejectedValue(appErr('git', 'merge failed'));
    const deps = makeDeps();
    await useMergeActions(deps).handleMergeBranch('feat');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'merge failed');
    expect(deps.refreshAll).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('resolve conflict', () => {
  it('handleResolveConflict passes the resolution through and refreshes', async () => {
    const resolve = vi.spyOn(mockIpc, 'resolveConflict').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useMergeActions(deps).handleResolveConflict('a.ts', 'ours');
    expect(resolve).toHaveBeenCalledWith(REPO, 'a.ts', 'ours');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('handleResolveConflictText takes an optional success message (the AI copy)', async () => {
    vi.spyOn(mockIpc, 'resolveConflictText').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useMergeActions(deps).handleResolveConflictText('a.ts', 'body', 'Resolved a.ts with AI');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Resolved a.ts with AI');
  });

  it('handleResolveConflictText toasts AND rethrows on error (editor stays open)', async () => {
    vi.spyOn(mockIpc, 'resolveConflictText').mockRejectedValue(appErr('io', 'denied'));
    const deps = makeDeps();
    await expect(
      useMergeActions(deps).handleResolveConflictText('a.ts', 'body'),
    ).rejects.toMatchObject({ message: 'denied' });
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'denied');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('openAiProposal (P68d §5.3)', () => {
  it('opens the proposed body in the ai-proposal slot, keeping ours/theirs', async () => {
    vi.spyOn(mockIpc, 'getConflict').mockResolvedValue(CONFLICT_FILE);
    const deps = makeDeps();
    await useMergeActions(deps).openAiProposal('a.ts', 'clean\n');
    expect(deps.setDiffSlot).toHaveBeenCalledWith(
      expect.objectContaining({
        key: 'ai-proposal:a.ts',
        state: 'ready',
        conflict: expect.objectContaining({ text: 'clean\n', ours: 'ours\n' }),
      }),
    );
  });

  it('a markerful body is still shown VERBATIM for review — the editor is the gate', async () => {
    // The safety gate that refuses to STAGE a markerful body lives in `useAiRuns`
    // (§5.2). This function's job is the opposite: show the user exactly what came
    // back, markers and all, so they can finish it by hand.
    vi.spyOn(mockIpc, 'getConflict').mockResolvedValue(CONFLICT_FILE);
    const deps = makeDeps();
    await useMergeActions(deps).openAiProposal('a.ts', MARKERFUL);
    expect(deps.setDiffSlot).toHaveBeenCalledWith(
      expect.objectContaining({ conflict: expect.objectContaining({ text: MARKERFUL }) }),
    );
  });

  it('a reqId bump during the LOCAL getConflict drops only the slot write', async () => {
    const deps = makeDeps();
    vi.spyOn(mockIpc, 'getConflict').mockImplementation(async () => {
      deps.fileDiffReqId.current += 1; // user opened another diff mid-flight
      return CONFLICT_FILE;
    });
    await useMergeActions(deps).openAiProposal('a.ts', 'clean\n');
    expect(deps.setDiffSlot).not.toHaveBeenCalled();
  });

  it('never bumps fileDiffReqId before an AI CLI call (§5.1, the item-5 rule)', async () => {
    // The function touches fileDiffReqId exactly ONCE, immediately before a fast
    // LOCAL read, and calls no `ipc.ai*` at all. That is the structural guarantee
    // that a file switch can no longer destroy a computed proposal.
    const ai = vi.spyOn(mockIpc, 'aiResolveConflict');
    const stream = vi.spyOn(mockIpc, 'aiResolveConflictStream');
    vi.spyOn(mockIpc, 'getConflict').mockResolvedValue(CONFLICT_FILE);
    const deps = makeDeps();
    await useMergeActions(deps).openAiProposal('a.ts', 'clean\n');
    expect(deps.fileDiffReqId.current).toBe(1);
    expect(ai).not.toHaveBeenCalled();
    expect(stream).not.toHaveBeenCalled();
  });

  it('getConflict failure toasts and writes no slot', async () => {
    vi.spyOn(mockIpc, 'getConflict').mockRejectedValue(appErr('other', 'gone'));
    const deps = makeDeps();
    await useMergeActions(deps).openAiProposal('a.ts', 'clean\n');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'gone');
    expect(deps.setDiffSlot).not.toHaveBeenCalled();
  });
});

describe('handleCommitMerge / handleAbortMerge', () => {
  it('commits the merge through the hook gate and toasts', async () => {
    const commitMerge = vi.spyOn(mockIpc, 'commitMerge').mockResolvedValue(COMMIT_RES);
    const deps = makeDeps();
    await useMergeActions(deps).handleCommitMerge('merge msg', null, true);
    expect(commitMerge).toHaveBeenCalledWith(REPO, 'merge msg', true);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Merge committed');
  });

  it('rethrows ONLY the hook-cancel sentinel; other errors toast', async () => {
    const deps = makeDeps({
      runWithHookGate: vi.fn(async () => {
        throw COMMIT_HOOK_CANCELED;
      }),
    });
    await expect(useMergeActions(deps).handleCommitMerge('m')).rejects.toBe(COMMIT_HOOK_CANCELED);
    expect(deps.pushToast).not.toHaveBeenCalled();

    vi.spyOn(mockIpc, 'commitMerge').mockRejectedValue(appErr('emptyMessage', 'empty'));
    const deps2 = makeDeps();
    await useMergeActions(deps2).handleCommitMerge('');
    expect(deps2.pushToast).toHaveBeenCalledWith('error', 'empty');
  });

  it('abort merges refreshes and toasts; errors toast', async () => {
    const abort = vi.spyOn(mockIpc, 'abortMerge').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useMergeActions(deps).handleAbortMerge();
    expect(abort).toHaveBeenCalledWith(REPO);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Merge aborted');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });
});
