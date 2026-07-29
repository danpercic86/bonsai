import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  BranchesSnapshot,
  CommitDiff,
  CommitResult,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  FetchResult,
  FileDiff,
  GraphLayout,
  IpcApi,
  MergeOutcome,
  OpenRepoResult,
  PullResult,
  PushResult,
  RebaseOutcome,
  RecentRepo,
  RepoChangedPayload,
  RepoOpState,
  SessionState,
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
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_workdir_file_diff', { repoId, path, origPath, staged });
  },

  getCommitDiff(repoId: string, oid: string): Promise<CommitDiff> {
    return invoke<CommitDiff>('get_commit_diff', { repoId, oid });
  },

  getCommitFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_commit_file_diff', { repoId, oid, path, origPath });
  },

  listBranches(repoId: string): Promise<BranchesSnapshot> {
    return invoke<BranchesSnapshot>('list_branches', { repoId });
  },

  createBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('create_branch', { repoId, name });
  },

  checkoutBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('checkout_branch', { repoId, name });
  },

  deleteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_branch', { repoId, name });
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
};
