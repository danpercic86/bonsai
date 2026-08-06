import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { shortOid } from '../workspaceUtils';
import type { BisectOutcome } from '../../ipc';
import type { BaseActionDeps, Setter } from './types';

/** P39b: git bisect (start / mark / skip / reset). */
export function useBisectActions(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    setPendingBisectBad: Setter<string | null>;
  },
) {
  const { repoId, pushToast, setMutating, refreshAll, setPendingBisectBad } = deps;

  // Surface a start/mark/skip outcome as a toast; the banner (RepoOpState.bisect,
  // refetched by refreshAll) renders the live progress + controls.
  function reportBisectOutcome(res: BisectOutcome) {
    if (res.kind === 'found') {
      pushToast('success', `Bisect found first bad commit ${shortOid(res.firstBad)}`);
    } else if (res.kind === 'cannotDetermine') {
      pushToast(
        'info',
        `Bisect cannot determine the culprit: only skipped commits remain (${res.skipped.length}). Reset to finish.`,
      );
    } else {
      pushToast(
        'info',
        `Bisecting: ${res.revisionsRemaining} revision(s) left, ~${res.estimatedSteps} step(s)`,
      );
    }
  }

  // Two-click start: the commit menu recorded a BAD oid; `good` is an older
  // commit picked as known-good. Backend guards non-ancestor good / same oid /
  // dirty worktree → surface its error and keep the pending-bad so the user can
  // retry with a different good.
  async function handleStartBisect(bad: string, good: string) {
    setMutating(true);
    try {
      const res = await ipc.startBisect(repoId, bad, [good]);
      setPendingBisectBad(null);
      reportBisectOutcome(res);
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleBisectMark(isGood: boolean) {
    setMutating(true);
    try {
      const res = await ipc.bisectMark(repoId, isGood);
      reportBisectOutcome(res);
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleBisectSkip() {
    setMutating(true);
    try {
      const res = await ipc.bisectSkip(repoId);
      reportBisectOutcome(res);
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Reset = leave bisect + re-checkout the original branch/worktree. Confirm-
  // gated (routed through the shared Abort ConfirmDialog).
  async function handleBisectReset() {
    setMutating(true);
    try {
      await ipc.bisectReset(repoId);
      await refreshAll();
      pushToast('success', 'Bisect reset');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return { handleStartBisect, handleBisectMark, handleBisectSkip, handleBisectReset };
}
