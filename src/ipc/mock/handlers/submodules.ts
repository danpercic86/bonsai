// P60d: submodule handlers split out of worktrees.ts into their own domain
// group (list/init/update/sync from P19 + add/deinit/remove from P60d). The
// mock submodule list is stateful per repo (state.submodules), so add pushes a
// row, deinit flips it to uninitialized, and remove drops it — the sidebar
// section reflects every op after the container's refetch.
import type { IpcApi } from '../../types';
import type {
  AppError,
  SubmoduleDeinitOutcome,
  SubmoduleInfo,
  SubmoduleRemoveOutcome,
} from '../../types';
import { randomOid } from '../../fixtures/oids';
import { delay, query, requireRepo } from '../repoState';

/** P73 §8.3 harness seams: `#fail` in the id or `?submodule=<seam>` drives the
 *  error/slow paths of every submodule op without a real repo. `notEmpty` and
 *  `urlMismatch` mirror the two backend refusals verbatim so the composed toast
 *  (`Couldn't check out <name>. <sentence>`) is verifiable in the harness. */
function failSeam(name: string): void {
  if (name.includes('#fail') || query('submodule') === 'fail') {
    const err: AppError = {
      kind: 'git',
      message: 'Mock: submodule operation failed.',
    };
    throw err;
  }
}

/** The `urlMismatch` refusal body, mirrored verbatim from the backend (see
 *  submodule_reconnect.rs msg_url_mismatch). Exported so the P74 toast harness
 *  seam composes the REAL sentence instead of inventing prose — keep the two in
 *  step by editing only here. */
export function msgUrlMismatch(name: string, url: string): string {
  return `Bonsai has cached data for a different remote URL ('https://example.com/old-${name}.git' instead of '${url}'). Run Sync on this submodule, then try again.`;
}

/** The `notEmpty` refusal body — the real backend interpolates the submodule
 *  PATH here (submodule_reconnect.rs msg_dirty_workdir), which differs from the
 *  name for a renamed submodule; the mock must not diverge. */
export function msgDirtyWorkdir(path: string): string {
  return `The folder already has files in it. Move or delete everything inside '${path}', then try again.`;
}

/** P82: dirty := a `modifiedWorkdir` fixture row OR the `?submodule=dirty` seam.
 *  Drives the deinit/remove `dirtyNeedsForce` refusal so the force-escalation
 *  dialog + retry are verifiable in a plain browser. `SubmoduleInfo.status` only
 *  carries the classified enum, so the seam also covers the "outOfSync AND dirty"
 *  case the backend still treats as dirty but the mock row cannot represent. */
function submoduleDirty(sub: SubmoduleInfo | undefined): boolean {
  return sub?.status === 'modifiedWorkdir' || query('submodule') === 'dirty';
}

/** init/update/sync additionally honour the P73 refusal + slow seams. */
async function submoduleSeam(name: string, sub?: SubmoduleInfo): Promise<void> {
  const seam = query('submodule');
  failSeam(name);
  if (seam === 'notEmpty') {
    const err: AppError = {
      kind: 'git',
      message: msgDirtyWorkdir(sub?.path ?? name),
    };
    throw err;
  }
  if (seam === 'urlMismatch') {
    const err: AppError = {
      kind: 'git',
      message: msgUrlMismatch(name, sub?.url ?? name),
    };
    throw err;
  }
  if (seam === 'auth') {
    const err: AppError = {
      kind: 'authFailed',
      message: `Authentication failed for ${sub?.url ?? 'the submodule remote'}.`,
    };
    throw err;
  }
  // The only way to observe the P73 §6 busy badge + the header sweep.
  if (seam === 'slow') await delay(4000);
}

export const submoduleHandlers = {
  async listSubmodules(repoId: string): Promise<SubmoduleInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.submodules);
  },

  // P73: init means init + CHECKOUT, so flipping to upToDate is the intended
  // semantics — this is no longer a mock/backend divergence. Note the UI no
  // longer calls this command; handleInitSubmodule invokes updateSubmodule.
  async initSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const sub = state.submodules.find((s) => s.name === name);
    await submoduleSeam(name, sub);
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
    await submoduleSeam(name, sub);
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
    const state = requireRepo(repoId);
    await submoduleSeam(
      name,
      state.submodules.find((s) => s.name === name),
    );
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

  // P60d/P82: deinit — flip to uninitialized + null the worktree oid; keep the
  // row. `force=false` on a dirty submodule refuses (`dirtyNeedsForce`, zero
  // mutation) so the UI escalation dialog + force retry are browser-verifiable.
  async deinitSubmodule(
    repoId: string,
    name: string,
    force: boolean,
  ): Promise<SubmoduleDeinitOutcome> {
    await delay(200);
    const state = requireRepo(repoId);
    failSeam(name);
    const sub = state.submodules.find((s) => s.name === name);
    if (!force && submoduleDirty(sub)) return { kind: 'dirtyNeedsForce' }; // zero mutation
    if (sub !== undefined) {
      sub.status = 'uninitialized';
      sub.wtOid = null;
    }
    return { kind: 'deinitialized' };
  },

  // P60d/P82: remove — drop the row entirely (destructive). Force semantics as
  // deinit: refuse a dirty submodule unless force=true.
  async removeSubmodule(
    repoId: string,
    name: string,
    force: boolean,
  ): Promise<SubmoduleRemoveOutcome> {
    await delay(200);
    const state = requireRepo(repoId);
    failSeam(name);
    const sub = state.submodules.find((s) => s.name === name);
    if (!force && submoduleDirty(sub)) return { kind: 'dirtyNeedsForce' }; // zero mutation
    const idx = state.submodules.findIndex((s) => s.name === name);
    if (idx !== -1) state.submodules.splice(idx, 1);
    return { kind: 'removed' };
  },
} satisfies Partial<IpcApi>;
