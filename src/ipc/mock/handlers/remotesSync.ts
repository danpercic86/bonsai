// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { delay, requireRepo, throwAuthFailed, throwNetworkError } from '../repoState';
import type { AppError, FetchResult, PullResult, PushResult } from '../../types';

export const remotesSyncHandlers = {
  async fetch(repoId: string): Promise<FetchResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (!state.fetched) {
      state.fetched = true;
      // The fetch "discovers" one new upstream commit on main.
      const main = state.branches.local.find((b) => b.name === 'main');
      if (main !== undefined && main.upstream !== null) {
        main.behind = 1;
      }
      return { remotes: [{ remote: 'origin', receivedObjects: 12, updatedRefs: 1 }] };
    }
    return { remotes: [{ remote: 'origin', receivedObjects: 0, updatedRefs: 0 }] };
  },

  async pull(repoId: string): Promise<PullResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'conflict') {
      const err: AppError = {
        kind: 'checkoutConflict',
        message:
          'cannot pull: local changes would be overwritten by the update. ' +
          'Commit or discard them first.',
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === state.headBranch);
    if (branch === undefined) {
      // Detached fixture etc. — button is disabled anyway; stay inert.
      return { kind: 'upToDate' };
    }
    if (branch.upstream === null) {
      const err: AppError = {
        kind: 'noUpstream',
        message: `cannot pull: branch '${branch.name}' has no upstream configured`,
      };
      throw err;
    }
    const ahead = branch.ahead ?? 0;
    const behind = branch.behind ?? 0;
    if (ahead > 0 && behind > 0) {
      // Would not fast-forward: change NOTHING (fetch already "happened").
      return { kind: 'wouldNotFastForward', branch: branch.name, ahead, behind };
    }
    if (behind > 0) {
      const from = state.headOid;
      state.headOid = randomOid();
      branch.behind = 0;
      return { kind: 'fastForwarded', branch: branch.name, from, to: state.headOid };
    }
    return { kind: 'upToDate' };
  },

  async push(repoId: string): Promise<PushResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'rejected') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          'push rejected: the remote contains commits you do not have. ' +
          'Fetch/pull first — Bonsai v1 never force-pushes.',
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === state.headBranch);
    if (branch === undefined) {
      return { kind: 'upToDate', remote: 'origin', branch: state.headBranch };
    }
    if (branch.upstream === null) {
      // First push of a new branch: push to origin/<name> AND set upstream.
      branch.upstream = `origin/${branch.name}`;
      branch.ahead = 0;
      branch.behind = 0;
      if (!state.branches.remote.some((r) => r.name === branch.upstream)) {
        state.branches.remote.push({ name: `origin/${branch.name}`, tip: branch.tip });
        state.branches.remote.sort((a, b) =>
          a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
        );
      }
      return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: true };
    }
    if ((branch.ahead ?? 0) > 0) {
      branch.ahead = 0;
      return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: false };
    }
    return { kind: 'upToDate', remote: 'origin', branch: branch.name };
  },

  // P37: force-push the current branch WITH A LEASE. `?remote=leasefail` drives
  // the refusal path (the remote moved since the last fetch); otherwise the
  // lease holds and the remote-tracking tip advances to the local tip.
  async forcePush(repoId: string): Promise<PushResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'leasefail') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          "force-push refused: 'origin/" +
          state.headBranch +
          "' has moved on the remote since you last fetched — someone may have pushed. " +
          'Fetch and review before force-pushing again.',
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === state.headBranch);
    if (branch === undefined || branch.upstream === null) {
      const err: AppError = { kind: 'noUpstream', message: 'cannot force-push: no upstream' };
      throw err;
    }
    // Lease held: force-update the remote-tracking tip to the local tip.
    branch.ahead = 0;
    branch.behind = 0;
    const rt = state.branches.remote.find((r) => r.name === branch.upstream);
    if (rt !== undefined) rt.tip = branch.tip;
    return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: false };
  },

  // Stateful op-state mock (P3c contract §7.2). A repo seeded with a merge/rebase
  // (via `?op=` or a path substring) starts paused; mergeBranch/rebaseBranch are
  // the clean-op demo paths.
} satisfies Partial<IpcApi>;
