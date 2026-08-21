import { invoke } from '@tauri-apps/api/core';
import type { ApplyStashOutcome, CreateStashResult, StashEntry, StashScope } from '../types';

export const stashCommands = {

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

  applyStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
    expectedOid?: string,
  ): Promise<ApplyStashOutcome> {
    return invoke<ApplyStashOutcome>('apply_stash', { repoId, index, skipReserved, expectedOid });
  },

  popStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
    expectedOid?: string,
  ): Promise<ApplyStashOutcome> {
    return invoke<ApplyStashOutcome>('pop_stash', { repoId, index, skipReserved, expectedOid });
  },

  dropStash(repoId: string, index: number, expectedOid?: string): Promise<void> {
    return invoke<void>('drop_stash', { repoId, index, expectedOid });
  },
};
