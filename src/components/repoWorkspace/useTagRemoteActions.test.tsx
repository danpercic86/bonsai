/** T3.2a — useTagRemoteActions: tag create/delete/push + remote add/remove/rename/set-url. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useTagRemoteActions } from './useTagRemoteActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';

afterEach(() => vi.restoreAllMocks());

const OID = 'a'.repeat(40);

type Deps = Parameters<typeof useTagRemoteActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refetchBranches: asyncFn(),
    refetchGraph: asyncFn(),
    refetchRemotes: asyncFn(),
    ...over,
  };
}

describe('tags', () => {
  it('createTag passes force:false, toasts, refetches branches + graph (not remotes)', async () => {
    const create = vi.spyOn(mockIpc, 'createTag').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useTagRemoteActions(deps).handleCreateTag(OID, 'v1.0', 'release');
    expect(create).toHaveBeenCalledWith(REPO, 'v1.0', OID, 'release', false);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Created tag v1.0');
    expect(deps.refetchBranches).toHaveBeenCalledTimes(1);
    expect(deps.refetchGraph).toHaveBeenCalledTimes(1);
    expect(deps.refetchRemotes).not.toHaveBeenCalled();
    expectMutatingCycle(deps.setMutating);
  });

  it('createTag error (tag exists) → error toast, no refetch', async () => {
    vi.spyOn(mockIpc, 'createTag').mockRejectedValue(appErr('git', 'tag exists'));
    const deps = makeDeps();
    await useTagRemoteActions(deps).handleCreateTag(OID, 'v1.0', null);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'tag exists');
    expect(deps.refetchBranches).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });

  it('deleteTag toasts + refetches; pushTag toasts and refetches NOTHING', async () => {
    const del = vi.spyOn(mockIpc, 'deleteTag').mockResolvedValue(undefined);
    const push = vi.spyOn(mockIpc, 'pushTag').mockResolvedValue(undefined);
    const deps = makeDeps();
    const actions = useTagRemoteActions(deps);
    await actions.handleDeleteTag('v1.0');
    expect(del).toHaveBeenCalledWith(REPO, 'v1.0');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Deleted tag v1.0');
    expect(deps.refetchGraph).toHaveBeenCalledTimes(1);

    await actions.handlePushTag('origin', 'v1.0');
    expect(push).toHaveBeenCalledWith(REPO, 'origin', 'v1.0', false);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Pushed tag v1.0 → origin');
    expect(deps.refetchGraph).toHaveBeenCalledTimes(1); // unchanged — push moves no local ref
    expect(deps.refetchRemotes).not.toHaveBeenCalled();
  });
});

describe('remotes', () => {
  it('addRemote refetches remotes + branches + graph and toasts', async () => {
    const add = vi.spyOn(mockIpc, 'addRemote').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useTagRemoteActions(deps).handleAddRemote('fork', 'https://x/y.git');
    expect(add).toHaveBeenCalledWith(REPO, 'fork', 'https://x/y.git');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Added remote fork');
    expect(deps.refetchRemotes).toHaveBeenCalledTimes(1);
    expect(deps.refetchBranches).toHaveBeenCalledTimes(1);
    expect(deps.refetchGraph).toHaveBeenCalledTimes(1);
  });

  it('removeRemote / renameRemote refetch all three; errors toast', async () => {
    const remove = vi.spyOn(mockIpc, 'removeRemote').mockResolvedValue(undefined);
    const rename = vi
      .spyOn(mockIpc, 'renameRemote')
      .mockRejectedValue(appErr('git', 'name in use'));
    const deps = makeDeps();
    const actions = useTagRemoteActions(deps);
    await actions.handleRemoveRemote('fork');
    expect(remove).toHaveBeenCalledWith(REPO, 'fork');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Removed remote fork');
    expect(deps.refetchRemotes).toHaveBeenCalledTimes(1);

    await actions.handleRenameRemote('origin', 'upstream');
    expect(rename).toHaveBeenCalledWith(REPO, 'origin', 'upstream');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'name in use');
    expect(deps.refetchRemotes).toHaveBeenCalledTimes(1); // no refetch on failure
  });

  it('setRemoteUrl refetches ONLY the remotes list', async () => {
    const setUrl = vi.spyOn(mockIpc, 'setRemoteUrl').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useTagRemoteActions(deps).handleSetRemoteUrl('origin', 'git@x:y.git');
    expect(setUrl).toHaveBeenCalledWith(REPO, 'origin', 'git@x:y.git');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Updated URL for origin');
    expect(deps.refetchRemotes).toHaveBeenCalledTimes(1);
    expect(deps.refetchBranches).not.toHaveBeenCalled();
    expect(deps.refetchGraph).not.toHaveBeenCalled();
  });
});
