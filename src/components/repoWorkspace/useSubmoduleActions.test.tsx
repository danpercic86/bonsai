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
    setSubmoduleBusy: vi.fn(),
    ...over,
  };
}

describe('init / update / sync (non-destructive to the superproject)', () => {
  it('P73: init means init + CHECKOUT — it calls updateSubmodule, never initSubmodule', async () => {
    const update = vi.spyOn(mockIpc, 'updateSubmodule').mockResolvedValue(undefined);
    const init = vi.spyOn(mockIpc, 'initSubmodule').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleInitSubmodule('libs/core');
    expect(update).toHaveBeenCalledWith(REPO, 'libs/core');
    expect(init).not.toHaveBeenCalled();
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Checked out libs/core');
    expect(deps.refetchSubmodules).toHaveBeenCalledTimes(1);
    expect(deps.refetchStatus).not.toHaveBeenCalled();
    expect(deps.refetchGraph).not.toHaveBeenCalled();
    expectMutatingCycle(deps.setMutating);
  });

  it('P73 §6.1: the row busy pill carries the participle and is cleared afterwards', async () => {
    vi.spyOn(mockIpc, 'updateSubmodule').mockResolvedValue(undefined);
    vi.spyOn(mockIpc, 'syncSubmodule').mockResolvedValue(undefined);
    const deps = makeDeps();
    const actions = useSubmoduleActions(deps);
    await actions.handleInitSubmodule('libs/core');
    expect(deps.setSubmoduleBusy).toHaveBeenNthCalledWith(1, {
      name: 'libs/core',
      label: 'checking out…',
    });
    expect(deps.setSubmoduleBusy).toHaveBeenLastCalledWith(null);
    await actions.handleSyncSubmodule('libs/core');
    expect(deps.setSubmoduleBusy).toHaveBeenNthCalledWith(3, {
      name: 'libs/core',
      label: 'syncing…',
    });
    expect(deps.setSubmoduleBusy).toHaveBeenLastCalledWith(null);
  });

  it('P73 §5.2: init failure names the action + target and keeps the backend sentence', async () => {
    const refusal =
      "The folder already has files in it. Move or delete everything inside 'libs/core', then try again.";
    vi.spyOn(mockIpc, 'updateSubmodule').mockRejectedValue(appErr('git', refusal));
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleInitSubmodule('libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      `Couldn't check out libs/core. ${refusal}`,
      'submodule:libs/core',
    );
    // Refetch on failure too, so the badge always reflects what is on disk.
    expect(deps.refetchSubmodules).toHaveBeenCalledTimes(1);
    expect(deps.setSubmoduleBusy).toHaveBeenLastCalledWith(null);
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });

  it('update error → prefixed error toast under the row dedupe key', async () => {
    vi.spyOn(mockIpc, 'updateSubmodule').mockRejectedValue(appErr('networkError', 'clone failed'));
    const deps = makeDeps();
    await useSubmoduleActions(deps).handleUpdateSubmodule('libs/core');
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      "Couldn't update libs/core. clone failed",
      'submodule:libs/core',
    );
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });

  it('a rejecting refetch is swallowed: the op toast stands and busy/mutating still clear', async () => {
    vi.spyOn(mockIpc, 'updateSubmodule').mockResolvedValue(undefined);
    const refetchSubmodules = vi.fn(() => Promise.reject(new Error('refetch exploded')));
    const deps = makeDeps({ refetchSubmodules });
    // Must RESOLVE (not reject): nothing above the hook awaits it, so an escaping
    // rejection would be an unhandled rejection.
    await expect(useSubmoduleActions(deps).handleInitSubmodule('libs/core')).resolves.toBeUndefined();
    expect(refetchSubmodules).toHaveBeenCalledTimes(1);
    // The operation's own toast still fired, and no error toast was invented.
    expect(deps.pushToast).toHaveBeenCalledTimes(1);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Checked out libs/core');
    // Both `finally` cleanups ran despite the refetch failure.
    expect(deps.setSubmoduleBusy).toHaveBeenLastCalledWith(null);
    expectMutatingCycle(deps.setMutating);
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

  it('deinit + remove refetch all three; errors get the prefixed toast + a refetch', async () => {
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
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      "Couldn't remove libs/core. has local changes",
      'submodule:libs/core',
    );
    // P73: the failure path refetches too (the row may not be what we thought).
    expect(deps.refetchStatus).toHaveBeenCalledTimes(2);
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});
