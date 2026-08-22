/** T3.2a — useBranchActions: create/checkout/delete/rename local + remote branches. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useBranchActions } from './useBranchActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';
import type { BranchesSnapshot } from '../../ipc';

afterEach(() => vi.restoreAllMocks());

const BRANCHES: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0, tip: 'h'.repeat(40) },
    { name: 'feat', isHead: false, upstream: 'origin/feat', ahead: 1, behind: 2, tip: 'f'.repeat(40) },
  ],
  remote: [],
  tags: [],
  head: { branchName: 'main', oid: 'h'.repeat(40), detached: false, unborn: false },
};

type Deps = Parameters<typeof useBranchActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    // Accepted-but-unused after P85 A1 (see useBranchActions deps note); still
    // required by the deps shape until P86 drops them from RepoWorkspace.
    refetchBranches: asyncFn(),
    refetchGraph: asyncFn(),
    branches: BRANCHES,
    setBranchesError: vi.fn(),
    setPendingCreateBranch: vi.fn(),
    setPendingRenameBranch: vi.fn(),
    ...over,
  };
}

describe('handleCreateBranch', () => {
  it('creates, refreshes via the echo-armed round, clears the error banner first', async () => {
    const create = vi.spyOn(mockIpc, 'createBranch').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useBranchActions(deps).handleCreateBranch('new-branch');
    expect(create).toHaveBeenCalledWith(REPO, 'new-branch');
    expect(deps.setBranchesError).toHaveBeenCalledWith(null);
    // P85 A1: one echo-armed refreshAll, not raw refetchBranches/refetchGraph.
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('RETHROWS errors (the create dialog owns surfacing) but clears mutating', async () => {
    vi.spyOn(mockIpc, 'createBranch').mockRejectedValue(appErr('branchExists', 'exists'));
    const deps = makeDeps();
    await expect(useBranchActions(deps).handleCreateBranch('main')).rejects.toMatchObject({
      kind: 'branchExists',
    });
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
    expect(deps.refreshAll).not.toHaveBeenCalled();
  });
});

describe('handleCheckoutBranch', () => {
  it('plain switch → success toast, refreshAll', async () => {
    const co = vi
      .spyOn(mockIpc, 'checkoutBranch')
      .mockResolvedValue({ stashed: false, fastForwarded: false, apply: null });
    const deps = makeDeps();
    await useBranchActions(deps).handleCheckoutBranch('feat');
    expect(co).toHaveBeenCalledWith(REPO, 'feat');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Switched to feat');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('stashed + fast-forwarded → extras listed with the resolved upstream label', async () => {
    vi.spyOn(mockIpc, 'checkoutBranch').mockResolvedValue({
      stashed: true,
      fastForwarded: true,
      apply: { kind: 'applied' },
    });
    const deps = makeDeps();
    await useBranchActions(deps).handleCheckoutBranch('feat');
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'Switched to feat (stashed & re-applied, fast-forwarded to origin/feat)',
    );
  });

  it('conflicted stash re-apply → warning toast (still a success path)', async () => {
    vi.spyOn(mockIpc, 'checkoutBranch').mockResolvedValue({
      stashed: true,
      fastForwarded: false,
      apply: { kind: 'conflicts', paths: ['a.ts'] },
    });
    const deps = makeDeps();
    await useBranchActions(deps).handleCheckoutBranch('feat');
    expect(deps.pushToast).toHaveBeenCalledWith('warning', expect.stringContaining('stash@{0}'));
  });

  it('errors toast (not the branches banner)', async () => {
    vi.spyOn(mockIpc, 'checkoutBranch').mockRejectedValue(appErr('checkoutConflict', 'blocked'));
    const deps = makeDeps();
    await useBranchActions(deps).handleCheckoutBranch('feat');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'blocked');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('handleCreateBranchHere', () => {
  it('success (no stash) toasts and always clears the pending dialog', async () => {
    const create = vi
      .spyOn(mockIpc, 'createBranchHere')
      .mockResolvedValue({ stashed: false, apply: null });
    const deps = makeDeps();
    await useBranchActions(deps).handleCreateBranchHere('o'.repeat(40), 'topic');
    expect(create).toHaveBeenCalledWith(REPO, 'topic', 'o'.repeat(40));
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Created and checked out topic');
    expect(deps.setPendingCreateBranch).toHaveBeenCalledWith(null);
  });

  it('error → toast AND the pending dialog is still cleared', async () => {
    vi.spyOn(mockIpc, 'createBranchHere').mockRejectedValue(appErr('invalidName', 'bad name'));
    const deps = makeDeps();
    await useBranchActions(deps).handleCreateBranchHere('o'.repeat(40), '..bad');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'bad name');
    expect(deps.setPendingCreateBranch).toHaveBeenCalledWith(null);
  });
});

describe('handleDeleteBranch / handleDeleteRemoteTracking', () => {
  it('delete refreshes via refreshAll; errors land in the branches banner, no toast', async () => {
    const del = vi.spyOn(mockIpc, 'deleteBranch').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useBranchActions(deps).handleDeleteBranch('feat');
    expect(del).toHaveBeenCalledWith(REPO, 'feat');
    // P85 A1: one echo-armed refreshAll (a delete can drop reachable commits).
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);

    del.mockRejectedValue(appErr('unmergedBranch', 'not merged'));
    await useBranchActions(deps).handleDeleteBranch('feat');
    expect(deps.setBranchesError).toHaveBeenCalledWith('not merged');
    expect(deps.pushToast).not.toHaveBeenCalled();
  });

  it('delete remote-tracking mirrors the same refreshAll + banner-error contract', async () => {
    const del = vi.spyOn(mockIpc, 'deleteRemoteBranch').mockRejectedValue(appErr('git', 'nope'));
    const deps = makeDeps();
    await useBranchActions(deps).handleDeleteRemoteTracking('origin/feat');
    expect(del).toHaveBeenCalledWith(REPO, 'origin/feat');
    expect(deps.setBranchesError).toHaveBeenCalledWith('nope');
  });
});

describe('handleRenameBranch', () => {
  it('unchanged name (after trim) → closes the dialog with NO ipc call', async () => {
    const rename = vi.spyOn(mockIpc, 'renameBranch');
    const deps = makeDeps();
    await useBranchActions(deps).handleRenameBranch('feat', ' feat ');
    expect(rename).not.toHaveBeenCalled();
    expect(deps.setPendingRenameBranch).toHaveBeenCalledWith(null);
    expect(deps.setMutating).not.toHaveBeenCalled();
  });

  it('renaming HEAD → refreshAll; toast notes preserved tracking', async () => {
    vi.spyOn(mockIpc, 'renameBranch').mockResolvedValue({
      wasHead: true,
      upstream: 'origin/main',
    });
    const deps = makeDeps();
    await useBranchActions(deps).handleRenameBranch('main', 'trunk');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'Renamed main → trunk (tracking origin/main preserved)',
    );
    expect(deps.setPendingRenameBranch).toHaveBeenCalledWith(null);
  });

  it('renaming a non-HEAD branch → refreshAll (A1); errors toast + close dialog', async () => {
    const rename = vi
      .spyOn(mockIpc, 'renameBranch')
      .mockResolvedValue({ wasHead: false, upstream: null });
    const deps = makeDeps();
    await useBranchActions(deps).handleRenameBranch('feat', 'feature');
    expect(rename).toHaveBeenCalledWith(REPO, 'feat', 'feature');
    // P85 A1: non-HEAD rename now also routes through the echo-armed refreshAll.
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Renamed feat → feature');

    rename.mockRejectedValue(appErr('branchExists', 'exists'));
    await useBranchActions(deps).handleRenameBranch('feat', 'main');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'exists');
    expect(deps.setPendingRenameBranch).toHaveBeenLastCalledWith(null);
  });
});

describe('handleCheckoutRemote', () => {
  it('creates/reuses the tracking branch and refreshes all; errors → banner', async () => {
    const co = vi.spyOn(mockIpc, 'checkoutRemoteBranch').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useBranchActions(deps).handleCheckoutRemote('origin/feat');
    expect(co).toHaveBeenCalledWith(REPO, 'origin/feat');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);

    co.mockRejectedValue(appErr('git', 'gone'));
    await useBranchActions(deps).handleCheckoutRemote('origin/feat');
    expect(deps.setBranchesError).toHaveBeenCalledWith('gone');
  });
});
