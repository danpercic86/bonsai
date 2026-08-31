import { invoke, Channel } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { CloneProgress, GitAvailability, GraphChunk, GraphLayout, OpenRepoResult, RepoHealth, StatusSnapshot } from '../types';

export const repoCommands = {
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

  streamGraph(repoId: string, onChunk: (c: GraphChunk) => void): Promise<void> {
    const channel = new Channel<GraphChunk>();
    channel.onmessage = onChunk;
    // Tauri auto-serializes the Channel as the `on_chunk` command argument
    // (mirrors historyIndexBuild / cloneRepo).
    return invoke<void>('stream_graph', { repoId, onChunk: channel });
  },

  // P70: git executable preflight (never rejects for git state).
  checkGitAvailability(): Promise<GitAvailability> {
    return invoke<GitAvailability>('check_git_availability');
  },

  // P29: repo health.
  getRepoHealth(repoId: string): Promise<RepoHealth> {
    return invoke<RepoHealth>('get_repo_health', { repoId });
  },
};
