/** P83 — forge PR merge/close mock transitions + the SUPPORTED_MERGE_METHODS
 *  parity guard. Fills the frontend AC gaps: Merge is refused when a PR is not
 *  mergeable (#124), merge/close flip the session PR-state overlay, and the TS
 *  merge-method table matches the Rust `MergeMethod::supported_for` source of
 *  truth. Offline; default provider is gitHub (no ?forge sentinel). */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { forgeHandlers } from './forge';
import { SUPPORTED_MERGE_METHODS } from '../../types';
import type { MergePrInput } from '../../types';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

const mergeInput = (over: Partial<MergePrInput> = {}): MergePrInput => ({
  method: 'merge',
  commitTitle: null,
  commitMessage: null,
  deleteSourceBranch: false,
  headSha: null,
  ...over,
});

async function openAuthed(label = 'pr'): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath(label)));
  await run(forgeHandlers.forgeSetToken(repoId, 'good-token'));
  return repoId;
}

describe('SUPPORTED_MERGE_METHODS parity (mirrors Rust MergeMethod::supported_for)', () => {
  it('matches the Rust table for all 5 forge kinds', () => {
    expect(SUPPORTED_MERGE_METHODS).toEqual({
      gitHub: ['merge', 'squash', 'rebase'],
      gitLab: ['merge', 'squash'],
      bitbucket: ['merge', 'squash', 'fastForward'],
      azureDevOps: ['merge', 'squash', 'rebase'],
      unknown: [],
    });
  });
});

describe('forgeMergePr transitions', () => {
  it('requires auth before merging', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('noauth')));
    const err = await runErr(forgeHandlers.forgeMergePr(repoId, 127, mergeInput()));
    expect(err.kind).toBe('forgeAuthRequired');
  });

  it('merges a mergeable open PR and flips its state to merged', async () => {
    const repoId = await openAuthed();
    const detail = await run(forgeHandlers.forgeMergePr(repoId, 127, mergeInput()));
    expect(detail.summary.state).toBe('merged');
    // the overlay persists: a later fetch still reports merged
    const after = await run(forgeHandlers.forgeGetPr(repoId, 127));
    expect(after.summary.state).toBe('merged');
  });

  it('rejects an unsupported merge method without transitioning', async () => {
    const repoId = await openAuthed();
    // fastForward is NOT in gitHub's supported set
    const err = await runErr(
      forgeHandlers.forgeMergePr(repoId, 123, mergeInput({ method: 'fastForward' })),
    );
    expect(err.kind).toBe('forgeApi');
    const after = await run(forgeHandlers.forgeGetPr(repoId, 123));
    expect(after.summary.state).toBe('open');
  });
});

describe('not-mergeable PR #124 (AC c)', () => {
  it('reports mergeable === false so the UI disables Merge', async () => {
    const repoId = await openAuthed();
    const detail = await run(forgeHandlers.forgeGetPr(repoId, 124));
    expect(detail.mergeable).toBe(false);
  });

  it('surfaces a not-mergeable forgeApi error and changes nothing', async () => {
    const repoId = await openAuthed();
    const err = await runErr(forgeHandlers.forgeMergePr(repoId, 124, mergeInput()));
    expect(err.kind).toBe('forgeApi');
    expect(err.message).toMatch(/not mergeable/i);
    const after = await run(forgeHandlers.forgeGetPr(repoId, 124));
    expect(after.summary.state).toBe('open');
  });
});

describe('forgeClosePr transitions', () => {
  // NB: the mock's `authenticated` flag is session-global (no per-repo reset),
  // so the auth gate is only exercised once, at the top of this file
  // ('requires auth before merging'); it applies identically to close.
  it('closes/declines a PR and flips its state to closed', async () => {
    const repoId = await openAuthed();
    const detail = await run(forgeHandlers.forgeClosePr(repoId, 125));
    expect(detail.summary.state).toBe('closed');
    const after = await run(forgeHandlers.forgeGetPr(repoId, 125));
    expect(after.summary.state).toBe('closed');
  });
});
