import { ipc } from '../../ipc';
import { errorMessage, isAppError } from '../../utils/errors';
import { shortOid } from '../workspaceUtils';
import type { BaseActionDeps, PendingCherrypick, Setter } from './types';

/** P20 §5/§6 + P47d: cherry-pick + revert handling. */
export function useCherrypickRevertActions(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    setPendingCherrypick: Setter<PendingCherrypick | null>;
  },
) {
  const { repoId, pushToast, setMutating, refreshAll, setPendingCherrypick } = deps;

  // An empty pick/revert (nothingToCommit) is an info, not an error toast (§8.1);
  // every other failure surfaces via the sticky error toast.
  function surfacePickRevertError(e: unknown) {
    if (isAppError(e) && e.kind === 'nothingToCommit') {
      pushToast('info', 'Nothing to apply — the change is already present');
    } else {
      pushToast('error', errorMessage(e));
    }
  }

  // P47d: cherry-pick surfaces an editable-message dialog. Fetch the source
  // commit's FULL message (the graph node only carries the summary line) and
  // open the prefilled dialog; confirmCherrypick runs the actual pick.
  async function handleCherrypick(oid: string) {
    const reqOid = oid;
    setPendingCherrypick({ oid, initialMessage: '', loading: true });
    try {
      const diff = await ipc.getCommitDiff(repoId, oid);
      setPendingCherrypick((prev) =>
        prev !== null && prev.oid === reqOid
          ? { ...prev, initialMessage: diff.details.message, loading: false }
          : prev,
      );
    } catch (e) {
      // Don't leave an empty dialog silently open — close and toast.
      setPendingCherrypick((prev) => (prev !== null && prev.oid === reqOid ? null : prev));
      pushToast('error', `Could not load the commit message: ${errorMessage(e)}`);
    }
  }

  async function confirmCherrypick(oid: string, message: string) {
    setMutating(true);
    try {
      const res = await ipc.cherrypickCommit(repoId, oid, message);
      switch (res.kind) {
        case 'committed':
          pushToast(
            'success',
            `Cherry-picked ${shortOid(res.oid)}` +
              (res.stashed ? ' · stashed changes restored' : ''),
          );
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Cherry-pick paused: ${res.paths.length} conflict(s) to resolve` +
              (res.stashed ? ' · your changes are stashed (stash@{0})' : ''),
          );
          break;
        case 'stashPopConflicts':
          pushToast(
            'error',
            `Cherry-picked ${shortOid(res.head)}, but re-applying your stashed changes hit ` +
              `${res.paths.length} conflict(s). Your changes are still on the stash (stash@{0}); ` +
              'resolve the conflicts, then drop the stash.',
          );
          break;
      }
      setPendingCherrypick(null);
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleRevert(oid: string) {
    setMutating(true);
    try {
      const res = await ipc.revertCommit(repoId, oid);
      switch (res.kind) {
        case 'committed':
          pushToast(
            'success',
            `Reverted ${shortOid(res.oid)}` + (res.stashed ? ' · stashed changes restored' : ''),
          );
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Revert paused: ${res.paths.length} conflict(s) to resolve` +
              (res.stashed ? ' · your changes are stashed (stash@{0})' : ''),
          );
          break;
        case 'stashPopConflicts':
          pushToast(
            'error',
            `Reverted ${shortOid(res.head)}, but re-applying your stashed changes hit ` +
              `${res.paths.length} conflict(s). Your changes are still on the stash (stash@{0}); ` +
              'resolve the conflicts, then drop the stash.',
          );
          break;
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleCherrypickContinue() {
    setMutating(true);
    try {
      const res = await ipc.cherrypickContinue(repoId);
      // Conflicts can't recur on a single-pick continue, but map defensively.
      if (res.kind === 'committed') {
        pushToast('success', `Cherry-picked ${shortOid(res.oid)}`);
      } else {
        pushToast('info', `Cherry-pick paused: ${res.paths.length} conflict(s) to resolve`);
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleRevertContinue() {
    setMutating(true);
    try {
      const res = await ipc.revertContinue(repoId);
      if (res.kind === 'committed') {
        pushToast('success', `Reverted ${shortOid(res.oid)}`);
      } else {
        pushToast('info', `Revert paused: ${res.paths.length} conflict(s) to resolve`);
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleCherrypickAbort() {
    setMutating(true);
    try {
      await ipc.cherrypickAbort(repoId);
      await refreshAll();
      pushToast('success', 'Cherry-pick aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRevertAbort() {
    setMutating(true);
    try {
      await ipc.revertAbort(repoId);
      await refreshAll();
      pushToast('success', 'Revert aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return {
    handleCherrypick,
    confirmCherrypick,
    handleRevert,
    handleCherrypickContinue,
    handleRevertContinue,
    handleCherrypickAbort,
    handleRevertAbort,
  };
}
