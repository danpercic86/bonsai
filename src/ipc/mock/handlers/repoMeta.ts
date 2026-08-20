// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { mockRepoHealth } from '../../fixtures/repoHealth';
import { delay, requireRepo, throwAuthFailed, throwNetworkError } from '../repoState';
import type { AppError, RemoteInfo, RepoHealth } from '../../types';
import { applyTagDeleteLocalToSync, applyTagPushToSync } from './tagSync';

export const repoMetaHandlers = {
  async getRepoHealth(repoId: string): Promise<RepoHealth> {
    await delay(300);
    requireRepo(repoId);
    const health = mockRepoHealth();
    if (repoId.endsWith('-err')) {
      health.stats = { data: null, error: 'simulated slow scan failed', elapsedMs: 1500 };
    }
    return health;
  },

  // Stateful tags mock (P22 §5.3). create/delete mutate state.branches.tags so
  // the sidebar Tags section reflects them after refetchBranches; push is a
  // no-op success (honoring the `?remote=` failure triggers).
  async createTag(
    repoId: string,
    name: string,
    _targetOid: string,
    _message: string | null,
    force: boolean,
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (!force && state.branches.tags.includes(name)) {
      const err: AppError = { kind: 'git', message: `tag '${name}' already exists` };
      throw err;
    }
    if (!state.branches.tags.includes(name)) {
      state.branches.tags.push(name);
      state.branches.tags.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
    }
  },

  async deleteTag(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    state.branches.tags = state.branches.tags.filter((t) => t !== name);
    // Keep any live tag-sync report consistent: the local side is gone.
    applyTagDeleteLocalToSync(repoId, name);
  },

  async pushTag(repoId: string, remote: string, tagName: string, _force: boolean): Promise<void> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'rejected') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          'push rejected by remote. Bonsai v1 never force-pushes — fetch/pull first.',
      };
      throw err;
    }
    // Local-only mock: no server, so the push simply succeeds. Reflect it into
    // the live tag-sync report so push-unpushed and force-move both flip the row
    // to `in-sync` on the next listTagSync — mirroring real IPC.
    applyTagPushToSync(repoId, remote, tagName);
  },

  // Stateful remotes mock (P22 §5.3). list/add/remove/rename/set-url mutate a
  // per-repo remotes list; dup/missing throw the appropriate AppError.
  async listRemotes(repoId: string): Promise<RemoteInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.remotes);
  },

  async addRemote(repoId: string, name: string, url: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.remotes.some((r) => r.name === name)) {
      const err: AppError = { kind: 'git', message: `remote '${name}' already exists` };
      throw err;
    }
    state.remotes.push({ name, url });
    state.remotes.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  async removeRemote(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (!state.remotes.some((r) => r.name === name)) {
      const err: AppError = { kind: 'noRemote', message: `remote '${name}' not found` };
      throw err;
    }
    state.remotes = state.remotes.filter((r) => r.name !== name);
    // Mirror libgit2's tracking-ref cleanup: drop `<name>/*` remote-tracking rows.
    state.branches.remote = state.branches.remote.filter(
      (r) => !r.name.startsWith(`${name}/`),
    );
  },

  async renameRemote(repoId: string, name: string, newName: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.remotes.find((r) => r.name === name);
    if (entry === undefined) {
      const err: AppError = { kind: 'noRemote', message: `remote '${name}' not found` };
      throw err;
    }
    if (state.remotes.some((r) => r.name === newName)) {
      const err: AppError = { kind: 'git', message: `remote '${newName}' already exists` };
      throw err;
    }
    entry.name = newName;
    state.remotes.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    // Move the remote-tracking refs `<name>/…` → `<newName>/…`.
    for (const r of state.branches.remote) {
      if (r.name === name || r.name.startsWith(`${name}/`)) {
        r.name = `${newName}${r.name.slice(name.length)}`;
      }
    }
    state.branches.remote.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  async setRemoteUrl(repoId: string, name: string, url: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.remotes.find((r) => r.name === name);
    if (entry === undefined) {
      const err: AppError = { kind: 'noRemote', message: `remote '${name}' not found` };
      throw err;
    }
    entry.url = url;
  },

} satisfies Partial<IpcApi>;
