import { invoke } from '@tauri-apps/api/core';
import type { CommitStatus, CreatePrInput, FileDiff, ForgeAccount, ForgeKind, ForgeRepoContext, ForgeViewer, MergePrInput, PrDetail, PrDiffStats, PrListQuery, PrPage, ReviewComment } from '../types';

export const forgeCommands = {

  // P62: forge / PR integration. Thin invoke wrappers; arg keys match the
  // command signatures (camelCase repoId first). No events, no channels.
  forgeRepoContext(repoId: string): Promise<ForgeRepoContext> {
    return invoke<ForgeRepoContext>('forge_repo_context', { repoId });
  },

  forgeListPrs(repoId: string, query: PrListQuery): Promise<PrPage> {
    return invoke<PrPage>('forge_list_prs', { repoId, query });
  },

  forgeGetPr(repoId: string, number: number): Promise<PrDetail> {
    return invoke<PrDetail>('forge_get_pr', { repoId, number });
  },

  forgeCreatePr(repoId: string, input: CreatePrInput): Promise<PrDetail> {
    return invoke<PrDetail>('forge_create_pr', { repoId, input });
  },

  forgeMergePr(repoId: string, number: number, input: MergePrInput): Promise<PrDetail> {
    return invoke<PrDetail>('forge_merge_pr', { repoId, number, input });
  },

  forgeClosePr(repoId: string, number: number): Promise<PrDetail> {
    return invoke<PrDetail>('forge_close_pr', { repoId, number });
  },

  // P89: locally-computed PR base…head diff. Arg keys mirror the Rust command
  // param names (snake_case: number, merge_base_oid, head_oid, orig_path, …).
  forgePrDiff(repoId: string, number: number): Promise<PrDiffStats> {
    return invoke<PrDiffStats>('forge_pr_diff', { repoId, number });
  },

  forgePrFileDiff(
    repoId: string,
    mergeBaseOid: string,
    headOid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('forge_pr_file_diff', {
      repoId,
      mergeBaseOid,
      headOid,
      path,
      origPath,
      fullContext,
      intraline,
    });
  },

  forgeListReviewComments(repoId: string, number: number): Promise<ReviewComment[]> {
    return invoke<ReviewComment[]>('forge_list_review_comments', { repoId, number });
  },

  forgeSetToken(repoId: string, token: string): Promise<ForgeViewer> {
    return invoke<ForgeViewer>('forge_set_token', { repoId, token });
  },

  forgeClearToken(repoId: string): Promise<void> {
    return invoke<void>('forge_clear_token', { repoId });
  },

  forgeCommitStatuses(repoId: string, shas: string[]): Promise<CommitStatus[]> {
    return invoke<CommitStatus[]>('forge_commit_statuses', { repoId, shas });
  },

  // P79: global forge account management (repo-independent). Arg keys match the
  // Rust command param names (camelCase host/kind/token).
  forgeListAccounts(): Promise<ForgeAccount[]> {
    return invoke<ForgeAccount[]>('forge_list_accounts');
  },

  forgeAddAccount(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer> {
    return invoke<ForgeViewer>('forge_add_account', { host, kind, token });
  },

  forgeSetTokenForHost(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer> {
    return invoke<ForgeViewer>('forge_set_token_for_host', { host, kind, token });
  },

  forgeRemoveAccount(accountId: string): Promise<void> {
    return invoke<void>('forge_remove_account', { accountId });
  },

  forgeSetHostDefault(host: string, accountId: string): Promise<void> {
    return invoke<void>('forge_set_host_default', { host, accountId });
  },

  forgeSetRepoAccount(repoId: string, accountId: string | null): Promise<void> {
    return invoke<void>('forge_set_repo_account', { repoId, accountId });
  },

  forgeClearTokenForHost(host: string): Promise<void> {
    return invoke<void>('forge_clear_token_for_host', { host });
  },

  forgeInvalidateViewer(host: string): Promise<void> {
    return invoke<void>('forge_invalidate_viewer', { host });
  },
};
