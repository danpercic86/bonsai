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
  PullResult,
  PushResult,
  RecentRepo,
  RepoChangedPayload,
  RepoInfo,
  RepoOpState,
  StatusSnapshot,
  UiSettings,
  UiSettingsPatch,
  Unsubscribe,
} from './types';

export const tauriIpc: IpcApi = {
  openRepo(path: string): Promise<RepoInfo> {
    return invoke<RepoInfo>('open_repo', { path });
  },

  async pickFolder(): Promise<string | null> {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Open repository',
    });
    return typeof selected === 'string' ? selected : null;
  },

  getStatus(): Promise<StatusSnapshot> {
    return invoke<StatusSnapshot>('get_status');
  },

  getGraph(): Promise<GraphLayout> {
    return invoke<GraphLayout>('get_graph');
  },

  stage(paths: string[]): Promise<void> {
    return invoke<void>('stage', { paths });
  },

  unstage(paths: string[]): Promise<void> {
    return invoke<void>('unstage', { paths });
  },

  commit(message: string): Promise<CommitResult> {
    return invoke<CommitResult>('commit', { message });
  },

  getWorkdirFileDiff(path: string, origPath: string | null, staged: boolean): Promise<FileDiff> {
    return invoke<FileDiff>('get_workdir_file_diff', { path, origPath, staged });
  },

  getCommitDiff(oid: string): Promise<CommitDiff> {
    return invoke<CommitDiff>('get_commit_diff', { oid });
  },

  getCommitFileDiff(oid: string, path: string, origPath: string | null): Promise<FileDiff> {
    return invoke<FileDiff>('get_commit_file_diff', { oid, path, origPath });
  },

  listBranches(): Promise<BranchesSnapshot> {
    return invoke<BranchesSnapshot>('list_branches');
  },

  createBranch(name: string): Promise<void> {
    return invoke<void>('create_branch', { name });
  },

  checkoutBranch(name: string): Promise<void> {
    return invoke<void>('checkout_branch', { name });
  },

  deleteBranch(name: string): Promise<void> {
    return invoke<void>('delete_branch', { name });
  },

  fetch(): Promise<FetchResult> {
    return invoke<FetchResult>('fetch');
  },

  pull(): Promise<PullResult> {
    return invoke<PullResult>('pull');
  },

  push(): Promise<PushResult> {
    return invoke<PushResult>('push');
  },

  getOpState(): Promise<RepoOpState> {
    return invoke<RepoOpState>('get_op_state');
  },

  mergeBranch(name: string): Promise<MergeOutcome> {
    return invoke<MergeOutcome>('merge_branch', { name });
  },

  commitMerge(message: string): Promise<CommitResult> {
    return invoke<CommitResult>('commit_merge', { message });
  },

  abortMerge(): Promise<void> {
    return invoke<void>('abort_merge');
  },

  listConflicts(): Promise<ConflictEntry[]> {
    return invoke<ConflictEntry[]>('list_conflicts');
  },

  getConflict(path: string): Promise<ConflictFile> {
    return invoke<ConflictFile>('get_conflict', { path });
  },

  resolveConflict(path: string, resolution: ConflictResolution): Promise<void> {
    return invoke<void>('resolve_conflict', { path, resolution });
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
};
