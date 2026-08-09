/** T3.2a — useWorktreeActions: add (plain + copy-aware) / lock / unlock / remove. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useWorktreeActions } from './useWorktreeActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';
import type { CopySelection, WorktreeInfo } from '../../ipc';

afterEach(() => vi.restoreAllMocks());

const WT: WorktreeInfo = {
  name: 'feat-wt',
  absPath: 'D:/wt/feat-wt',
  relPath: null,
  branch: 'feat',
  headOid: 'a'.repeat(40),
  locked: false,
  lockReason: null,
  isMain: false,
  isCurrent: false,
  prunable: false,
  valid: true,
};

type Deps = Parameters<typeof useWorktreeActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refetchWorktrees: asyncFn(),
    setNewWorktreeOpen: vi.fn(),
    ...over,
  };
}

describe('handleAddWorktree', () => {
  it('empty copy plan → plain addWorktree; toasts path, closes dialog, refetches', async () => {
    const add = vi.spyOn(mockIpc, 'addWorktree').mockResolvedValue(WT);
    const addWith = vi.spyOn(mockIpc, 'addWorktreeWithChanges');
    const deps = makeDeps();
    await useWorktreeActions(deps).handleAddWorktree('feat', 'feat-wt', []);
    expect(add).toHaveBeenCalledWith(REPO, 'feat', 'feat-wt');
    expect(addWith).not.toHaveBeenCalled();
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'Created worktree for feat at D:/wt/feat-wt',
    );
    expect(deps.setNewWorktreeOpen).toHaveBeenCalledWith(false);
    expect(deps.refetchWorktrees).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('with selections → copy-aware command; toast notes the copied count', async () => {
    const addWith = vi.spyOn(mockIpc, 'addWorktreeWithChanges').mockResolvedValue(WT);
    const selections: CopySelection[] = [
      { path: 'a.ts', action: 'copy' } as unknown as CopySelection,
      { path: 'b.ts', action: 'copy' } as unknown as CopySelection,
    ];
    const deps = makeDeps();
    await useWorktreeActions(deps).handleAddWorktree('feat', 'feat-wt', selections);
    expect(addWith).toHaveBeenCalledWith(REPO, 'feat', 'feat-wt', selections);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      expect.stringContaining('(+2 file(s) copied)'),
    );
  });

  it('errors RETHROW (dialog shows them inline and stays open); mutating cleared', async () => {
    vi.spyOn(mockIpc, 'addWorktree').mockRejectedValue(appErr('io', 'dir exists'));
    const deps = makeDeps();
    await expect(
      useWorktreeActions(deps).handleAddWorktree('feat', 'feat-wt', []),
    ).rejects.toMatchObject({ message: 'dir exists' });
    expect(deps.setNewWorktreeOpen).not.toHaveBeenCalled();
    expect(deps.refetchWorktrees).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('lock / unlock', () => {
  it('lock forwards the optional reason and toasts; unlock mirrors it', async () => {
    const lock = vi.spyOn(mockIpc, 'lockWorktree').mockResolvedValue(undefined);
    const unlock = vi.spyOn(mockIpc, 'unlockWorktree').mockResolvedValue(undefined);
    const deps = makeDeps();
    const actions = useWorktreeActions(deps);
    await actions.handleLockWorktree('feat-wt', 'on a USB drive');
    expect(lock).toHaveBeenCalledWith(REPO, 'feat-wt', 'on a USB drive');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Locked worktree feat-wt');

    await actions.handleUnlockWorktree('feat-wt');
    expect(unlock).toHaveBeenCalledWith(REPO, 'feat-wt');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Unlocked worktree feat-wt');
    expect(deps.refetchWorktrees).toHaveBeenCalledTimes(2);
  });

  it('lock error → error toast, no refetch', async () => {
    vi.spyOn(mockIpc, 'lockWorktree').mockRejectedValue(appErr('git', 'already locked'));
    const deps = makeDeps();
    await useWorktreeActions(deps).handleLockWorktree('feat-wt', undefined);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'already locked');
    expect(deps.refetchWorktrees).not.toHaveBeenCalled();
  });
});

describe('handleRemoveWorktree (confirm-gated upstream)', () => {
  it('removes + toasts + refetches; backend refusal (dirty/locked) surfaces as error toast', async () => {
    const remove = vi.spyOn(mockIpc, 'removeWorktree').mockResolvedValue(undefined);
    const deps = makeDeps();
    const actions = useWorktreeActions(deps);
    await actions.handleRemoveWorktree('feat-wt');
    expect(remove).toHaveBeenCalledWith(REPO, 'feat-wt');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Removed worktree feat-wt');
    expect(deps.refetchWorktrees).toHaveBeenCalledTimes(1);

    remove.mockRejectedValue(appErr('git', 'worktree is dirty'));
    await actions.handleRemoveWorktree('feat-wt');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'worktree is dirty');
    expect(deps.refetchWorktrees).toHaveBeenCalledTimes(1);
  });
});
