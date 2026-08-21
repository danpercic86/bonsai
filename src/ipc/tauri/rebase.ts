import { invoke } from '@tauri-apps/api/core';
import type { BisectOutcome, RebaseOutcome, RebaseTodoOp } from '../types';

export const rebaseCommands = {

  rebaseBranch(repoId: string, onto: string): Promise<RebaseOutcome> {
    return invoke<RebaseOutcome>('rebase_branch', { repoId, onto });
  },

  rebaseContinue(repoId: string): Promise<RebaseOutcome> {
    return invoke<RebaseOutcome>('rebase_continue', { repoId });
  },

  rebaseSkip(repoId: string): Promise<RebaseOutcome> {
    return invoke<RebaseOutcome>('rebase_skip', { repoId });
  },

  rebaseAbort(repoId: string): Promise<void> {
    return invoke<void>('rebase_abort', { repoId });
  },

  // P23b: interactive rebase — seed the plan, then start the replay. Continue/
  // Skip/Abort reuse the plain rebase* wrappers above (the backend delegates).
  getInteractivePlan(repoId: string, baseOid: string): Promise<RebaseTodoOp[]> {
    return invoke<RebaseTodoOp[]>('get_interactive_plan', { repoId, baseOid });
  },

  startInteractiveRebase(
    repoId: string,
    ontoOid: string,
    todos: RebaseTodoOp[],
  ): Promise<RebaseOutcome> {
    return invoke<RebaseOutcome>('start_interactive_rebase', { repoId, ontoOid, todos });
  },

  // P39: git bisect — start + mark/skip/reset. Progress rides on get_op_state
  // (RepoOpState.bisect); the frontend refetches after each mutation.
  startBisect(repoId: string, bad: string, good: string[]): Promise<BisectOutcome> {
    return invoke<BisectOutcome>('start_bisect', { repoId, bad, good });
  },

  bisectMark(repoId: string, isGood: boolean): Promise<BisectOutcome> {
    return invoke<BisectOutcome>('bisect_mark', { repoId, isGood });
  },

  bisectSkip(repoId: string): Promise<BisectOutcome> {
    return invoke<BisectOutcome>('bisect_skip', { repoId });
  },

  bisectReset(repoId: string): Promise<void> {
    return invoke<void>('bisect_reset', { repoId });
  },
};
