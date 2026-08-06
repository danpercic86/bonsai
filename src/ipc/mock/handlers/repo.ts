// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { recordRecent } from '../persistence';
import { MOCK_REPO_PATH, buildInfo, createRepoState, delay, mockCanonical, repos, resolveRepoId, throwAuthFailed, throwNetworkError } from '../repoState';
import type { AppError, CloneProgress, OpenRepoResult } from '../../types';

export const repoHandlers = {
  async openRepo(path: string): Promise<OpenRepoResult> {
    await delay(150);

    if (path.includes('error')) {
      const err: AppError = { kind: 'io', message: 'mock: path does not exist' };
      throw err;
    }
    // Non-usable opens still return a repoId (for the frontend's error UI) but
    // create NO entry and touch no other tab (contract §4.2).
    if (path.includes('not-a-repo')) {
      return { repoId: mockCanonical(path), info: { path, isRepo: false, bare: false, head: null } };
    }
    if (path.includes('bare')) {
      return {
        repoId: mockCanonical(path),
        info: {
          path,
          isRepo: true,
          bare: true,
          head: { branchName: 'main', oid: '', detached: false, unborn: true },
        },
      };
    }

    // Usable open (isRepo && !bare — unborn included): create/focus an entry.
    const repoId = resolveRepoId(path);
    recordRecent(path);
    let state = repos.get(repoId);
    if (state === undefined) {
      state = createRepoState(path);
      repos.set(repoId, state);
    }
    return { repoId, info: buildInfo(state, path) };
  },

  // P21: clone a remote repo, streaming a few monotonic progress ticks (an
  // object-download phase then a delta-resolve phase, §2.1) so the harness bar
  // animates end-to-end, then return a path the EXISTING openRepo can seed.
  async cloneRepo(
    url: string,
    dest: string,
    onProgress: (p: CloneProgress) => void,
  ): Promise<string> {
    // Failure triggers compose with the M6 messages: `authfail` / `network` in the
    // URL throw the SAME AppErrors after a couple of ticks (exercise the in-dialog
    // error path).
    const failAuth = /authfail/i.test(url);
    const failNet = /network/i.test(url);
    const total = 20;
    for (let i = 1; i <= total; i++) {
      await delay(120);
      onProgress({
        receivedObjects: i,
        totalObjects: total,
        indexedDeltas: 0,
        totalDeltas: 0,
        receivedBytes: i * 4096,
      });
      if (i === 3 && failAuth) throwAuthFailed();
      if (i === 3 && failNet) throwNetworkError();
    }
    for (let i = 1; i <= 10; i++) {
      await delay(80);
      onProgress({
        receivedObjects: total,
        totalObjects: total,
        indexedDeltas: i,
        totalDeltas: 10,
        receivedBytes: total * 4096,
      });
    }
    // The frontend already computed dest = <parent>/<name>; the real backend
    // clones INTO dest and returns its workdir, so mirror that (return dest).
    // openRepo then seeds a normal default repo (dest avoids the reserved
    // 'error'|'not-a-repo'|'bare'|'unborn' substrings for typical URLs).
    return dest;
  },

  // P21: init (or open) a repo at `path`. Return a path containing 'unborn' so
  // createRepoState seeds an EMPTY (unborn) repo — honest: init makes a
  // brand-new repo with no commits.
  async initRepo(path: string): Promise<string> {
    await delay(150);
    return `${path}/new-unborn-repo`;
  },

  closeRepo(repoId: string): Promise<void> {
    // Idempotent: deleting an unknown/already-closed id is a no-op.
    repos.delete(repoId);
    return Promise.resolve();
  },

  async pickFolder(): Promise<string | null> {
    await delay(150);
    return MOCK_REPO_PATH;
  },

} satisfies Partial<IpcApi>;
