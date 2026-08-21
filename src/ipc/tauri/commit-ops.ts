import { invoke } from '@tauri-apps/api/core';
import type { CherrypickOutcome, CommitResult, ResetMode, RevertOutcome } from '../types';

export const commitOpsCommands = {

  commitAmend(
    repoId: string,
    message: string,
    sign: boolean | null = null,
    skipHooks = false,
  ): Promise<CommitResult> {
    return invoke<CommitResult>('commit_amend', { repoId, message, sign, skipHooks });
  },

  resetBranch(repoId: string, oid: string, mode: ResetMode): Promise<void> {
    return invoke<void>('reset_branch', { repoId, oid, mode });
  },

  discardPaths(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('discard_paths', { repoId, paths });
  },

  discardPathsForce(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('discard_paths_force', { repoId, paths });
  },

  cherrypickCommit(
    repoId: string,
    oid: string,
    message: string | null = null,
  ): Promise<CherrypickOutcome> {
    return invoke<CherrypickOutcome>('cherrypick_commit', { repoId, oid, message });
  },

  cherrypickContinue(repoId: string): Promise<CherrypickOutcome> {
    return invoke<CherrypickOutcome>('cherrypick_continue', { repoId });
  },

  cherrypickAbort(repoId: string): Promise<void> {
    return invoke<void>('cherrypick_abort', { repoId });
  },

  revertCommit(repoId: string, oid: string): Promise<RevertOutcome> {
    return invoke<RevertOutcome>('revert_commit', { repoId, oid });
  },

  revertContinue(repoId: string): Promise<RevertOutcome> {
    return invoke<RevertOutcome>('revert_continue', { repoId });
  },

  revertAbort(repoId: string): Promise<void> {
    return invoke<void>('revert_abort', { repoId });
  },
};
