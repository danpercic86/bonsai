// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { seedPickRevertConflict } from '../opStateSeed';
import { PICK_REVERT_CONFLICT_OID_SUFFIX, STASH_POP_CONFLICT_OID_SUFFIX, delay, requireRepo } from '../repoState';
import type { AppError, CherrypickOutcome, GitActivityCategory, ResetMode, RevertOutcome } from '../../types';
import { runMockActivity } from '../gitActivity';
import { mockPickOutcome } from '../gitActivityOutcome';
import { mockCommitTarget, mockHeadTarget } from '../gitActivityTargetArg';

/** §1 rows 22-24: the reset MODE picks one of three categories. */
const RESET_CATEGORY: Record<ResetMode, GitActivityCategory> = {
  soft: 'resetSoft',
  mixed: 'resetMixed',
  hard: 'resetHard',
};

/** The handler BODIES; the public `resetRevertHandlers` below wraps the §1 rows. */
const resetRevertBodies = {
  async resetBranch(repoId: string, oid: string, _mode: ResetMode): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    // Visual fidelity: drop synthetic lane-0 rows above the target (inclusive
    // move of HEAD onto `oid`). A plain move of headOid is enough for the
    // harness; guard against an unknown oid by leaving the list untouched.
    const target = state.commits.findIndex((c) => c.oid === oid);
    if (target > 0) {
      state.commits = state.commits.slice(target);
    }
    state.headOid = oid;
  },

  async discardPaths(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const drop = new Set(paths);
    state.status.unstaged = state.status.unstaged.filter((e) => !drop.has(e.path));
  },

  // P36: force-discard a mixed set — tracked paths reverted (dropped from
  // unstaged) AND untracked paths deleted (dropped from untracked), mirroring the
  // backend split so the Changes panel reflects a bulk force-discard.
  async discardPathsForce(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const drop = new Set(paths);
    state.status.unstaged = state.status.unstaged.filter((e) => !drop.has(e.path));
    state.status.untracked = state.status.untracked.filter((e) => !drop.has(e.path));
  },

  // P20 §5/§8.4 + P47: cherry-pick. An oid ending in the conflict suffix pauses
  // with a conflict (op-state → cherryPick); one ending in the stash-pop suffix
  // commits cleanly but conflicts re-applying the autostash; any other oid
  // commits a new top node. `stashed` mirrors the backend: true when the tracked
  // worktree is dirty (autostash was needed). `message`, when supplied, becomes
  // the new commit's summary (drives the editable-message flow, P47).
  async cherrypickCommit(
    repoId: string,
    oid: string,
    message?: string | null,
  ): Promise<CherrypickOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — finish or abort it first',
      };
      throw err;
    }
    const stashed = state.status.unstaged.length > 0 || state.status.staged.length > 0;
    if (oid.endsWith(PICK_REVERT_CONFLICT_OID_SUFFIX)) {
      seedPickRevertConflict(state, 'cherryPick');
      return { kind: 'conflicts', paths: ['src/app.ts'], stashed };
    }
    state.headOid = randomOid();
    const summary =
      message != null && message.length > 0
        ? message.split('\n', 1)[0]
        : `Cherry-pick ${oid.slice(0, 7)}`;
    state.commits.unshift({ oid: state.headOid, summary });
    // stashPopConflicts is only reachable when an autostash was actually created
    // (a clean tree can never hit a stash-apply conflict — mirrors the backend).
    if (stashed && oid.endsWith(STASH_POP_CONFLICT_OID_SUFFIX)) {
      return { kind: 'stashPopConflicts', head: state.headOid, paths: ['src/app.ts'] };
    }
    return { kind: 'committed', oid: state.headOid, stashed };
  },

  async cherrypickContinue(repoId: string): Promise<CherrypickOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'cherryPick') {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'no cherry-pick in progress',
      };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.status.conflicted = [];
    state.conflictTexts = new Map();
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: 'Cherry-pick (resolved)' });
    // Continue never re-applies a retained autostash (F5) → stashed: false.
    return { kind: 'committed', oid: state.headOid, stashed: false };
  },

  async cherrypickAbort(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'cherryPick') {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'no cherry-pick in progress',
      };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
  },

  // P20 §6/§8.4 + P47: revert. Same demo-triggers + autostash plumbing as
  // cherry-pick, but no editable message (revert keeps its deterministic text).
  async revertCommit(repoId: string, oid: string): Promise<RevertOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — finish or abort it first',
      };
      throw err;
    }
    const stashed = state.status.unstaged.length > 0 || state.status.staged.length > 0;
    if (oid.endsWith(PICK_REVERT_CONFLICT_OID_SUFFIX)) {
      seedPickRevertConflict(state, 'revert');
      return { kind: 'conflicts', paths: ['src/app.ts'], stashed };
    }
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: `Revert "${oid.slice(0, 7)}"` });
    // stashPopConflicts is only reachable when an autostash was actually created
    // (a clean tree can never hit a stash-apply conflict — mirrors the backend).
    if (stashed && oid.endsWith(STASH_POP_CONFLICT_OID_SUFFIX)) {
      return { kind: 'stashPopConflicts', head: state.headOid, paths: ['src/app.ts'] };
    }
    return { kind: 'committed', oid: state.headOid, stashed };
  },

  async revertContinue(repoId: string): Promise<RevertOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'revert') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no revert in progress' };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.status.conflicted = [];
    state.conflictTexts = new Map();
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: 'Revert (resolved)' });
    // Continue never re-applies a retained autostash (F5) → stashed: false.
    return { kind: 'committed', oid: state.headOid, stashed: false };
  },

  async revertAbort(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'revert') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no revert in progress' };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
  },

  // Stateful submodule mock (P19 §5). init flips uninitialized→upToDate;
  // update brings uninitialized/outOfSync→upToDate; sync is a config no-op.
} satisfies Partial<IpcApi>;

/** P119 §5.2 — the repo-changing ops run inside the git-activity bracket with
 *  the §1 category + target (and the §2.6 classifier where one exists). */
export const resetRevertHandlers = {
  ...resetRevertBodies,
  resetBranch: (repoId: string, oid: string, mode: ResetMode) =>
    runMockActivity(RESET_CATEGORY[mode], mockCommitTarget(oid), () =>
      resetRevertBodies.resetBranch(repoId, oid, mode),
    ),
  // `RunSubject::many`: one path → that path; ≥2 → a count, no target.
  discardPaths: (repoId: string, paths: string[]) =>
    runMockActivity('discard', paths.length === 1 ? (paths[0] ?? null) : null, () =>
      resetRevertBodies.discardPaths(repoId, paths), { count: paths.length },
    ),
  discardPathsForce: (repoId: string, paths: string[]) =>
    runMockActivity('discard', paths.length === 1 ? (paths[0] ?? null) : null, () =>
      resetRevertBodies.discardPathsForce(repoId, paths), { count: paths.length },
    ),
  cherrypickCommit: (repoId: string, oid: string, message?: string | null) =>
    runMockActivity('cherryPick', mockCommitTarget(oid), () =>
      resetRevertBodies.cherrypickCommit(repoId, oid, message), { classify: mockPickOutcome },
    ),
  cherrypickContinue: (repoId: string) =>
    runMockActivity('cherryPickContinue', mockHeadTarget(repoId), () =>
      resetRevertBodies.cherrypickContinue(repoId), { classify: mockPickOutcome },
    ),
  cherrypickAbort: (repoId: string) =>
    runMockActivity('cherryPickAbort', mockHeadTarget(repoId), () =>
      resetRevertBodies.cherrypickAbort(repoId),
    ),
  revertCommit: (repoId: string, oid: string) =>
    runMockActivity('revert', mockCommitTarget(oid), () => resetRevertBodies.revertCommit(repoId, oid), {
      classify: mockPickOutcome,
    }),
  revertContinue: (repoId: string) =>
    runMockActivity('revertContinue', mockHeadTarget(repoId), () =>
      resetRevertBodies.revertContinue(repoId), { classify: mockPickOutcome },
    ),
  revertAbort: (repoId: string) =>
    runMockActivity('revertAbort', mockHeadTarget(repoId), () => resetRevertBodies.revertAbort(repoId)),
} satisfies Partial<IpcApi>;

