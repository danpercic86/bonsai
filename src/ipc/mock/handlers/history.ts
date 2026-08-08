// Semantic history-search mock (P57a). Exercises the build -> status flow in the
// browser harness against the active graph fixtures. Fixtures carry no diffs, so
// this is UI-plumbing only (same caveat as P50's search mock); the real BM25
// index lives Rust-side (bonsai-core::git::history_index). P57b/c add the
// retrieval + AI-answer handlers here.
import type { AppError, IndexProgress, IndexStatus, IpcApi } from '../../types';
import { delay, query, requireRepo } from '../repoState';
import { resolveLayout } from './layout';

/** Flipped once `historyIndexBuild` completes, so `historyIndexStatus` (and, in
 *  P57b, retrieval) sees a built index. Module-level — mirrors the real per-repo
 *  persisted store closely enough for the harness build -> status loop. */
let mockBuilt = false;

/** `?historyFail` rejects the build with a `git` AppError so the harness can
 *  drive the error path (mirrors search.ts's `#fail` sentinel). Read once at
 *  module init. */
const HISTORY_FAIL = query('historyFail') !== null;

function nowSecs(): number {
  return Math.floor(Date.now() / 1000);
}

export const historyHandlers = {
  async historyIndexBuild(
    repoId: string,
    onProgress: (p: IndexProgress) => void,
  ): Promise<IndexStatus> {
    const state = requireRepo(repoId);
    if (HISTORY_FAIL) {
      const err: AppError = { kind: 'git', message: 'Mock: index build failed' };
      throw err;
    }
    const layout = resolveLayout(state);
    const total = layout.nodes.length;
    const headOid = layout.nodes[0]?.id ?? null;

    // Counting, then an Extracting loop with a climbing `processed`, then Writing
    // and Done — mirrors the Rust build's IndexProgress cadence.
    onProgress({ phase: 'counting', processed: 0, total, newCommits: total });
    const steps = 12;
    for (let i = 1; i <= steps; i++) {
      await delay(60);
      const processed = Math.round((total * i) / steps);
      onProgress({ phase: 'extracting', processed, total, newCommits: total });
    }
    onProgress({ phase: 'writing', processed: total, total, newCommits: total });
    mockBuilt = true;
    onProgress({ phase: 'done', processed: total, total, newCommits: total });

    return {
      built: true,
      indexedCommits: total,
      headOid,
      stale: false,
      newCommits: 0,
      schema: 1,
      builtAt: nowSecs(),
    };
  },

  async historyIndexStatus(repoId: string): Promise<IndexStatus> {
    const state = requireRepo(repoId);
    await delay(60);
    const layout = resolveLayout(state);
    return {
      built: mockBuilt,
      indexedCommits: mockBuilt ? layout.nodes.length : 0,
      headOid: mockBuilt ? (layout.nodes[0]?.id ?? null) : null,
      stale: false,
      newCommits: 0,
      schema: 1,
      builtAt: mockBuilt ? nowSecs() : null,
    };
  },
} satisfies Partial<IpcApi>;
