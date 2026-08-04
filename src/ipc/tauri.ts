import { invoke, Channel } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  AgentAsset,
  AgentAssetInput,
  AgentAssetInventory,
  AgentAssetKind,
  AiAnalysis,
  AiAnalysisMode,
  AiDiffTarget,
  AiDigestRange,
  AiAvailability,
  AiAssetInventory,
  AiGeneratedAsset,
  AiResolveProposal,
  AiSummary,
  ApplyStashOutcome,
  AssetContent,
  ContextProfile,
  ProfileActivation,
  ProfilePreviewEntry,
  ProfileStore,
  BlameLine,
  BranchDeleteResult,
  BranchesSnapshot,
  CloneProgress,
  CommitDiff,
  CommitMessageProposal,
  CherrypickOutcome,
  CommitResult,
  CompareDiff,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  CheckoutResult,
  CreateBranchHereResult,
  CreateStashResult,
  StashScope,
  FetchResult,
  FileDiff,
  FileHistoryEntry,
  GraphLayout,
  IpcApi,
  JobKind,
  JobStatus,
  JobStatusChangedPayload,
  LineSelection,
  McpStatus,
  MergeOutcome,
  OpenRepoResult,
  PullResult,
  PushResult,
  RebaseOutcome,
  RebaseTodoOp,
  RecentRepo,
  RemoteInfo,
  RepoChangedPayload,
  RepoHealth,
  RepoOpState,
  ResetMode,
  RevertOutcome,
  SessionState,
  StaleReport,
  StashEntry,
  StatusSnapshot,
  SubmoduleInfo,
  UiSettings,
  UiSettingsPatch,
  Unsubscribe,
  WorktreeContextStatus,
  WorktreeInfo,
  CopyCandidate,
  CopyPlanEntry,
  CopySelection,
} from './types';

export const tauriIpc: IpcApi = {
  openRepo(path: string): Promise<OpenRepoResult> {
    return invoke<OpenRepoResult>('open_repo', { path });
  },

  cloneRepo(url: string, dest: string, onProgress: (p: CloneProgress) => void): Promise<string> {
    const channel = new Channel<CloneProgress>();
    channel.onmessage = onProgress;
    // Tauri auto-serializes the Channel as the `on_progress` command argument.
    return invoke<string>('clone_repo', { url, dest, onProgress: channel });
  },

  initRepo(path: string): Promise<string> {
    return invoke<string>('init_repo', { path });
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

  discardPartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    return invoke<void>('discard_partial', { repoId, path, origPath, selection });
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

  checkoutBranch(repoId: string, name: string): Promise<CheckoutResult> {
    return invoke<CheckoutResult>('checkout_branch', { repoId, name });
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

  listStaleBranches(repoId: string, base?: string): Promise<StaleReport> {
    return invoke<StaleReport>('list_stale_branches', { repoId, base });
  },

  deleteBranches(repoId: string, names: string[], base?: string): Promise<BranchDeleteResult[]> {
    return invoke<BranchDeleteResult[]>('delete_branches', { repoId, names, base });
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

  // P28: AI "what changed" digest over a selectable range (read-only prose).
  aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis> {
    return invoke<AiAnalysis>('ai_digest', { repoId, range });
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

  // P23d: per-line blame + per-file commit history (read-only).
  blameFile(repoId: string, path: string, atOid: string | null): Promise<BlameLine[]> {
    return invoke<BlameLine[]>('blame_file', { repoId, path, atOid });
  },

  fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]> {
    return invoke<FileHistoryEntry[]>('file_history', { repoId, path, limit });
  },

  listStashes(repoId: string): Promise<StashEntry[]> {
    return invoke<StashEntry[]>('list_stashes', { repoId });
  },

  createStash(
    repoId: string,
    message: string | null,
    scope: StashScope,
  ): Promise<CreateStashResult> {
    return invoke<CreateStashResult>('create_stash', { repoId, message, scope });
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

  commitAmend(repoId: string, message: string): Promise<CommitResult> {
    return invoke<CommitResult>('commit_amend', { repoId, message });
  },

  resetBranch(repoId: string, oid: string, mode: ResetMode): Promise<void> {
    return invoke<void>('reset_branch', { repoId, oid, mode });
  },

  discardPaths(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('discard_paths', { repoId, paths });
  },

  cherrypickCommit(repoId: string, oid: string): Promise<CherrypickOutcome> {
    return invoke<CherrypickOutcome>('cherrypick_commit', { repoId, oid });
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

  listSubmodules(repoId: string): Promise<SubmoduleInfo[]> {
    return invoke<SubmoduleInfo[]>('list_submodules', { repoId });
  },

  initSubmodule(repoId: string, name: string): Promise<void> {
    return invoke<void>('init_submodule', { repoId, name });
  },

  updateSubmodule(repoId: string, name: string): Promise<void> {
    return invoke<void>('update_submodule', { repoId, name });
  },

  syncSubmodule(repoId: string, name: string): Promise<void> {
    return invoke<void>('sync_submodule', { repoId, name });
  },

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

  // P29: repo health.
  getRepoHealth(repoId: string): Promise<RepoHealth> {
    return invoke<RepoHealth>('get_repo_health', { repoId });
  },

  // P22: tags.
  createTag(
    repoId: string,
    name: string,
    targetOid: string,
    message: string | null,
    force: boolean,
  ): Promise<void> {
    return invoke<void>('create_tag', { repoId, name, targetOid, message, force });
  },

  deleteTag(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_tag', { repoId, name });
  },

  pushTag(repoId: string, remote: string, tagName: string, force: boolean): Promise<void> {
    return invoke<void>('push_tag', { repoId, remote, tagName, force });
  },

  // P22: remotes.
  listRemotes(repoId: string): Promise<RemoteInfo[]> {
    return invoke<RemoteInfo[]>('list_remotes', { repoId });
  },

  addRemote(repoId: string, name: string, url: string): Promise<void> {
    return invoke<void>('add_remote', { repoId, name, url });
  },

  removeRemote(repoId: string, name: string): Promise<void> {
    return invoke<void>('remove_remote', { repoId, name });
  },

  renameRemote(repoId: string, name: string, newName: string): Promise<void> {
    return invoke<void>('rename_remote', { repoId, name, newName });
  },

  setRemoteUrl(repoId: string, name: string, url: string): Promise<void> {
    return invoke<void>('set_remote_url', { repoId, name, url });
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

  // P30: background-job scheduler.
  getJobStatus(repoId: string): Promise<JobStatus[]> {
    return invoke<JobStatus[]>('get_job_status', { repoId });
  },

  runJobNow(repoId: string, job: JobKind): Promise<void> {
    return invoke<void>('run_job_now', { repoId, job });
  },

  onJobStatusChanged(cb: (p: JobStatusChangedPayload) => void): Promise<Unsubscribe> {
    return listen<JobStatusChangedPayload>('job-status-changed', (e) => cb(e.payload));
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

  // P24: AI-asset inventory + drift.
  listAiAssets(repoId: string, canonical?: string): Promise<AiAssetInventory> {
    return invoke<AiAssetInventory>('list_ai_assets', { repoId, canonical });
  },

  readAiAsset(repoId: string, path: string): Promise<AssetContent> {
    return invoke<AssetContent>('read_ai_asset', { repoId, path });
  },

  // P26: agent-asset (skills / subagents / slash commands) read path.
  listAgentAssets(repoId: string): Promise<AgentAssetInventory> {
    return invoke<AgentAssetInventory>('list_agent_assets', { repoId });
  },

  readAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAsset> {
    return invoke<AgentAsset>('read_agent_asset', { repoId, kind, name });
  },

  saveAgentAsset(repoId: string, asset: AgentAssetInput): Promise<AgentAssetInventory> {
    return invoke<AgentAssetInventory>('save_agent_asset', { repoId, asset });
  },

  deleteAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAssetInventory> {
    return invoke<AgentAssetInventory>('delete_agent_asset', { repoId, kind, name });
  },

  // P24: context-profile store (CRUD + preview + activate).
  listProfiles(repoId: string): Promise<ProfileStore> {
    return invoke<ProfileStore>('list_profiles', { repoId });
  },

  saveProfile(repoId: string, profile: ContextProfile): Promise<ProfileStore> {
    return invoke<ProfileStore>('save_profile', { repoId, profile });
  },

  deleteProfile(repoId: string, name: string): Promise<ProfileStore> {
    return invoke<ProfileStore>('delete_profile', { repoId, name });
  },

  previewProfile(repoId: string, name: string): Promise<ProfilePreviewEntry[]> {
    return invoke<ProfilePreviewEntry[]>('preview_profile', { repoId, name });
  },

  activateProfile(repoId: string, name: string): Promise<ProfileActivation> {
    return invoke<ProfileActivation>('activate_profile', { repoId, name });
  },

  // P31: per-worktree AI contexts (matrix + worktree-targeted preview/activate).
  listWorktreeContexts(repoId: string): Promise<WorktreeContextStatus[]> {
    return invoke<WorktreeContextStatus[]>('list_worktree_contexts', { repoId });
  },

  previewWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfilePreviewEntry[]> {
    return invoke<ProfilePreviewEntry[]>('preview_worktree_profile', {
      repoId,
      worktreeKey,
      name,
    });
  },

  activateWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfileActivation> {
    return invoke<ProfileActivation>('activate_worktree_profile', {
      repoId,
      worktreeKey,
      name,
    });
  },

  // P24e: translate one instruction file into another agent's flavor. Writes
  // NOTHING — returns proposed text the user reviews before saving.
  aiGenerateAsset(
    repoId: string,
    sourceAssetId: string,
    targetAgent: string,
    guidance?: string,
  ): Promise<AiGeneratedAsset> {
    return invoke<AiGeneratedAsset>('ai_generate_asset', {
      repoId,
      sourceAssetId,
      targetAgent,
      guidance,
    });
  },
};
