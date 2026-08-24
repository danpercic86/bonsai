import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { RefreshAll } from './refreshScope';
import type { BaseActionDeps, Setter, SubmoduleBusy } from './types';

// Re-exported so the container imports the busy-state type from the hook that
// owns it (one import line in an already-oversized RepoWorkspace.tsx).
export type { SubmoduleBusy } from './types';

/** P19 + P60d: submodule init/update/sync (non-destructive to the superproject)
 *  and add/deinit/remove (which DO change the superproject index/worktree).
 *  init/update/sync only need a submodule refetch; add/deinit/remove also
 *  refetch status + graph (the gitlink is staged/removed).
 *
 *  P73: "Initialize and check out" is `updateSubmodule` (= sm.update(init:true)),
 *  not `initSubmodule` — init alone only writes .git/config, so the old success
 *  toast contradicted the row badge. Every row op reports failure as
 *  `Couldn't <verb> <name>. <backend sentence>` under the dedupe key
 *  `submodule:<name>` (§5.2) and refetches either way, so the badge always shows
 *  what is actually on disk. */
export function useSubmoduleActions(
  deps: BaseActionDeps & {
    refreshAll: RefreshAll;
    refetchSubmodules: () => Promise<void>;
    /** P73 §6.1: row-local busy pill; set per op, cleared in `finally`. */
    setSubmoduleBusy: Setter<SubmoduleBusy | null>;
    /** P82 (F-A7-7): the plain deinit/remove refused because the submodule
     *  worktree is dirty. Opens the danger force-escalation dialog instead of a
     *  success toast; on confirm the container re-invokes the same op with
     *  `force=true`. */
    onSubmoduleDirtyRefused: (name: string, op: 'deinit' | 'remove') => void;
  },
) {
  const {
    repoId,
    pushToast,
    setMutating,
    setSubmoduleBusy,
    refreshAll,
    refetchSubmodules,
    onSubmoduleDirtyRefused,
  } = deps;

  // P88a row 14: add/deinit/remove edit the superproject index + `.gitmodules`
  // (worktree) → the echo-armed `refreshAll('worktree')` covers status + arms the
  // watcher echo; the submodule list has no scope slice, so keep its refetch.
  async function refreshAfterChange() {
    await Promise.all([refreshAll('worktree'), refetchSubmodules()]);
  }

  /** P73 §5-§6: the shared shape of every row-scoped submodule op — busy pill,
   *  success/failure copy naming the target, and a refetch on both paths. */
  async function runRowOp<T>(op: {
    name: string;
    /** Present participle shown in the row badge while in flight. */
    busyLabel: string;
    /** Imperative verb for the failure prefix ("check out", "update", …). */
    verb: string;
    successText: string;
    call: () => Promise<T>;
    refresh: () => Promise<void>;
    /** P82: inspect the RESOLVED outcome (deinit/remove now return a typed
     *  outcome, not void). Return true to SUPPRESS the success toast — the op
     *  did not actually complete (e.g. a dirty refusal opened the force dialog
     *  instead of mutating). */
    onResolved?: (result: T) => boolean;
  }) {
    setMutating(true);
    setSubmoduleBusy({ name: op.name, label: op.busyLabel });
    try {
      const result = await op.call();
      if (op.onResolved?.(result) !== true) {
        pushToast('success', op.successText);
      }
    } catch (e) {
      // The backend sentence is appended verbatim (it carries the remedy); the
      // prefix only names the action + target. Keyed per row so retries replace.
      const text = `Couldn't ${op.verb} ${op.name}. ${errorMessage(e)}`;
      pushToast('error', text, `submodule:${op.name}`);
    } finally {
      // Refetch on failure too: a partially-applied op — or a refusal proving the
      // row is not what the UI thought — must not leave a stale badge.
      // A refetch failure must not escape as an unhandled rejection (nothing
      // above this hook awaits `runRowOp`): the op's own success/error toast has
      // already been shown, and the next watcher tick or manual refresh will
      // reconcile the list. Swallow it, but always clear the busy pill.
      try {
        await op.refresh();
      } catch {
        // ignored — see above
      } finally {
        setSubmoduleBusy(null);
        setMutating(false);
      }
    }
  }

  async function handleInitSubmodule(name: string) {
    await runRowOp({
      name,
      busyLabel: 'checking out…',
      verb: 'check out',
      successText: `Checked out ${name}`,
      call: () => ipc.updateSubmodule(repoId, name),
      refresh: refetchSubmodules,
    });
  }

  async function handleUpdateSubmodule(name: string) {
    await runRowOp({
      name,
      busyLabel: 'updating…',
      verb: 'update',
      successText: `Updated ${name}`,
      call: () => ipc.updateSubmodule(repoId, name),
      refresh: refetchSubmodules,
    });
  }

  async function handleSyncSubmodule(name: string) {
    await runRowOp({
      name,
      busyLabel: 'syncing…',
      verb: 'sync',
      successText: `Synced URL for ${name}`,
      call: () => ipc.syncSubmodule(repoId, name),
      refresh: refetchSubmodules,
    });
  }

  // P82: `force` defaults false (the safe confirm). On a `dirtyNeedsForce`
  // refusal we open the danger escalation dialog (no success toast, no mutation);
  // the container re-invokes with force=true after the user confirms.
  async function handleDeinitSubmodule(name: string, force = false) {
    await runRowOp({
      name,
      busyLabel: 'deinitializing…',
      verb: 'deinitialize',
      successText: `Deinitialized ${name}`,
      call: () => ipc.deinitSubmodule(repoId, name, force),
      refresh: refreshAfterChange,
      onResolved: (outcome) => {
        if (!force && outcome.kind === 'dirtyNeedsForce') {
          onSubmoduleDirtyRefused(name, 'deinit');
          return true;
        }
        return false;
      },
    });
  }

  async function handleRemoveSubmodule(name: string, force = false) {
    await runRowOp({
      name,
      busyLabel: 'removing…',
      verb: 'remove',
      successText: `Removed ${name}`,
      call: () => ipc.removeSubmodule(repoId, name, force),
      refresh: refreshAfterChange,
      onResolved: (outcome) => {
        if (!force && outcome.kind === 'dirtyNeedsForce') {
          onSubmoduleDirtyRefused(name, 'remove');
          return true;
        }
        return false;
      },
    });
  }

  // No busy pill for add: there is no row yet, and the section "+" is already
  // disabled while mutating (P73 §6.1).
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

  return {
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
    handleAddSubmodule,
    handleDeinitSubmodule,
    handleRemoveSubmodule,
  };
}
