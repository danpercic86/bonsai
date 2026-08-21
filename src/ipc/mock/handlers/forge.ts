// P62b: forge / PR integration mock — offline, sentinel-aware (overview F6).
// Data is canned in ../../fixtures/forge; ZERO network. Sentinels:
//   (default)     → unauthenticated: forgeRepoContext reports authenticated:false
//                   so the panel shows <ForgeConnect>; forgeSetToken then flips it.
//   ?forge=auth   → starts authenticated (viewer warm) → the PR list renders at once.
//   ?forge=off    → every command throws {kind:'networkError'} (offline path).
//   ?forge=expired→ token present but viewer cold; the first forgeListPrs throws
//                   {kind:'authFailed'} once, driving the P79 reauth flow (§4).
//   token incl. 'bad' in forgeSetToken(ForHost) → throws {kind:'authFailed'}
//                   (mirrors compose's '#fail'), storing/flipping nothing.
// P79: a module-level `accounts` index backs the global Accounts settings
// section; forgeSetToken*/clear* keep it in sync so both views agree.
// Spread into mockIpc via forgeHandlers.
import { AI_OFF, delay, query as urlParam, requireRepo } from '../repoState';
import {
  commitStatusFor,
  FORGE_ACCOUNT_GITHUB,
  FORGE_ACCOUNT_GITHUB_2,
  FORGE_ACCOUNT_LONG,
  FORGE_MULTI_OWNER,
  FORGE_PR_DETAIL,
  FORGE_PR_LIST,
  FORGE_REPO_CONTEXT,
  FORGE_REVIEW_COMMENTS,
  FORGE_VIEWER,
} from '../../fixtures/forge';
import type {
  AccountSource,
  AppError,
  CommitStatus,
  CreatePrInput,
  ForgeAccount,
  ForgeKind,
  ForgeRepoContext,
  ForgeViewer,
  IpcApi,
  PrDescription,
  PrDetail,
  PrListQuery,
  PrPage,
  PrSummary,
  ReviewComment,
} from '../../types';

const FORGE_OFF = urlParam('forge') === 'off';
// P64b/P64c/P64d: a `?forge=gitlab|bitbucket|azure` sentinel makes
// forgeRepoContext report that provider so the panel exercises connect → list →
// detail → create for a non-GitHub forge. The neutral PR/status fixtures render
// UNCHANGED (the mock sits at the neutral-DTO boundary) — only the repo-context
// provider/host (+ Azure's project) swap.
// `?forge=unsupported` ⇒ provider 'unknown' (an origin that is no known SaaS
// forge) so the harness/e2e can exercise the PrPanel unsupported empty state.
const FORGE_KIND: ForgeKind =
  urlParam('forge') === 'gitlab'
    ? 'gitLab'
    : urlParam('forge') === 'bitbucket'
      ? 'bitbucket'
      : urlParam('forge') === 'azure'
        ? 'azureDevOps'
        : urlParam('forge') === 'unsupported'
          ? 'unknown'
          : 'gitHub';
// Host + web URL matching the detected provider, so the connect hint + "open in
// browser" links look right in the harness.
const FORGE_HOST: Record<ForgeKind, string> = {
  gitHub: FORGE_REPO_CONTEXT.host,
  gitLab: 'gitlab.com',
  bitbucket: 'bitbucket.org',
  azureDevOps: 'dev.azure.com',
  // A self-hosted-looking origin so the unsupported state names a non-forge host.
  unknown: 'git.example.com',
};
// Azure DevOps needs a 3-part org/project/repo identity; the mock supplies a
// sample project so the harness renders the Azure context faithfully. `null`
// for every other provider (matches the real backend).
const FORGE_PROJECT: string | null = FORGE_KIND === 'azureDevOps' ? 'sample-project' : null;
// P79: `?forge=expired` seeds a host whose token is still present (authenticated)
// but whose viewer is COLD, and arms a one-shot authFailed on the first
// forgeListPrs so the harness exercises the expiry → reconnect (§4) flow.
const FORGE_EXPIRED = urlParam('forge') === 'expired';
// P80: `?forge=multi` seeds TWO github.com accounts (distinct logins) with one
// host default, so the harness exercises account switching, owner match, and
// per-repo override without a native window.
const FORGE_MULTI = urlParam('forge') === 'multi';
// Mutable across the browser session: forgeSetToken / forgeClearToken toggle it
// and forgeRepoContext reflects it. Seeded true by ?forge=auth and ?forge=expired
// (the token is present in both; expiry only cools the viewer, below).
let authenticated = urlParam('forge') === 'auth' || FORGE_EXPIRED || FORGE_MULTI;
// Whether the viewer cache is warm. `?forge=expired` starts token-present but
// viewer-cold; forgeInvalidateViewer cools it without clearing the token.
let viewerWarm = urlParam('forge') === 'auth' || FORGE_MULTI;
// One-shot: the first forgeListPrs under ?forge=expired rejects authFailed.
let expiredArmed = FORGE_EXPIRED;

// P80: the module-level multi-account state (mirrors the backend settings index).
// Seeded from ?forge=auth (one warm github.com account + a long-content account),
// ?forge=expired (github.com present but viewer cold), and ?forge=multi (TWO
// github.com accounts, one host default). forgeSetToken*/add/remove/clear keep it
// in sync so the PR-panel and settings views agree in the harness.
let accounts: ForgeAccount[] = FORGE_MULTI
  ? [{ ...FORGE_ACCOUNT_GITHUB }, { ...FORGE_ACCOUNT_GITHUB_2 }]
  : urlParam('forge') === 'auth'
    ? [{ ...FORGE_ACCOUNT_GITHUB }, { ...FORGE_ACCOUNT_LONG }]
    : FORGE_EXPIRED
      ? [{ ...FORGE_ACCOUNT_GITHUB, login: null, avatarUrl: null }]
      : [];
// host → default accountId (repos inherit it).
const hostDefaults: Record<string, string> =
  FORGE_MULTI || urlParam('forge') === 'auth' || FORGE_EXPIRED
    ? { 'github.com': FORGE_ACCOUNT_GITHUB.accountId }
    : {};
// repoId → pinned accountId (per-repo override).
const repoOverrides: Record<string, string> = {};

/** The `accountId` for a host/login (mirrors the backend `account_id`). */
function accountId(kind: ForgeKind, host: string, login: string | null): string {
  const base = `${kind}:${host.toLowerCase()}`;
  return login ? `${base}:${login.toLowerCase()}` : base;
}

/** The owner/namespace used for the owner-match resolution step. */
function repoOwner(): string {
  return FORGE_MULTI ? FORGE_MULTI_OWNER : FORGE_REPO_CONTEXT.owner;
}

/** P80 §4 resolution mirror: per-repo override → owner match (single
 *  login==owner) → host default → single → first. Pure. */
function resolveAccount(repoId: string): { account: ForgeAccount | null; source: AccountSource } {
  const host = FORGE_HOST[FORGE_KIND];
  const onHost = accounts.filter((a) => a.host === host);
  if (onHost.length === 0) return { account: null, source: 'none' };
  // 1. per-repo override (a manual pin always wins).
  const pinned = repoOverrides[repoId];
  if (pinned) {
    const a = onHost.find((x) => x.accountId === pinned);
    if (a) return { account: a, source: 'override' };
    // deleted pin → fall through (never error).
  }
  // 2. owner match (login==owner, case-insensitive, exactly one).
  const owner = repoOwner().toLowerCase();
  if (owner) {
    const matches = onHost.filter((a) => (a.login ?? '').toLowerCase() === owner);
    if (matches.length === 1) return { account: matches[0], source: 'ownerMatch' };
  }
  // 3. host default.
  const def = hostDefaults[host];
  if (def) {
    const a = onHost.find((x) => x.accountId === def);
    if (a) return { account: a, source: 'hostDefault' };
  }
  // 4. single.
  if (onHost.length === 1) return { account: onHost[0], source: 'single' };
  // 5. multiple, no default → first (most-recent); UI nudges.
  return { account: onHost[0], source: 'hostDefault' };
}

/** Insert-or-replace an account keyed by accountId (mirrors the backend upsert).
 *  Sets the host default when none exists. Never stores a token. */
function upsertAccount(host: string, kind: ForgeKind, login: string | null, avatarUrl: string | null): void {
  const id = accountId(kind, host, login);
  accounts = accounts.filter((a) => a.accountId !== id);
  const isHostDefault = !hostDefaults[host] || hostDefaults[host] === id;
  accounts.unshift({ accountId: id, host, kind, login, avatarUrl, connected: true, isHostDefault });
  if (!hostDefaults[host]) hostDefaults[host] = id;
  syncHostDefaultFlags();
}

/** Remove an account by accountId, cleaning references (promote/clear the host
 *  default, drop repo overrides). Idempotent. */
function removeAccountById(id: string): void {
  const rec = accounts.find((a) => a.accountId === id);
  accounts = accounts.filter((a) => a.accountId !== id);
  for (const k of Object.keys(repoOverrides)) {
    if (repoOverrides[k] === id) delete repoOverrides[k];
  }
  if (rec && hostDefaults[rec.host] === id) {
    delete hostDefaults[rec.host];
    const next = accounts.find((a) => a.host === rec.host);
    if (next) hostDefaults[rec.host] = next.accountId;
  }
  syncHostDefaultFlags();
}

/** Sign out every account on a host (P79 clear-token-for-host). */
function removeAccountsForHost(host: string): void {
  const ids = new Set(accounts.filter((a) => a.host === host).map((a) => a.accountId));
  accounts = accounts.filter((a) => a.host !== host);
  delete hostDefaults[host];
  for (const k of Object.keys(repoOverrides)) {
    if (ids.has(repoOverrides[k])) delete repoOverrides[k];
  }
  syncHostDefaultFlags();
}

/** Recompute each account's `isHostDefault` from `hostDefaults`. */
function syncHostDefaultFlags(): void {
  accounts = accounts.map((a) => ({ ...a, isHostDefault: hostDefaults[a.host] === a.accountId }));
}

/** `?forge=off` ⇒ simulate an offline/unreachable forge on every command. */
function offGuard(): void {
  if (FORGE_OFF) {
    const err: AppError = { kind: 'networkError', message: 'mock: forge is offline (?forge=off)' };
    throw err;
  }
}

export const forgeHandlers = {
  async forgeRepoContext(repoId: string): Promise<ForgeRepoContext> {
    await delay(120);
    requireRepo(repoId);
    offGuard();
    const host = FORGE_HOST[FORGE_KIND];
    const { repo } = FORGE_REPO_CONTEXT;
    const owner = repoOwner();
    // Azure uses the org/project/_git/repo browser form; the others use host/owner/repo.
    const webUrl =
      FORGE_KIND === 'azureDevOps'
        ? `https://${host}/${owner}/${FORGE_PROJECT}/_git/${repo}`
        : `https://${host}/${owner}/${repo}`;
    // P80: resolve the account for this repo; authenticated/viewer reflect it.
    const { account, source } = resolveAccount(repoId);
    const resolvedConnected = account?.connected ?? false;
    return {
      ...FORGE_REPO_CONTEXT,
      provider: FORGE_KIND,
      project: FORGE_PROJECT,
      owner,
      ...(FORGE_KIND === 'gitHub' ? {} : { host, webUrl }),
      // `authenticated` follows the live connect toggle AND a resolved account
      // (both must hold: the token toggle drives ?forge=auth/off flows, the
      // resolved account drives ?forge=multi).
      authenticated: authenticated && (accounts.length === 0 || resolvedConnected),
      viewer: authenticated && viewerWarm ? FORGE_VIEWER : null,
      resolvedAccountId: account?.accountId ?? null,
      accountSource: source,
    };
  },

  async forgeListPrs(repoId: string, query: PrListQuery): Promise<PrPage> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
    // P79 (§4): under ?forge=expired the first list call rejects authFailed once,
    // driving the PR panel into the reauth flow (invalidate viewer + reconnect).
    if (expiredArmed) {
      expiredArmed = false;
      const err: AppError = { kind: 'authFailed', message: 'mock: saved token expired or was revoked' };
      throw err;
    }
    const items = FORGE_PR_LIST.filter((pr) => query.state === 'all' || pr.state === query.state);
    return { items, page: query.page, hasNext: false };
  },

  async forgeGetPr(repoId: string, number: number): Promise<PrDetail> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
    const summary = FORGE_PR_LIST.find((pr) => pr.number === number);
    // #128 has a fully-authored fixture detail; unknown numbers fall back to it.
    if (summary === undefined || summary.number === FORGE_PR_DETAIL.summary.number) {
      return { ...FORGE_PR_DETAIL, summary: summary ?? FORGE_PR_DETAIL.summary };
    }
    // Synthesize a COHERENT detail for the other known rows so opening e.g.
    // merged PR #120 shows its own body / mergeable / labels, not #128's.
    return {
      summary,
      body:
        `## ${summary.title}\n\n` +
        `Merges \`${summary.sourceBranch}\` into \`${summary.targetBranch}\`.\n`,
      mergeable: summary.state === 'open' ? true : null,
      additions: 40 + (summary.number % 50),
      deletions: 10 + (summary.number % 20),
      changedFiles: 1 + (summary.number % 6),
      labels: summary.isDraft ? ['work-in-progress'] : [],
    };
  },

  async forgeCreatePr(repoId: string, input: CreatePrInput): Promise<PrDetail> {
    await delay(200);
    requireRepo(repoId);
    offGuard();
    if (!authenticated) {
      const err: AppError = {
        kind: 'forgeAuthRequired',
        message: 'mock: connect a GitHub account before opening a PR',
      };
      throw err;
    }
    // Echo the submitted fields into a fresh open PR detail.
    const summary: PrSummary = {
      ...FORGE_PR_DETAIL.summary,
      number: 999,
      title: input.title,
      state: 'open',
      isDraft: input.draft,
      author: FORGE_VIEWER.login,
      sourceBranch: input.sourceBranch,
      targetBranch: input.targetBranch,
      comments: 0,
      url: 'https://github.com/octo-org/bonsai/pull/999',
    };
    return { ...FORGE_PR_DETAIL, summary, body: input.body, mergeable: null, labels: [] };
  },

  async forgeListReviewComments(repoId: string, _number: number): Promise<ReviewComment[]> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
    return FORGE_REVIEW_COMMENTS;
  },

  async forgeSetToken(repoId: string, token: string): Promise<ForgeViewer> {
    await delay(200);
    requireRepo(repoId);
    offGuard();
    // Mirrors compose's '#fail' sentinel: a token containing 'bad' is rejected
    // by the (mock) GET /user validation, and nothing is stored/flipped.
    if (token.includes('bad')) {
      const err: AppError = { kind: 'authFailed', message: 'mock: token rejected by GET /user' };
      throw err;
    }
    authenticated = true;
    viewerWarm = true;
    // P80 (OD-3): add/update the account AND pin it as this repo's override.
    upsertAccount(FORGE_HOST[FORGE_KIND], FORGE_KIND, FORGE_VIEWER.login, FORGE_VIEWER.avatarUrl);
    repoOverrides[repoId] = accountId(FORGE_KIND, FORGE_HOST[FORGE_KIND], FORGE_VIEWER.login);
    return FORGE_VIEWER;
  },

  async forgeClearToken(repoId: string): Promise<void> {
    await delay(120);
    requireRepo(repoId);
    offGuard();
    // P80 (OD-2): clear the repo's override ONLY; the account stays connected.
    delete repoOverrides[repoId];
    // Reflect the default (no-account) flow where clearing removes the only
    // account and drops back to unauthenticated in the harness.
    if (accounts.length === 0) {
      authenticated = false;
      viewerWarm = false;
    }
  },

  // P64: AI PR-description generation (provider-agnostic; pure local git + the
  // claude CLI in the real backend). Read-only; WRITES NOTHING; never posts. The
  // proposal fills the create-PR form for the user to review/edit before Create.
  // Sentinels: `?ai=off` ⇒ aiUnavailable (the consent/CLI gate); a `#fail` marker
  // in `head` ⇒ aiFailed; else a canned title + short Markdown body echoing the
  // resolved base/head so the harness shows what was grounded.
  async aiGeneratePrDescription(
    repoId: string,
    base: string,
    head: string,
  ): Promise<PrDescription> {
    await delay(700);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiUnavailable',
        message: 'mock: AI features are disabled (?ai=off)',
      };
      throw err;
    }
    if (head.includes('#fail')) {
      const err: AppError = {
        kind: 'aiFailed',
        message: `mock: nothing to describe: ${head} has no commits beyond ${base}`,
      };
      throw err;
    }
    return {
      title: `Add ${head} onto ${base}`,
      body: [
        `Bring the work on \`${head}\` into \`${base}\`.`,
        '',
        '## Changes',
        '- Wire the AI PR-description command behind the create-PR form',
        '- Ground the draft in the real base..head commits and net diffstat',
        '',
        '## Notes',
        'Generated locally — review and edit before opening the pull request.',
      ].join('\n'),
      base,
      head,
      commitCount: 3,
      costUsd: 0.009,
    };
  },

  async forgeCommitStatuses(repoId: string, shas: string[]): Promise<CommitStatus[]> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
    // Best-effort parity with the batch contract (§9): map each sha via
    // commitStatusFor, dropping unknowns (the real backend omits not-found).
    // The frontend keys the result by sha, so order/gaps are harmless.
    return shas
      .map((sha) => commitStatusFor(sha))
      .filter((s): s is CommitStatus => s !== null);
  },

  // P79: global forge account management (repo-independent). Offline, mirrors the
  // backend's index sync + error text. Never carries a token.
  async forgeListAccounts(): Promise<ForgeAccount[]> {
    await delay(120);
    offGuard();
    return accounts.map((a) => ({ ...a }));
  },

  async forgeAddAccount(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer> {
    await delay(200);
    offGuard();
    // OD-6: Azure DevOps has no repo-less identity endpoint (mirror the backend).
    if (kind === 'azureDevOps') {
      const err: AppError = {
        kind: 'forgeUnsupported',
        message: 'Azure DevOps accounts must be added from an open Azure DevOps repository',
      };
      throw err;
    }
    // Mirrors forgeSetToken's 'bad' sentinel: a rejected token stores nothing.
    if (token.includes('bad')) {
      const err: AppError = { kind: 'authFailed', message: 'mock: token rejected by GET /user' };
      throw err;
    }
    // A distinct login per host lets the harness add a SECOND github.com account.
    const login = accounts.some((a) => a.host === host && a.login === FORGE_VIEWER.login)
      ? `${FORGE_VIEWER.login}-2`
      : FORGE_VIEWER.login;
    upsertAccount(host, kind, login, FORGE_VIEWER.avatarUrl);
    return { ...FORGE_VIEWER, login };
  },

  /** P79 back-compat alias for forgeAddAccount (same behavior). */
  async forgeSetTokenForHost(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer> {
    return forgeHandlers.forgeAddAccount(host, kind, token);
  },

  async forgeRemoveAccount(accountId: string): Promise<void> {
    await delay(120);
    offGuard();
    removeAccountById(accountId);
  },

  async forgeSetHostDefault(host: string, accountId: string): Promise<void> {
    await delay(80);
    offGuard();
    if (!accounts.some((a) => a.host === host && a.accountId === accountId)) {
      const err: AppError = { kind: 'other', message: 'account is not on the given host' };
      throw err;
    }
    hostDefaults[host] = accountId;
    syncHostDefaultFlags();
  },

  async forgeSetRepoAccount(repoId: string, accountId: string | null): Promise<void> {
    await delay(80);
    offGuard();
    if (accountId === null) delete repoOverrides[repoId];
    else repoOverrides[repoId] = accountId;
  },

  async forgeClearTokenForHost(host: string): Promise<void> {
    await delay(120);
    offGuard();
    removeAccountsForHost(host);
  },

  async forgeInvalidateViewer(host: string): Promise<void> {
    await delay(60);
    // Expiry flow: cool the viewer WITHOUT clearing the token (the account stays
    // in the index; forgeRepoContext now reports viewer:null, authenticated:true).
    if (host === FORGE_HOST[FORGE_KIND]) {
      viewerWarm = false;
    }
  },
} satisfies Partial<IpcApi>;
