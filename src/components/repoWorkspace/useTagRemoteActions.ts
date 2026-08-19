import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { reportRemoteOpError } from '../../ipc/gitNotFound';
import type { BaseActionDeps } from './types';

/** P22: tag + remote management. Tag create/delete refetch branches (tag list,
 *  §2.0) + graph (pill). Add/remove/rename move remote-tracking refs (and thus
 *  graph pills), so those refetch remotes + branches + graph; set-url changes
 *  only the RemoteInfo list. */
export function useTagRemoteActions(
  deps: BaseActionDeps & {
    refetchBranches: () => Promise<void>;
    refetchGraph: () => Promise<void>;
    refetchRemotes: () => Promise<void>;
  },
) {
  const { repoId, pushToast, setMutating, refetchBranches, refetchGraph, refetchRemotes } = deps;

  async function handleCreateTag(oid: string, name: string, message: string | null) {
    setMutating(true);
    try {
      await ipc.createTag(repoId, name, oid, message, /* force */ false);
      pushToast('success', `Created tag ${name}`);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleDeleteTag(name: string) {
    setMutating(true);
    try {
      await ipc.deleteTag(repoId, name);
      pushToast('success', `Deleted tag ${name}`);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handlePushTag(remote: string, name: string) {
    setMutating(true);
    try {
      await ipc.pushTag(repoId, remote, name, /* force */ false);
      pushToast('success', `Pushed tag ${name} → ${remote}`);
    } catch (e) {
      // P70 (UI §10.3): pushing a tag authenticates an HTTPS remote through the
      // credential helper, so it can fail with `gitNotFound` — route it through
      // the shared reporter (latch + ONE keyed toast) instead of dumping the
      // 692-char Rust paragraph into a sticky, unkeyed toast.
      reportRemoteOpError('Push tag', e, pushToast);
    } finally {
      setMutating(false);
    }
  }

  async function handleAddRemote(name: string, url: string) {
    setMutating(true);
    try {
      await ipc.addRemote(repoId, name, url);
      pushToast('success', `Added remote ${name}`);
      await Promise.all([refetchRemotes(), refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRemoveRemote(name: string) {
    setMutating(true);
    try {
      await ipc.removeRemote(repoId, name);
      pushToast('success', `Removed remote ${name}`);
      await Promise.all([refetchRemotes(), refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRenameRemote(name: string, newName: string) {
    setMutating(true);
    try {
      await ipc.renameRemote(repoId, name, newName);
      pushToast('success', `Renamed remote ${name} → ${newName}`);
      await Promise.all([refetchRemotes(), refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleSetRemoteUrl(name: string, url: string) {
    setMutating(true);
    try {
      await ipc.setRemoteUrl(repoId, name, url);
      pushToast('success', `Updated URL for ${name}`);
      await refetchRemotes();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return {
    handleCreateTag,
    handleDeleteTag,
    handlePushTag,
    handleAddRemote,
    handleRemoveRemote,
    handleRenameRemote,
    handleSetRemoteUrl,
  };
}
