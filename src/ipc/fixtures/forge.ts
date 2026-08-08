// P62b: canned, offline forge/PR fixtures served by the mock IPC layer
// (src/ipc/mock/handlers/forge.ts). Zero network. Shapes mirror the Rust DTOs
// in crates/bonsai-forge/src/types.rs. The set covers the harness flows:
// repo context → PR list (≥3, incl. one draft + one with comments) → detail
// (labels + mergeable) → review/conversation comments → viewer, plus one
// CommitStatus for the P63 graph-badge wiring.
import type {
  CommitStatus,
  ForgeRepoContext,
  ForgeViewer,
  PrDetail,
  PrSummary,
  ReviewComment,
} from '../types';

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
 *  `open` filter shows 3 rows and `all` shows 4. */
export const FORGE_PR_LIST: PrSummary[] = [
  {
    number: 128,
    title: 'Render PR/CI status badges beside graph nodes',
    state: 'open',
    isDraft: false,
    author: 'ada-lovelace',
    authorAvatarUrl: null,
    sourceBranch: 'feat/graph-badges',
    targetBranch: 'main',
    comments: 3,
    createdAt: '2026-08-01T09:12:00Z',
    updatedAt: '2026-08-06T14:05:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/128',
    headSha: '1a2b3c4d5e6f70819aabbccddeeff00112233445',
  },
  {
    number: 127,
    title: 'WIP: deterministic lane colors while scrolling',
    state: 'open',
    isDraft: true,
    author: 'linus-t',
    authorAvatarUrl: null,
    sourceBranch: 'wip/lane-colors',
    targetBranch: 'main',
    comments: 0,
    createdAt: '2026-07-30T18:40:00Z',
    updatedAt: '2026-08-05T11:22:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/127',
    headSha: 'aabbccddeeff00112233445566778899aabbccdd',
  },
  {
    number: 125,
    title: 'Fix scroll jank over 20k-commit histories',
    state: 'open',
    isDraft: false,
    author: 'grace-h',
    authorAvatarUrl: null,
    sourceBranch: 'fix/scroll-jank',
    targetBranch: 'main',
    comments: 1,
    createdAt: '2026-07-28T08:00:00Z',
    updatedAt: '2026-08-04T16:31:00Z',
    url: 'https://github.com/octo-org/bonsai/pull/125',
    headSha: '99887766554433221100ffeeddccbbaa99887766',
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

/** One normalized commit status for PR #128's head sha — consumed by P63's
 *  graph-badge rendering. Overall `success` (all three checks passed). */
export const FORGE_COMMIT_STATUS: CommitStatus = {
  sha: '1a2b3c4d5e6f70819aabbccddeeff00112233445',
  state: 'success',
  total: 3,
  passed: 3,
  failed: 0,
  pending: 0,
  contexts: [
    {
      name: 'ci/build',
      state: 'success',
      description: 'Build succeeded',
      targetUrl: 'https://github.com/octo-org/bonsai/actions/runs/1001',
    },
    {
      name: 'ci/test',
      state: 'success',
      description: '1024 passed',
      targetUrl: 'https://github.com/octo-org/bonsai/actions/runs/1002',
    },
    {
      name: 'ci/clippy',
      state: 'success',
      description: 'No warnings',
      targetUrl: 'https://github.com/octo-org/bonsai/actions/runs/1003',
    },
  ],
};
