import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { reportRemoteOpError } from '../../ipc/gitNotFound';
import type { RefreshAll } from './refreshScope';
import type { BaseActionDeps } from './types';

/** P22: tag + remote management. Tag create/delete change the ref/pill set, so
 *  they route through the echo-armed `refreshAll('refsOnly')` (graph + branch/tag
 *  list + compare) plus a FORCED tag-sync verdict (no scope forces ls-remote).
 *  Add/remove/rename move remote-tracking refs → `refreshAll('remoteMeta')`;
 *  set-url writes only `.git/config` (watcher ignores it) so it stays on a raw
 *  `refetchRemotes()` (P88a OD-P88-1). */
export function useTagRemoteActions(
  deps: BaseActionDeps & {
    refreshAll: RefreshAll;
    refetchRemotes: () => Promise<void>;
    /** P77: re-run listTagSync after a resolve op mutates local/remote tags. */
    refetchTagSync: (opts?: { force?: boolean }) => Promise<void>;
  },
) {
  const { repoId, pushToast, setMutating, refreshAll, refetchRemotes, refetchTagSync } = deps;

  /** P77 §5 / P88a row 1: refetch the surfaces a tag resolve touches — the ref set
   *  (graph + branch/tag list + compare, via `refsOnly`) and the FORCED sync verdict
   *  (no scope forces an ls-remote, so keep the explicit forced tagSync). Armed
   *  refresh drops the tag write's own watcher echo → one round. */
  async function refreshAfterTagOp() {
    await Promise.all([refreshAll('refsOnly'), refetchTagSync({ force: true })]);
  }

  async function handleCreateTag(oid: string, name: string, message: string | null) {
    setMutating(true);
    try {
      await ipc.createTag(repoId, name, oid, message, /* force */ false);
      pushToast('success', `Created tag ${name}`);
      await refreshAfterTagOp();
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
      await refreshAfterTagOp();
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
      // P77: a pushed tag flips unpushed → in-sync; refresh the verdict (no-op
      // when the Tags section was never opened this session).
      await refetchTagSync({ force: true });
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

  // P77 §3 item 1: force-update a STALE local tag to the remote's target. Local
  // pointer move (reversible via reflog) — no confirm. Success states the change.
  async function handleForceRefreshTag(remote: string, name: string) {
    setMutating(true);
    try {
      await ipc.forceRefreshTag(repoId, remote, name);
      pushToast('success', `Updated ${name} to match ${remote}.`);
      await refreshAfterTagOp();
    } catch (e) {
      pushToast('error', `Couldn't update ${name}. ${errorMessage(e)}`, `tagsync:${name}`);
    } finally {
      setMutating(false);
    }
  }

  // P77 §3 item 2: create a local tag from a remote-only ghost row (one-tag fetch
  // brings the tag object across, preserving annotation).
  async function handleFetchRemoteTag(remote: string, name: string) {
    setMutating(true);
    try {
      await ipc.forceRefreshTag(repoId, remote, name);
      pushToast('success', `Created local tag ${name}.`);
      await refreshAfterTagOp();
    } catch (e) {
      pushToast('error', `Couldn't create ${name} locally. ${errorMessage(e)}`, `tagsync:${name}`);
    } finally {
      setMutating(false);
    }
  }

  // P77 §4.1: delete a tag ON the remote (destructive — routed through the confirm
  // dialog before this fires).
  async function handleDeleteRemoteTag(remote: string, name: string) {
    setMutating(true);
    try {
      await ipc.deleteRemoteTag(repoId, remote, name);
      pushToast('success', `Deleted ${name} on ${remote}.`);
      await refreshAfterTagOp();
    } catch (e) {
      pushToast(
        'error',
        `Couldn't delete ${name} on ${remote}. ${errorMessage(e)}`,
        `tagsync:${name}`,
      );
    } finally {
      setMutating(false);
    }
  }

  // P77 §4.2: force-move a tag ON the remote (reuse push_tag force=true) — the
  // destructive counterpart to force-refresh. Confirmed before this fires.
  async function handleForceMoveRemoteTag(remote: string, name: string, newShort: string) {
    setMutating(true);
    try {
      await ipc.pushTag(repoId, remote, name, /* force */ true);
      pushToast('success', `Moved ${name} on ${remote} to ${newShort}.`);
      await refreshAfterTagOp();
    } catch (e) {
      pushToast(
        'error',
        `Couldn't move ${name} on ${remote}. ${errorMessage(e)}`,
        `tagsync:${name}`,
      );
    } finally {
      setMutating(false);
    }
  }

  async function handleAddRemote(name: string, url: string) {
    setMutating(true);
    try {
      await ipc.addRemote(repoId, name, url);
      pushToast('success', `Added remote ${name}`);
      // P88a rows 2-4: add/remove/rename move remote-tracking refs ⇒ remoteMeta
      // (graph + branches + remotes + compare + non-forced tagSync), echo-armed.
      await refreshAll('remoteMeta');
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
      // P88a rows 2-4: add/remove/rename move remote-tracking refs ⇒ remoteMeta
      // (graph + branches + remotes + compare + non-forced tagSync), echo-armed.
      await refreshAll('remoteMeta');
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
      // P88a rows 2-4: add/remove/rename move remote-tracking refs ⇒ remoteMeta
      // (graph + branches + remotes + compare + non-forced tagSync), echo-armed.
      await refreshAll('remoteMeta');
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
    handleForceRefreshTag,
    handleFetchRemoteTag,
    handleDeleteRemoteTag,
    handleForceMoveRemoteTag,
    handleAddRemote,
    handleRemoveRemote,
    handleRenameRemote,
    handleSetRemoteUrl,
  };
}
