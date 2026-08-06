import { ipc } from '../../ipc';
import { errorMessage, isAppError } from '../../utils/errors';
import { shortOid } from '../workspaceUtils';
import type { BaseActionDeps, Setter } from './types';

/** M6 + P37b: fetch / pull / push / force-push-with-lease. */
export function useRemoteOps(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    refetchBranches: () => Promise<void>;
    refetchGraph: () => Promise<void>;
    setRemoteOp: Setter<'fetch' | 'pull' | 'push' | null>;
    setPendingForcePush: Setter<boolean>;
  },
) {
  const {
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    refetchBranches,
    refetchGraph,
    setRemoteOp,
    setPendingForcePush,
  } = deps;

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
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
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
          pushToast(
            'warning',
            `Cannot fast-forward: '${res.branch}' has ${res.ahead} local commit(s) not on ` +
              'upstream. Bonsai v1 does not merge — push your commits or reconcile via the CLI.',
          );
          break;
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  // Shared push of the current branch (toast + refresh). Used by the Push
  // toolbar action and by Commit & Push. Never throws — push errors are toasted.
  async function pushCurrentBranch() {
    beginRemoteOp('push');
    try {
      const res = await ipc.push(repoId);
      if (res.kind === 'upToDate') {
        pushToast('success', 'Already up to date');
      } else {
        pushToast(
          'success',
          `Pushed ${res.branch} → ${res.remote}/${res.branch}` +
            (res.setUpstream ? ' (upstream set)' : ''),
        );
      }
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
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
      const res = await ipc.forcePush(repoId);
      if (res.kind === 'upToDate') {
        pushToast('info', 'Already up to date');
      } else {
        pushToast('success', `Force-pushed ${res.branch} → ${res.remote}/${res.branch}`);
      }
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
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
