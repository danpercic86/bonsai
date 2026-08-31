/** T3.4 — URL-flag seams. Module-init flags (?ai=off, ?historyFail) need
 *  vi.resetModules + a rewritten location (the useUpdateController.test.tsx
 *  pattern); openRepo-time flags (?hooks=, ?fixture=, ?op=, ?branch=) only
 *  need replaceState before opening. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../test/mockIpcKit';
import { MERGE_DEEP_PATH } from '../fixtures/conflicts';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.resetModules();
  window.history.replaceState({}, '', '/');
});

/** Reload the mock module graph under `search` and open a fresh default repo. */
async function loadWith(search: string) {
  vi.resetModules();
  window.history.replaceState({}, '', search === '' ? '/' : `/?${search}`);
  const repo = (await import('./handlers/repo')).repoHandlers;
  const { repoId } = await run(repo.openRepo(freshRepoPath('seam')));
  return { repoId };
}

describe('?ai=off (module-init AI_OFF)', () => {
  it('availability reports not-installed; AI commands reject aiFailed', async () => {
    const { repoId } = await loadWith('ai=off');
    const ai = (await import('./handlers/ai')).aiHandlers;
    expect(await run(ai.checkAiAvailability())).toMatchObject({
      installed: false,
      loggedIn: false,
      version: null,
    });
    expect((await runErr(ai.generateCommitMessage(repoId))).kind).toBe('aiFailed');
    expect((await runErr(ai.aiPlanOperation(repoId, 'stash it'))).kind).toBe('aiFailed');
    expect(
      (await runErr(ai.aiDigest(repoId, { kind: 'lastDays', days: 1 }))).kind,
    ).toBe('aiFailed');
    const history = (await import('./handlers/history')).historyHandlers;
    expect((await runErr(history.aiSearchHistory(repoId, 'q', 5))).kind).toBe('aiFailed');
  });
});

describe('?historyFail (module-init)', () => {
  it('historyIndexBuild rejects with a git AppError', async () => {
    const { repoId } = await loadWith('historyFail=1');
    const history = (await import('./handlers/history')).historyHandlers;
    const err = await runErr(history.historyIndexBuild(repoId, () => undefined));
    expect(err).toEqual({ kind: 'git', message: 'Mock: index build failed' });
  });
});

describe('?hooks= (read at openRepo)', () => {
  it('hooks=fail blocks commit/amend/commitMerge; skipHooks bypasses', async () => {
    const { repoId } = await loadWith('hooks=fail');
    const status = (await import('./handlers/status')).statusHandlers;
    const stash = (await import('./handlers/stash')).stashHandlers;
    const err = await runErr(status.commit(repoId, 'feat: blocked'));
    expect(err.kind).toBe('hookRejected');
    expect(err.message).toContain('pre-commit hook failed');
    expect((await runErr(stash.commitAmend(repoId, 'amend blocked'))).kind).toBe(
      'hookRejected',
    );
    const result = await run(status.commit(repoId, 'feat: skipped', null, true));
    expect(result.oid).toHaveLength(40);
  });

  it('hooks=failpush blocks push/forcePush; skipHooks bypasses', async () => {
    const { repoId } = await loadWith('hooks=failpush');
    const status = (await import('./handlers/status')).statusHandlers;
    const sync = (await import('./handlers/remotesSync')).remotesSyncHandlers;
    await run(status.commit(repoId, 'to push'));
    const err = await runErr(sync.push(repoId));
    expect(err.kind).toBe('hookRejected');
    expect(err.message).toContain('pre-push hook failed');
    expect(await run(sync.push(repoId, true))).toMatchObject({ kind: 'pushed' });
  });
});

describe('?fixture= (read at openRepo)', () => {
  it('noconfig: commit rejects configMissing until an identity is set', async () => {
    const { repoId } = await loadWith('fixture=noconfig');
    const status = (await import('./handlers/status')).statusHandlers;
    const config = (await import('./handlers/config')).configHandlers;
    expect((await runErr(status.commit(repoId, 'msg'))).kind).toBe('configMissing');
    await run(config.setConfig(repoId, 'local', 'user.name', 'A'));
    await run(config.setConfig(repoId, 'local', 'user.email', 'a@x.dev'));
    expect((await run(status.commit(repoId, 'msg'))).oid).toHaveLength(40);
  });

  it('20k: getGraph serves the synthetic 20k layout', async () => {
    const { repoId } = await loadWith('fixture=20k');
    const diff = (await import('./handlers/diff')).diffHandlers;
    const layout = await run(diff.getGraph(repoId));
    expect(layout.nodes.length).toBeGreaterThanOrEqual(20_000);
  });
});

describe('?op=merge (query fallback when the path has no substring)', () => {
  it('opens paused with the seeded conflicts', async () => {
    const { repoId } = await loadWith('op=merge');
    const merge = (await import('./handlers/merge')).mergeHandlers;
    const op = await run(merge.getOpState(repoId));
    expect(op).toMatchObject({ kind: 'merge', incoming: 'feature/login' });
    // P68d added a second bothModified path (deep, i18n JSON) to the merge fixture.
    expect((await run(merge.listConflicts(repoId))).map((c) => c.path)).toEqual([
      'README.md',
      'src/auth.ts',
      MERGE_DEEP_PATH,
    ]);
  });
});

describe('?branch=cbhconflict (read per call)', () => {
  it('createBranchHere on a dirty tree reports a conflicted carry-over', async () => {
    const { repoId } = await loadWith('branch=cbhconflict');
    const branches = (await import('./handlers/branches')).branchHandlers;
    const result = await run(
      branches.createBranchHere(repoId, 'hot/conflicted-carry', '9'.repeat(40)),
    );
    expect(result).toEqual({
      stashed: true,
      apply: { kind: 'conflicts', paths: ['src/app.ts'] },
    });
    const { requireRepo } = await import('./repoState');
    expect(
      requireRepo(repoId).status.conflicted.some((e) => e.path === 'src/app.ts'),
    ).toBe(true);
  });
});

describe('?git= / ?gitDelay= (module-init P70 seams)', () => {
  it('absent ⇒ a healthy PATH-resolved git and no remote-op rejections', async () => {
    const { repoId } = await loadWith('');
    const gitEnv = (await import('./handlers/gitEnv')).gitEnvHandlers;
    expect(await run(gitEnv.checkGitAvailability())).toMatchObject({
      found: true,
      source: 'path',
      version: '2.47.1',
    });
    const remotes = (await import('./handlers/remotesSync')).remotesSyncHandlers;
    await expect(run(remotes.fetch(repoId))).resolves.toBeDefined();
  });

  it('?git=missing ⇒ not found, and fetch/pull/push reject with gitNotFound', async () => {
    const { repoId } = await loadWith('git=missing');
    const { gitEnvHandlers, MOCK_GIT_NOT_FOUND_MESSAGE } = await import('./handlers/gitEnv');
    const status = await run(gitEnvHandlers.checkGitAvailability());
    expect(status).toMatchObject({ found: false, path: null, source: 'fallback' });
    // The honest copy — it must deny the auth reading AND exempt ssh-agent.
    expect(status.detail).toBe(MOCK_GIT_NOT_FOUND_MESSAGE);
    expect(status.detail).toContain('NOT an authentication failure');
    expect(status.detail).toContain('SSH remotes using an ssh-agent are unaffected');

    const remotes = (await import('./handlers/remotesSync')).remotesSyncHandlers;
    // Thunks, not eager promises: three simultaneously-rejecting promises would
    // surface as unhandled rejections before the loop got to await them.
    const calls = [
      () => remotes.fetch(repoId),
      () => remotes.pull(repoId),
      () => remotes.push(repoId, false),
    ];
    for (const call of calls) {
      const err = await runErr(call());
      expect(err.kind).toBe('gitNotFound');
      expect(err.message).toBe(MOCK_GIT_NOT_FOUND_MESSAGE);
    }
  });

  it('?git=badpath ⇒ Variant B: found-but-unrunnable, path populated, no rejections', async () => {
    const { repoId } = await loadWith('git=badpath');
    const gitEnv = (await import('./handlers/gitEnv')).gitEnvHandlers;
    const status = await run(gitEnv.checkGitAvailability());
    expect(status.found).toBe(false);
    expect(status.source).toBe('override');
    expect(status.path).toContain('git.exe');
    const remotes = (await import('./handlers/remotesSync')).remotesSyncHandlers;
    await expect(run(remotes.fetch(repoId))).resolves.toBeDefined();
  });

  it('?git=registry ⇒ found via a NON-PATH rung (the detail line proves it)', async () => {
    await loadWith('git=registry');
    const gitEnv = (await import('./handlers/gitEnv')).gitEnvHandlers;
    const status = await run(gitEnv.checkGitAvailability());
    expect(status).toMatchObject({ found: true, source: 'registry' });
    expect(status.detail).toContain('(registry)');
  });

  it('?git=longpath ⇒ a ≥250-char path and a ≥900-char detail (truncation fixtures)', async () => {
    await loadWith('git=longpath');
    const gitEnv = (await import('./handlers/gitEnv')).gitEnvHandlers;
    const status = await run(gitEnv.checkGitAvailability());
    expect(status.path?.length ?? 0).toBeGreaterThanOrEqual(250);
    expect(status.detail.length).toBeGreaterThanOrEqual(900);
    expect(status.source).toBe('wellKnown');
  });

  it('?gitDelay is clamped and composes with ?git=', async () => {
    await loadWith('git=missing&gitDelay=1200');
    const gitEnv = (await import('./handlers/gitEnv')).gitEnvHandlers;
    const pending = gitEnv.checkGitAvailability();
    let settled = false;
    void pending.then(() => {
      settled = true;
    });
    await vi.advanceTimersByTimeAsync(1000);
    expect(settled).toBe(false);
    await vi.advanceTimersByTimeAsync(300);
    expect((await pending).found).toBe(false);
  });
});
