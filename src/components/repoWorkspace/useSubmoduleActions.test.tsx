/** T3.2a — useSubmoduleActions: init/update/sync (list-only) vs add/deinit/remove (index-touching). */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useSubmoduleActions } from './useSubmoduleActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';
import type { SubmoduleInfo } from '../../ipc';

afterEach(() => vi.restoreAllMocks());

const INFO: SubmoduleInfo = {
  name: 'libs/core',
  path: 'libs/core',
  absPath: 'D:/repo/libs/core',
  url: 'https://x/core.git',
  headOid: null,
  indexOid: 'a'.repeat(40),
  wtOid: 'a'.repeat(40),
  status: 'clean' as SubmoduleInfo['status'],
};

type Deps = Parameters<typeof useSubmoduleActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refetchSubmodules: asyncFn(),
    refetchStatus: asyncFn(),
    refetchGraph: asyncFn(),
    ...over,
  };
}

describe('init / update / sync (non-destructive to the superproject)', () => {
  it('init toasts and refetches ONLY the submodule list', async () => {
    const init = vi.spyOn(mockIpc, 'initSubmodule').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleInitSubmodule('libs/core');
    expect(init).toHaveBeenCalledWith(REPO, 'libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Initialized libs/core');
    expect(deps.refetchSubmodules).toHaveBeenCalledTimes(1);
    expect(deps.refetchStatus).not.toHaveBeenCalled();
    expect(deps.refetchGraph).not.toHaveBeenCalled();
    expectMutatingCycle(deps.setMutating);
  });

  it('update error → error toast, no refetch, mutating cleared', async () => {
    vi.spyOn(mockIpc, 'updateSubmodule').mockRejectedValue(appErr('networkError', 'clone failed'));
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleUpdateSubmodule('libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'clone failed');
    expect(deps.refetchSubmodules).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });

  it('sync toasts the URL update', async () => {
    const sync = vi.spyOn(mockIpc, 'syncSubmodule').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleSyncSubmodule('libs/core');
    expect(sync).toHaveBeenCalledWith(REPO, 'libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Synced URL for libs/core');
  });
});

describe('add / deinit / remove (superproject index changes)', () => {
  it('add toasts the resolved path and refetches submodules + status + graph', async () => {
    const add = vi.spyOn(mockIpc, 'addSubmodule').mockResolvedValue(INFO);
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleAddSubmodule('https://x/core.git', 'libs/core');
    expect(add).toHaveBeenCalledWith(REPO, 'https://x/core.git', 'libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Added submodule libs/core');
    expect(deps.refetchSubmodules).toHaveBeenCalledTimes(1);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);
    expect(deps.refetchGraph).toHaveBeenCalledTimes(1);
  });

  it('deinit + remove refetch all three; errors toast without refetch', async () => {
    const deinit = vi.spyOn(mockIpc, 'deinitSubmodule').mockResolvedValue(undefined);
    const remove = vi
      .spyOn(mockIpc, 'removeSubmodule')
      .mockRejectedValue(appErr('git', 'has local changes'));
    const deps = makeDeps();
    const actions = useSubmoduleActions(deps);
    await actions.handleDeinitSubmodule('libs/core');
    expect(deinit).toHaveBeenCalledWith(REPO, 'libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Deinitialized libs/core');
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);

    await actions.handleRemoveSubmodule('libs/core');
    expect(remove).toHaveBeenCalledWith(REPO, 'libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'has local changes');
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1); // unchanged after the failure
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});
