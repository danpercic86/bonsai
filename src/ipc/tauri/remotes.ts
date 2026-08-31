import { invoke } from '@tauri-apps/api/core';
import type { FetchResult, PullResult, PushResult, RemoteInfo } from '../types';

export const remotesCommands = {

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
};
