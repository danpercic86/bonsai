/** T3.4 — merge.ts: fresh-merge outcomes, the T3.4 gap fix (a fresh conflicted
 *  merge now seeds coherent opState/conflict state, matching ?op=merge), the
 *  full conflict → resolve → commitMerge cycle, and abort/guard rejections. */
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { mergeHandlers } from './merge';
import { statusHandlers } from './status';
import { diffHandlers } from './diff';
import { requireRepo } from '../repoState';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

// NB: the label must avoid the reserved 'merge'/'rebase' path-substring seams.
async function openDefault(label = 'mg'): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath(label)));
  return repoId;
}

describe('mergeBranch outcomes', () => {
  it('clean merge commits a 2-parent node on top and bumps ahead', async () => {
    const repoId = await openDefault();
    const before = await run(diffHandlers.getGraph(repoId));
    const outcome = await run(mergeHandlers.mergeBranch(repoId, 'demo-clean'));
    expect(outcome.kind).toBe('merged');
    if (outcome.kind !== 'merged') return;
    const after = await run(diffHandlers.getGraph(repoId));
    expect(after.nodes.length).toBe(before.nodes.length + 1);
    const state = requireRepo(repoId);
    expect(state.headOid).toBe(outcome.oid);
    expect(state.commits[0].summary).toBe("Merge branch 'demo-clean'");
    expect(state.branches.local.find((b) => b.name === 'main')?.ahead).toBe(1);
  });

  it('"autostash" name reports stashed:true on a clean merge', async () => {
    const repoId = await openDefault();
    const outcome = await run(mergeHandlers.mergeBranch(repoId, 'demo-autostash'));
    expect(outcome).toMatchObject({ kind: 'merged', stashed: true });
  });

  it('"stash-conflict" name returns stashPopConflicts without touching opState', async () => {
    const repoId = await openDefault();
    const outcome = await run(mergeHandlers.mergeBranch(repoId, 'demo-stash-conflict'));
    expect(outcome).toMatchObject({ kind: 'stashPopConflicts', paths: ['src/app.ts'] });
    expect(await run(mergeHandlers.getOpState(repoId))).toEqual({ kind: 'none' });
  });

  it('rejects operationInProgress while an op is paused', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    const err = await runErr(mergeHandlers.mergeBranch(repoId, 'another'));
    expect(err.kind).toBe('operationInProgress');
  });

  it('unknown repoId rejects noRepo', async () => {
    const err = await runErr(mergeHandlers.mergeBranch('/never/opened', 'x'));
    expect(err.kind).toBe('noRepo');
  });
});

describe('T3.4 gap fix: fresh conflicted merge seeds coherent state', () => {
  it('mergeBranch("...conflict...") seeds opState + conflicts + conflicted status', async () => {
    const repoId = await openDefault();
    const outcome = await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    expect(outcome).toMatchObject({
      kind: 'conflicts',
      paths: ['README.md', 'src/auth.ts'],
      stashed: true,
    });
    // getOpState reflects the paused merge with the ACTUAL branch name.
    const op = await run(mergeHandlers.getOpState(repoId));
    expect(op).toMatchObject({ kind: 'merge', incoming: 'demo-conflict' });
    if (op.kind === 'merge') expect(op.message).toContain("Merge branch 'demo-conflict'");
    // listConflicts serves both entries, path-ascending like the backend.
    const conflicts = await run(mergeHandlers.listConflicts(repoId));
    expect(conflicts.map((c) => c.path)).toEqual(['README.md', 'src/auth.ts']);
    // getConflict works for both paths (this rejected before the fix).
    const auth = await run(mergeHandlers.getConflict(repoId, 'src/auth.ts'));
    expect(auth.kind).toBe('bothModified');
    expect(auth.text).toContain('<<<<<<<');
    const readme = await run(mergeHandlers.getConflict(repoId, 'README.md'));
    expect(readme.kind).toBe('deletedByThem');
    // Status mirrors the conflicts; README.md left the unstaged list.
    const status = await run(statusHandlers.getStatus(repoId));
    expect(status.conflicted.map((e) => e.path)).toEqual(['README.md', 'src/auth.ts']);
    expect(status.unstaged.some((e) => e.path === 'README.md')).toBe(false);
  });

  it('the full flow works without the URL seed: resolve both → commitMerge', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    // Gated while conflicts remain.
    const gated = await runErr(mergeHandlers.commitMerge(repoId, 'Merge!'));
    expect(gated.kind).toBe('unresolvedConflicts');
    // Resolve src/auth.ts via edited text, README.md by taking THEIRS (deletion).
    await run(mergeHandlers.resolveConflictText(repoId, 'src/auth.ts', 'merged body'));
    await run(mergeHandlers.resolveConflict(repoId, 'README.md', 'theirs'));
    expect(await run(mergeHandlers.listConflicts(repoId))).toEqual([]);
    // Taking theirs on deletedByThem stages the deletion.
    const status = await run(statusHandlers.getStatus(repoId));
    expect(status.staged.find((e) => e.path === 'README.md')?.status).toBe('deleted');
    const result = await run(mergeHandlers.commitMerge(repoId, "Merge branch 'demo-conflict'"));
    expect(result.branch).toBe('main');
    // Op cleared; merge node on top of the graph.
    expect(await run(mergeHandlers.getOpState(repoId))).toEqual({ kind: 'none' });
    const state = requireRepo(repoId);
    expect(state.commits[0]).toMatchObject({ oid: result.oid, mergeParentBase: 1 });
    expect(state.status.conflicted).toEqual([]);
  });

  it('abortMerge restores a clean none state', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    await run(mergeHandlers.abortMerge(repoId));
    expect(await run(mergeHandlers.getOpState(repoId))).toEqual({ kind: 'none' });
    expect(await run(mergeHandlers.listConflicts(repoId))).toEqual([]);
    const state = requireRepo(repoId);
    expect(state.conflictTexts.size).toBe(0);
    expect(state.status.conflicted).toEqual([]);
    // A new clean merge is possible again.
    const outcome = await run(mergeHandlers.mergeBranch(repoId, 'demo-clean'));
    expect(outcome.kind).toBe('merged');
  });
});

describe('conflict-command guards', () => {
  it('commitMerge / abortMerge without a merge → noOperationInProgress', async () => {
    const repoId = await openDefault();
    expect((await runErr(mergeHandlers.commitMerge(repoId, 'm'))).kind).toBe(
      'noOperationInProgress',
    );
    expect((await runErr(mergeHandlers.abortMerge(repoId))).kind).toBe('noOperationInProgress');
  });

  it('empty message rejects emptyMessage (after the conflicts guard)', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    await run(mergeHandlers.resolveConflictText(repoId, 'src/auth.ts', 'x'));
    await run(mergeHandlers.resolveConflict(repoId, 'README.md', 'ours'));
    expect((await runErr(mergeHandlers.commitMerge(repoId, '   '))).kind).toBe('emptyMessage');
  });

  it('getConflict / resolveConflict on a non-conflicted path reject cleanly', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    expect((await runErr(mergeHandlers.getConflict(repoId, 'nope.ts'))).kind).toBe('git');
    expect((await runErr(mergeHandlers.resolveConflict(repoId, 'nope.ts', 'ours'))).kind).toBe(
      'git',
    );
    expect(
      (await runErr(mergeHandlers.resolveConflictText(repoId, 'nope.ts', 'x'))).kind,
    ).toBe('git');
  });
});
