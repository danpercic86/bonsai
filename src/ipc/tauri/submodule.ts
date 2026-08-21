import { invoke } from '@tauri-apps/api/core';
import type { SubmoduleDeinitOutcome, SubmoduleInfo, SubmoduleRemoveOutcome } from '../types';

export const submoduleCommands = {

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

  deinitSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleDeinitOutcome> {
    return invoke<SubmoduleDeinitOutcome>('deinit_submodule', { repoId, name, force });
  },

  removeSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleRemoveOutcome> {
    return invoke<SubmoduleRemoveOutcome>('remove_submodule', { repoId, name, force });
  },
};
