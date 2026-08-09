/** T3.4 — rebase.ts (clean + interactive + conflict pause/continue/skip/abort),
 *  bisect (start/mark/skip/reset over the synthetic chain), and resetRevert.ts
 *  cherry-pick / revert demo triggers. */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { rebaseHandlers } from './rebase';
import { mergeHandlers } from './merge';
import { bisectHistoryHandlers } from './bisectHistory';
import { resetRevertHandlers } from './resetRevert';
import { requireRepo } from '../repoState';
import { buildMockGraph } from '../../fixtures/graph';
import type { BisectOutcome, RebaseTodoOp } from '../../types';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('rb')));
  return repoId;
}

/** A repo whose PATH contains 'rebase' opens paused at step 2/3 (seed). */
async function openPausedRebase(): Promise<string> {
  const { repoId } = await run(
    repoHandlers.openRepo(`/mock/t34-rebase-${Math.random().toString(36).slice(2)}`),
  );
  return repoId;
}

describe('rebaseBranch (clean demo)', () => {
  it('replays 3 commits atop the graph and bumps ahead by 3', async () => {
    const repoId = await openDefault();
    const outcome = await run(rebaseHandlers.rebaseBranch(repoId, 'main'));
    expect(outcome).toMatchObject({ kind: 'rebased', branch: 'main', steps: 3 });
    const state = requireRepo(repoId);
    expect(state.commits.slice(0, 3).map((c) => c.summary)).toEqual([
      'pick: replayed 3',
      'pick: replayed 2',
      'pick: replayed 1',
    ]);
    if (outcome.kind === 'rebased') expect(state.headOid).toBe(outcome.head);
    expect(state.branches.local.find((b) => b.name === 'main')?.ahead).toBe(3);
  });

  it('rejects while another op is paused', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'x-conflict'));
    expect((await runErr(rebaseHandlers.rebaseBranch(repoId, 'main'))).kind).toBe(
      'operationInProgress',
    );
  });
});

describe('paused rebase (seeded 2/3) continue / skip / abort', () => {
  it('continue with conflicts rejects; after resolving it finishes and prepends', async () => {
    const repoId = await openPausedRebase();
    expect((await runErr(rebaseHandlers.rebaseContinue(repoId))).kind).toBe(
      'unresolvedConflicts',
    );
    await run(mergeHandlers.resolveConflictText(repoId, 'src/auth.ts', 'resolved'));
    const outcome = await run(rebaseHandlers.rebaseContinue(repoId));
    expect(outcome).toMatchObject({ kind: 'rebased', steps: 3 });
    const state = requireRepo(repoId);
    expect(state.opState).toEqual({ kind: 'none' });
    expect(state.commits).toHaveLength(3);
  });

  it('skip is allowed WITH conflicts and clears them', async () => {
    const repoId = await openPausedRebase();
    const outcome = await run(rebaseHandlers.rebaseSkip(repoId));
    expect(outcome.kind).toBe('rebased');
    const state = requireRepo(repoId);
    expect(state.conflicts).toEqual([]);
    expect(state.status.conflicted).toEqual([]);
  });

  it('abort rewinds: no prepended commits, op cleared', async () => {
    const repoId = await openPausedRebase();
    await run(rebaseHandlers.rebaseAbort(repoId));
    const state = requireRepo(repoId);
    expect(state.opState).toEqual({ kind: 'none' });
    expect(state.commits).toHaveLength(0);
    expect((await runErr(rebaseHandlers.rebaseContinue(repoId))).kind).toBe(
      'noOperationInProgress',
    );
  });
});

describe('interactive rebase', () => {
  // NB: getInteractivePlan resolves rows WITHOUT the stash offshoots getGraph
  // injects, so base oids are taken from the raw fixture layout.
  it('getInteractivePlan: ≤3 all-pick todos oldest-first; guards base==HEAD/unknown', async () => {
    const repoId = await openDefault();
    const layout = buildMockGraph();
    const base = layout.nodes[3].id;
    const plan = await run(rebaseHandlers.getInteractivePlan(repoId, base));
    expect(plan).toHaveLength(3);
    expect(plan.every((t) => t.action === 'pick' && t.newMessage === null)).toBe(true);
    // Oldest-first == rows just above the base, reversed.
    expect(plan.map((t) => t.oid)).toEqual([
      layout.nodes[2].id,
      layout.nodes[1].id,
      layout.nodes[0].id,
    ]);
    expect(
      (await runErr(rebaseHandlers.getInteractivePlan(repoId, layout.nodes[0].id))).kind,
    ).toBe('git');
    expect(
      (await runErr(rebaseHandlers.getInteractivePlan(repoId, 'f0'.repeat(20)))).kind,
    ).toBe('git');
  });

  it('clean replay: rewritten commits REPLACE the originals (reword applied)', async () => {
    const repoId = await openDefault();
    const layout = buildMockGraph();
    const base = layout.nodes[3].id;
    const todos = await run(rebaseHandlers.getInteractivePlan(repoId, base));
    const edited: RebaseTodoOp[] = todos.map((t, i) =>
      i === 0 ? { ...t, action: 'reword', newMessage: 'reworded: first\n\nbody' } : t,
    );
    const outcome = await run(rebaseHandlers.startInteractiveRebase(repoId, base, edited));
    expect(outcome).toMatchObject({ kind: 'rebased', steps: 3 });
    const state = requireRepo(repoId);
    expect(state.opState).toEqual({ kind: 'none' });
    // 3 rewritten mock commits, newest-first; the reworded one is the OLDEST.
    expect(state.commits).toHaveLength(3);
    expect(state.commits[2].summary).toBe('reworded: first');
    if (outcome.kind === 'rebased') expect(state.commits[0].oid).toBe(outcome.head);
  });

  it('plan guards: all-drop rejects; squash-first rejects', async () => {
    const repoId = await openDefault();
    const layout = buildMockGraph();
    const base = layout.nodes[2].id;
    const todos = await run(rebaseHandlers.getInteractivePlan(repoId, base));
    expect(
      (
        await runErr(
          rebaseHandlers.startInteractiveRebase(
            repoId,
            base,
            todos.map((t) => ({ ...t, action: 'drop' as const })),
          ),
        )
      ).kind,
    ).toBe('git');
    expect(
      (
        await runErr(
          rebaseHandlers.startInteractiveRebase(repoId, base, [
            { ...todos[0], action: 'squash' as const },
            ...todos.slice(1),
          ]),
        )
      ).kind,
    ).toBe('git');
  });

  it('a c0ffee-suffixed todo pauses on a conflict; continue finishes the plan', async () => {
    const repoId = await openDefault();
    const layout = buildMockGraph();
    const base = layout.nodes[2].id;
    const todos = await run(rebaseHandlers.getInteractivePlan(repoId, base));
    const withConflict: RebaseTodoOp[] = [
      { oid: 'ab'.repeat(17) + 'c0ffee', action: 'pick', newMessage: null },
      ...todos,
    ];
    const outcome = await run(rebaseHandlers.startInteractiveRebase(repoId, base, withConflict));
    expect(outcome).toMatchObject({ kind: 'conflicts', currentStep: 1, totalSteps: 3 });
    expect(requireRepo(repoId).opState.kind).toBe('rebase');
    await run(mergeHandlers.resolveConflictText(repoId, 'src/auth.ts', 'ok'));
    const finished = await run(rebaseHandlers.rebaseContinue(repoId));
    expect(finished).toMatchObject({ kind: 'rebased', steps: 3 });
    expect(requireRepo(repoId).interactive).toBeNull();
  });
});

describe('bisect', () => {
  it('start seeds the chain, reports testing with the midpoint, and rides opState', async () => {
    const repoId = await openDefault();
    const bad = 'ba'.repeat(20);
    const good = 'g0'.repeat(20);
    const outcome = await run(bisectHistoryHandlers.startBisect(repoId, bad, [good]));
    expect(outcome.kind).toBe('testing');
    if (outcome.kind === 'testing') {
      expect(outcome.revisionsRemaining).toBe(6);
      expect(outcome.estimatedSteps).toBe(3);
    }
    const op = await run(mergeHandlers.getOpState(repoId));
    expect(op.kind).toBe('bisect');
    if (op.kind === 'bisect') {
      expect(op.bad).toBe(bad);
      expect(op.good).toEqual([good]);
    }
  });

  it('marking converges to found within the estimated steps', async () => {
    const repoId = await openDefault();
    let outcome: BisectOutcome = await run(
      bisectHistoryHandlers.startBisect(repoId, 'ba'.repeat(20), ['g0'.repeat(20)]),
    );
    let guard = 0;
    while (outcome.kind === 'testing' && guard < 10) {
      outcome = await run(bisectHistoryHandlers.bisectMark(repoId, false)); // always bad
      guard += 1;
    }
    expect(outcome.kind).toBe('found');
    expect(guard).toBeLessThanOrEqual(3);
    const op = await run(mergeHandlers.getOpState(repoId));
    if (op.kind === 'bisect') expect(op.firstBad).not.toBeNull();
    await run(bisectHistoryHandlers.bisectReset(repoId));
    expect(await run(mergeHandlers.getOpState(repoId))).toEqual({ kind: 'none' });
  });

  it('skipping everything yields cannotDetermine', async () => {
    const repoId = await openDefault();
    let outcome: BisectOutcome = await run(
      bisectHistoryHandlers.startBisect(repoId, 'ba'.repeat(20), ['g0'.repeat(20)]),
    );
    let guard = 0;
    while (outcome.kind === 'testing' && guard < 10) {
      outcome = await run(bisectHistoryHandlers.bisectSkip(repoId));
      guard += 1;
    }
    expect(outcome.kind).toBe('cannotDetermine');
    if (outcome.kind === 'cannotDetermine') expect(outcome.skipped).toHaveLength(6);
  });

  it('guards: same good/bad rejects; mark/skip/reset without a bisect reject', async () => {
    const repoId = await openDefault();
    expect(
      (await runErr(bisectHistoryHandlers.startBisect(repoId, 'aa'.repeat(20), ['aa'.repeat(20)])))
        .kind,
    ).toBe('git');
    expect((await runErr(bisectHistoryHandlers.bisectMark(repoId, true))).kind).toBe(
      'noOperationInProgress',
    );
    expect((await runErr(bisectHistoryHandlers.bisectSkip(repoId))).kind).toBe(
      'noOperationInProgress',
    );
    expect((await runErr(bisectHistoryHandlers.bisectReset(repoId))).kind).toBe(
      'noOperationInProgress',
    );
  });
});

describe('cherry-pick / revert demo triggers', () => {
  it('clean cherry-pick commits a new top node; custom message wins', async () => {
    const repoId = await openDefault();
    const outcome = await run(
      resetRevertHandlers.cherrypickCommit(repoId, '12'.repeat(20), 'picked: custom\nbody'),
    );
    expect(outcome.kind).toBe('committed');
    const state = requireRepo(repoId);
    expect(state.commits[0].summary).toBe('picked: custom');
    expect(outcome.kind === 'committed' && outcome.stashed).toBe(true); // seeded dirty
  });

  it('c0ffee suffix pauses with a cherryPick op; continue gated then commits', async () => {
    const repoId = await openDefault();
    const oid = 'ab'.repeat(17) + 'c0ffee';
    const outcome = await run(resetRevertHandlers.cherrypickCommit(repoId, oid, null));
    expect(outcome).toMatchObject({ kind: 'conflicts', paths: ['src/app.ts'] });
    expect(requireRepo(repoId).opState).toEqual({ kind: 'cherryPick' });
    expect((await runErr(resetRevertHandlers.cherrypickContinue(repoId))).kind).toBe(
      'unresolvedConflicts',
    );
    await run(mergeHandlers.resolveConflict(repoId, 'src/app.ts', 'ours'));
    const done = await run(resetRevertHandlers.cherrypickContinue(repoId));
    expect(done).toMatchObject({ kind: 'committed', stashed: false });
    expect(requireRepo(repoId).opState).toEqual({ kind: 'none' });
  });

  it('deadbe suffix on a dirty tree → stashPopConflicts after committing', async () => {
    const repoId = await openDefault();
    const oid = 'ab'.repeat(17) + 'deadbe';
    const outcome = await run(resetRevertHandlers.revertCommit(repoId, oid));
    expect(outcome.kind).toBe('stashPopConflicts');
    // The commit itself landed.
    expect(requireRepo(repoId).commits[0].summary).toContain('Revert');
  });

  it('revert abort restores none; continue without an op rejects', async () => {
    const repoId = await openDefault();
    const oid = 'cd'.repeat(17) + 'c0ffee';
    await run(resetRevertHandlers.revertCommit(repoId, oid));
    expect(requireRepo(repoId).opState).toEqual({ kind: 'revert' });
    await run(resetRevertHandlers.revertAbort(repoId));
    expect(requireRepo(repoId).opState).toEqual({ kind: 'none' });
    expect((await runErr(resetRevertHandlers.revertContinue(repoId))).kind).toBe(
      'noOperationInProgress',
    );
  });
});
