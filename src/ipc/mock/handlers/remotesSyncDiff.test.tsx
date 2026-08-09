/** T3.4 — remotesSync.ts (fetch/pull/push/forcePush state machine) and diff.ts
 *  (graph + diff routing, ref-tip fallback, image-diff seams). The ?remote= /
 *  ?hooks= flags are read at openRepo time, so replaceState before opening. */
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { remotesSyncHandlers } from './remotesSync';
import { statusHandlers } from './status';
import { branchHandlers } from './branches';
import { diffHandlers } from './diff';
import { requireRepo } from '../repoState';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());
afterEach(() => window.history.replaceState({}, '', '/'));

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('rs')));
  return repoId;
}

describe('fetch → pull fast-forward story', () => {
  it('first fetch discovers 1 upstream commit; pull fast-forwards and clears it', async () => {
    const repoId = await openDefault();
    const first = await run(remotesSyncHandlers.fetch(repoId));
    expect(first.remotes).toEqual([{ remote: 'origin', receivedObjects: 12, updatedRefs: 1 }]);
    let main = requireRepo(repoId).branches.local.find((b) => b.name === 'main');
    expect(main?.behind).toBe(1);
    // Second fetch is a no-op.
    const second = await run(remotesSyncHandlers.fetch(repoId));
    expect(second.remotes[0].updatedRefs).toBe(0);
    const pull = await run(remotesSyncHandlers.pull(repoId));
    expect(pull.kind).toBe('fastForwarded');
    if (pull.kind === 'fastForwarded') {
      expect(pull.branch).toBe('main');
      expect(requireRepo(repoId).headOid).toBe(pull.to);
    }
    main = requireRepo(repoId).branches.local.find((b) => b.name === 'main');
    expect(main?.behind).toBe(0);
    expect(await run(remotesSyncHandlers.pull(repoId))).toEqual({ kind: 'upToDate' });
  });

  it('a diverged branch pulls to wouldNotFastForward, changing NOTHING', async () => {
    const repoId = await openDefault();
    const state = requireRepo(repoId);
    state.status.staged = [];
    state.status.unstaged = [];
    state.status.untracked = [];
    await run(branchHandlers.checkoutBranch(repoId, 'feature/sidebar')); // ahead 2 / behind 1
    const headBefore = state.headOid;
    const pull = await run(remotesSyncHandlers.pull(repoId));
    expect(pull).toEqual({
      kind: 'wouldNotFastForward',
      branch: 'feature/sidebar',
      ahead: 2,
      behind: 1,
      upstream: 'origin/feature/sidebar',
    });
    expect(state.headOid).toBe(headBefore);
  });

  it('a branch without an upstream pulls to noUpstream', async () => {
    const repoId = await openDefault();
    await run(branchHandlers.checkoutBranch(repoId, 'exp')); // upstream null
    expect((await runErr(remotesSyncHandlers.pull(repoId))).kind).toBe('noUpstream');
  });
});

describe('push', () => {
  it('commit → ahead 1 → push clears it; a second push is upToDate', async () => {
    const repoId = await openDefault();
    await run(statusHandlers.commit(repoId, 'to push'));
    const push = await run(remotesSyncHandlers.push(repoId));
    expect(push).toEqual({ kind: 'pushed', remote: 'origin', branch: 'main', setUpstream: false });
    expect(requireRepo(repoId).branches.local.find((b) => b.name === 'main')?.ahead).toBe(0);
    expect(await run(remotesSyncHandlers.push(repoId))).toEqual({
      kind: 'upToDate',
      remote: 'origin',
      branch: 'main',
    });
  });

  it('first push of a new branch sets the upstream and creates the remote ref', async () => {
    const repoId = await openDefault();
    await run(branchHandlers.checkoutBranch(repoId, 'exp'));
    const push = await run(remotesSyncHandlers.push(repoId));
    expect(push).toEqual({ kind: 'pushed', remote: 'origin', branch: 'exp', setUpstream: true });
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.local.find((b) => b.name === 'exp')).toMatchObject({
      upstream: 'origin/exp',
      ahead: 0,
      behind: 0,
    });
    expect(snap.remote.some((r) => r.name === 'origin/exp')).toBe(true);
  });

  it('forcePush without an upstream rejects noUpstream; with one, syncs the remote tip', async () => {
    const repoId = await openDefault();
    const state = requireRepo(repoId);
    state.status.staged = [];
    state.status.unstaged = [];
    state.status.untracked = [];
    await run(branchHandlers.checkoutBranch(repoId, 'fix/watcher-debounce'));
    expect((await runErr(remotesSyncHandlers.forcePush(repoId))).kind).toBe('noUpstream');
    await run(branchHandlers.checkoutBranch(repoId, 'feature/sidebar'));
    const push = await run(remotesSyncHandlers.forcePush(repoId));
    expect(push.kind).toBe('pushed');
    const snap = await run(branchHandlers.listBranches(repoId));
    const local = snap.local.find((b) => b.name === 'feature/sidebar');
    expect(local).toMatchObject({ ahead: 0, behind: 0 });
    expect(snap.remote.find((r) => r.name === 'origin/feature/sidebar')?.tip).toBe(local?.tip);
  });
});

describe('?remote= failure triggers (seeded at openRepo)', () => {
  async function openWithRemote(mode: string): Promise<string> {
    window.history.replaceState({}, '', `/?remote=${mode}`);
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('rt')));
    return repoId;
  }

  it('authfail: fetch/pull/push all reject authFailed', async () => {
    const repoId = await openWithRemote('authfail');
    expect((await runErr(remotesSyncHandlers.fetch(repoId))).kind).toBe('authFailed');
    expect((await runErr(remotesSyncHandlers.pull(repoId))).kind).toBe('authFailed');
    expect((await runErr(remotesSyncHandlers.push(repoId))).kind).toBe('authFailed');
  });

  it('network / rejected / conflict / leasefail map to their AppError kinds', async () => {
    const net = await openWithRemote('network');
    expect((await runErr(remotesSyncHandlers.fetch(net))).kind).toBe('networkError');
    const rej = await openWithRemote('rejected');
    expect((await runErr(remotesSyncHandlers.push(rej))).kind).toBe('pushRejected');
    const con = await openWithRemote('conflict');
    expect((await runErr(remotesSyncHandlers.pull(con))).kind).toBe('checkoutConflict');
    const lease = await openWithRemote('leasefail');
    const err = await runErr(remotesSyncHandlers.forcePush(lease));
    expect(err.kind).toBe('pushRejected');
    expect(err.message).toContain('force-push refused');
  });
});

describe('diff routing', () => {
  it('getGraph reflects mock commits + stash offshoots; headIndex row carries HEAD', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    expect(layout.nodes.length).toBeGreaterThan(0);
    const result = await run(statusHandlers.commit(repoId, 'top row'));
    const after = await run(diffHandlers.getGraph(repoId));
    expect(after.nodes.length).toBe(layout.nodes.length + 1);
    // The new commit is the topmost COMMIT row (stash offshoots are prepended
    // above it by withStashNodes and carry author '').
    const firstCommitRow = after.nodes.find((n) => n.author !== '');
    expect(firstCommitRow).toMatchObject({ id: result.oid, summary: 'top row' });
  });

  it('getCommitDiff routes by row; unknown oid rejects; ref tips fall back', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    // Skip the injected stash offshoots (author '') — they are not commit rows.
    const commitNode = layout.nodes.find((n) => n.author !== '');
    expect(commitNode).toBeDefined();
    if (!commitNode) return;
    const diff = await run(diffHandlers.getCommitDiff(repoId, commitNode.id));
    expect(diff.details.oid).toBe(commitNode.id);
    expect(diff.files.length).toBeGreaterThan(0);
    // A branch tip that is NOT a walkable row still resolves (isRefTip fallback).
    const tipDiff = await run(diffHandlers.getCommitDiff(repoId, 'a'.repeat(40)));
    expect(tipDiff.details.oid).toBe('a'.repeat(40));
    expect((await runErr(diffHandlers.getCommitDiff(repoId, 'f0'.repeat(20)))).kind).toBe('git');
  });

  it('compareWithHead: HEAD-vs-itself up front; unknown oid rejects', async () => {
    const repoId = await openDefault();
    const state = requireRepo(repoId);
    const self = await run(diffHandlers.compareWithHead(repoId, state.headOid));
    expect(self.files).toHaveLength(0); // "No differences"
    expect((await runErr(diffHandlers.compareWithHead(repoId, 'f0'.repeat(20)))).kind).toBe(
      'git',
    );
  });

  it('getWorkdirFileDiff serves the live model for src/main.rs (staged vs unstaged)', async () => {
    const repoId = await openDefault();
    const unstaged = await run(
      diffHandlers.getWorkdirFileDiff(repoId, 'src/main.rs', null, false, false, false),
    );
    expect(unstaged.hunks.length).toBeGreaterThan(0); // seeded workdir edits
    const staged = await run(
      diffHandlers.getWorkdirFileDiff(repoId, 'src/main.rs', null, true, false, false),
    );
    expect(staged.hunks.length).toBeGreaterThan(0); // seeded staged insert (index ≠ head)
    // Distinct diffs: the staged and unstaged sides differ.
    expect(staged.hunks).not.toEqual(unstaged.hunks);
    // fullContext collapses to one whole-file hunk.
    const full = await run(
      diffHandlers.getWorkdirFileDiff(repoId, 'src/main.rs', null, false, true, false),
    );
    expect(full.hunks).toHaveLength(1);
  });

  it('getImageDiff seams: added/deleted/huge shape the sides', async () => {
    const repoId = await openDefault();
    const req = (path: string) =>
      ({ kind: 'workdir', path, origPath: null, staged: false }) as const;
    const norm = await run(diffHandlers.getImageDiff(repoId, req('assets/logo.png')));
    expect(norm.old).not.toBeNull();
    expect(norm.new).not.toBeNull();
    expect(norm.old?.mime).toBe('image/png');
    const added = await run(diffHandlers.getImageDiff(repoId, req('assets/added.png')));
    expect(added.old).toBeNull();
    expect(added.new).not.toBeNull();
    const deleted = await run(diffHandlers.getImageDiff(repoId, req('assets/deleted.png')));
    expect(deleted.new).toBeNull();
    const huge = await run(diffHandlers.getImageDiff(repoId, req('assets/huge.png')));
    expect(huge.old).toBeNull();
    expect(huge.oldTooLarge).toBe(true);
  });
});
