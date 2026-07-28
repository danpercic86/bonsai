import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  BranchesSnapshot,
  CommitDiff,
  CommitResult,
  FetchResult,
  FileDiff,
  GraphLayout,
  IpcApi,
  PullResult,
  PushResult,
  RecentRepo,
  RepoChangedPayload,
  RepoInfo,
  StatusSnapshot,
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
};
