// P60d: submodule handlers split out of worktrees.ts into their own domain
// group (list/init/update/sync from P19 + add/deinit/remove from P60d). The
// mock submodule list is stateful per repo (state.submodules), so add pushes a
// row, deinit flips it to uninitialized, and remove drops it — the sidebar
// section reflects every op after the container's refetch.
import type { IpcApi } from '../../types';
import type { AppError, SubmoduleInfo } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { delay, query, requireRepo } from '../repoState';

/** Harness fail-seam: a `#fail` in the id or `?submodule=fail` → a git error,
 *  so the toast/error path is reachable without a real repo. */
function failSeam(id: string): void {
  if (id.includes('#fail') || query('submodule') === 'fail') {
    const err: AppError = { kind: 'git', message: 'Mock: submodule operation failed' };
    throw err;
  }
}

export const submoduleHandlers = {
  async listSubmodules(repoId: string): Promise<SubmoduleInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.submodules);
  },

  async initSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const sub = state.submodules.find((s) => s.name === name);
    // Unknown name → no-op (the mock list is authoritative; unreachable from UI).
    if (sub !== undefined && sub.status === 'uninitialized') {
      sub.status = 'upToDate';
      sub.wtOid = sub.indexOid;
    }
  },

  async updateSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const sub = state.submodules.find((s) => s.name === name);
    // Init-then-update semantics — clears uninitialized / outOfSync.
    if (sub !== undefined) {
      sub.status = 'upToDate';
      sub.wtOid = sub.indexOid;
    }
  },

  async syncSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    // Sync mutates config (URL propagation), not the listed fields — no-op here.
    // Validate the repo is open (mirrors the real command surface).
    void name;
    requireRepo(repoId);
  },

  // P60d: add via git2 (clones) → push a new row. Blank url/path → invalidName.
  async addSubmodule(repoId: string, url: string, path: string): Promise<SubmoduleInfo> {
    await delay(200);
    const state = requireRepo(repoId);
    if (url.trim() === '' || path.trim() === '') {
      const err: AppError = { kind: 'invalidName', message: 'submodule url/path is empty' };
      throw err;
    }
    failSeam(path);
    const oid = randomOid();
    const row: SubmoduleInfo = {
      name: path,
      path,
      absPath: `${state.path}/${path}`,
      url,
      headOid: oid,
      indexOid: oid,
      wtOid: oid,
      status: 'upToDate',
    };
    state.submodules.push(row);
    return structuredClone(row);
  },

  // P60d: deinit — flip to uninitialized + null the worktree oid; keep the row.
  async deinitSubmodule(repoId: string, name: string): Promise<void> {
    await delay(200);
    const state = requireRepo(repoId);
    failSeam(name);
    const sub = state.submodules.find((s) => s.name === name);
    if (sub !== undefined) {
      sub.status = 'uninitialized';
      sub.wtOid = null;
    }
  },

  // P60d: remove — drop the row entirely (destructive).
  async removeSubmodule(repoId: string, name: string): Promise<void> {
    await delay(200);
    const state = requireRepo(repoId);
    failSeam(name);
    const idx = state.submodules.findIndex((s) => s.name === name);
    if (idx !== -1) state.submodules.splice(idx, 1);
  },
} satisfies Partial<IpcApi>;
