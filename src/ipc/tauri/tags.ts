import { invoke } from '@tauri-apps/api/core';
import type { TagAutoSyncReport, TagSyncReport } from '../types';

export const tagsCommands = {

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

  // P77: tag sync.
  listTagSync(repoId: string, remote: string | null): Promise<TagSyncReport> {
    return invoke<TagSyncReport>('list_tag_sync', { repoId, remote });
  },

  // P84: best-effort automatic tag reconciliation (adopt/move/skip).
  autoSyncTags(repoId: string, remote: string | null): Promise<TagAutoSyncReport> {
    return invoke<TagAutoSyncReport>('auto_sync_tags', { repoId, remote });
  },

  forceRefreshTag(repoId: string, remote: string, tagName: string): Promise<void> {
    return invoke<void>('force_refresh_tag', { repoId, remote, tagName });
  },

  deleteRemoteTag(repoId: string, remote: string, tagName: string): Promise<void> {
    return invoke<void>('delete_remote_tag', { repoId, remote, tagName });
  },
};
