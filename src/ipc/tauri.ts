import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  AiAnalysis,
  AiAnalysisMode,
  AiDiffTarget,
  AiAvailability,
  AiResolveProposal,
  AiSummary,
  ApplyStashOutcome,
  BranchesSnapshot,
  CommitDiff,
  CommitMessageProposal,
  CommitResult,
  CompareDiff,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  CreateBranchHereResult,
  CreateStashResult,
  FetchResult,
  FileDiff,
  GraphLayout,
  IpcApi,
  LineSelection,
  McpStatus,
  MergeOutcome,
  OpenRepoResult,
  PullResult,
  PushResult,
  RebaseOutcome,
  RecentRepo,
  RepoChangedPayload,
  RepoOpState,
  SessionState,
  StashEntry,
  StatusSnapshot,
  UiSettings,
  UiSettingsPatch,
  Unsubscribe,
} from './types';

export const tauriIpc: IpcApi = {
  openRepo(path: string): Promise<OpenRepoResult> {
    return invoke<OpenRepoResult>('open_repo', { path });
  },

  closeRepo(repoId: string): Promise<void> {
    return invoke<void>('close_repo', { repoId });
  },

  async pickFolder(): Promise<string | null> {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Open repository',
    });
    return typeof selected === 'string' ? selected : null;
  },

  getStatus(repoId: string): Promise<StatusSnapshot> {
    return invoke<StatusSnapshot>('get_status', { repoId });
  },

  getGraph(repoId: string): Promise<GraphLayout> {
    return invoke<GraphLayout>('get_graph', { repoId });
  },

  stage(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('stage', { repoId, paths });
  },

  unstage(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('unstage', { repoId, paths });
  },

  commit(repoId: string, message: string): Promise<CommitResult> {
    return invoke<CommitResult>('commit', { repoId, message });
  },

  getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_workdir_file_diff', {
      repoId,
      path,
      origPath,
      staged,
      fullContext,
    });
  },

  stagePartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    return invoke<void>('stage_partial', { repoId, path, origPath, selection });
  },

  unstagePartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    return invoke<void>('unstage_partial', { repoId, path, origPath, selection });
  },

  getCommitDiff(repoId: string, oid: string): Promise<CommitDiff> {
    return invoke<CommitDiff>('get_commit_diff', { repoId, oid });
  },

  getCommitFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_commit_file_diff', { repoId, oid, path, origPath, fullContext });
  },

  compareWithHead(repoId: string, oid: string): Promise<CompareDiff> {
    return invoke<CompareDiff>('compare_with_head', { repoId, oid });
  },

  compareWithHeadFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('compare_with_head_file_diff', {
      repoId,
      oid,
      path,
      origPath,
      fullContext,
    });
  },

  listBranches(repoId: string): Promise<BranchesSnapshot> {
    return invoke<BranchesSnapshot>('list_branches', { repoId });
  },

  createBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('create_branch', { repoId, name });
  },

  createBranchHere(repoId: string, name: string, oid: string): Promise<CreateBranchHereResult> {
    return invoke<CreateBranchHereResult>('create_branch_here', { repoId, name, oid });
  },

  checkoutBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('checkout_branch', { repoId, name });
  },

  deleteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_branch', { repoId, name });
  },

  checkoutRemoteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('checkout_remote', { repoId, name });
  },

  deleteRemoteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_remote_tracking', { repoId, name });
  },

  fetch(repoId: string): Promise<FetchResult> {
    return invoke<FetchResult>('fetch', { repoId });
  },

  pull(repoId: string): Promise<PullResult> {
    return invoke<PullResult>('pull', { repoId });
  },

  push(repoId: string): Promise<PushResult> {
    return invoke<PushResult>('push', { repoId });
  },

  getOpState(repoId: string): Promise<RepoOpState> {
    return invoke<RepoOpState>('get_op_state', { repoId });
  },

  mergeBranch(repoId: string, name: string): Promise<MergeOutcome> {
    return invoke<MergeOutcome>('merge_branch', { repoId, name });
  },

  commitMerge(repoId: string, message: string): Promise<CommitResult> {
    return invoke<CommitResult>('commit_merge', { repoId, message });
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

  // P13: Claude Code CLI health probe + AI conflict resolution (proposal only).
  checkAiAvailability(): Promise<AiAvailability> {
    return invoke<AiAvailability>('check_ai_availability');
  },

  aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal> {
    return invoke<AiResolveProposal>('ai_resolve_conflict', { repoId, path });
  },

  // P15a: generate a commit message from the staged diff (proposal only).
  generateCommitMessage(repoId: string): Promise<CommitMessageProposal> {
    return invoke<CommitMessageProposal>('generate_commit_message', { repoId });
  },

  // P15b: explain/review a diff target (read-only prose).
  aiAnalyzeDiff(
    repoId: string,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
  ): Promise<AiAnalysis> {
    return invoke<AiAnalysis>('ai_analyze_diff', { repoId, target, mode });
  },

  // P15c: summarize the commits/diff unique to `target` vs `base` (read-only prose).
  aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary> {
    return invoke<AiSummary>('ai_summarize_range', { repoId, base, target });
  },

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

  listStashes(repoId: string): Promise<StashEntry[]> {
    return invoke<StashEntry[]>('list_stashes', { repoId });
  },

  createStash(
    repoId: string,
    message: string | null,
    includeUntracked: boolean,
  ): Promise<CreateStashResult> {
    return invoke<CreateStashResult>('create_stash', { repoId, message, includeUntracked });
  },

  applyStash(repoId: string, index: number): Promise<ApplyStashOutcome> {
    return invoke<ApplyStashOutcome>('apply_stash', { repoId, index });
  },

  popStash(repoId: string, index: number): Promise<ApplyStashOutcome> {
    return invoke<ApplyStashOutcome>('pop_stash', { repoId, index });
  },

  dropStash(repoId: string, index: number): Promise<void> {
    return invoke<void>('drop_stash', { repoId, index });
  },

  getRecentRepos(): Promise<RecentRepo[]> {
    return invoke<RecentRepo[]>('get_recent_repos');
  },

  removeRecentRepo(path: string): Promise<RecentRepo[]> {
    return invoke<RecentRepo[]>('remove_recent_repo', { path });
  },

  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe> {
    return listen<RepoChangedPayload>('repo-changed', (e) => cb(e.payload));
  },

  onWindowFocus(cb: () => void): Promise<Unsubscribe> {
    return getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) cb();
    });
  },

  getUiSettings(): Promise<UiSettings> {
    return invoke<UiSettings>('get_ui_settings');
  },

  setUiSettings(patch: UiSettingsPatch): Promise<UiSettings> {
    return invoke<UiSettings>('set_ui_settings', { patch });
  },

  getSession(): Promise<SessionState> {
    return invoke<SessionState>('get_session');
  },

  setSession(session: SessionState): Promise<void> {
    return invoke<void>('set_session', { session });
  },

  // P16: embedded MCP server.
  setActiveRepo(repoId: string | null): Promise<void> {
    return invoke<void>('set_active_repo', { repoId });
  },

  getMcpStatus(): Promise<McpStatus> {
    return invoke<McpStatus>('get_mcp_status');
  },

  setMcpEnabled(enabled: boolean): Promise<McpStatus> {
    return invoke<McpStatus>('set_mcp_enabled', { enabled });
  },

  setMcpAllowWrite(allowWrite: boolean): Promise<McpStatus> {
    return invoke<McpStatus>('set_mcp_allow_write', { allowWrite });
  },

  onMcpServerChanged(cb: (s: McpStatus) => void): Promise<Unsubscribe> {
    return listen<McpStatus>('mcp-server-changed', (e) => cb(e.payload));
  },
};
