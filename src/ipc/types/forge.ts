import type { FileDiffHeader } from './diff';

// --- P62 forge / PR integration (mirrors crates/bonsai-forge/src/types.rs) ---

/** Which forge backs `origin` (detected from the remote URL). */
export type ForgeKind = 'gitHub' | 'gitLab' | 'bitbucket' | 'azureDevOps' | 'unknown';
/** PR lifecycle state. */
export type PrState = 'open' | 'closed' | 'merged';
/** List-query filter (maps to GitHub's `?state=`). */
export type PrStateFilter = 'open' | 'closed' | 'all';
/** Normalized CI/commit-status rollup (defined in P62; P63 renders it as a
 *  commit-graph badge). */
export type CheckRollup = 'success' | 'pending' | 'failure' | 'error' | 'neutral' | 'none';
/** Whether a comment is a diff-line review comment or a PR conversation comment. */
export type CommentKind = 'review' | 'conversation';

/** The authenticated user (GitHub `GET /user`). */
export interface ForgeViewer {
  login: string;
  avatarUrl: string | null;
}
/** Repo identity derived from `origin` + keychain presence (no network). */
export interface ForgeRepoContext {
  provider: ForgeKind;
  host: string;
  owner: string;
  repo: string;
  /** Azure DevOps' team project (`owner` carries the org). Null elsewhere. */
  project: string | null;
  remoteName: string;
  webUrl: string;
  /** A token is present in the keychain for `host` (no network check). */
  authenticated: boolean;
  /** Non-null only when a validated viewer is cache-warm (after set-token). */
  viewer: ForgeViewer | null;
  /** P80: the account resolved for this repo (`accountId`), or null when no
   *  account exists on the host. */
  resolvedAccountId: string | null;
  /** P80: how the resolved account was chosen. */
  accountSource: AccountSource;
}
/** P80: how the account backing a repo was resolved (see `resolve_account`). */
export type AccountSource = 'override' | 'ownerMatch' | 'hostDefault' | 'single' | 'none';
/** P79/P80: one connected/known forge account for the Accounts settings section.
 *  `login`/`avatarUrl` are best-effort display hints; never a token. */
export interface ForgeAccount {
  /** P80: stable identity "kind:host:login" (or "kind:host" if login unknown). */
  accountId: string;
  host: string;
  kind: ForgeKind;
  login: string | null;
  avatarUrl: string | null;
  connected: boolean;
  /** P80: whether this account is the host's default (repos inherit it). */
  isHostDefault: boolean;
}
/** One row in a PR list. */
export interface PrSummary {
  number: number;
  title: string;
  state: PrState;
  isDraft: boolean;
  author: string;
  authorAvatarUrl: string | null;
  /** head ref (branch name only). */
  sourceBranch: string;
  /** base ref (branch name only). */
  targetBranch: string;
  comments: number;
  createdAt: string;
  updatedAt: string;
  /** `html_url` for opening in a browser. */
  url: string;
  /** head sha, for the P63 status lookup. */
  headSha: string;
}
/** A single PR with its body + diff stats. */
export interface PrDetail {
  summary: PrSummary;
  /** Markdown body; may be empty. */
  body: string;
  /** null while GitHub is still computing mergeability. */
  mergeable: boolean | null;
  additions: number;
  deletions: number;
  changedFiles: number;
  labels: string[];
}
/** P89: locally-computed base…head (three-dot) diff stats for a PR. Counts +
 *  changed-files list are computed by bonsai-core from the fetched endpoints,
 *  replacing the forge-reported `PrDetail` counts once loaded. `files` is
 *  headers-only (sorted path-ascending); per-file hunks are fetched on demand
 *  via `forgePrFileDiff`. Mirrors the Rust `PrDiffStats`. */
export interface PrDiffStats {
  additions: number;
  deletions: number;
  changedFiles: number;
  /** merge-base(base,head) — the OLD side of the diff; "" for unrelated histories. */
  mergeBaseOid: string;
  baseOid: string;
  headOid: string;
  /** Sorted path-ascending; headers only (hunks fetched per file). */
  files: FileDiffHeader[];
}
/** PR list request. `perPage` is capped `<= 50` by the provider. */
export interface PrListQuery {
  state: PrStateFilter;
  page: number;
  perPage: number;
}
/** One page of PR summaries. `hasNext` derives from the `Link` header. */
export interface PrPage {
  items: PrSummary[];
  page: number;
  hasNext: boolean;
}
/** Inputs for creating a PR. `maintainerCanModify` defaults to true in the UI. */
export interface CreatePrInput {
  title: string;
  body: string;
  sourceBranch: string;
  targetBranch: string;
  draft: boolean;
  maintainerCanModify: boolean;
}
/** Neutral merge strategy. Not every variant is valid on every forge — the UI
 *  filters via {@link SUPPORTED_MERGE_METHODS}. */
export type MergeMethod = 'merge' | 'squash' | 'rebase' | 'fastForward';
/** Inputs for merging a PR (mirrors `bonsai_forge::MergePrInput`). */
export interface MergePrInput {
  method: MergeMethod;
  commitTitle: string | null;
  commitMessage: string | null;
  deleteSourceBranch: boolean;
  /** Azure only; filled backend-side when null. */
  headSha: string | null;
}
/** Merge methods each forge supports, in display order; the first entry is the
 *  forge default. Mirrors `MergeMethod::supported_for` — keep in sync. */
export const SUPPORTED_MERGE_METHODS: Record<ForgeKind, MergeMethod[]> = {
  gitHub: ['merge', 'squash', 'rebase'],
  gitLab: ['merge', 'squash'],
  bitbucket: ['merge', 'squash', 'fastForward'],
  azureDevOps: ['merge', 'squash', 'rebase'],
  unknown: [],
};
/** A merged review/conversation comment. */
export interface ReviewComment {
  id: number;
  author: string;
  authorAvatarUrl: string | null;
  body: string;
  path: string | null;
  line: number | null;
  createdAt: string;
  url: string;
  kind: CommentKind;
}
/** One check/status context inside a {@link CommitStatus}. */
export interface StatusContext {
  name: string;
  state: CheckRollup;
  description: string | null;
  targetUrl: string | null;
}
/** Merged legacy-status + check-runs rollup for one commit. Defined + populated
 *  in P62; wired to an IPC command + rendered as a graph badge in P63.
 *  `contexts` is capped at 50 individual checks. */
export interface CommitStatus {
  sha: string;
  state: CheckRollup;
  total: number;
  passed: number;
  failed: number;
  pending: number;
  contexts: StatusContext[];
}

/** P63: external "open PR N" request threaded from a graph PR-badge click into
 *  the P62 `PrPanel`. `seq` is a bump counter so clicking the SAME PR badge
 *  twice re-navigates (the panel keys its open-detail effect on `seq`). */
export interface PrNavRequest {
  number: number;
  seq: number;
}

/** P64: an AI-generated PR proposal (title + Markdown body) grounded in the
 *  commits unique to `head` vs `base`, plus the echoed requested range + cost.
 *  Mirrors the Rust `PrDescription`; generating WRITES NOTHING and never posts —
 *  it fills the create-PR form for the user to review/edit before Create.
 *  `body` may be `''` (why-not-what). */
export interface PrDescription {
  title: string;
  body: string;
  base: string;
  head: string;
  commitCount: number;
  costUsd: number | null;
}
