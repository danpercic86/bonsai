/** T3.4 — worktrees.ts (shared-list lifecycle + copy plan) and submodules.ts
 *  (stateful transitions + #fail seam). The worktree list is module-shared
 *  across default repos, so these tests restore what they add/remove. */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { worktreeHandlers } from './worktrees';
import { submoduleHandlers } from './submodules';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('wt')));
  return repoId;
}

describe('worktrees', () => {
  it('lists the seeded rows; isCurrent falls back to the main row', async () => {
    const repoId = await openDefault();
    const rows = await run(worktreeHandlers.listWorktrees(repoId));
    expect(rows.length).toBeGreaterThanOrEqual(4);
    expect(rows.find((w) => w.isMain)?.isCurrent).toBe(true); // no path match → main
    expect(rows.find((w) => w.name === 'release-1.2')).toMatchObject({
      locked: true,
      lockReason: 'pinned for QA',
    });
  });

  it('addWorktree slugs the name, suffixes collisions, and pushes a row', async () => {
    const repoId = await openDefault();
    const added = await run(worktreeHandlers.addWorktree(repoId, 'topic/one', 'My Fancy WT!'));
    expect(added.name).toBe('My-Fancy-WT');
    expect(added.absPath).toContain('.worktrees/repo/My-Fancy-WT');
    try {
      // Same display name for a DIFFERENT branch → collision suffix -2.
      const second = await run(worktreeHandlers.addWorktree(repoId, 'topic/two', 'My Fancy WT!'));
      expect(second.name).toBe('My-Fancy-WT-2');
      await run(worktreeHandlers.removeWorktree(repoId, second.name));
    } finally {
      await run(worktreeHandlers.removeWorktree(repoId, added.name));
    }
  });

  it('guards: empty branch, duplicate branch, unslugable name', async () => {
    const repoId = await openDefault();
    expect((await runErr(worktreeHandlers.addWorktree(repoId, '  ', 'x'))).kind).toBe(
      'invalidName',
    );
    expect(
      (await runErr(worktreeHandlers.addWorktree(repoId, 'feature/login', 'dup'))).kind,
    ).toBe('git'); // feature-login already checks out that branch
    expect((await runErr(worktreeHandlers.addWorktree(repoId, 'ok', '...'))).kind).toBe(
      'invalidName',
    );
  });

  it('remove guards: main, locked, unknown; lock/unlock round-trips', async () => {
    const repoId = await openDefault();
    expect((await runErr(worktreeHandlers.removeWorktree(repoId, 'repo'))).kind).toBe('git');
    expect((await runErr(worktreeHandlers.removeWorktree(repoId, 'release-1.2'))).kind).toBe(
      'git',
    );
    expect((await runErr(worktreeHandlers.removeWorktree(repoId, 'ghost'))).kind).toBe('git');
    // lock → double-lock rejects → unlock → double-unlock rejects.
    await run(worktreeHandlers.lockWorktree(repoId, 'feature-login', '  qa hold  '));
    let rows = await run(worktreeHandlers.listWorktrees(repoId));
    expect(rows.find((w) => w.name === 'feature-login')).toMatchObject({
      locked: true,
      lockReason: 'qa hold',
    });
    expect(
      (await runErr(worktreeHandlers.lockWorktree(repoId, 'feature-login'))).kind,
    ).toBe('git');
    await run(worktreeHandlers.unlockWorktree(repoId, 'feature-login'));
    rows = await run(worktreeHandlers.listWorktrees(repoId));
    expect(rows.find((w) => w.name === 'feature-login')).toMatchObject({
      locked: false,
      lockReason: null,
    });
    expect(
      (await runErr(worktreeHandlers.unlockWorktree(repoId, 'feature-login'))).kind,
    ).toBe('git');
  });

  it('non-default fixtures refuse add and surface no copy candidates', async () => {
    const { repoId } = await run(repoHandlers.openRepo('/mock/t34-detached-wt'));
    expect((await runErr(worktreeHandlers.addWorktree(repoId, 'b', 'n'))).kind).toBe('git');
    expect(await run(worktreeHandlers.listCopyCandidates(repoId))).toEqual([]);
  });

  it('copy plan: src/staged-change.ts always conflicts, others clean', async () => {
    const repoId = await openDefault();
    const candidates = await run(worktreeHandlers.listCopyCandidates(repoId));
    expect(candidates.some((c) => c.path === 'src/staged-change.ts')).toBe(true);
    const plan = await run(
      worktreeHandlers.previewWorktreeCopy(repoId, 'topic/x', [
        'src/staged-change.ts',
        '.env.local',
      ]),
    );
    expect(plan).toEqual([
      { path: 'src/staged-change.ts', verdict: 'conflict' },
      { path: '.env.local', verdict: 'clean' },
    ]);
    expect(
      (await runErr(worktreeHandlers.previewWorktreeCopy(repoId, '  ', ['x']))).kind,
    ).toBe('branchNotFound');
  });
});

describe('submodules', () => {
  // P73: init means init + CHECKOUT, so upToDate is the intended outcome.
  it('lists seeded rows; init (= init + checkout) flips uninitialized → upToDate', async () => {
    const repoId = await openDefault();
    const subs = await run(submoduleHandlers.listSubmodules(repoId));
    expect(subs.length).toBeGreaterThan(0);
    const uninit = subs.find((s) => s.status === 'uninitialized');
    expect(uninit).toBeDefined();
    if (!uninit) return;
    await run(submoduleHandlers.initSubmodule(repoId, uninit.name));
    const after = await run(submoduleHandlers.listSubmodules(repoId));
    const row = after.find((s) => s.name === uninit.name);
    expect(row?.status).toBe('upToDate');
    expect(row?.wtOid).toBe(row?.indexOid);
  });

  it('update clears outOfSync; sync is a config no-op', async () => {
    const repoId = await openDefault();
    const subs = await run(submoduleHandlers.listSubmodules(repoId));
    const stale = subs.find((s) => s.status === 'outOfSync');
    expect(stale).toBeDefined();
    if (!stale) return;
    await run(submoduleHandlers.updateSubmodule(repoId, stale.name));
    const after = await run(submoduleHandlers.listSubmodules(repoId));
    expect(after.find((s) => s.name === stale.name)?.status).toBe('upToDate');
    await run(submoduleHandlers.syncSubmodule(repoId, stale.name)); // no throw
    expect((await runErr(submoduleHandlers.syncSubmodule('/nope', 'x'))).kind).toBe('noRepo');
  });

  it('add pushes an upToDate row; blank url/path rejects; #fail seam throws', async () => {
    const repoId = await openDefault();
    expect(
      (await runErr(submoduleHandlers.addSubmodule(repoId, ' ', 'libs/x'))).kind,
    ).toBe('invalidName');
    expect(
      (await runErr(submoduleHandlers.addSubmodule(repoId, 'https://x.git', 'libs/#fail'))).kind,
    ).toBe('git');
    const row = await run(
      submoduleHandlers.addSubmodule(repoId, 'https://example.com/x.git', 'libs/x'),
    );
    expect(row).toMatchObject({ name: 'libs/x', status: 'upToDate' });
    const after = await run(submoduleHandlers.listSubmodules(repoId));
    expect(after.some((s) => s.name === 'libs/x')).toBe(true);
  });

  it('deinit keeps the row (uninitialized, wtOid null); remove drops it', async () => {
    const repoId = await openDefault();
    await run(submoduleHandlers.addSubmodule(repoId, 'https://example.com/y.git', 'libs/y'));
    await run(submoduleHandlers.deinitSubmodule(repoId, 'libs/y'));
    let subs = await run(submoduleHandlers.listSubmodules(repoId));
    expect(subs.find((s) => s.name === 'libs/y')).toMatchObject({
      status: 'uninitialized',
      wtOid: null,
    });
    await run(submoduleHandlers.removeSubmodule(repoId, 'libs/y'));
    subs = await run(submoduleHandlers.listSubmodules(repoId));
    expect(subs.some((s) => s.name === 'libs/y')).toBe(false);
  });
});

// P73 §8.3: the init/update error + slow seams. Before P73 neither command had a
// reachable failure path in the harness, so the toast copy for the two backend
// refusals could not be verified anywhere.
describe('submodule seams (?submodule=…)', () => {
  async function withSeam<T>(seam: string, fn: () => Promise<T>): Promise<T> {
    window.history.replaceState({}, '', `/?submodule=${seam}`);
    try {
      return await fn();
    } finally {
      window.history.replaceState({}, '', '/');
    }
  }

  it('notEmpty / urlMismatch / auth reject update + init with the backend sentences', async () => {
    const repoId = await openDefault();
    const notEmpty = await withSeam('notEmpty', () =>
      runErr(submoduleHandlers.updateSubmodule(repoId, 'vendor/libcore')),
    );
    expect(notEmpty.kind).toBe('git');
    expect(notEmpty.message).toBe(
      "The folder already has files in it. Move or delete everything inside 'vendor/libcore', then try again.",
    );

    const mismatch = await withSeam('urlMismatch', () =>
      runErr(submoduleHandlers.updateSubmodule(repoId, 'vendor/libcore')),
    );
    expect(mismatch.kind).toBe('git');
    expect(mismatch.message).toContain('Bonsai has cached data for a different remote URL');
    expect(mismatch.message).toContain('Run Sync on this submodule, then try again.');

    const auth = await withSeam('auth', () =>
      runErr(submoduleHandlers.initSubmodule(repoId, 'vendor/libcore')),
    );
    expect(auth.kind).toBe('authFailed');

    // No seam mutates state — the row is exactly as seeded.
    const subs = await run(submoduleHandlers.listSubmodules(repoId));
    expect(subs.find((s) => s.name === 'vendor/libcore')?.status).toBe('uninitialized');
  });

  it('fail also covers sync; slow eventually succeeds', async () => {
    const repoId = await openDefault();
    const failed = await withSeam('fail', () =>
      runErr(submoduleHandlers.syncSubmodule(repoId, 'docs/spec')),
    );
    expect(failed.kind).toBe('git');
    await withSeam('slow', () => run(submoduleHandlers.updateSubmodule(repoId, 'docs/spec')));
    const subs = await run(submoduleHandlers.listSubmodules(repoId));
    expect(subs.find((s) => s.name === 'docs/spec')?.status).toBe('upToDate');
  });
});
