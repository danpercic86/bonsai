// P89/P93 mock handlers for the PR diff surface: base…head stats, per-file
// hunks, and review comments.
//
// Own module (CLAUDE.md file-size discipline — `forge.ts` reached the 500-line
// soft limit) because these three handlers are the only purely fixture-driven
// ones in that file: they touch no mutable session state, just `requireRepo`,
// the offline gate, and the `prDiff` fixtures.
import { PR_DIFF_STATS, PR_DIFF_STATS_EMPTY, mockPrFileDiff } from '../../fixtures/prDiff';
import { FORGE_REVIEW_COMMENTS } from '../../fixtures/forge';
import { delay, query as urlParam, requireRepo } from '../repoState';
import { offGuard } from './forgeOffline';
import type { FileDiff, IpcApi, PrDiffStats, ReviewComment } from '../../types';

export const forgePrDiffHandlers = {
  // P89: locally-computed PR base…head diff. Auto-fetch is a no-op in the mock;
  // returns canned stats + headers. `?forge=empty` ⇒ base===head (empty state);
  // `?forge=off` ⇒ networkError (fetch-failed/offline path, via offGuard).
  async forgePrDiff(repoId: string, _number: number): Promise<PrDiffStats> {
    await delay(250);
    requireRepo(repoId);
    offGuard();
    if (urlParam('forge') === 'empty') return PR_DIFF_STATS_EMPTY;
    return PR_DIFF_STATS;
  },

  // P89: hunks for ONE file of the PR diff — pure local (no offGuard/refetch),
  // routed by path exactly like the backend's `pr_file_diff`.
  async forgePrFileDiff(
    repoId: string,
    _mergeBaseOid: string,
    _headOid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    await delay(120);
    requireRepo(repoId);
    // P93: the fixture honours both flags (File view / Highlight changes) and
    // rejects for the `fail` path sentinel.
    return mockPrFileDiff(path, origPath, fullContext, intraline);
  },

  async forgeListReviewComments(repoId: string, _number: number): Promise<ReviewComment[]> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
    return FORGE_REVIEW_COMMENTS;
  },
} satisfies Partial<IpcApi>;
