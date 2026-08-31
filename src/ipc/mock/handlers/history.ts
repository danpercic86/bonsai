// Semantic history-search mock (P57a/b). Exercises the build -> status ->
// retrieve flow in the browser harness against the active graph fixtures.
// Fixtures carry no diffs, so this is UI-plumbing only (same caveat as P50's
// search mock): the real BM25 ranking lives Rust-side
// (bonsai-core::git::history_index). P57c adds the AI-answer handler here.
import type {
  AppError,
  HistoryAnswer,
  HistoryHit,
  HistoryQuery,
  HistorySearchResults,
  IndexProgress,
  IndexStatus,
  IpcApi,
} from '../../types';
import { AI_OFF, delay, query, requireRepo } from '../repoState';
import { resolveLayout } from './layout';
import type { MockRepoState } from '../repoState';

/** Mirrors the Rust `DEFAULT_TOP_K`/`MAX_TOP_K`: `topK` 0 ⇒ default, clamped. */
const DEFAULT_TOP_K = 20;
const MAX_TOP_K = 50;

/** Lowercase, split on non-alphanumeric, drop empties — a naive stand-in for the
 *  Rust tokenizer, enough to drive the retrieval UI in the harness. */
function words(text: string): string[] {
  return text
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((w) => w.length > 0);
}

/** Flipped once `historyIndexBuild` completes, so `historyIndexStatus` (and, in
 *  P57b, retrieval) sees a built index. Module-level — mirrors the real per-repo
 *  persisted store closely enough for the harness build -> status loop. */
let mockBuilt = false;

/** `?historyFail` rejects the build with a `git` AppError so the harness can
 *  drive the error path (mirrors search.ts's `#fail` sentinel). Read once at
 *  module init. */
const HISTORY_FAIL = query('historyFail') !== null;

/** `?historySkip=N` makes the mock build report N unreadable commits skipped
 *  (audit #2 §3.3 warning path). Read once at module init; 0 when absent. */
const HISTORY_SKIP = Number(query('historySkip') ?? '0') || 0;

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
      // `?historySkip=N` simulates unreadable commits skipped by the build so
      // the harness can drive the warning toast (audit #2 §3.3).
      skippedCommits: HISTORY_SKIP,
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
      skippedCommits: 0,
    };
  },

  async historySearch(repoId: string, q: HistoryQuery): Promise<HistorySearchResults> {
    const state = requireRepo(repoId);
    await delay(120);
    // No index yet ⇒ empty + a rebuild hint (mirrors the Rust no-store path).
    if (!mockBuilt) {
      return { hits: [], indexStale: true, indexedCommits: 0 };
    }
    const indexedCommits = resolveLayout(state).nodes.length;
    return { hits: rankHits(state, q.text, q.topK), indexStale: false, indexedCommits };
  },

  async aiSearchHistory(repoId: string, question: string, topK: number): Promise<HistoryAnswer> {
    const state = requireRepo(repoId);
    // AI-off convention (mirrors the ai.ts handlers): `?ai=off` simulates a
    // missing CLI → aiFailed, so the harness can drive the error banner.
    if (AI_OFF) {
      const err: AppError = { kind: 'aiFailed', message: 'Claude Code CLI not found on PATH' };
      throw err;
    }
    await delay(700);
    // Retrieve exactly as `historySearch` does; no index / no relevant commits ⇒
    // aiFailed BEFORE the (mock) CLI, mirroring the Rust `answer_history` guard.
    const hits = mockBuilt ? rankHits(state, question, topK) : [];
    if (hits.length === 0) {
      const err: AppError = {
        kind: 'aiFailed',
        message: mockBuilt
          ? 'no commits in the history match that question'
          : 'history index not built — build it first',
      };
      throw err;
    }
    // Fixtures carry no diffs, so the answer is canned UI-plumbing (same caveat as
    // historySearch); the real grounded synthesis lives Rust-side.
    return {
      text: 'Based on the retrieved commits, the change was introduced to address the behavior in question (mock answer).',
      cited: hits.slice(0, 2).map((h) => h.oid.slice(0, 7)),
      retrieved: hits,
      costUsd: 0.01,
    };
  },
} satisfies Partial<IpcApi>;

/** Naive token-overlap ranking shared by `historySearch` + `aiSearchHistory`:
 *  how many query terms appear (as substrings) in a node's summary+author, then
 *  a fake strictly-descending score so the UI's relevance bar renders in rank
 *  order. Fixtures carry no diffs, so this is UI-plumbing only (same caveat as
 *  P50's search mock) — the real BM25 ranking is Rust-side. */
function rankHits(state: MockRepoState, text: string, topK: number): HistoryHit[] {
  const layout = resolveLayout(state);
  const terms = words(text);
  if (terms.length === 0) return [];
  const scored = layout.nodes
    .map((node) => {
      const hay = `${node.summary} ${node.author}`.toLowerCase();
      const overlap = terms.reduce((acc, t) => acc + (hay.includes(t) ? 1 : 0), 0);
      return { node, overlap };
    })
    .filter((s) => s.overlap > 0)
    .sort((a, b) => b.overlap - a.overlap || b.node.ts - a.node.ts);
  const k = topK > 0 ? Math.min(topK, MAX_TOP_K) : DEFAULT_TOP_K;
  return scored.slice(0, k).map((s, i) => ({
    oid: s.node.id,
    summary: s.node.summary,
    authorName: s.node.author,
    authorTs: s.node.ts,
    score: scored.length - i,
  }));
}
