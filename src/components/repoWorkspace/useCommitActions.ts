import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { shortOid } from '../workspaceUtils';
import { COMMIT_PUSH_CANCELED } from '../commitPushSignal';
import { nextFileAfter, type WorkdirChange } from '../../utils/nextFile';
import type { BranchesSnapshot, FileDiff, HeadInfo, ResetMode, StatusSnapshot } from '../../ipc';
import type { DiffSlot } from '../StatusPanel';
import type { BaseActionDeps, PendingDiscardForce, Setter } from './types';

type CommitPushResolver = { current: { resolve: () => void; reject: (e: unknown) => void } | null };

/** Stage/unstage/commit/amend/reset/discard + Commit & Push (M3/M6/P20). */
export function useCommitActions(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    refetchStatus: () => Promise<void>;
    reportStatusError: (message: string) => void;
    fetchDiffSlot: (key: string, fetcher: () => Promise<FileDiff>) => Promise<void>;
    pushCurrentBranch: () => Promise<void>;
    status: StatusSnapshot | null;
    statusRef: { current: StatusSnapshot | null };
    diffSlotRef: { current: DiffSlot | null };
    diffViewModeRef: { current: 'diff' | 'file' | 'split' };
    head: HeadInfo | null;
    headBranch: BranchesSnapshot['local'][number] | null;
    setAmend: Setter<boolean>;
    setAmendMessage: Setter<string | null>;
    pendingCommitPush: string | null;
    setPendingCommitPush: Setter<string | null>;
    commitPushResolver: CommitPushResolver;
    setPendingDiscardForce: Setter<PendingDiscardForce | null>;
  },
) {
  const {
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    refetchStatus,
    reportStatusError,
    fetchDiffSlot,
    pushCurrentBranch,
    status,
    statusRef,
    diffSlotRef,
    diffViewModeRef,
    head,
    headBranch,
    setAmend,
    setAmendMessage,
    pendingCommitPush,
    setPendingCommitPush,
    commitPushResolver,
    setPendingDiscardForce,
  } = deps;

  async function handleStage(paths: string[]) {
    setMutating(true);
    // P46 WS3: when the file open in the diff overlay is the one being staged,
    // auto-advance to the NEXT changed file (visible [unstaged, untracked]
    // order). Compute the target from the PRE-stage snapshot; refetchStatus
    // collapses the staged slot, then we open the target below.
    let nextTarget: WorkdirChange | null = null;
    const slot = diffSlotRef.current;
    if (
      slot !== null &&
      status !== null &&
      (slot.key.startsWith('unstaged:') || slot.key.startsWith('untracked:'))
    ) {
      const openPath = slot.key.slice(slot.key.indexOf(':') + 1);
      if (paths.includes(openPath)) {
        const changes: WorkdirChange[] = [
          ...status.unstaged.map((e) => ({
            section: 'unstaged' as const,
            path: e.path,
            origPath: e.origPath,
          })),
          ...status.untracked.map((e) => ({
            section: 'untracked' as const,
            path: e.path,
            origPath: e.origPath,
          })),
        ];
        nextTarget = nextFileAfter(changes, openPath, paths);
      }
    }
    try {
      await ipc.stage(repoId, paths);
      await refetchStatus();
      if (nextTarget !== null) {
        const target = nextTarget;
        const fresh = statusRef.current;
        const stillThere =
          fresh?.[target.section].some((e) => e.path === target.path) ?? false;
        if (stillThere) {
          void fetchDiffSlot(`${target.section}:${target.path}`, () =>
            ipc.getWorkdirFileDiff(
              repoId,
              target.path,
              target.origPath,
              false,
              diffViewModeRef.current === 'file',
            ),
          );
        }
      }
    } catch (e) {
      reportStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleUnstage(paths: string[]) {
    setMutating(true);
    try {
      await ipc.unstage(repoId, paths);
      await refetchStatus();
    } catch (e) {
      reportStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleCommit(message: string) {
    setMutating(true);
    try {
      await ipc.commit(repoId, message);
      await refreshAll();
    } finally {
      setMutating(false);
    }
  }

  // Commit & Push (normal commit box, primary button). If the current branch has
  // no upstream, gate on a ConfirmDialog first; otherwise commit + push directly.
  async function handleCommitAndPush(message: string): Promise<void> {
    if (headBranch !== null && headBranch.upstream === null) {
      // Park the message + defer resolution until the dialog is answered.
      return new Promise<void>((resolve, reject) => {
        commitPushResolver.current = { resolve, reject };
        setPendingCommitPush(message);
      });
    }
    await doCommitAndPush(message);
  }

  // The actual commit-then-push. Commit errors rethrow (surfaced by CommitBox);
  // push errors are toasted and the commit is kept.
  async function doCommitAndPush(message: string): Promise<void> {
    setMutating(true);
    try {
      await ipc.commit(repoId, message);
    } finally {
      setMutating(false);
    }
    await refreshAll();
    await pushCurrentBranch();
  }

  function handleConfirmCommitPush() {
    const message = pendingCommitPush;
    const resolver = commitPushResolver.current;
    commitPushResolver.current = null;
    setPendingCommitPush(null);
    if (message === null) {
      resolver?.resolve();
      return;
    }
    void (async () => {
      try {
        await doCommitAndPush(message);
        resolver?.resolve();
      } catch (e) {
        resolver?.reject(e);
      }
    })();
  }

  function handleCancelCommitPush() {
    const resolver = commitPushResolver.current;
    commitPushResolver.current = null;
    setPendingCommitPush(null);
    // No commit performed. Reject with the cancellation sentinel so CommitBox
    // leaves the typed message intact and shows no error banner.
    resolver?.reject(COMMIT_PUSH_CANCELED);
  }

  // P20 §2: amend the current tip. Rethrows so CommitBox surfaces
  // configMissing/emptyMessage in its own error banner.
  async function handleCommitAmend(message: string) {
    setMutating(true);
    try {
      await ipc.commitAmend(repoId, message);
      setAmend(false);
      setAmendMessage(null);
      await refreshAll();
      pushToast('success', 'Amended last commit');
    } finally {
      setMutating(false);
    }
  }

  // P20 §2.3: toggle amend on/off. Toggling ON fetches HEAD's full message once
  // (reusing getCommitDiff().details.message — no dedicated backend getter) so
  // the box remounts prefilled. Toggling OFF drops back to the normal commit box.
  async function handleToggleAmend(next: boolean) {
    if (!next) {
      setAmend(false);
      setAmendMessage(null);
      return;
    }
    if (head === null || head.unborn) return;
    try {
      const diff = await ipc.getCommitDiff(repoId, head.oid);
      setAmendMessage(diff.details.message);
      setAmend(true);
    } catch (e) {
      pushToast('error', `Could not load the last commit message: ${errorMessage(e)}`);
    }
  }

  // P20 §3: reset the current branch (called after the shared ConfirmDialog).
  async function handleResetBranch(oid: string, mode: ResetMode) {
    setMutating(true);
    try {
      await ipc.resetBranch(repoId, oid, mode);
      await refreshAll();
      const branchLabel = headBranch?.name ?? 'HEAD';
      pushToast('success', `Reset ${branchLabel} to ${shortOid(oid)} (${mode})`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P20 §4: discard unstaged edits to tracked files (called after ConfirmDialog).
  async function handleDiscard(paths: string[]) {
    setMutating(true);
    try {
      await ipc.discardPaths(repoId, paths);
      await refreshAll();
      pushToast('success', `Discarded changes to ${paths.length} file(s)`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Bulk "Discard all": partition the requested paths into modified (tracked)
  // vs created (untracked) using the current status snapshot so the confirm
  // dialog can warn about the permanent deletion of new files, then arm it.
  function requestDiscardForce(paths: string[]) {
    if (paths.length === 0) return;
    const untracked = new Set((status?.untracked ?? []).map((e) => e.path));
    let modified = 0;
    const created: string[] = [];
    for (const p of paths) {
      if (untracked.has(p)) created.push(p);
      else modified += 1;
    }
    setPendingDiscardForce({ paths, modified, created: created.length, untracked: created });
  }

  // Force-discard: reverts modified tracked files to the index AND deletes
  // new/untracked files (called after the ConfirmDialog).
  async function handleDiscardForce(paths: string[]) {
    setMutating(true);
    try {
      await ipc.discardPathsForce(repoId, paths);
      await refreshAll();
      pushToast('success', `Discarded ${paths.length} file(s)`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P15a: ask the backend for a proposed commit message from the staged diff.
  async function handleGenerateCommitMessage(): Promise<string> {
    const proposal = await ipc.generateCommitMessage(repoId);
    return proposal.message;
  }

  return {
    handleStage,
    handleUnstage,
    handleCommit,
    handleCommitAndPush,
    handleConfirmCommitPush,
    handleCancelCommitPush,
    handleCommitAmend,
    handleToggleAmend,
    handleResetBranch,
    handleDiscard,
    requestDiscardForce,
    handleDiscardForce,
    handleGenerateCommitMessage,
  };
}
