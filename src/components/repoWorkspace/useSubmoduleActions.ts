import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { BaseActionDeps } from './types';

/** P19 + P60d: submodule init/update/sync (non-destructive to the superproject)
 *  and add/deinit/remove (which DO change the superproject index/worktree).
 *  init/update/sync only need a submodule refetch; add/deinit/remove also
 *  refetch status + graph (the gitlink is staged/removed). */
export function useSubmoduleActions(
  deps: BaseActionDeps & {
    refetchSubmodules: () => Promise<void>;
    refetchStatus: () => Promise<void>;
    refetchGraph: () => Promise<void>;
  },
) {
  const { repoId, pushToast, setMutating, refetchSubmodules, refetchStatus, refetchGraph } = deps;

  async function handleInitSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.initSubmodule(repoId, name);
      pushToast('success', `Initialized ${name}`);
      await refetchSubmodules();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleUpdateSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.updateSubmodule(repoId, name);
      pushToast('success', `Updated ${name}`);
      await refetchSubmodules();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleSyncSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.syncSubmodule(repoId, name);
      pushToast('success', `Synced URL for ${name}`);
      await refetchSubmodules();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // add/deinit/remove edit the superproject index + worktree → refetch status +
  // graph alongside the submodule list.
  async function refreshAfterChange() {
    await Promise.all([refetchSubmodules(), refetchStatus(), refetchGraph()]);
  }

  async function handleAddSubmodule(url: string, path: string) {
    setMutating(true);
    try {
      const info = await ipc.addSubmodule(repoId, url, path);
      pushToast('success', `Added submodule ${info.path}`);
      await refreshAfterChange();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleDeinitSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.deinitSubmodule(repoId, name);
      pushToast('success', `Deinitialized ${name}`);
      await refreshAfterChange();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRemoveSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.removeSubmodule(repoId, name);
      pushToast('success', `Removed ${name}`);
      await refreshAfterChange();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return {
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
    handleAddSubmodule,
    handleDeinitSubmodule,
    handleRemoveSubmodule,
  };
}
