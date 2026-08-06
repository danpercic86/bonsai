// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { buildConfigView, validateEnumOrThrow, validateKeyOrThrow } from '../../fixtures/config';
import { delay, requireRepo } from '../repoState';
import type { ConfigLevelArg, ConfigView } from '../../types';

export const configHandlers = {
  async getConfig(repoId: string, level: ConfigLevelArg): Promise<ConfigView> {
    await delay(80);
    const state = requireRepo(repoId);
    return buildConfigView(state.config, level);
  },

  async setConfig(
    repoId: string,
    level: ConfigLevelArg,
    key: string,
    value: string,
  ): Promise<void> {
    await delay(80);
    const state = requireRepo(repoId);
    validateKeyOrThrow(key);
    validateEnumOrThrow(key, value);
    state.config[level][key.trim()] = value.trim();
  },

  async unsetConfig(repoId: string, level: ConfigLevelArg, key: string): Promise<void> {
    await delay(80);
    const state = requireRepo(repoId);
    validateKeyOrThrow(key);
    delete state.config[level][key.trim()];
  },

  // P44: apply an identity (live in-memory profile fields, NOT a persisted id)
  // to the repo's Local config store, then return the refreshed Local view —
  // full round-trip in the harness. Signing key is written only when set +
  // non-empty (mirrors the Rust core fn); an empty key leaves any existing one
  // untouched.
  async applyIdentityProfile(
    repoId: string,
    userName: string,
    userEmail: string,
    signingKey: string | null,
  ): Promise<ConfigView> {
    await delay(120);
    const state = requireRepo(repoId);
    state.config.local['user.name'] = userName.trim();
    state.config.local['user.email'] = userEmail.trim();
    if (signingKey && signingKey.trim() !== '') {
      state.config.local['user.signingkey'] = signingKey.trim();
    }
    return buildConfigView(state.config, 'local');
  },

  // Stateful stash mock (P9 §6.5). Indices are positional into the mutating
  // stack: every create/pop/drop re-indexes so index 0 stays the most recent.
} satisfies Partial<IpcApi>;
