// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { MERGE_AUTH_OURS, MERGE_AUTH_TEXT, MERGE_AUTH_THEIRS } from '../../fixtures/conflicts';
import { buildMockGraph, buildMockGraphDetached, prependCommits } from '../../fixtures/graph';
import { generateLayout20k } from '../../fixtures/graph20k';
import { randomOid } from '../../fixtures/oids';
import { applyInteractivePlan, finishInteractiveRebase, finishRebase } from '../rebaseBisectHelpers';
import { INTERACTIVE_REBASE_CONFLICT_OID_SUFFIX, delay, requireRepo } from '../repoState';
import type { AppError, RebaseOutcome, RebaseTodoOp } from '../../types';

export const rebaseHandlers = {
  async rebaseBranch(repoId: string, _onto: string): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    // Clean-rebase demo: replay 3 plain commits atop the graph so they appear.
    // commits[0] is the topmost row = the new HEAD tip, so it carries the oid.
    state.headOid = randomOid();
    state.commits.unshift(
      { oid: state.headOid, summary: 'pick: replayed 3' },
      { oid: randomOid(), summary: 'pick: replayed 2' },
      { oid: randomOid(), summary: 'pick: replayed 1' },
    );
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 3;
    }
    return { kind: 'rebased', branch: state.headBranch, head: state.headOid, steps: 3 };
  },

  async rebaseContinue(repoId: string): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    // P23b: an interactive rebase finishes by prepending its rewritten commits.
    if (state.interactive !== null) {
      return finishInteractiveRebase(state, false);
    }
    // Advance the current step (so a mid-call getOpState would reflect it), then
    // finish: the seeded demo has no further conflict, so a single continue
    // completes the remaining steps (2/3 → done).
    const totalSteps = state.opState.totalSteps;
    state.opState = { ...state.opState, currentStep: state.opState.currentStep + 1 };
    return finishRebase(state, totalSteps);
  },

  async rebaseSkip(repoId: string): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    // P23b: skip an interactive op — drop the current (conflicting) op, finish.
    if (state.interactive !== null) {
      return finishInteractiveRebase(state, true);
    }
    // Skip is allowed WITH conflicts — dropping the offending commit resolves it.
    const totalSteps = state.opState.totalSteps;
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
    state.opState = { ...state.opState, currentStep: state.opState.currentStep + 1 };
    return finishRebase(state, totalSteps);
  },

  async rebaseAbort(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    // Abort rewinds: restore the pre-rebase state, prepend NOTHING. For an
    // interactive rebase this also drops the pending plan (the branch ref was
    // never moved in the real engine).
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
    state.interactive = null;
  },

  // P23b §7.2: interactive-rebase plan seed + start. getInteractivePlan returns
  // the commits `baseOid..HEAD` of the active mock layout as all-`pick` todos
  // (oldest-first). startInteractiveRebase applies the plan deterministically
  // (see applyInteractivePlan) and either finishes immediately (rewritten
  // commits prepended) or pauses on a conflict that drives the EXISTING OpBanner.
  async getInteractivePlan(repoId: string, baseOid: string): Promise<RebaseTodoOp[]> {
    await delay(150);
    const state = requireRepo(repoId);
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    const idx = layout.nodes.findIndex((n) => n.id === baseOid);
    if (idx === -1) {
      const err: AppError = { kind: 'git', message: 'mock: base commit is not in the graph' };
      throw err;
    }
    if (idx === 0) {
      const err: AppError = {
        kind: 'git',
        message: `nothing to rebase: ${baseOid.slice(0, 7)} is HEAD`,
      };
      throw err;
    }
    // Rows above the base (indices 0..idx-1) are newer than it; the replayed
    // range base..HEAD in execution (oldest-first) order is those rows reversed.
    // Cap at the 3 nearest the base for a compact editor.
    const slice = layout.nodes.slice(Math.max(0, idx - 3), idx);
    const oldestFirst = slice.slice().reverse();
    return oldestFirst.map((n) => ({ oid: n.id, action: 'pick', newMessage: null }));
  },

  async startInteractiveRebase(
    repoId: string,
    ontoOid: string,
    todos: RebaseTodoOp[],
  ): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    const kept = todos.filter((t) => t.action !== 'drop');
    if (kept.length === 0) {
      const err: AppError = {
        kind: 'git',
        message: 'nothing to rebase: the plan drops every commit',
      };
      throw err;
    }
    if (kept[0].action !== 'pick' && kept[0].action !== 'reword') {
      const err: AppError = { kind: 'git', message: 'a squash/fixup must follow a pick' };
      throw err;
    }
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    const summaryOf = (oid: string): string =>
      layout.nodes.find((n) => n.id === oid)?.summary ?? `picked ${oid.slice(0, 7)}`;
    const rewritten = applyInteractivePlan(todos, summaryOf);
    const totalSteps = kept.length;
    // Every replayed commit (base..old-HEAD) is rewritten → remove the originals
    // from the mock commit list so the rewritten set replaces them on finish.
    const originalOids = todos.map((t) => t.oid);

    const conflictTriggered =
      state.interactiveConflictDemo ||
      todos.some(
        (t) => t.action !== 'drop' && t.oid.endsWith(INTERACTIVE_REBASE_CONFLICT_OID_SUFFIX),
      );
    if (conflictTriggered) {
      // Pause on a conflict: seed the merge-conflict fixture + op-state so the
      // EXISTING OpBanner + conflict rows + rebaseContinue/Skip/Abort take over.
      state.opState = {
        kind: 'rebase',
        headName: state.headBranch,
        onto: ontoOid,
        currentStep: 1,
        totalSteps,
      };
      state.conflicts = [
        {
          path: 'src/auth.ts',
          kind: 'bothModified',
          hasBase: true,
          hasOurs: true,
          hasTheirs: true,
        },
      ];
      state.conflictTexts = new Map();
      state.conflictTexts.set('src/auth.ts', {
        path: 'src/auth.ts',
        kind: 'bothModified',
        binary: false,
        tooLarge: false,
        missing: false,
        text: MERGE_AUTH_TEXT,
        ours: MERGE_AUTH_OURS,
        theirs: MERGE_AUTH_THEIRS,
      });
      state.status.conflicted = [{ path: 'src/auth.ts', origPath: null, status: 'conflicted' }];
      state.interactive = {
        headName: state.headBranch,
        ontoOid,
        rewritten,
        originalOids,
        totalSteps,
      };
      return { kind: 'conflicts', paths: ['src/auth.ts'], currentStep: 1, totalSteps };
    }

    // Clean replay: remove the original range commits, then prepend the rewritten
    // commits atop the graph (the top row carries the new HEAD tip) and finish.
    const removedClean = new Set(originalOids);
    state.commits = state.commits.filter((c) => !removedClean.has(c.oid));
    state.headOid = randomOid();
    const prepend =
      rewritten.length > 0
        ? rewritten.map((c, i) => (i === 0 ? { ...c, oid: state.headOid } : c))
        : [];
    if (prepend.length > 0) state.commits.unshift(...prepend);
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + prepend.length;
    }
    return {
      kind: 'rebased',
      branch: state.headBranch,
      head: state.headOid,
      steps: prepend.length,
    };
  },

  // P39: git bisect — a deterministic binary search over a synthetic candidate
  // chain seeded between the bad and good commits. Progress rides on getOpState
  // (RepoOpState.bisect); mark/skip narrow the window to a `found` result.
} satisfies Partial<IpcApi>;
