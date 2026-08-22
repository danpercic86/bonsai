import { ipc } from '../../ipc';
import { errorMessage, isAppError } from '../../utils/errors';
import { reportRemoteOpError } from '../../ipc/gitNotFound';
import { isGitNotFound } from '../../ipc/errors';
import { shortOid } from '../workspaceUtils';
import { COMMIT_HOOK_CANCELED } from '../commitPushSignal';
import type { BaseActionDeps, Setter } from './types';

/** P60b: a fast-forward-only pull hit a diverged branch — drives NonFfPullDialog.
 *  `upstream` is the resolved shorthand the reconcile actions pass to
 *  mergeBranch/rebaseBranch. */
export interface NonFfPullInfo {
  branch: string;
  upstream: string;
  ahead: number;
  behind: number;
}

/** M6 + P37b: fetch / pull / push / force-push-with-lease. */
export function useRemoteOps(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    // P85 A1: no longer USED here (fetch/push/force-push route through refreshAll),
    // but kept in the deps shape because RepoWorkspace.tsx still passes them in its
    // object literal (an excess-property error otherwise). P86 removes both the
    // call-site args and these two props together.
    refetchBranches: () => Promise<void>;
    refetchGraph: () => Promise<void>;
    setRemoteOp: Setter<'fetch' | 'pull' | 'push' | null>;
    setPendingForcePush: Setter<boolean>;
    /** P60b: open the non-FF reconcile dialog (Merge / Rebase / Cancel). */
    setPendingNonFfPull: Setter<NonFfPullInfo | null>;
    /** P59a-2: wrap a push attempt so a `pre-push` `hookRejected` opens the
     *  HookOutputDialog (+ "Push anyway" retry with skipHooks:true) instead of
     *  surfacing raw. Shared with the commit paths (one dialog + one retry). */
    runWithHookGate: (
      attempt: (skipHooks: boolean) => Promise<void>,
      skipHooks: boolean,
    ) => Promise<void>;
  },
) {
  const {
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    setRemoteOp,
    setPendingForcePush,
    setPendingNonFfPull,
    runWithHookGate,
  } = deps;

  // P85 A1: fetch/push/force-push route their post-op refresh through the
  // ECHO-ARMED refreshAll (like handlePull already does), NOT raw
  // refetchGraph/refetchBranches — so the op's own `.git/refs/**` watcher echo is
  // dropped and only ONE refresh round runs. `refreshAll` never throws (P81), so
  // the push/force-push hook-gate `attempt` bodies keep the same success
  // semantics. A3: fetch's tag counts now arrive via the async `tag-auto-sync`
  // event (fired off the response) rather than in the fetch result.

  function beginRemoteOp(op: 'fetch' | 'pull' | 'push') {
    setMutating(true);
    setRemoteOp(op);
  }

  function endRemoteOp() {
    setMutating(false);
    setRemoteOp(null);
  }

  async function handleFetch() {
    beginRemoteOp('fetch');
    try {
      const res = await ipc.fetch(repoId);
      const n = res.remotes.length;
      const k = res.remotes.reduce((sum, r) => sum + r.updatedRefs, 0);
      pushToast(
        'success',
        `Fetched ${n} remote${n === 1 ? '' : 's'}` +
          (k > 0 ? ` — ${k} ref${k === 1 ? '' : 's'} updated` : ''),
      );
      await refreshAll();
    } catch (e) {
      // P70 (UI §10.3): a user-PRESSED remote op still gets exactly one toast —
      // coalesced by key, so three presses never stack three sticky errors.
      reportRemoteOpError('Fetch', e, pushToast);
    } finally {
      endRemoteOp();
    }
  }

  async function handlePull() {
    beginRemoteOp('pull');
    try {
      const res = await ipc.pull(repoId);
      switch (res.kind) {
        case 'upToDate':
          pushToast('success', 'Already up to date');
          break;
        case 'fastForwarded':
          pushToast('success', `Fast-forwarded ${res.branch} to ${shortOid(res.to)}`);
          break;
        case 'wouldNotFastForward':
          // P60b: the fetch DID land but the branch diverged — offer Merge /
          // Rebase (routed through the existing commands) via the confirm dialog.
          setPendingNonFfPull({
            branch: res.branch,
            upstream: res.upstream,
            ahead: res.ahead,
            behind: res.behind,
          });
          break;
      }
      await refreshAll();
    } catch (e) {
      reportRemoteOpError('Pull', e, pushToast);
    } finally {
      endRemoteOp();
    }
  }

  // Shared push of the current branch (toast + refresh). Used by the Push
  // toolbar action and by Commit & Push. Never throws — push errors are toasted.
  // P59a-2: routed through the hook gate so a blocking `pre-push` opens the
  // HookOutputDialog ("Push anyway" retries with skipHooks:true); the attempt
  // performs the push AND its success side-effects, so the retry re-runs both.
  async function pushCurrentBranch() {
    beginRemoteOp('push');
    try {
      await runWithHookGate(async (skipHooks) => {
        const res = await ipc.push(repoId, skipHooks);
        if (res.kind === 'upToDate') {
          pushToast('success', 'Already up to date');
        } else {
          pushToast(
            'success',
            `Pushed ${res.branch} → ${res.remote}/${res.branch}` +
              (res.setUpstream ? ' (upstream set)' : ''),
          );
        }
        await refreshAll();
      }, false);
    } catch (e) {
      // Dialog dismissed (pre-push not skipped): nothing pushed, no error banner.
      if (e !== COMMIT_HOOK_CANCELED) reportRemoteOpError('Push', e, pushToast);
    } finally {
      endRemoteOp();
    }
  }

  async function handlePush() {
    await pushCurrentBranch();
  }

  // P37b: force-push with lease. Opens a danger confirm first; on confirm,
  // force_push refuses (pushRejected) if the remote moved since our last fetch.
  function handleForcePush() {
    setPendingForcePush(true);
  }

  async function doForcePush() {
    setPendingForcePush(false);
    beginRemoteOp('push');
    try {
      // P59a-2: same hook gate as the normal push — a blocking `pre-push` opens
      // the dialog with a "Push anyway" retry (skipHooks:true).
      await runWithHookGate(async (skipHooks) => {
        const res = await ipc.forcePush(repoId, skipHooks);
        if (res.kind === 'upToDate') {
          pushToast('info', 'Already up to date');
        } else {
          pushToast('success', `Force-pushed ${res.branch} → ${res.remote}/${res.branch}`);
        }
        await refreshAll();
      }, false);
    } catch (e) {
      // Dialog dismissed (pre-push not skipped): nothing pushed, no error banner.
      if (e === COMMIT_HOOK_CANCELED) return;
      if (isGitNotFound(e)) {
        reportRemoteOpError('Push', e, pushToast);
        return;
      }
      // Any pushRejected from a force-push resolves the same way: fetch first.
      const hint = isAppError(e) && e.kind === 'pushRejected' ? ' — fetch and retry' : '';
      pushToast('error', errorMessage(e) + hint);
    } finally {
      endRemoteOp();
    }
  }

  return {
    handleFetch,
    handlePull,
    pushCurrentBranch,
    handlePush,
    handleForcePush,
    doForcePush,
  };
}
