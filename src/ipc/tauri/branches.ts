import { invoke } from '@tauri-apps/api/core';
import type { BranchDeleteResult, BranchesSnapshot, CheckoutResult, CreateBranchHereResult, RenameBranchResult, StaleReport } from '../types';

export const branchesCommands = {

  listBranches(repoId: string): Promise<BranchesSnapshot> {
    return invoke<BranchesSnapshot>('list_branches', { repoId });
  },

  createBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('create_branch', { repoId, name });
  },

  createBranchHere(repoId: string, name: string, oid: string): Promise<CreateBranchHereResult> {
    return invoke<CreateBranchHereResult>('create_branch_here', { repoId, name, oid });
  },

  checkoutBranch(repoId: string, name: string): Promise<CheckoutResult> {
    return invoke<CheckoutResult>('checkout_branch', { repoId, name });
  },

  checkoutCommit(repoId: string, oid: string): Promise<CheckoutResult> {
    return invoke<CheckoutResult>('checkout_commit', { repoId, oid });
  },

  deleteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_branch', { repoId, name });
  },

  renameBranch(repoId: string, oldName: string, newName: string): Promise<RenameBranchResult> {
    return invoke<RenameBranchResult>('rename_branch', { repoId, oldName, newName });
  },

  checkoutRemoteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('checkout_remote', { repoId, name });
  },

  deleteRemoteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_remote_tracking', { repoId, name });
  },

  listStaleBranches(repoId: string, base?: string): Promise<StaleReport> {
    return invoke<StaleReport>('list_stale_branches', { repoId, base });
  },

  deleteBranches(repoId: string, names: string[], base?: string): Promise<BranchDeleteResult[]> {
    return invoke<BranchDeleteResult[]>('delete_branches', { repoId, names, base });
  },
};
