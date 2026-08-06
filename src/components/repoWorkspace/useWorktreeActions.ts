import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { CopySelection } from '../../ipc';
import type { BaseActionDeps, Setter } from './types';

/** P27: worktree create/lock/unlock/remove. Create/lock/unlock/remove never
 *  change the CURRENT repo's status/graph (remove refuses the open worktree
 *  server-side) → refetchWorktrees suffices. */
export function useWorktreeActions(
  deps: BaseActionDeps & {
    refetchWorktrees: () => Promise<void>;
    setNewWorktreeOpen: Setter<boolean>;
  },
) {
  const { repoId, pushToast, setMutating, refetchWorktrees, setNewWorktreeOpen } = deps;

  // Called by WorktreeCreateDialog; rethrows so the dialog shows the error
  // inline and stays open (success closes it here + toasts the derived path).
  async function handleAddWorktree(
    branch: string,
    name: string,
    selections: CopySelection[],
  ): Promise<void> {
    setMutating(true);
    try {
      // Route to the copy-aware command only when there is something to copy;
      // an empty plan is a plain create (P32 Part B).
      const wt =
        selections.length > 0
          ? await ipc.addWorktreeWithChanges(repoId, branch, name, selections)
          : await ipc.addWorktree(repoId, branch, name);
      const copied = selections.length > 0 ? ` (+${selections.length} file(s) copied)` : '';
      pushToast('success', `Created worktree for ${branch} at ${wt.absPath}${copied}`);
      setNewWorktreeOpen(false);
      await refetchWorktrees();
    } finally {
      setMutating(false);
    }
  }

  // Called after the lock-reason PromptDialog (empty reason → no reason).
  async function handleLockWorktree(name: string, reason: string | undefined) {
    setMutating(true);
    try {
      await ipc.lockWorktree(repoId, name, reason);
      pushToast('success', `Locked worktree ${name}`);
      await refetchWorktrees();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleUnlockWorktree(name: string) {
    setMutating(true);
    try {
      await ipc.unlockWorktree(repoId, name);
      pushToast('success', `Unlocked worktree ${name}`);
      await refetchWorktrees();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Called AFTER the ConfirmDialog naming the directory. The backend
  // independently refuses main/current/locked/dirty (P27 §2.6) — its message
  // surfaces via the error toast.
  async function handleRemoveWorktree(name: string) {
    setMutating(true);
    try {
      await ipc.removeWorktree(repoId, name);
      pushToast('success', `Removed worktree ${name}`);
      await refetchWorktrees();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return { handleAddWorktree, handleLockWorktree, handleUnlockWorktree, handleRemoveWorktree };
}
