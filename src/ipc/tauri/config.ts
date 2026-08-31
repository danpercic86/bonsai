import { invoke } from '@tauri-apps/api/core';
import type { ConfigLevelArg, ConfigView } from '../types';

export const configCommands = {

  getConfig(repoId: string, level: ConfigLevelArg): Promise<ConfigView> {
    return invoke<ConfigView>('get_config', { repoId, level });
  },

  setConfig(repoId: string, level: ConfigLevelArg, key: string, value: string): Promise<void> {
    return invoke<void>('set_config', { repoId, level, key, value });
  },

  unsetConfig(repoId: string, level: ConfigLevelArg, key: string): Promise<void> {
    return invoke<void>('unset_config', { repoId, level, key });
  },

  applyIdentityProfile(
    repoId: string,
    userName: string,
    userEmail: string,
    signingKey: string | null,
  ): Promise<ConfigView> {
    return invoke<ConfigView>('apply_identity_profile', {
      repoId,
      userName,
      userEmail,
      signingKey,
    });
  },
};
