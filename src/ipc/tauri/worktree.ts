import { invoke } from '@tauri-apps/api/core';
import type { CopyCandidate, CopyPlanEntry, CopySelection, WorktreeInfo } from '../types';

export const worktreeCommands = {

  // P27: worktrees.
  listWorktrees(repoId: string): Promise<WorktreeInfo[]> {
    return invoke<WorktreeInfo[]>('list_worktrees', { repoId });
  },

  addWorktree(repoId: string, branch: string, name: string): Promise<WorktreeInfo> {
    return invoke<WorktreeInfo>('add_worktree', { repoId, branch, name });
  },

  removeWorktree(repoId: string, name: string): Promise<void> {
    return invoke<void>('remove_worktree', { repoId, name });
  },

  lockWorktree(repoId: string, name: string, reason?: string): Promise<void> {
    return invoke<void>('lock_worktree', { repoId, name, reason: reason ?? null });
  },

  unlockWorktree(repoId: string, name: string): Promise<void> {
    return invoke<void>('unlock_worktree', { repoId, name });
  },

  // P32 Part B: copy uncommitted changes into a new worktree.
  listCopyCandidates(repoId: string): Promise<CopyCandidate[]> {
    return invoke<CopyCandidate[]>('list_copy_candidates', { repoId });
  },

  previewWorktreeCopy(repoId: string, branch: string, paths: string[]): Promise<CopyPlanEntry[]> {
    return invoke<CopyPlanEntry[]>('preview_worktree_copy', { repoId, branch, paths });
  },

  addWorktreeWithChanges(
    repoId: string,
    branch: string,
    name: string,
    selections: CopySelection[],
  ): Promise<WorktreeInfo> {
    return invoke<WorktreeInfo>('add_worktree_with_changes', { repoId, branch, name, selections });
  },
};
