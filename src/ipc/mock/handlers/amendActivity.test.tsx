/** P87 FU-2 — the mock `commitAmend` must emit a git-activity run, exactly as
 *  the Rust `commit_amend_inner` wraps its body in
 *  `with_activity(GitActivityCategory::Amend)`. Without it the browser harness
 *  showed no dock row for an amend, so the mock under-covered the IPC surface.
 *
 *  Own file: `subscribeGitActivity` registers a process-wide listener that the
 *  mock never unregisters, which would switch `runMockActivity` from
 *  passthrough to sequenced for every other test sharing the module graph.
 */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { stashHandlers } from './stash';
import { statusHandlers } from './status';
import { subscribeGitActivity } from '../gitActivity';
import type { GitActivityEvent } from '../../types';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

const events: GitActivityEvent[] = [];
subscribeGitActivity((e) => events.push(e));

/** The categories of every `started` event seen so far. */
function startedCategories(): Array<string | undefined> {
  return events.filter((e) => e.kind === 'started').map((e) => e.category);
}

describe('commitAmend git activity (FU-2)', () => {
  it('emits a started/finished run under the `amend` category', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('amend')));
    events.length = 0;

    await run(stashHandlers.commitAmend(repoId, 'amended message'));

    expect(startedCategories()).toEqual(['amend']);
    const finished = events.filter((e) => e.kind === 'finished');
    expect(finished).toHaveLength(1);
    expect(finished[0].code).toBe(0);
    expect(finished[0].success).toBe(true);
    // Same commit-family hook script the plain `commit` path records.
    expect(events.filter((e) => e.kind === 'hookDone').map((e) => e.hook)).toEqual([
      'pre-commit',
      'commit-msg',
      'post-commit',
    ]);
    // One run id across the whole stream, seq strictly increasing.
    const ids = new Set(events.map((e) => e.id));
    expect(ids.size).toBe(1);
    const seqs = events.map((e) => e.seq);
    expect(seqs).toEqual([...seqs].sort((a, b) => a - b));
  });

  it('marks the run failed when the amend rejects', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('amend-fail')));
    events.length = 0;

    const err = await runErr(stashHandlers.commitAmend(repoId, '   '));
    expect(err.kind).toBe('emptyMessage');

    expect(startedCategories()).toEqual(['amend']);
    const finished = events.filter((e) => e.kind === 'finished');
    expect(finished).toHaveLength(1);
    expect(finished[0].success).toBe(false);
  });

  it('matches the shape `commit` emits (same wrapper, different category)', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('amend-vs-commit')));
    events.length = 0;

    await run(statusHandlers.commit(repoId, 'a commit'));
    const commitKinds = events.map((e) => e.kind);

    events.length = 0;
    await run(stashHandlers.commitAmend(repoId, 'an amend'));
    const amendKinds = events.map((e) => e.kind);

    expect(amendKinds).toEqual(commitKinds);
  });
});
