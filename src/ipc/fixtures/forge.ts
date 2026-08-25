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
import { MOCK_OID } from './branches';
import type {
  CheckRollup,
  CommitStatus,
  ForgeAccount,
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

/** P79/P80: the seed forge account served to the Accounts settings section when
 *  the harness starts authenticated (`?forge=auth`). A warm github.com sign-in
 *  with login + avatar; `connected` true, host default. Never carries a token. */
export const FORGE_ACCOUNT_GITHUB: ForgeAccount = {
  accountId: 'gitHub:github.com:octocat',
  host: 'github.com',
  kind: 'gitHub',
  login: FORGE_VIEWER.login,
  avatarUrl: FORGE_VIEWER.avatarUrl,
  connected: true,
  isHostDefault: true,
};

/** P79: a second account with an intentionally long host + login, so the
 *  harness can verify ellipsis + tooltip truncation in the account cards. */
export const FORGE_ACCOUNT_LONG: ForgeAccount = {
  accountId: 'gitLab:gitlab.self-hosted.very-long-enterprise-subdomain.example.com:a-rather-long-enterprise-account-login-name',
  host: 'gitlab.self-hosted.very-long-enterprise-subdomain.example.com',
  kind: 'gitLab',
  login: 'a-rather-long-enterprise-account-login-name',
  avatarUrl: null,
  connected: true,
  isHostDefault: true,
};

/** P80 `?forge=multi`: a SECOND github.com account (distinct login) coexisting
 *  with {@link FORGE_ACCOUNT_GITHUB} on the same host, so the harness exercises
 *  account switching, owner match, and per-repo override without a native window.
 *  This account's login matches the multi-repo owner (`danpercic86`), so it wins
 *  an owner match; {@link FORGE_ACCOUNT_GITHUB} is the host default. */
export const FORGE_ACCOUNT_GITHUB_2: ForgeAccount = {
  accountId: 'gitHub:github.com:danpercic86',
  host: 'github.com',
  kind: 'gitHub',
  login: 'danpercic86',
  avatarUrl: null,
  connected: true,
  isHostDefault: false,
};

/** P80 `?forge=multi`: the repo owner used for the owner-match step (matches
 *  {@link FORGE_ACCOUNT_GITHUB_2}'s login, case-insensitively). */
export const FORGE_MULTI_OWNER = 'danpercic86';

/** Baseline identity for the fixture repo. The mock overrides `authenticated`
 *  + `viewer` from its live connect state before returning this. */
export const FORGE_REPO_CONTEXT: ForgeRepoContext = {
  provider: 'gitHub',
  host: 'github.com',
  owner: 'octo-org',
  repo: 'bonsai',
  project: null,
  remoteName: 'origin',
  webUrl: 'https://github.com/octo-org/bonsai',
  authenticated: false,
  viewer: null,
  resolvedAccountId: null,
  accountSource: 'none',
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
    // P72: carries the external-launch `#fail` sentinel (see
    // `mock/handlers/external.ts`) so the harness/e2e can drive the
    // "Open in browser" FAILURE toast from a real PR detail view. The three
    // open PRs above keep clean URLs for the success path.
    url: 'https://github.com/octo-org/bonsai/pull/120#fail',
    headSha: 'ffeeddccbbaa99887766554433221100ffeeddcc',
  },
  {
    // P83: an OPEN but NOT-mergeable PR (conflicts) so the harness shows the
    // disabled Merge button + not-mergeable reason, and the mock rejects a merge
    // attempt with a clear forgeApi message.
    number: 124,
    title: 'Refactor lane assignment (has conflicts with main)',
    state: 'open',
    isDraft: false,
    author: 'linus-t',
    authorAvatarUrl: null,
    sourceBranch: 'refactor/lanes',
    targetBranch: 'main',
    comments: 2,
    createdAt: '2026-07-27T10:00:00Z',
    updatedAt: '2026-08-03T09:00:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/124',
    headSha: '1122334455667788990011223344556677889900',
  },
  {
    // P83: an OPEN PR whose mergeability is still being computed (mergeable=null)
    // → Merge disabled with the "still checking" reason.
    number: 123,
    title: 'Add keyboard shortcuts for the graph pane',
    state: 'open',
    isDraft: false,
    author: 'grace-h',
    authorAvatarUrl: null,
    sourceBranch: 'feat/shortcuts',
    targetBranch: 'main',
    comments: 0,
    createdAt: '2026-07-25T14:20:00Z',
    updatedAt: '2026-08-02T12:00:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/123',
    headSha: '99aabbccddeeff00112233445566778899aabbcc',
  },
  {
    // P83: a CLOSED (not merged) PR so the action bar is absent.
    number: 119,
    title: 'Experiment: WebGL graph renderer (closed)',
    state: 'closed',
    isDraft: false,
    author: 'ada-lovelace',
    authorAvatarUrl: null,
    sourceBranch: 'exp/webgl',
    targetBranch: 'main',
    comments: 4,
    createdAt: '2026-07-18T08:00:00Z',
    updatedAt: '2026-07-22T17:00:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/119',
    headSha: 'aabbccddeeff00112233445566778899aabbccdd',
  },
];

/** P83: per-number mergeability overrides for the open fixture rows so
 *  `forgeGetPr` reports a coherent `mergeable` (false ⇒ conflicts disable Merge,
 *  null ⇒ pending). Numbers absent here fall back to the default (open ⇒ true).
 *  #124 is the not-mergeable row the mock rejects a merge attempt against. */
export const FORGE_PR_MERGEABLE: Record<number, boolean | null> = {
  124: false,
  123: null,
};

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

/** A status context with NO target URL (link-out column collapses — §2.3). */
function ctxNoUrl(name: string, state: CheckRollup, description: string | null): StatusContext {
  return { name, state, description, targetUrl: null };
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

  // --- P90: keyed by the SIDEBAR branch-snapshot tips (src/ipc/fixtures/
  // branches.ts) so clicking a branch in the sidebar drives the Checks tab to a
  // real state (the graph-tip keys above key the badge cache; these key the tab).
  //
  // `main` (MOCK_OID) — the pathological MIXED case: all five glyphs, the §4.9
  // failure-first sort, a link-out mix (one row with no target_url), and a 90-char
  // name + 200-char description + already-long branch context for ellipsis proof.
  [MOCK_OID]: mkStatus(MOCK_OID, 'failure', [
    ctx('build / linux', 'success', 'Compiled in 42s'),
    ctx('build / windows', 'success', 'Compiled in 51s'),
    ctx('test / integration', 'failure', '3 failing'),
    ctx('deploy / preview', 'error', 'Errored: timeout contacting the preview cluster'),
    ctxNoUrl('lint', 'pending', 'Queued…'),
    ctx('codecov / patch', 'neutral', 'Neutral — no coverage delta'),
    ctx(
      'e2e / very-long-suite-name-that-should-ellipsize-across-the-available-row-width-xx',
      'success',
      'Ran 1,284 scenarios across chromium, firefox and webkit; the longest scenario took 3m12s and the whole shard finished well within the configured timeout budget for this branch.',
    ),
  ]),

  // `feature/sidebar` tip ('a'*40) — all-pass (rollup pill green, §4.8).
  ['a'.repeat(40)]: mkStatus('a'.repeat(40), 'success', [
    ctx('build', 'success', 'Build succeeded'),
    ctx('test', 'success', '512 passed'),
    ctxNoUrl('format', 'success', null),
  ]),

  // `feat` tip ('d'*40) — a single failing test drives overall Failure.
  ['d'.repeat(40)]: mkStatus('d'.repeat(40), 'failure', [
    ctx('build', 'success', 'Build succeeded'),
    ctx('test', 'failure', '2 tests failed'),
  ]),

  // `exp` tip ('e'*40) — forge returns an EMPTY set ⇒ noChecks (§4.6).
  ['e'.repeat(40)]: mkStatus('e'.repeat(40), 'none', []),

  // `dev` tip ('2'*40) — a deploy still running ⇒ Pending (§3 pending pill).
  ['2'.repeat(40)]: mkStatus('2'.repeat(40), 'pending', [
    ctx('build', 'success', 'Build succeeded'),
    ctxNoUrl('deploy', 'pending', 'Deploying to staging'),
  ]),
};

/** Canned CI/commit status for a branch-tip sha, or null when none is defined.
 *  The mock's `forgeCommitStatuses` drops unknowns (best-effort parity with the
 *  batch contract §9); the frontend keys results by sha. */
export function commitStatusFor(sha: string): CommitStatus | null {
  return FORGE_COMMIT_STATUSES[sha] ?? null;
}
