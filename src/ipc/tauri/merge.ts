import { invoke } from '@tauri-apps/api/core';
import type { CommitResult, ConflictEntry, ConflictFile, ConflictResolution, MergeOutcome, RepoHooksDisclosure, RepoOpState } from '../types';

export const mergeCommands = {

  getRepoHooksDisclosure(repoId: string): Promise<RepoHooksDisclosure> {
    return invoke<RepoHooksDisclosure>('get_repo_hooks_disclosure', { repoId });
  },

  ackRepoHooks(repoId: string): Promise<void> {
    return invoke<void>('ack_repo_hooks', { repoId });
  },

  getOpState(repoId: string): Promise<RepoOpState> {
    return invoke<RepoOpState>('get_op_state', { repoId });
  },

  mergeBranch(repoId: string, name: string): Promise<MergeOutcome> {
    return invoke<MergeOutcome>('merge_branch', { repoId, name });
  },

  commitMerge(repoId: string, message: string, skipHooks = false): Promise<CommitResult> {
    return invoke<CommitResult>('commit_merge', { repoId, message, skipHooks });
  },

  abortMerge(repoId: string): Promise<void> {
    return invoke<void>('abort_merge', { repoId });
  },

  listConflicts(repoId: string): Promise<ConflictEntry[]> {
    return invoke<ConflictEntry[]>('list_conflicts', { repoId });
  },

  getConflict(repoId: string, path: string): Promise<ConflictFile> {
    return invoke<ConflictFile>('get_conflict', { repoId, path });
  },

  resolveConflict(repoId: string, path: string, resolution: ConflictResolution): Promise<void> {
    return invoke<void>('resolve_conflict', { repoId, path, resolution });
  },

  resolveConflictText(repoId: string, path: string, content: string): Promise<void> {
    return invoke<void>('resolve_conflict_text', { repoId, path, content });
  },

  // P68 #7 / H1: gated AI stage — the backend re-reads the sides and rejects a
  // novel body (`aiNeedsReview`) before writing.
  aiApplyResolution(repoId: string, path: string, content: string): Promise<void> {
    return invoke<void>('ai_apply_resolution', { repoId, path, content });
  },
};
