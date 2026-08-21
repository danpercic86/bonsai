import { invoke, Channel } from '@tauri-apps/api/core';
import type { BlameLine, FileHistoryEntry, HistoryAnswer, HistoryQuery, HistorySearchResults, IndexProgress, IndexStatus, ReflogEntry, SearchQuery, SearchResults, UndoPlan } from '../types';

export const historyCommands = {

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
};
