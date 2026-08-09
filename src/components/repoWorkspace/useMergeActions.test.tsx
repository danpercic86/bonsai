/** T3.2a — useMergeActions: merge, conflict resolution, AI resolve, merge commit. */
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
const COMMIT_RES: CommitResult = { oid: 'c'.repeat(40), summary: 'merge', branch: 'main' };

type Deps = Parameters<typeof useMergeActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    aiConflictAutonomy: 'proposeReview',
    setAiResolvingPath: vi.fn(),
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

describe('handleAiResolveConflict', () => {
  it('autoResolve + clean proposal → stages the AI text directly', async () => {
    vi.spyOn(mockIpc, 'aiResolveConflict').mockResolvedValue({
      path: 'a.ts',
      proposedText: 'merged body\n',
      costUsd: null,
    });
    const stageText = vi.spyOn(mockIpc, 'resolveConflictText').mockResolvedValue(undefined);
    const deps = makeDeps({ aiConflictAutonomy: 'autoResolve' });
    await useMergeActions(deps).handleAiResolveConflict('a.ts');
    expect(stageText).toHaveBeenCalledWith(REPO, 'a.ts', 'merged body\n');
    expect(deps.pushToast).toHaveBeenCalledWith('success', expect.stringContaining('Resolved a.ts'));
    expect(deps.setAiResolvingPath).toHaveBeenLastCalledWith(null);
  });

  it('autoResolve + markerful proposal → NEVER auto-stages; falls back to review editor', async () => {
    const markerful = '<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feat\n';
    vi.spyOn(mockIpc, 'aiResolveConflict').mockResolvedValue({
      path: 'a.ts',
      proposedText: markerful,
      costUsd: null,
    });
    const stageText = vi.spyOn(mockIpc, 'resolveConflictText');
    vi.spyOn(mockIpc, 'getConflict').mockResolvedValue(CONFLICT_FILE);
    const deps = makeDeps({ aiConflictAutonomy: 'autoResolve' });
    await useMergeActions(deps).handleAiResolveConflict('a.ts');
    expect(stageText).not.toHaveBeenCalled();
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      expect.stringContaining('unresolved markers'),
    );
    expect(deps.setDiffSlot).toHaveBeenCalledWith(
      expect.objectContaining({
        key: 'ai-proposal:a.ts',
        conflict: expect.objectContaining({ text: markerful }),
      }),
    );
  });

  it('proposeReview opens the synthesized proposal in the conflict editor', async () => {
    vi.spyOn(mockIpc, 'aiResolveConflict').mockResolvedValue({
      path: 'a.ts',
      proposedText: 'clean\n',
      costUsd: null,
    });
    vi.spyOn(mockIpc, 'getConflict').mockResolvedValue(CONFLICT_FILE);
    const deps = makeDeps();
    await useMergeActions(deps).handleAiResolveConflict('a.ts');
    expect(deps.setDiffSlot).toHaveBeenCalledWith(
      expect.objectContaining({
        key: 'ai-proposal:a.ts',
        state: 'ready',
        conflict: expect.objectContaining({ text: 'clean\n', ours: 'ours\n' }),
      }),
    );
    expect(deps.setAiResolvingPath).toHaveBeenNthCalledWith(1, 'a.ts');
    expect(deps.setAiResolvingPath).toHaveBeenLastCalledWith(null);
  });

  it('proposal fetch error → toast, spinner cleared, no further IPC', async () => {
    vi.spyOn(mockIpc, 'aiResolveConflict').mockRejectedValue(appErr('other', 'ai down'));
    const getConflict = vi.spyOn(mockIpc, 'getConflict');
    const deps = makeDeps();
    await useMergeActions(deps).handleAiResolveConflict('a.ts');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'ai down');
    expect(deps.setAiResolvingPath).toHaveBeenLastCalledWith(null);
    expect(getConflict).not.toHaveBeenCalled();
  });

  it('stale request guard: a reqId bump during getConflict drops the slot write', async () => {
    vi.spyOn(mockIpc, 'aiResolveConflict').mockResolvedValue({
      path: 'a.ts',
      proposedText: 'clean\n',
      costUsd: null,
    });
    const deps = makeDeps();
    vi.spyOn(mockIpc, 'getConflict').mockImplementation(async () => {
      deps.fileDiffReqId.current += 1; // user opened another diff mid-flight
      return CONFLICT_FILE;
    });
    await useMergeActions(deps).handleAiResolveConflict('a.ts');
    expect(deps.setDiffSlot).not.toHaveBeenCalled();
    expect(deps.setAiResolvingPath).toHaveBeenLastCalledWith(null);
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
