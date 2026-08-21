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
  FORGE_PR_DETAIL,
  FORGE_PR_LIST,
  FORGE_PR_MERGEABLE,
  FORGE_REPO_CONTEXT,
  FORGE_REVIEW_COMMENTS,
  FORGE_VIEWER,
} from '../../fixtures/forge';
import { SUPPORTED_MERGE_METHODS } from '../../types';
import {
  accountStore,
  FORGE_EXPIRED,
  FORGE_HOST,
  FORGE_KIND,
  FORGE_MULTI,
  FORGE_PROJECT,
} from './forgeAccountStore';
import type {
  AppError,
  CommitStatus,
  CreatePrInput,
  ForgeAccount,
  ForgeKind,
  ForgeRepoContext,
  ForgeViewer,
  IpcApi,
  MergePrInput,
  PrDescription,
  PrDetail,
  PrListQuery,
  PrPage,
  PrState,
  PrSummary,
  ReviewComment,
} from '../../types';

const FORGE_OFF = urlParam('forge') === 'off';
// Provider/host selection + the P80 multi-account index live in the account
// store module (extracted to keep this file focused). `accountStore` is the
// mutable index; the sentinel consts drive the provider/host/project.
// Mutable across the browser session: forgeSetToken / forgeClearToken toggle it
// and forgeRepoContext reflects it. Seeded true by ?forge=auth and ?forge=expired
// (the token is present in both; expiry only cools the viewer, below).
let authenticated = urlParam('forge') === 'auth' || FORGE_EXPIRED || FORGE_MULTI;
// Whether the viewer cache is warm. `?forge=expired` starts token-present but
// viewer-cold; forgeInvalidateViewer cools it without clearing the token.
let viewerWarm = urlParam('forge') === 'auth' || FORGE_MULTI;
// One-shot: the first forgeListPrs under ?forge=expired rejects authFailed.
let expiredArmed = FORGE_EXPIRED;

// P83: session-persistent PR-state overlay so merge/close transitions survive
// within the browser session and forgeGetPr/forgeListPrs reflect them.
const prStateOverlay = new Map<number, PrState>();

/** The effective PR state for a number: the overlay wins over the fixture. */
function effectiveState(number: number, fixtureState: PrState): PrState {
  return prStateOverlay.get(number) ?? fixtureState;
}

/** The effective mergeability for an open PR (overlay-agnostic). */
function effectiveMergeable(number: number, state: PrState): boolean | null {
  if (state !== 'open') return null;
  return number in FORGE_PR_MERGEABLE ? FORGE_PR_MERGEABLE[number] : true;
}

/** Fetch the fixture summary for a number, applying the overlay state. */
function overlaidSummary(number: number): PrSummary | undefined {
  const summary = FORGE_PR_LIST.find((pr) => pr.number === number);
  if (!summary) return undefined;
  return { ...summary, state: effectiveState(number, summary.state) };
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
    const owner = accountStore.repoOwner();
    // Azure uses the org/project/_git/repo browser form; the others use host/owner/repo.
    const webUrl =
      FORGE_KIND === 'azureDevOps'
        ? `https://${host}/${owner}/${FORGE_PROJECT}/_git/${repo}`
        : `https://${host}/${owner}/${repo}`;
    // P80: resolve the account for this repo; authenticated/viewer reflect it.
    const { account, source } = accountStore.resolveAccount(repoId);
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
      authenticated: authenticated && (accountStore.accounts.length === 0 || resolvedConnected),
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
    const items = FORGE_PR_LIST.map((pr) => ({
      ...pr,
      state: effectiveState(pr.number, pr.state),
    })).filter((pr) => query.state === 'all' || pr.state === query.state);
    return { items, page: query.page, hasNext: false };
  },

  async forgeGetPr(repoId: string, number: number): Promise<PrDetail> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
    const summary = overlaidSummary(number);
    // #128 has a fully-authored fixture detail; unknown numbers fall back to it.
    if (summary === undefined || number === FORGE_PR_DETAIL.summary.number) {
      const base = summary ?? FORGE_PR_DETAIL.summary;
      return {
        ...FORGE_PR_DETAIL,
        summary: base,
        mergeable: effectiveMergeable(base.number, base.state),
      };
    }
    // Synthesize a COHERENT detail for the other known rows so opening e.g.
    // merged PR #120 shows its own body / mergeable / labels, not #128's.
    return {
      summary,
      body:
        `## ${summary.title}\n\n` +
        `Merges \`${summary.sourceBranch}\` into \`${summary.targetBranch}\`.\n`,
      mergeable: effectiveMergeable(summary.number, summary.state),
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

  // P83: merge a PR. Requires auth; rejects an unsupported method, a
  // non-open PR, or a not-mergeable row (#124) with a clear forgeApi message —
  // mirroring the backend. On success flips the overlay state to merged.
  async forgeMergePr(repoId: string, number: number, input: MergePrInput): Promise<PrDetail> {
    await delay(250);
    requireRepo(repoId);
    offGuard();
    if (!authenticated) {
      const err: AppError = {
        kind: 'forgeAuthRequired',
        message: 'mock: connect an account before merging a PR',
      };
      throw err;
    }
    const summary = overlaidSummary(number);
    const state = summary ? summary.state : effectiveState(number, 'open');
    if (state !== 'open') {
      const err: AppError = { kind: 'forgeApi', message: 'mock: PR is not open' };
      throw err;
    }
    if (!SUPPORTED_MERGE_METHODS[FORGE_KIND].includes(input.method)) {
      const err: AppError = {
        kind: 'forgeApi',
        message: `mock: ${input.method} not supported on ${FORGE_KIND}`,
      };
      throw err;
    }
    if (effectiveMergeable(number, state) === false) {
      const err: AppError = { kind: 'forgeApi', message: 'mock: not mergeable — conflicts' };
      throw err;
    }
    prStateOverlay.set(number, 'merged');
    return forgeHandlers.forgeGetPr(repoId, number);
  },

  // P83: close/decline/abandon a PR without merging. Requires auth; flips the
  // overlay to closed.
  async forgeClosePr(repoId: string, number: number): Promise<PrDetail> {
    await delay(200);
    requireRepo(repoId);
    offGuard();
    if (!authenticated) {
      const err: AppError = {
        kind: 'forgeAuthRequired',
        message: 'mock: connect an account before closing a PR',
      };
      throw err;
    }
    prStateOverlay.set(number, 'closed');
    return forgeHandlers.forgeGetPr(repoId, number);
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
    accountStore.upsertAccount(
      FORGE_HOST[FORGE_KIND],
      FORGE_KIND,
      FORGE_VIEWER.login,
      FORGE_VIEWER.avatarUrl,
    );
    accountStore.repoOverrides[repoId] = accountStore.accountId(
      FORGE_KIND,
      FORGE_HOST[FORGE_KIND],
      FORGE_VIEWER.login,
    );
    return FORGE_VIEWER;
  },

  async forgeClearToken(repoId: string): Promise<void> {
    await delay(120);
    requireRepo(repoId);
    offGuard();
    // P80 (OD-2): clear the repo's override ONLY; the account stays connected.
    delete accountStore.repoOverrides[repoId];
    // Reflect the default (no-account) flow where clearing removes the only
    // account and drops back to unauthenticated in the harness.
    if (accountStore.accounts.length === 0) {
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
    return accountStore.accounts.map((a) => ({ ...a }));
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
    const login = accountStore.accounts.some((a) => a.host === host && a.login === FORGE_VIEWER.login)
      ? `${FORGE_VIEWER.login}-2`
      : FORGE_VIEWER.login;
    accountStore.upsertAccount(host, kind, login, FORGE_VIEWER.avatarUrl);
    return { ...FORGE_VIEWER, login };
  },

  /** P79 back-compat alias for forgeAddAccount (same behavior). */
  async forgeSetTokenForHost(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer> {
    return forgeHandlers.forgeAddAccount(host, kind, token);
  },

  async forgeRemoveAccount(accountId: string): Promise<void> {
    await delay(120);
    offGuard();
    accountStore.removeAccountById(accountId);
  },

  async forgeSetHostDefault(host: string, accountId: string): Promise<void> {
    await delay(80);
    offGuard();
    if (!accountStore.accounts.some((a) => a.host === host && a.accountId === accountId)) {
      const err: AppError = { kind: 'other', message: 'account is not on the given host' };
      throw err;
    }
    accountStore.setHostDefault(host, accountId);
  },

  async forgeSetRepoAccount(repoId: string, accountId: string | null): Promise<void> {
    await delay(80);
    offGuard();
    if (accountId === null) delete accountStore.repoOverrides[repoId];
    else accountStore.repoOverrides[repoId] = accountId;
  },

  async forgeClearTokenForHost(host: string): Promise<void> {
    await delay(120);
    offGuard();
    accountStore.removeAccountsForHost(host);
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
