// @vitest-environment jsdom
/** P119 §6 — the MOCK's half of AC3/AC4/AC6/AC7/AC8: every §5.2 handler emits
 *  exactly one `started` (the §1 category + target, or `targetCount`) and one
 *  `finished`; conflict-capable rows end `outcome: 'conflicts'` under
 *  `?gitConflicts`; the unlogged writes emit nothing; a throwing handler ends
 *  with its message line.
 *
 *  Outcome-agnostic by design: `started` fires BEFORE the body, so a row whose
 *  body rejects (abort with nothing in progress) still proves the bracket.
 *  Every case loads a FRESH module graph under its `?seam` (the seams are read
 *  once at module init — the `gitActivitySeams.test.tsx` pattern), and its own
 *  file because the activity subscriber is process-wide and never removed.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { MOCK_OID } from '../../fixtures/branches';
import { freshRepoPath } from '../../../test/mockIpcKit';
import type { MockRepoState } from '../repoState';
import type { GitActivityCategory, GitActivityEvent, IpcApi } from '../../types';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.resetModules();
  window.history.replaceState({}, '', '/');
});

interface Harness {
  ipc: IpcApi;
  events: GitActivityEvent[];
  state: (repoId: string) => MockRepoState;
  seedRebase: (repoId: string) => void;
  gitActivity: typeof import('../gitActivity');
}

/** A fresh mock (+ its seams) under `search`, with a subscriber attached. */
async function load(search = ''): Promise<Harness> {
  vi.resetModules();
  window.history.replaceState({}, '', search === '' ? '/' : `/?${search}`);
  const { mockIpc } = await import('../../mock');
  const gitActivity = await import('../gitActivity');
  const repoState = await import('../repoState');
  const opStateSeed = await import('../opStateSeed');
  const events: GitActivityEvent[] = [];
  gitActivity.subscribeGitActivity((e) => events.push(e));
  return {
    ipc: mockIpc,
    events,
    state: (id) => repoState.requireRepo(id),
    seedRebase: (id) => opStateSeed.seedOpState(repoState.requireRepo(id), 'rebase'),
    gitActivity,
  };
}

/** Settle `p` under fake timers, swallowing a rejection (outcome-agnostic). */
async function settle(p: Promise<unknown>): Promise<void> {
  const done = p.then(
    () => undefined,
    () => undefined,
  );
  await vi.advanceTimersByTimeAsync(10_000);
  await done;
}

async function openFresh(h: Harness): Promise<string> {
  const opened = h.ipc.openRepo(freshRepoPath('act'));
  await vi.advanceTimersByTimeAsync(10_000);
  return (await opened).repoId;
}

const OID = 'a1b2c3d4e5f60718293a4b5c6d7e8f9012345678';
const HEAD7 = MOCK_OID.slice(0, 7);

interface Row {
  category: GitActivityCategory;
  /** Exact `started.target`; `null` = the key is absent. */
  target?: string | null;
  count?: number;
  /** A §2.6 conflict-capable row (`?gitConflicts` forces its outcome). */
  conflicts?: boolean;
  setup?: (h: Harness, repoId: string) => Promise<void> | void;
  call: (ipc: IpcApi, repoId: string) => Promise<unknown>;
}

/** §5.2, one row per wrapped handler (the name is the IPC method). */
const ROWS: Record<string, Row> = {
  createBranch: { category: 'createBranch', target: 'topic-x', call: (i, r) => i.createBranch(r, 'topic-x') },
  createBranchHere: {
    category: 'createBranch', target: 'here-x', conflicts: true,
    call: (i, r) => i.createBranchHere(r, 'here-x', OID),
  },
  checkoutBranch: {
    category: 'checkoutBranch', target: 'feature/sidebar', conflicts: true,
    call: (i, r) => i.checkoutBranch(r, 'feature/sidebar'),
  },
  checkoutCommit: {
    category: 'checkoutCommit', target: OID.slice(0, 7), conflicts: true,
    call: (i, r) => i.checkoutCommit(r, OID),
  },
  checkoutRemoteBranch: {
    category: 'checkoutRemote', target: 'origin/release',
    call: (i, r) => i.checkoutRemoteBranch(r, 'origin/release'),
  },
  deleteBranch: { category: 'deleteBranch', target: 'feature/gone', call: (i, r) => i.deleteBranch(r, 'feature/gone') },
  deleteBranches: {
    category: 'deleteBranches', target: null, count: 3,
    call: (i, r) => i.deleteBranches(r, ['feature/merged-a', 'feature/merged-b', 'feature/gone']),
  },
  renameBranch: { category: 'renameBranch', target: 'renamed', call: (i, r) => i.renameBranch(r, 'feature/gone', 'renamed') },
  deleteRemoteBranch: {
    category: 'deleteRemoteTracking', target: 'origin/dev',
    call: (i, r) => i.deleteRemoteBranch(r, 'origin/dev'),
  },
  mergeBranch: { category: 'merge', target: 'feature/sidebar', conflicts: true, call: (i, r) => i.mergeBranch(r, 'feature/sidebar') },
  abortMerge: { category: 'abortMerge', target: 'main', call: (i, r) => i.abortMerge(r) },
  rebaseBranch: { category: 'rebase', target: 'origin/main', conflicts: true, call: (i, r) => i.rebaseBranch(r, 'origin/main') },
  startInteractiveRebase: {
    category: 'interactiveRebase', target: OID.slice(0, 7), conflicts: true,
    call: (i, r) => i.startInteractiveRebase(r, OID, []),
  },
  rebaseContinue: {
    category: 'rebaseContinue', target: 'feature/topic', conflicts: true,
    setup: (h, r) => h.seedRebase(r), call: (i, r) => i.rebaseContinue(r),
  },
  rebaseSkip: {
    category: 'rebaseSkip', target: 'feature/topic', conflicts: true,
    setup: (h, r) => h.seedRebase(r), call: (i, r) => i.rebaseSkip(r),
  },
  rebaseAbort: {
    category: 'rebaseAbort', target: 'feature/topic',
    setup: (h, r) => h.seedRebase(r), call: (i, r) => i.rebaseAbort(r),
  },
  resetSoft: { category: 'resetSoft', target: HEAD7, call: (i, r) => i.resetBranch(r, MOCK_OID, 'soft') },
  resetMixed: { category: 'resetMixed', target: HEAD7, call: (i, r) => i.resetBranch(r, MOCK_OID, 'mixed') },
  resetHard: { category: 'resetHard', target: HEAD7, call: (i, r) => i.resetBranch(r, MOCK_OID, 'hard') },
  discardPaths: { category: 'discard', target: null, count: 3, call: (i, r) => i.discardPaths(r, ['a.ts', 'b.ts', 'c.ts']) },
  discardPathsForce: { category: 'discard', target: 'src/a.ts', call: (i, r) => i.discardPathsForce(r, ['src/a.ts']) },
  discardPartial: { category: 'discard', target: 'src/main.rs', call: (i, r) => i.discardPartial(r, 'src/main.rs', null, []) },
  cherrypickCommit: { category: 'cherryPick', target: OID.slice(0, 7), conflicts: true, call: (i, r) => i.cherrypickCommit(r, OID) },
  cherrypickContinue: { category: 'cherryPickContinue', target: 'main', conflicts: true, call: (i, r) => i.cherrypickContinue(r) },
  cherrypickAbort: { category: 'cherryPickAbort', target: 'main', call: (i, r) => i.cherrypickAbort(r) },
  revertCommit: { category: 'revert', target: OID.slice(0, 7), conflicts: true, call: (i, r) => i.revertCommit(r, OID) },
  revertContinue: { category: 'revertContinue', target: 'main', conflicts: true, call: (i, r) => i.revertContinue(r) },
  revertAbort: { category: 'revertAbort', target: 'main', call: (i, r) => i.revertAbort(r) },
  createStash: { category: 'stashCreate', target: 'main', call: (i, r) => i.createStash(r, null, 'all') },
  applyStash: { category: 'stashApply', target: 'stash@{0}', conflicts: true, call: (i, r) => i.applyStash(r, 0, false) },
  popStash: { category: 'stashPop', target: 'stash@{0}', conflicts: true, call: (i, r) => i.popStash(r, 0, false) },
  dropStash: { category: 'stashDrop', target: 'stash@{1}', call: (i, r) => i.dropStash(r, 1) },
  createTag: { category: 'createTag', target: 'v9.9.9', call: (i, r) => i.createTag(r, 'v9.9.9', OID, null, false) },
  deleteTag: { category: 'deleteTag', target: 'v9.9.9', call: (i, r) => i.deleteTag(r, 'refs/tags/v9.9.9') },
  pushTag: { category: 'pushTag', target: 'v1.0.0', call: (i, r) => i.pushTag(r, 'origin', 'v1.0.0', false) },
  forceRefreshTag: { category: 'forceRefreshTag', target: 'v1.0.0', call: (i, r) => i.forceRefreshTag(r, 'origin', 'v1.0.0') },
  deleteRemoteTag: { category: 'deleteRemoteTag', target: 'v1.0.0', call: (i, r) => i.deleteRemoteTag(r, 'origin', 'v1.0.0') },
  // No URL is ever a target (AC6): the credential-carrying URL must not appear.
  addRemote: { category: 'addRemote', target: 'upstream', call: (i, r) => i.addRemote(r, 'upstream', 'https://tok@host/r.git') },
  removeRemote: { category: 'removeRemote', target: 'origin', call: (i, r) => i.removeRemote(r, 'origin') },
  renameRemote: { category: 'renameRemote', target: 'origin2', call: (i, r) => i.renameRemote(r, 'origin', 'origin2') },
  setRemoteUrl: { category: 'setRemoteUrl', target: 'origin', call: (i, r) => i.setRemoteUrl(r, 'origin', 'https://tok@host/r.git') },
  initSubmodule: { category: 'submoduleInit', target: 'vendor/libcore', call: (i, r) => i.initSubmodule(r, 'vendor/libcore') },
  updateSubmodule: { category: 'submoduleUpdate', target: 'vendor/libcore', call: (i, r) => i.updateSubmodule(r, 'vendor/libcore') },
  syncSubmodule: { category: 'submoduleSync', target: 'vendor/libcore', call: (i, r) => i.syncSubmodule(r, 'vendor/libcore') },
  addSubmodule: { category: 'submoduleAdd', target: 'vendor/new', call: (i, r) => i.addSubmodule(r, 'https://tok@host/y.git', 'vendor/new') },
  deinitSubmodule: { category: 'submoduleDeinit', target: 'vendor/libcore', call: (i, r) => i.deinitSubmodule(r, 'vendor/libcore', true) },
  removeSubmodule: { category: 'submoduleRemove', target: 'vendor/theme', call: (i, r) => i.removeSubmodule(r, 'vendor/theme', true) },
  addWorktree: { category: 'worktreeAdd', target: 'wt-a', call: (i, r) => i.addWorktree(r, 'feature/sidebar', 'wt-a') },
  // One run, never nested: its body calls addWorktree's BODY, not the wrap.
  addWorktreeWithChanges: {
    category: 'worktreeAdd', target: 'wt-b',
    call: (i, r) => i.addWorktreeWithChanges(r, 'feature/sidebar', 'wt-b', []),
  },
  removeWorktree: { category: 'worktreeRemove', target: 'wt-a', call: (i, r) => i.removeWorktree(r, 'wt-a') },
  lockWorktree: { category: 'worktreeLock', target: 'wt-a', call: (i, r) => i.lockWorktree(r, 'wt-a') },
  unlockWorktree: { category: 'worktreeUnlock', target: 'wt-a', call: (i, r) => i.unlockWorktree(r, 'wt-a') },
  startBisect: { category: 'bisectStart', target: OID.slice(0, 7), call: (i, r) => i.startBisect(r, OID, [MOCK_OID]) },
  bisectGood: { category: 'bisectGood', target: HEAD7, call: (i, r) => i.bisectMark(r, true) },
  bisectBad: { category: 'bisectBad', target: HEAD7, call: (i, r) => i.bisectMark(r, false) },
  bisectSkip: { category: 'bisectSkip', target: HEAD7, call: (i, r) => i.bisectSkip(r) },
  bisectReset: { category: 'bisectReset', target: null, call: (i, r) => i.bisectReset(r) },
  applyComposedCommits: { category: 'composeCommits', target: 'main', call: (i, r) => i.applyComposedCommits(r, { groups: [] }) },
  // The destination LEAF, never the (credential-carrying) URL.
  cloneRepo: { category: 'cloneRepo', target: 'my-clone', call: (i) => i.cloneRepo('https://tok@host/r.git', 'C:\\src\\my-clone\\', () => undefined) },
  initRepo: { category: 'initRepo', target: 'new-repo', call: (i) => i.initRepo('/work/new-repo') },
};

/** Run one row on a fresh repo and return only the events of its call. */
async function driveRow(h: Harness, row: Row): Promise<GitActivityEvent[]> {
  const repoId = await openFresh(h);
  await row.setup?.(h, repoId);
  h.events.splice(0);
  await settle(row.call(h.ipc, repoId));
  return [...h.events];
}

describe('every §5.2 handler brackets its run (AC3, AC6)', () => {
  it.each(Object.entries(ROWS))('%s → one started + one finished, exact target', async (_name, row) => {
    const h = await load();
    const events = await driveRow(h, row);
    const started = events.filter((e) => e.kind === 'started');
    expect(started).toHaveLength(1);
    expect(events.filter((e) => e.kind === 'finished')).toHaveLength(1);
    expect(events.at(-1)?.kind).toBe('finished');
    expect(started[0].category).toBe(row.category);
    if (row.target === null) expect(started[0]).not.toHaveProperty('target');
    else expect(started[0].target).toBe(row.target);
    if (row.count === undefined) expect(started[0]).not.toHaveProperty('targetCount');
    else expect(started[0].targetCount).toBe(row.count);
    expect(JSON.stringify(events)).not.toContain('tok@');
  });
});

describe('?gitConflicts (AC4)', () => {
  it('every successful conflict-capable row ends conflicts; no other row does', async () => {
    let forced = 0;
    for (const row of Object.values(ROWS)) {
      const h = await load('gitConflicts');
      const finished = (await driveRow(h, row)).at(-1);
      if (row.conflicts === true && finished?.success === true) {
        expect(finished.outcome).toBe('conflicts');
        forced += 1;
      } else {
        expect(finished).not.toHaveProperty('outcome');
      }
    }
    // Not vacuous: most capable rows succeed on a fresh default repo.
    expect(forced).toBeGreaterThanOrEqual(8);
  });
});

describe('merge outcome (AC5)', () => {
  it.each([
    ['feature/sidebar', 'fastForwarded'],
    ['demo-clean', 'merged'],
    ['main', 'upToDate'],
    ['demo-uptodate', 'upToDate'],
    ['demo-conflict', 'conflicts'],
  ])('mergeBranch(%s) → %s', async (name, outcome) => {
    const h = await load();
    const events = await driveRow(h, { category: 'merge', call: (i, r) => i.mergeBranch(r, name) });
    expect(events.at(-1)).toMatchObject({ kind: 'finished', success: true, outcome });
  });
});

describe('unlogged writes emit nothing (AC8)', () => {
  const NONE: Record<string, (i: IpcApi, r: string) => Promise<unknown>> = {
    stage: (i, r) => i.stage(r, ['src/app.ts']),
    unstage: (i, r) => i.unstage(r, ['src/app.ts']),
    stagePartial: (i, r) => i.stagePartial(r, 'src/main.rs', null, []),
    unstagePartial: (i, r) => i.unstagePartial(r, 'src/main.rs', null, []),
    resolveConflict: (i, r) => i.resolveConflict(r, 'src/auth.ts', 'ours'),
    resolveConflictText: (i, r) => i.resolveConflictText(r, 'src/auth.ts', 'x'),
    autoSyncTags: (i, r) => i.autoSyncTags(r, null),
    getStatus: (i, r) => i.getStatus(r),
  };
  it.each(Object.entries(NONE))('%s', async (_name, call) => {
    const h = await load();
    const events = await driveRow(h, { category: 'merge', call });
    expect(events).toEqual([]);
  });
});

describe('a throwing handler ends with its message line (AC7)', () => {
  it('an AppError rejection: stderrLine(message) then a failed finished', async () => {
    const h = await load();
    const events = await driveRow(h, { category: 'deleteBranch', call: (i, r) => i.deleteBranch(r, 'nope') });
    expect(events.slice(-2)).toEqual([
      expect.objectContaining({ kind: 'stderrLine', line: "branch 'nope' not found" }),
      expect.objectContaining({ kind: 'finished', success: false }),
    ]);
  });

  it('a plain Error (the stash wrong-target guard) gets the line too', async () => {
    const h = await load();
    const events = await driveRow(h, { category: 'stashDrop', call: (i, r) => i.dropStash(r, 0, 'f'.repeat(40)) });
    expect(events.at(-2)).toMatchObject({ kind: 'stderrLine', line: 'stash list changed; refresh and retry' });
  });

  it('the reason is one line: breaks → spaces, controls stripped, capped at 2000', async () => {
    const h = await load();
    const reason = { kind: 'git', message: 'first\r\nsecond\nthird\u0007' };
    await settle(h.gitActivity.runMockActivity('deleteTag', 'v1', () => Promise.reject(reason)));
    expect(h.events.at(-2)?.line).toBe('first second third');
    h.events.splice(0);
    const long = { kind: 'git', message: 'x'.repeat(2500) };
    await settle(h.gitActivity.runMockActivity('deleteTag', 'v1', () => Promise.reject(long)));
    const line = h.events.at(-2)?.line ?? '';
    expect([...line]).toHaveLength(2000);
    expect(line.endsWith('…')).toBe(true);
  });
});

describe('the P119-ui §8 seams', () => {
  it('?gitMultiTarget — deleteBranches / discard start with targetCount 3', async () => {
    const h = await load('gitMultiTarget');
    const a = await driveRow(h, { category: 'deleteBranches', call: (i, r) => i.deleteBranches(r, ['feature/gone']) });
    expect(a[0]).toMatchObject({ kind: 'started', targetCount: 3 });
    const b = await driveRow(h, { category: 'discard', call: (i, r) => i.discardPaths(r, ['a.ts']) });
    expect(b[0]).toMatchObject({ kind: 'started', targetCount: 3 });
    expect(b[0]).not.toHaveProperty('target');
  });

  it('?gitOpFail — checkoutBranch fails with the real refusal text, reason line last', async () => {
    const h = await load('gitOpFail');
    const events = await driveRow(h, { category: 'checkoutBranch', call: (i, r) => i.checkoutBranch(r, 'main') });
    expect(events.slice(-2)).toEqual([
      expect.objectContaining({ kind: 'stderrLine', line: "branch 'main' is already checked out at '/repo/.worktrees/main'" }),
      expect.objectContaining({ kind: 'finished', success: false }),
    ]);
  });

  it('?gitLongTarget — a 90+-char submodule name and discard path', async () => {
    const h = await load('gitLongTarget');
    const sub = await driveRow(h, { category: 'submoduleInit', call: (i, r) => i.initSubmodule(r, 'vendor/libcore') });
    expect(sub[0].target).toBe(h.gitActivity.MOCK_LONG_SUBMODULE_TARGET);
    const path = await driveRow(h, { category: 'discard', call: (i, r) => i.discardPaths(r, ['a.ts']) });
    expect(path[0].target).toBe(h.gitActivity.MOCK_LONG_PATH_TARGET);
    expect(h.gitActivity.MOCK_LONG_SUBMODULE_TARGET.length).toBeGreaterThanOrEqual(90);
    expect(h.gitActivity.MOCK_LONG_PATH_TARGET.length).toBeGreaterThanOrEqual(90);
  });

  it('?refreshSlow — a status read (the refresh round) takes 2 s, and logs nothing', async () => {
    const h = await load('refreshSlow');
    const repoId = await openFresh(h);
    let resolved = false;
    const p = h.ipc.getStatus(repoId).then(() => (resolved = true));
    await vi.advanceTimersByTimeAsync(1_500);
    expect(resolved).toBe(false);
    await vi.advanceTimersByTimeAsync(600);
    await p;
    expect(resolved).toBe(true);
  });
});
