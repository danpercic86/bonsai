// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
// P87b: each op delegates to `runMockActivity(category, inner)` so the git-activity
// stream (View C phase readout + View D log) is exercised in the browser harness.
import type { IpcApi } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { delay, requireRepo, throwAuthFailed, throwNetworkError } from '../repoState';
import { prePushRejectionFor } from '../hooksGate';
import { runMockActivity } from '../gitActivity';
import { throwIfGitMocksMissing } from './gitEnv';
import { tagSyncHandlers } from './tagSync';
import { repoChangedListeners, tagAutoSyncListeners } from '../events';
import type { AppError, FetchResult, PullResult, PushResult, RepoChangedPayload, TagAutoSyncEvent } from '../../types';

/** P85 A3 / P86a CI-2: mirror the backend's FIRE-AND-FORGET fetch tag auto-sync.
 *  Runs the mock auto-sync OFF the fetch response, then — only when it actually
 *  changed local tags (adopted or moved) — dispatches `repo-changed{reason:'tags'}`
 *  (refresh the tag list) + `tag-auto-sync` (count toast) through the mock event
 *  registries.
 *
 *  The emit is scheduled at a SMALL realistic offset (a second network round-trip
 *  right after the fetch), so it lands INSIDE the fetch round's echo-suppression
 *  window — exactly like the backend. That is what makes it exercise CI-1: the
 *  `reason:'tags'` event only refreshes because RepoWorkspace routes it through the
 *  echo-BYPASSING `external` origin. (P85 deferred this ~1500 ms to clear the
 *  window, which masked the bug.) Repo closed before it runs ⇒ noop. */
function scheduleMockTagAutoSync(repoId: string): void {
  window.setTimeout(() => {
    void tagSyncHandlers
      .autoSyncTags(repoId, null)
      .then((report) => {
        if (report.adopted.length === 0 && report.moved.length === 0) return;
        // Fidelity: reflect the adopted/moved tags into the branches snapshot so a
        // refresh (refetchBranches) actually surfaces them — the real backend wrote
        // them under refs/tags/*. Best-effort; skip if the repo closed meanwhile.
        try {
          const state = requireRepo(repoId);
          const names = new Set(state.branches.tags);
          for (const t of [...report.adopted, ...report.moved]) names.add(t);
          state.branches.tags = [...names].sort((a, b) =>
            a.toLowerCase().localeCompare(b.toLowerCase()),
          );
        } catch {
          /* repo closed — nothing to reflect */
        }
        const rc: RepoChangedPayload = { repoId, reason: 'tags' };
        for (const cb of repoChangedListeners) cb(rc);
        const ev: TagAutoSyncEvent = { repoId, report };
        for (const cb of tagAutoSyncListeners) cb(ev);
      })
      .catch(() => {
        /* repo closed before the deferred sync ran — nothing to surface */
      });
  }, 50);
}

export const remotesSyncHandlers = {
  async fetch(repoId: string): Promise<FetchResult> {
    return runMockActivity('fetch', () => fetchInner(repoId));
  },

  async pull(repoId: string): Promise<PullResult> {
    return runMockActivity('pull', () => pullInner(repoId));
  },

  async push(repoId: string, skipHooks?: boolean): Promise<PushResult> {
    return runMockActivity('push', () => pushInner(repoId, skipHooks));
  },

  // P37: force-push the current branch WITH A LEASE. `?remote=leasefail` drives
  // the refusal path (the remote moved since the last fetch); otherwise the
  // lease holds and the remote-tracking tip advances to the local tip.
  async forcePush(repoId: string, skipHooks?: boolean): Promise<PushResult> {
    return runMockActivity('forcePush', () => forcePushInner(repoId, skipHooks));
  },
} satisfies Partial<IpcApi>;

async function fetchInner(repoId: string): Promise<FetchResult> {
  await delay(400);
  // P70 `?git=missing`: an HTTPS remote whose credential helper cannot be
  // launched (see throwIfGitMocksMissing for why SSH is NOT modelled).
  throwIfGitMocksMissing();
  const state = requireRepo(repoId);
  if (state.remoteTrigger === 'authfail') throwAuthFailed();
  if (state.remoteTrigger === 'network') throwNetworkError();
  // P85 A3: auto tag-sync runs after EVERY fetch, but OFF the response path —
  // fire-and-forget, surfaced via the deferred repo-changed{tags} +
  // tag-auto-sync events (mirrors the backend). FetchResult.tagAutoSync is gone.
  scheduleMockTagAutoSync(repoId);
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
}

async function pullInner(repoId: string): Promise<PullResult> {
  await delay(400);
  // P70 `?git=missing`: an HTTPS remote whose credential helper cannot be
  // launched (see throwIfGitMocksMissing for why SSH is NOT modelled).
  throwIfGitMocksMissing();
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
    // Would not fast-forward: change NOTHING (fetch already "happened"). P60b:
    // carry the resolved upstream shorthand so the dialog can offer merge/rebase.
    const upstream = branch.upstream ?? `origin/${branch.name}`;
    return { kind: 'wouldNotFastForward', branch: branch.name, ahead, behind, upstream };
  }
  if (behind > 0) {
    const from = state.headOid;
    state.headOid = randomOid();
    branch.behind = 0;
    return { kind: 'fastForwarded', branch: branch.name, from, to: state.headOid };
  }
  return { kind: 'upToDate' };
}

async function pushInner(repoId: string, skipHooks?: boolean): Promise<PushResult> {
  await delay(400);
  // P70 `?git=missing`: an HTTPS remote whose credential helper cannot be
  // launched (see throwIfGitMocksMissing for why SSH is NOT modelled).
  throwIfGitMocksMissing();
  const state = requireRepo(repoId);
  // P59a-2: the pre-push hook runs BEFORE the push; a block aborts it.
  const prePush = prePushRejectionFor(state, skipHooks);
  if (prePush !== null) throw prePush;
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
}

async function forcePushInner(repoId: string, skipHooks?: boolean): Promise<PushResult> {
  await delay(400);
  const state = requireRepo(repoId);
  // P59a-2: the pre-push hook runs BEFORE the force-push; a block aborts it.
  const prePush = prePushRejectionFor(state, skipHooks);
  if (prePush !== null) throw prePush;
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
}
