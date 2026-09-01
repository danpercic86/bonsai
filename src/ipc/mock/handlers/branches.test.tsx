/** T3.4 — branches.ts: create/checkout/rename/delete reflected in
 *  listBranches + HEAD; dirty-safe checkout outcomes; remote-branch checkout /
 *  delete; stale-branch cleanup safety rules; resetBranch semantics. */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { branchHandlers } from './branches';
import { statusHandlers } from './status';
import { resetRevertHandlers } from './resetRevert';
import { requireRepo } from '../repoState';
import { MOCK_OID } from '../../fixtures/branches';
import type { HeadInfo } from '../../types';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('br')));
  return repoId;
}

/** Clears the seeded dirty status so checkouts take the clean path. */
function makeClean(repoId: string): void {
  const s = requireRepo(repoId);
  s.status.staged = [];
  s.status.unstaged = [];
  s.status.untracked = [];
  s.status.conflicted = [];
}

describe('createBranch', () => {
  it('adds a sorted, non-HEAD local entry visible via listBranches', async () => {
    const repoId = await openDefault();
    await run(branchHandlers.createBranch(repoId, '  topic/new  '));
    const snap = await run(branchHandlers.listBranches(repoId));
    const entry = snap.local.find((b) => b.name === 'topic/new');
    expect(entry).toMatchObject({ isHead: false, upstream: null });
    const names = snap.local.map((b) => b.name.toLowerCase());
    expect(names).toEqual([...names].sort((a, b) => a.localeCompare(b)));
  });

  it('rejects invalid names and duplicates', async () => {
    const repoId = await openDefault();
    expect((await runErr(branchHandlers.createBranch(repoId, 'bad name'))).kind).toBe(
      'invalidName',
    );
    expect((await runErr(branchHandlers.createBranch(repoId, 'main'))).kind).toBe('branchExists');
  });
});

describe('checkoutBranch', () => {
  it('clean tree: moves HEAD; snapshot head + isHead flags follow', async () => {
    const repoId = await openDefault();
    makeClean(repoId);
    const result = await run(branchHandlers.checkoutBranch(repoId, 'feature/sidebar'));
    expect(result).toEqual({ stashed: false, fastForwarded: false, apply: null });
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.head.branchName).toBe('feature/sidebar');
    expect(snap.local.filter((b) => b.isHead).map((b) => b.name)).toEqual(['feature/sidebar']);
  });

  it('auto fast-forwards a strictly-behind tracking branch (feature/merged-a)', async () => {
    const repoId = await openDefault();
    makeClean(repoId);
    const result = await run(branchHandlers.checkoutBranch(repoId, 'feature/merged-a'));
    expect(result.fastForwarded).toBe(true);
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.local.find((b) => b.name === 'feature/merged-a')?.behind).toBe(0);
  });

  it('dirty tree: carries work across (stashed:true, apply applied)', async () => {
    const repoId = await openDefault();
    const result = await run(branchHandlers.checkoutBranch(repoId, 'feature/sidebar'));
    expect(result.stashed).toBe(true);
    expect(result.apply).toEqual({ kind: 'applied' });
  });

  it('fix/watcher-debounce is the designated conflicted re-apply fixture', async () => {
    const repoId = await openDefault();
    const result = await run(branchHandlers.checkoutBranch(repoId, 'fix/watcher-debounce'));
    expect(result.apply).toEqual({ kind: 'conflicts', paths: ['src/app.ts'] });
    const status = await run(statusHandlers.getStatus(repoId));
    expect(status.conflicted.some((e) => e.path === 'src/app.ts')).toBe(true);
  });

  it('unknown branch → branchNotFound; __wt_locked__ → branchCheckedOutElsewhere', async () => {
    const repoId = await openDefault();
    expect((await runErr(branchHandlers.checkoutBranch(repoId, 'nope'))).kind).toBe(
      'branchNotFound',
    );
    expect((await runErr(branchHandlers.checkoutBranch(repoId, '__wt_locked__'))).kind).toBe(
      'branchCheckedOutElsewhere',
    );
  });
});

describe('deleteBranch', () => {
  it('removes a non-HEAD branch; guards HEAD, unknown, and unmerged', async () => {
    const repoId = await openDefault();
    await run(branchHandlers.createBranch(repoId, 'doomed'));
    await run(branchHandlers.deleteBranch(repoId, 'doomed'));
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.local.some((b) => b.name === 'doomed')).toBe(false);
    expect((await runErr(branchHandlers.deleteBranch(repoId, 'main'))).kind).toBe('git');
    expect((await runErr(branchHandlers.deleteBranch(repoId, 'nope'))).kind).toBe(
      'branchNotFound',
    );
    expect(
      (await runErr(branchHandlers.deleteBranch(repoId, 'experiment-unmerged'))).kind,
    ).toBe('unmergedBranch');
  });
});

describe('renameBranch', () => {
  it('renames in place preserving tracking; moves HEAD when current', async () => {
    const repoId = await openDefault();
    const result = await run(branchHandlers.renameBranch(repoId, 'main', 'trunk'));
    expect(result).toEqual({ wasHead: true, upstream: 'origin/main' });
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.head.branchName).toBe('trunk');
    expect(snap.local.some((b) => b.name === 'main')).toBe(false);
    expect(snap.local.find((b) => b.name === 'trunk')).toMatchObject({
      isHead: true,
      upstream: 'origin/main',
    });
  });

  it('error order: invalidName → branchNotFound → branchExists', async () => {
    const repoId = await openDefault();
    expect((await runErr(branchHandlers.renameBranch(repoId, 'nope', 'bad name'))).kind).toBe(
      'invalidName',
    );
    expect((await runErr(branchHandlers.renameBranch(repoId, 'nope', 'fine'))).kind).toBe(
      'branchNotFound',
    );
    expect((await runErr(branchHandlers.renameBranch(repoId, 'feat', 'main'))).kind).toBe(
      'branchExists',
    );
  });
});

describe('remote-tracking branches', () => {
  it('checkoutRemoteBranch creates a local tracking branch and switches to it', async () => {
    const repoId = await openDefault();
    // origin/release is the fixture's remote with NO matching local.
    await run(branchHandlers.checkoutRemoteBranch(repoId, 'origin/release'));
    const snap = await run(branchHandlers.listBranches(repoId));
    const local = snap.local.find((b) => b.name === 'release');
    expect(local).toMatchObject({
      isHead: true,
      upstream: 'origin/release',
      tip: '1'.repeat(40),
      ahead: 0,
      behind: 0,
    });
    expect(snap.head.branchName).toBe('release');
    // Unknown remote-tracking ref rejects cleanly.
    expect(
      (await runErr(branchHandlers.checkoutRemoteBranch(repoId, 'origin/nope'))).kind,
    ).toBe('branchNotFound');
  });

  it('deleteRemoteBranch drops only the remote-tracking ref; unknown rejects', async () => {
    const repoId = await openDefault();
    const snap0 = await run(branchHandlers.listBranches(repoId));
    const victim = snap0.remote[0];
    await run(branchHandlers.deleteRemoteBranch(repoId, victim.name));
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.remote.some((r) => r.name === victim.name)).toBe(false);
    expect(snap.local.length).toBe(snap0.local.length);
    expect((await runErr(branchHandlers.deleteRemoteBranch(repoId, 'origin/nope'))).kind).toBe(
      'branchNotFound',
    );
  });
});

describe('stale-branch cleanup (deleteBranches safety rules)', () => {
  it('deletes only verified-stale names; skips current/base/not-stale', async () => {
    const repoId = await openDefault();
    const results = await run(
      branchHandlers.deleteBranches(repoId, [
        'main',
        'feature/merged-a',
        'experiment-unmerged',
        'feature/gone',
      ]),
    );
    expect(results).toEqual([
      // 'main' is both base AND current; the current-branch guard fires first.
      { name: 'main', status: 'skippedCurrent', message: 'checked-out branch' },
      { name: 'feature/merged-a', status: 'deleted', message: null },
      {
        name: 'experiment-unmerged',
        status: 'skippedNotStale',
        message: 'not detected as stale',
      },
      { name: 'feature/gone', status: 'deleted', message: null },
    ]);
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.local.some((b) => b.name === 'feature/merged-a')).toBe(false);
    expect(snap.local.some((b) => b.name === 'experiment-unmerged')).toBe(true);
    // The live report shrank accordingly.
    const report = await run(branchHandlers.listStaleBranches(repoId));
    expect(report.branches.map((b) => b.name)).toEqual(['feature/merged-b']);
  });

  it('the current HEAD branch is skippedCurrent even when stale-classified', async () => {
    const repoId = await openDefault();
    makeClean(repoId);
    await run(branchHandlers.checkoutBranch(repoId, 'feature/merged-a'));
    const results = await run(
      branchHandlers.deleteBranches(repoId, ['feature/merged-a', 'main']),
    );
    expect(results[0].status).toBe('skippedCurrent');
    expect(results[1].status).toBe('skippedBase'); // main no longer current here
  });
});

describe('createBranchHere + resetBranch', () => {
  it('createBranchHere checks out a new branch at the oid, carrying dirty work', async () => {
    const repoId = await openDefault();
    const oid = '1234'.repeat(10);
    const result = await run(branchHandlers.createBranchHere(repoId, 'hotfix/at-oid', oid));
    expect(result).toEqual({ stashed: true, apply: { kind: 'applied' } });
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.head).toMatchObject({ branchName: 'hotfix/at-oid', oid });
    expect(snap.local.find((b) => b.name === 'hotfix/at-oid')?.tip).toBe(oid);
  });

  it('resetBranch drops synthetic rows above the target and moves HEAD', async () => {
    const repoId = await openDefault();
    const first = await run(statusHandlers.commit(repoId, 'first'));
    requireRepo(repoId).status.staged = [{ path: 'x', origPath: null, status: 'added' }];
    await run(statusHandlers.commit(repoId, 'second'));
    expect(requireRepo(repoId).commits).toHaveLength(2);
    await run(resetRevertHandlers.resetBranch(repoId, first.oid, 'mixed'));
    const state = requireRepo(repoId);
    expect(state.headOid).toBe(first.oid);
    expect(state.commits.map((c) => c.summary)).toEqual(['first']);
    // Unknown oid: HEAD moves (mock simplification) but the list is untouched.
    await run(resetRevertHandlers.resetBranch(repoId, 'ff'.repeat(20), 'hard'));
    expect(requireRepo(repoId).commits).toHaveLength(1);
  });
});

/** P99 — mock fidelity: `openRepo` and `listBranches` must agree about HEAD,
 *  the way the backend's shared `read_head_info` forces them to. */
describe('listBranches HEAD fidelity across repo kinds', () => {
  async function open(label: string): Promise<{ repoId: string; head: HeadInfo | null }> {
    const { repoId, info } = await run(repoHandlers.openRepo(freshRepoPath(label)));
    return { repoId, head: info.head };
  }

  it('reports the SAME head as openRepo for an unborn repo (unborn, empty oid)', async () => {
    const { repoId, head } = await open('unborn');
    const snap = await run(branchHandlers.listBranches(repoId));
    // The consistency property itself — a future drift between the two handlers
    // fails here even if both are individually plausible.
    expect(snap.head).toEqual(head);
    expect(snap.head).toEqual({ branchName: 'main', oid: '', detached: false, unborn: true });
  });

  it('returns empty ref lists for an unborn repo (no refs exist pre-first-commit)', async () => {
    const { repoId } = await open('unborn');
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.local).toEqual([]);
    expect(snap.remote).toEqual([]);
    expect(snap.tags).toEqual([]);
  });

  it('leaves the default kind unchanged (born HEAD on main, refs present)', async () => {
    const { repoId, head } = await open('br-default');
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.head).toEqual(head);
    expect(snap.head).toEqual({
      branchName: 'main',
      oid: MOCK_OID,
      detached: false,
      unborn: false,
    });
    expect(snap.local.some((b) => b.name === 'main' && b.isHead)).toBe(true);
    expect(snap.remote.length).toBeGreaterThan(0);
    expect(snap.tags.length).toBeGreaterThan(0);
  });

  it('leaves the detached kind unchanged (detached HEAD, no local isHead)', async () => {
    const { repoId, head } = await open('detached');
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.head).toEqual(head);
    expect(snap.head).toEqual({
      branchName: null,
      oid: MOCK_OID,
      detached: true,
      unborn: false,
    });
    expect(snap.local.every((b) => !b.isHead)).toBe(true);
    expect(snap.local.length).toBeGreaterThan(0);
  });

  // A repo cannot hold a commit AND an unborn HEAD: the first commit is what
  // creates the default branch. Before P99's follow-up the mock stayed `unborn`
  // forever, so "No commits yet" / "No branches yet" survived a commit.
  it('leaves unborn behind on the first commit (HEAD born, one isHead branch)', async () => {
    const { repoId } = await open('unborn');
    await run(statusHandlers.stage(repoId, ['README.md']));
    const result = await run(statusHandlers.commit(repoId, 'first commit'));
    const snap = await run(branchHandlers.listBranches(repoId));
    expect(snap.head.unborn).toBe(false);
    expect(snap.head.oid).toBe(result.oid);
    expect(snap.head.branchName).toBe('main');
    expect(snap.head.detached).toBe(false);
    const heads = snap.local.filter((b) => b.isHead);
    expect(heads).toEqual([
      { name: 'main', isHead: true, upstream: null, ahead: null, behind: null, tip: result.oid },
    ]);
    expect(snap.local).toHaveLength(1);
  });
});
