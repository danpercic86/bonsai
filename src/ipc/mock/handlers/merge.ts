// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { delay, requireRepo } from '../repoState';
import { sortByPath, upsert } from '../statusHelpers';
import type { AppError, CommitResult, ConflictEntry, ConflictFile, ConflictResolution, MergeOutcome, RepoOpState } from '../../types';

export const mergeHandlers = {
  async getOpState(repoId: string): Promise<RepoOpState> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.opState);
  },

  async mergeBranch(repoId: string, name: string): Promise<MergeOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    // P8 demo triggers (keyed on the branch name, like the `?op=` convention)
    // so the browser harness can exercise every new outcome shape:
    //   "stash-conflict" -> stashPopConflicts (repo stays clean, no graph mutation)
    //   "conflict"       -> paused merge with an autostash retained on the stack
    //   "autostash"      -> clean merge that stashed and restored local changes
    if (name.includes('stash-conflict')) {
      return { kind: 'stashPopConflicts', head: randomOid(), paths: ['src/app.ts'] };
    }
    if (name.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts', 'README.md'], stashed: true };
    }
    const stashed = name.includes('autostash');
    // Clean-merge demo: auto-committed 2-parent node on top of the graph.
    state.headOid = randomOid();
    state.commits.unshift({
      oid: state.headOid,
      summary: `Merge branch '${name}'`,
      mergeParentBase: 1, // the 'feat' fixture tip
    });
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    return { kind: 'merged', oid: state.headOid, stashed };
  },

  async commitMerge(repoId: string, message: string): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'merge') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no merge in progress' };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot commit: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.status.conflicted = [];
    state.headOid = randomOid();
    const summary = message.trim().split('\n', 1)[0] ?? '';
    // Faithful twin: a visible 2-parent merge node on top of the graph
    // (second parent = the 'feat' fixture tip, base row 1).
    state.commits.unshift({ oid: state.headOid, summary, mergeParentBase: 1 });
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    return { oid: state.headOid, summary, branch: state.headBranch };
  },

  async abortMerge(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'merge') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no merge in progress' };
      throw err;
    }
    // Restore the pre-merge state.
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
  },

  async listConflicts(repoId: string): Promise<ConflictEntry[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.conflicts);
  },

  async getConflict(repoId: string, path: string): Promise<ConflictFile> {
    await delay(150);
    const state = requireRepo(repoId);
    const file = state.conflictTexts.get(path);
    if (file === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    return structuredClone(file);
  },

  async resolveConflict(
    repoId: string,
    path: string,
    resolution: ConflictResolution,
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.conflicts.find((c) => c.path === path);
    if (entry === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    state.conflicts = state.conflicts.filter((c) => c.path !== path);
    state.conflictTexts.delete(path);
    state.status.conflicted = state.status.conflicted.filter((e) => e.path !== path);
    // Taking THEIRS on a deletedByThem conflict accepts their deletion: the
    // file shows up as a staged deletion in the mock lists (contract §7.2).
    if (resolution === 'theirs' && entry.kind === 'deletedByThem') {
      upsert(state.status.staged, { path, origPath: null, status: 'deleted' });
      sortByPath(state.status.staged);
    }
  },

  async resolveConflictText(repoId: string, path: string, content: string): Promise<void> {
    // Backend writes `content` verbatim + stages it (P12 §1.2); the mock only
    // mirrors the resulting state change (the text editor runs for text kinds,
    // so no deletedByThem special-case is needed here).
    void content;
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.conflicts.find((c) => c.path === path);
    if (entry === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    state.conflicts = state.conflicts.filter((c) => c.path !== path);
    state.conflictTexts.delete(path);
    state.status.conflicted = state.status.conflicted.filter((e) => e.path !== path);
  },

  // P13: cheap CLI health probe. `?ai=off` simulates no claude on PATH; never
  // rejects for CLI state (matches the backend's never-Err check_availability).
} satisfies Partial<IpcApi>;
