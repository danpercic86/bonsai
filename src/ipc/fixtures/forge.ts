// P62b: canned, offline forge/PR fixtures served by the mock IPC layer
// (src/ipc/mock/handlers/forge.ts). Zero network. Shapes mirror the Rust DTOs
// in crates/bonsai-forge/src/types.rs. The set covers the harness flows:
// repo context → PR list (≥3, incl. one draft + one with comments) → detail
// (labels + mergeable) → review/conversation comments → viewer.
//
// P63 graph-signal coordination (contract §9): the fixture PRs' `sourceBranch`
// values are aligned to LOCAL branch names present in the mock GraphLayout
// (src/ipc/fixtures/graph.ts: `feat`, `exp`, `gh-pages`) so P63b can attach PR
// badges to their pills, and `commitStatusFor(sha)` returns a CommitStatus for
// each of those branch-tip shas covering every CheckRollup (success / failure /
// pending / neutral / none).
import type {
  CheckRollup,
  CommitStatus,
  ForgeRepoContext,
  ForgeViewer,
  PrDetail,
  PrSummary,
  ReviewComment,
  StatusContext,
} from '../types';

/** Deterministic 40-hex branch-tip oid for BASE graph row `n` — mirrors
 *  `buildMockGraph`'s `oid` in graph.ts, so these CI fixtures key off the SAME
 *  tip shas the mock GraphLayout uses (P63b: `ciBySha.get(node.id)`). */
function tipOid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}

// Branch-tip shas in the mock GraphLayout (graph.ts §3.5), by local-branch name.
const FEAT_TIP = tipOid(1); //  local `feat`
const EXP_TIP = tipOid(2); //   local `exp`
const MAIN_TIP = tipOid(0); //  `main` / `dev` (both on row 0)
const ORIGIN_FEAT_TIP = tipOid(4); // remote-only `origin/feat`
const GH_PAGES_TIP = tipOid(28); // local `gh-pages`

/** The authenticated user returned by forgeSetToken / cached in the context. */
export const FORGE_VIEWER: ForgeViewer = {
  login: 'octocat',
  avatarUrl: 'https://avatars.githubusercontent.com/u/583231?v=4',
};

/** Baseline identity for the fixture repo. The mock overrides `authenticated`
 *  + `viewer` from its live connect state before returning this. */
export const FORGE_REPO_CONTEXT: ForgeRepoContext = {
  provider: 'gitHub',
  host: 'github.com',
  owner: 'octo-org',
  repo: 'bonsai',
  remoteName: 'origin',
  webUrl: 'https://github.com/octo-org/bonsai',
  authenticated: false,
  viewer: null,
};

/** PR list: two open (one draft), one open with comments, one merged — so an
 *  `open` filter shows 3 rows and `all` shows 4. The three OPEN PRs'
 *  `sourceBranch` + `headSha` are aligned to real mock-graph branch tips
 *  (`feat`/`exp`/`gh-pages`) so P63b can light PR badges on those pills; the
 *  merged PR keeps a deleted-branch name (merged branches are usually gone, and
 *  v1 fetches OPEN only — OQ-3). */
export const FORGE_PR_LIST: PrSummary[] = [
  {
    number: 128,
    title: 'Render PR/CI status badges beside graph nodes',
    state: 'open',
    isDraft: false,
    author: 'ada-lovelace',
    authorAvatarUrl: null,
    // Aligned to the mock graph's local `feat` tip (row 1) → CI success.
    sourceBranch: 'feat',
    targetBranch: 'main',
    comments: 3,
    createdAt: '2026-08-01T09:12:00Z',
    updatedAt: '2026-08-06T14:05:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/128',
    headSha: FEAT_TIP,
  },
  {
    number: 127,
    title: 'WIP: deterministic lane colors while scrolling',
    state: 'open',
    isDraft: true,
    author: 'linus-t',
    authorAvatarUrl: null,
    // Aligned to the mock graph's local `exp` tip (row 2) → CI failure; draft.
    sourceBranch: 'exp',
    targetBranch: 'main',
    comments: 0,
    createdAt: '2026-07-30T18:40:00Z',
    updatedAt: '2026-08-05T11:22:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/127',
    headSha: EXP_TIP,
  },
  {
    number: 125,
    title: 'Fix scroll jank over 20k-commit histories',
    state: 'open',
    isDraft: false,
    author: 'grace-h',
    authorAvatarUrl: null,
    // Aligned to the mock graph's local `gh-pages` tip (row 28) → CI `none`
    // (a PR badge with NO CI dot — exercises the none⇒null path in P63b).
    sourceBranch: 'gh-pages',
    targetBranch: 'main',
    comments: 1,
    createdAt: '2026-07-28T08:00:00Z',
    updatedAt: '2026-08-04T16:31:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/125',
    headSha: GH_PAGES_TIP,
  },
  {
    number: 120,
    title: 'Right-pane working-directory status panel',
    state: 'merged',
    isDraft: false,
    author: 'ada-lovelace',
    authorAvatarUrl: null,
    sourceBranch: 'feat/status-panel',
    targetBranch: 'main',
    comments: 8,
    createdAt: '2026-07-20T12:15:00Z',
    updatedAt: '2026-07-26T10:02:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/120',
    headSha: 'ffeeddccbbaa99887766554433221100ffeeddcc',
  },
];

/** Detail for PR #128 — the row with comments. Carries labels + a resolved
 *  `mergeable` so the presentational detail view (P62c) has everything. */
export const FORGE_PR_DETAIL: PrDetail = {
  summary: FORGE_PR_LIST[0],
  body:
    '## Summary\n\n' +
    'Draws a small status badge next to each commit dot in the graph, driven by\n' +
    '`CommitStatus` (rollup of the legacy status API + check-runs).\n\n' +
    '## Notes\n\n' +
    '- Badge colors follow the `CheckRollup` palette.\n' +
    '- Virtualized to visible rows — no extra DOM nodes.\n',
  mergeable: true,
  additions: 214,
  deletions: 37,
  changedFiles: 9,
  labels: ['enhancement', 'graph', 'phase-4'],
};

/** Merged review (diff-line) + conversation comments for PR #128, sorted
 *  oldest→newest (as the backend merges + sorts them). */
export const FORGE_REVIEW_COMMENTS: ReviewComment[] = [
  {
    id: 5001,
    author: 'grace-h',
    authorAvatarUrl: null,
    body: 'Love this — does the badge cache invalidate on fetch?',
    path: null,
    line: null,
    createdAt: '2026-08-02T10:00:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/128#issuecomment-5001',
    kind: 'conversation',
  },
  {
    id: 5002,
    author: 'linus-t',
    authorAvatarUrl: null,
    body: 'Prefer clamping `contexts` before the rollup, not after.',
    path: 'crates/bonsai-forge/src/github/rollup.rs',
    line: 42,
    createdAt: '2026-08-03T13:20:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/128#discussion_r5002',
    kind: 'review',
  },
  {
    id: 5003,
    author: 'ada-lovelace',
    authorAvatarUrl: null,
    body: 'Good catch — pushed a fixup that caps at 50 first.',
    path: 'crates/bonsai-forge/src/github/rollup.rs',
    line: 42,
    createdAt: '2026-08-03T15:45:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/128#discussion_r5003',
    kind: 'review',
  },
];

/** One status context with a canned target URL. */
function ctx(name: string, state: CheckRollup, description: string): StatusContext {
  return {
    name,
    state,
    description,
    targetUrl: `https://github.com/octo-org/bonsai/actions/${name.replace(/\W+/g, '-')}`,
  };
}

/** Assemble a CommitStatus, counting passed/failed/pending from `contexts` the
 *  same way the Rust rollup does (neutral/none don't count toward the tallies). */
function mkStatus(sha: string, state: CheckRollup, contexts: StatusContext[]): CommitStatus {
  let passed = 0;
  let failed = 0;
  let pending = 0;
  for (const c of contexts) {
    if (c.state === 'success') passed += 1;
    else if (c.state === 'failure' || c.state === 'error') failed += 1;
    else if (c.state === 'pending') pending += 1;
  }
  return { sha, state, total: contexts.length, passed, failed, pending, contexts };
}

/** Canned CI/commit statuses keyed by branch-tip sha — consumed by P63's graph
 *  badges. Covers every CheckRollup across the mock GraphLayout's branch tips
 *  so the harness shows one of each (success / failure / pending / neutral /
 *  none). The three OPEN fixture PRs' head shas coincide with the first, second
 *  and last entries here. */
const FORGE_COMMIT_STATUSES: Record<string, CommitStatus> = {
  // `feat` tip (PR #128 head) — all green.
  [FEAT_TIP]: mkStatus(FEAT_TIP, 'success', [
    ctx('ci/build', 'success', 'Build succeeded'),
    ctx('ci/test', 'success', '1024 passed'),
    ctx('ci/clippy', 'success', 'No warnings'),
  ]),
  // `exp` tip (PR #127 head) — a failing test drives the overall Failure.
  [EXP_TIP]: mkStatus(EXP_TIP, 'failure', [
    ctx('ci/build', 'success', 'Build succeeded'),
    ctx('ci/test', 'failure', '3 tests failed'),
  ]),
  // `main`/`dev` tip — a deploy still running ⇒ Pending (CI-only, no PR).
  [MAIN_TIP]: mkStatus(MAIN_TIP, 'pending', [
    ctx('ci/build', 'success', 'Build succeeded'),
    ctx('ci/deploy', 'pending', 'Deploying to staging'),
  ]),
  // `origin/feat` tip — a single skipped check ⇒ Neutral (CI-only, no PR).
  [ORIGIN_FEAT_TIP]: mkStatus(ORIGIN_FEAT_TIP, 'neutral', [
    ctx('ci/lint', 'neutral', 'Skipped (no matching files)'),
  ]),
  // `gh-pages` tip (PR #125 head) — no checks configured ⇒ None (nothing drawn).
  [GH_PAGES_TIP]: mkStatus(GH_PAGES_TIP, 'none', []),
};

/** Canned CI/commit status for a branch-tip sha, or null when none is defined.
 *  The mock's `forgeCommitStatuses` drops unknowns (best-effort parity with the
 *  batch contract §9); the frontend keys results by sha. */
export function commitStatusFor(sha: string): CommitStatus | null {
  return FORGE_COMMIT_STATUSES[sha] ?? null;
}
