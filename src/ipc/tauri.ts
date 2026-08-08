import { invoke, Channel } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/plugin-dialog';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import type {
  AgentAsset,
  AgentAssetInput,
  AgentAssetInventory,
  AgentAssetKind,
  AiAnalysis,
  AiAnalysisMode,
  AiChangelog,
  AiDiffTarget,
  AiDigestRange,
  AiAvailability,
  AiAssetInventory,
  AiGeneratedAsset,
  AiResolveProposal,
  AiSummary,
  ApplyStashOutcome,
  AssetContent,
  BisectOutcome,
  ContextProfile,
  ProfileActivation,
  ProfilePreviewEntry,
  ProfileStore,
  BlameLine,
  BranchDeleteResult,
  BranchesSnapshot,
  BranchNameProposal,
  BranchNameSource,
  ChangelogRange,
  CloneProgress,
  CommitDiff,
  CommitMessageProposal,
  CherrypickOutcome,
  CommitResult,
  CompareDiff,
  ComposeApplyResult,
  ComposePlan,
  ComposeProposal,
  OperationPlan,
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
  ImageDiff,
  ImageDiffRequest,
  HistoryAnswer,
  HistoryQuery,
  HistorySearchResults,
  IndexProgress,
  IndexStatus,
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
  RenameBranchResult,
  RecentRepo,
  RemoteInfo,
  RepoChangedPayload,
  ReflogEntry,
  ConfigLevelArg,
  ConfigView,
  RepoHealth,
  RepoOpState,
  ResetMode,
  RevertOutcome,
  SearchQuery,
  SearchResults,
  SessionState,
  SigningStatus,
  StaleReport,
  StashEntry,
  StatusSnapshot,
  SubmoduleInfo,
  UiSettings,
  UiSettingsPatch,
  Unsubscribe,
  UndoPlan,
  UpdateCheckResult,
  UpdateProgress,
  VerifyResults,
  AppError,
  WorktreeContextStatus,
  WorktreeInfo,
  CopyCandidate,
  CopyPlanEntry,
  CopySelection,
} from './types';

// P42 (INV-1 / D1): the Tauri updater flow is stateful JS-side — `check()`
// returns an `Update` handle that must be held to call `.downloadAndInstall()`.
// We keep it in this module-level var between `checkForUpdate()` and
// `downloadAndInstallUpdate()` rather than bridging it across a Rust command.
let pendingUpdate: Update | null = null;

/** Maps a thrown updater/plugin error into a Bonsai `AppError`. A signature or
 *  manifest-parse failure is `updateFailed`; a genuine connectivity failure is
 *  `networkError`. */
function toUpdateAppError(e: unknown): AppError {
  const message = e instanceof Error ? e.message : String(e);
  const lower = message.toLowerCase();
  const isNetwork =
    lower.includes('network') ||
    lower.includes('connect') ||
    lower.includes('timed out') ||
    lower.includes('timeout') ||
    lower.includes('dns') ||
    lower.includes('unreachable') ||
    lower.includes('sending request');
  return { kind: isNetwork ? 'networkError' : 'updateFailed', message };
}

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

  commit(
    repoId: string,
    message: string,
    sign: boolean | null = null,
    skipHooks = false,
  ): Promise<CommitResult> {
    return invoke<CommitResult>('commit', { repoId, message, sign, skipHooks });
  },

  getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_workdir_file_diff', {
      repoId,
      path,
      origPath,
      staged,
      fullContext,
      intraline,
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
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_commit_file_diff', {
      repoId,
      oid,
      path,
      origPath,
      fullContext,
      intraline,
    });
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
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('compare_with_head_file_diff', {
      repoId,
      oid,
      path,
      origPath,
      fullContext,
      intraline,
    });
  },

  getImageDiff(repoId: string, request: ImageDiffRequest): Promise<ImageDiff> {
    return invoke<ImageDiff>('get_image_diff', { repoId, request });
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

  fetch(repoId: string): Promise<FetchResult> {
    return invoke<FetchResult>('fetch', { repoId });
  },

  pull(repoId: string): Promise<PullResult> {
    return invoke<PullResult>('pull', { repoId });
  },

  push(repoId: string, skipHooks = false): Promise<PushResult> {
    return invoke<PushResult>('push', { repoId, skipHooks });
  },

  forcePush(repoId: string, skipHooks = false): Promise<PushResult> {
    return invoke<PushResult>('force_push', { repoId, skipHooks });
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

  // P56a: grouped Markdown release notes for a tag/ref range (or since the last
  // tag). Read-only; WRITES NOTHING; does NOT emit repo-changed. Fully local.
  aiChangelog(repoId: string, range: ChangelogRange): Promise<AiChangelog> {
    return invoke<AiChangelog>('ai_changelog', { repoId, range });
  },

  // P53a: AI "why does this line exist" — blame-why (read-only prose).
  aiExplainLine(
    repoId: string,
    path: string,
    lineNo: number,
    atOid: string | null,
  ): Promise<AiAnalysis> {
    return invoke<AiAnalysis>('ai_explain_line', { repoId, path, lineNo, atOid });
  },

  // P15c: summarize the commits/diff unique to `target` vs `base` (read-only prose).
  aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary> {
    return invoke<AiSummary>('ai_summarize_range', { repoId, base, target });
  },

  // P53c: AI branch-name suggestions from a grounding source (read-only; writes nothing).
  aiSuggestBranchName(repoId: string, source: BranchNameSource): Promise<BranchNameProposal> {
    return invoke<BranchNameProposal>('ai_suggest_branch_name', { repoId, source });
  },

  // P54a: propose grouping the working-tree changes into logical commits (read-only).
  aiComposeCommits(repoId: string, guidance: string | null): Promise<ComposeProposal> {
    return invoke<ComposeProposal>('ai_compose_commits', { repoId, guidance });
  },

  // P55a: map a natural-language request to ONE allowlisted, previewable git op
  // (read-only; WRITES NOTHING). The mutation runs later via the resolved op's
  // existing typed command on explicit confirm (safeOpDispatch, P55c).
  aiPlanOperation(repoId: string, request: string): Promise<OperationPlan> {
    return invoke<OperationPlan>('ai_plan_operation', { repoId, request });
  },

  // P54b: apply a reviewed composer plan as an ordered stage+commit sequence (atomic).
  applyComposedCommits(repoId: string, plan: ComposePlan): Promise<ComposeApplyResult> {
    return invoke<ComposeApplyResult>('apply_composed_commits', { repoId, plan });
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

  // P23d: per-line blame + per-file commit history (read-only).
  blameFile(repoId: string, path: string, atOid: string | null): Promise<BlameLine[]> {
    return invoke<BlameLine[]>('blame_file', { repoId, path, atOid });
  },

  fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]> {
    return invoke<FileHistoryEntry[]>('file_history', { repoId, path, limit });
  },

  readReflog(repoId: string, refName: string): Promise<ReflogEntry[]> {
    return invoke<ReflogEntry[]>('read_reflog', { repoId, refName });
  },
  describeLastUndo(repoId: string): Promise<UndoPlan> {
    return invoke<UndoPlan>('describe_last_undo', { repoId });
  },

  searchCommits(repoId: string, query: SearchQuery): Promise<SearchResults> {
    return invoke<SearchResults>('search_commits', { repoId, query });
  },

  signingStatus(repoId: string): Promise<SigningStatus> {
    return invoke<SigningStatus>('signing_status', { repoId });
  },

  verifyCommits(repoId: string, oids: string[]): Promise<VerifyResults> {
    return invoke<VerifyResults>('verify_commits', { repoId, oids });
  },

  historyIndexBuild(
    repoId: string,
    onProgress: (p: IndexProgress) => void,
  ): Promise<IndexStatus> {
    const channel = new Channel<IndexProgress>();
    channel.onmessage = onProgress;
    // Tauri auto-serializes the Channel as the `on_progress` command argument
    // (mirrors cloneRepo).
    return invoke<IndexStatus>('history_index_build', { repoId, onProgress: channel });
  },

  historyIndexStatus(repoId: string): Promise<IndexStatus> {
    return invoke<IndexStatus>('history_index_status', { repoId });
  },

  historySearch(repoId: string, query: HistoryQuery): Promise<HistorySearchResults> {
    return invoke<HistorySearchResults>('history_search', { repoId, query });
  },

  aiSearchHistory(repoId: string, question: string, topK: number): Promise<HistoryAnswer> {
    return invoke<HistoryAnswer>('ai_search_history', { repoId, question, topK });
  },

  getConfig(repoId: string, level: ConfigLevelArg): Promise<ConfigView> {
    return invoke<ConfigView>('get_config', { repoId, level });
  },

  setConfig(repoId: string, level: ConfigLevelArg, key: string, value: string): Promise<void> {
    return invoke<void>('set_config', { repoId, level, key, value });
  },

  unsetConfig(repoId: string, level: ConfigLevelArg, key: string): Promise<void> {
    return invoke<void>('unset_config', { repoId, level, key });
  },

  applyIdentityProfile(
    repoId: string,
    userName: string,
    userEmail: string,
    signingKey: string | null,
  ): Promise<ConfigView> {
    return invoke<ConfigView>('apply_identity_profile', {
      repoId,
      userName,
      userEmail,
      signingKey,
    });
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

  applyStash(repoId: string, index: number, skipReserved: boolean): Promise<ApplyStashOutcome> {
    return invoke<ApplyStashOutcome>('apply_stash', { repoId, index, skipReserved });
  },

  popStash(repoId: string, index: number, skipReserved: boolean): Promise<ApplyStashOutcome> {
    return invoke<ApplyStashOutcome>('pop_stash', { repoId, index, skipReserved });
  },

  dropStash(repoId: string, index: number): Promise<void> {
    return invoke<void>('drop_stash', { repoId, index });
  },

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

  addSubmodule(repoId: string, url: string, path: string): Promise<SubmoduleInfo> {
    return invoke<SubmoduleInfo>('add_submodule', { repoId, url, path });
  },

  deinitSubmodule(repoId: string, name: string): Promise<void> {
    return invoke<void>('deinit_submodule', { repoId, name });
  },

  removeSubmodule(repoId: string, name: string): Promise<void> {
    return invoke<void>('remove_submodule', { repoId, name });
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

  openInTerminal(path: string): Promise<void> {
    return invoke<void>('open_in_terminal', { path });
  },

  revealInFileManager(path: string): Promise<void> {
    return invoke<void>('reveal_in_file_manager', { path });
  },

  openInEditor(path: string): Promise<void> {
    return invoke<void>('open_in_editor', { path });
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

  registerMcpWithClaude(scope: 'user' | 'local', repoPath: string | null): Promise<void> {
    return invoke('register_mcp_with_claude', { scope, repoPath });
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

  // P42: auto-update (INV-1 / D1). React talks ONLY to these wrappers; the JS
  // updater/process plugins are imported here, never in components.
  async checkForUpdate(): Promise<UpdateCheckResult> {
    const currentVersion = await getVersion();
    try {
      const u = await check();
      pendingUpdate = u;
      return {
        available: u !== null,
        currentVersion,
        version: u?.version ?? null,
        notes: u?.body ?? null,
        date: u?.date ?? null,
      };
    } catch (e) {
      throw toUpdateAppError(e);
    }
  },

  async downloadAndInstallUpdate(onProgress: (p: UpdateProgress) => void): Promise<void> {
    if (pendingUpdate === null) {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'No update found — call checkForUpdate first',
      };
      throw err;
    }
    let downloadedBytes = 0;
    let contentLength: number | null = null;
    try {
      await pendingUpdate.downloadAndInstall((evt) => {
        switch (evt.event) {
          case 'Started':
            contentLength = evt.data.contentLength ?? null;
            onProgress({ phase: 'started', downloadedBytes: 0, contentLength });
            break;
          case 'Progress':
            downloadedBytes += evt.data.chunkLength;
            onProgress({ phase: 'downloading', downloadedBytes, contentLength });
            break;
          case 'Finished':
            onProgress({ phase: 'finished', downloadedBytes, contentLength });
            break;
        }
      });
    } catch (e) {
      throw toUpdateAppError(e);
    }
  },

  relaunchApp(): Promise<void> {
    return relaunch();
  },
};
