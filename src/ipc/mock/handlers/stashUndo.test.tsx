/** T3.4 — stash.ts (list/create/apply/pop/drop + commitAmend) and undo.ts
 *  (describeLastUndo plans incl. the ?undo= seam, read live per call). */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { stashHandlers } from './stash';
import { undoHandlers } from './undo';
import { requireRepo } from '../repoState';

beforeAll(() => vi.useFakeTimers());
afterAll(() => {
  vi.useRealTimers();
  window.history.replaceState({}, '', '/');
});

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('st')));
  return repoId;
}

describe('stash stack', () => {
  it('lists the 3 seeded entries, index-ascending from 0', async () => {
    const repoId = await openDefault();
    const list = await run(stashHandlers.listStashes(repoId));
    expect(list.map((e) => e.index)).toEqual([0, 1, 2]);
    expect(list[2].message).toContain('reserved');
  });

  it('createStash pushes stash@{0}, re-indexes, and clears per scope', async () => {
    const repoId = await openDefault();
    const result = await run(stashHandlers.createStash(repoId, null, 'all'));
    expect(result).toEqual({ created: true });
    const list = await run(stashHandlers.listStashes(repoId));
    expect(list).toHaveLength(4);
    expect(list.map((e) => e.index)).toEqual([0, 1, 2, 3]);
    expect(list[0].message).toContain('WIP on main');
    const state = requireRepo(repoId);
    expect(state.status.staged).toEqual([]);
    expect(state.status.unstaged).toEqual([]);
    expect(state.status.untracked.length).toBeGreaterThan(0); // 'all' keeps untracked
  });

  it('scope "staged" clears only staged; "allWithUntracked" clears everything', async () => {
    const repoId = await openDefault();
    await run(stashHandlers.createStash(repoId, null, 'staged'));
    let state = requireRepo(repoId);
    expect(state.status.staged).toEqual([]);
    expect(state.status.unstaged.length).toBeGreaterThan(0);
    await run(stashHandlers.createStash(repoId, null, 'allWithUntracked'));
    state = requireRepo(repoId);
    expect(state.status.unstaged).toEqual([]);
    expect(state.status.untracked).toEqual([]);
  });

  it('created:false when the scope has nothing to stash (stack untouched)', async () => {
    const repoId = await openDefault();
    requireRepo(repoId).status.staged = [];
    expect(await run(stashHandlers.createStash(repoId, null, 'staged'))).toEqual({
      created: false,
    });
    expect(await run(stashHandlers.listStashes(repoId))).toHaveLength(3);
  });

  it('applyStash leaves the stack; popStash removes + re-indexes', async () => {
    const repoId = await openDefault();
    expect(await run(stashHandlers.applyStash(repoId, 0, false))).toEqual({ kind: 'applied' });
    expect(await run(stashHandlers.listStashes(repoId))).toHaveLength(3);
    expect(await run(stashHandlers.popStash(repoId, 0, false))).toEqual({ kind: 'applied' });
    const list = await run(stashHandlers.listStashes(repoId));
    expect(list).toHaveLength(2);
    expect(list.map((e) => e.index)).toEqual([0, 1]);
  });

  it('reserved-path flow: blocked first, applied-skipping on retry, stash KEPT', async () => {
    const repoId = await openDefault();
    const blocked = await run(stashHandlers.popStash(repoId, 2, false));
    expect(blocked.kind).toBe('reservedPaths');
    const retried = await run(stashHandlers.popStash(repoId, 2, true));
    expect(retried.kind).toBe('appliedSkippingReserved');
    expect(await run(stashHandlers.listStashes(repoId))).toHaveLength(3); // lossless
  });

  it('dropStash removes the entry and re-indexes survivors', async () => {
    const repoId = await openDefault();
    await run(stashHandlers.dropStash(repoId, 1));
    const list = await run(stashHandlers.listStashes(repoId));
    expect(list).toHaveLength(2);
    expect(list.map((e) => e.index)).toEqual([0, 1]);
  });
});

describe('commitAmend', () => {
  it('rewrites the tip in place: new oid, same commit count, staged folded in', async () => {
    const repoId = await openDefault();
    const state = requireRepo(repoId);
    state.commits.unshift({ oid: 'ab'.repeat(20), summary: 'original' });
    const before = state.commits.length;
    const result = await run(stashHandlers.commitAmend(repoId, 'amended: better message'));
    expect(result.summary).toBe('amended: better message');
    expect(state.commits).toHaveLength(before);
    expect(state.commits[0]).toMatchObject({ oid: result.oid, summary: result.summary });
    expect(state.status.staged).toEqual([]);
  });

  it('message-only amend allowed (no nothing-to-commit guard); empty message rejects', async () => {
    const repoId = await openDefault();
    requireRepo(repoId).status.staged = [];
    const result = await run(stashHandlers.commitAmend(repoId, 'reworded'));
    expect(result.oid).toHaveLength(40);
    expect((await runErr(stashHandlers.commitAmend(repoId, '  '))).kind).toBe('emptyMessage');
  });
});

describe('describeLastUndo (?undo= seam read live per call)', () => {
  function setUndo(mode: string | null): void {
    window.history.replaceState({}, '', mode === null ? '/' : `/?undo=${mode}`);
  }

  it('default: the seeded reset entry — undoable, mixed, dirty-aware', async () => {
    setUndo(null);
    const repoId = await openDefault();
    const plan = await run(undoHandlers.describeLastUndo(repoId));
    expect(plan).toMatchObject({
      kind: 'reset',
      undoable: true,
      resetMode: 'mixed',
      requiresCleanWorktree: false,
      worktreeDirty: true, // seeded staged+unstaged
      reason: null,
    });
    expect(plan.targetShort).toBe(plan.targetOid.slice(0, 7));
  });

  it('?undo=commit → mixed commit undo; a clean tree reports worktreeDirty:false', async () => {
    setUndo('commit');
    const repoId = await openDefault();
    const state = requireRepo(repoId);
    state.status.staged = [];
    state.status.unstaged = [];
    const plan = await run(undoHandlers.describeLastUndo(repoId));
    expect(plan).toMatchObject({ kind: 'commit', resetMode: 'mixed', worktreeDirty: false });
  });

  it('?undo=merge → hard reset requiring a clean worktree', async () => {
    setUndo('merge');
    const repoId = await openDefault();
    const plan = await run(undoHandlers.describeLastUndo(repoId));
    expect(plan).toMatchObject({
      kind: 'merge',
      undoable: true,
      resetMode: 'hard',
      requiresCleanWorktree: true,
      worktreeDirty: true,
    });
  });

  it('?undo=switch → not undoable, with the branch-switch reason', async () => {
    setUndo('switch');
    const repoId = await openDefault();
    const plan = await run(undoHandlers.describeLastUndo(repoId));
    expect(plan.undoable).toBe(false);
    expect(plan.kind).toBe('branchSwitch');
    expect(plan.resetMode).toBeNull();
    expect(plan.reason).toContain('check out the previous branch');
  });

  it('?undo=none → empty reflog: nothing to undo', async () => {
    setUndo('none');
    const repoId = await openDefault();
    const plan = await run(undoHandlers.describeLastUndo(repoId));
    expect(plan).toMatchObject({ undoable: false, reason: 'nothing to undo', targetOid: '' });
  });
});
