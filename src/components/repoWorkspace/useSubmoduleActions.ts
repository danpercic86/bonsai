import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { BaseActionDeps } from './types';

/** P19: submodule init/update/sync. Non-destructive to the superproject → no
 *  confirm dialog; refetchSubmodules suffices (submodule ops don't change the
 *  superproject status/graph in v1). */
export function useSubmoduleActions(
  deps: BaseActionDeps & { refetchSubmodules: () => Promise<void> },
) {
  const { repoId, pushToast, setMutating, refetchSubmodules } = deps;

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

  return { handleInitSubmodule, handleUpdateSubmodule, handleSyncSubmodule };
}
