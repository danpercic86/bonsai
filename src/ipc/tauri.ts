import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  CommitResult,
  GraphLayout,
  IpcApi,
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

  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe> {
    return listen<RepoChangedPayload>('repo-changed', (e) => cb(e.payload));
  },

  onWindowFocus(cb: () => void): Promise<Unsubscribe> {
    return getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) cb();
    });
  },
};
