// P62b: forge / PR integration mock — offline, sentinel-aware (overview F6).
// Data is canned in ../../fixtures/forge; ZERO network. Sentinels:
//   (default)     → unauthenticated: forgeRepoContext reports authenticated:false
//                   so the panel shows <ForgeConnect>; forgeSetToken then flips it.
//   ?forge=auth   → starts authenticated (viewer warm) → the PR list renders at once.
//   ?forge=off    → every command throws {kind:'networkError'} (offline path).
//   token incl. 'bad' in forgeSetToken → throws {kind:'authFailed'} (mirrors
//                   compose's '#fail'), storing/flipping nothing.
// Spread into mockIpc via forgeHandlers.
import { AI_OFF, delay, query as urlParam, requireRepo } from '../repoState';
import {
  commitStatusFor,
  FORGE_PR_DETAIL,
  FORGE_PR_LIST,
  FORGE_REPO_CONTEXT,
  FORGE_REVIEW_COMMENTS,
  FORGE_VIEWER,
} from '../../fixtures/forge';
import type {
  AppError,
  CommitStatus,
  CreatePrInput,
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
// Mutable across the browser session: forgeSetToken / forgeClearToken toggle it
// and forgeRepoContext reflects it. Seeded true only by ?forge=auth.
let authenticated = urlParam('forge') === 'auth';

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
    return {
      ...FORGE_REPO_CONTEXT,
      authenticated,
      viewer: authenticated ? FORGE_VIEWER : null,
    };
  },

  async forgeListPrs(repoId: string, query: PrListQuery): Promise<PrPage> {
    await delay(150);
    requireRepo(repoId);
    offGuard();
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
    return FORGE_VIEWER;
  },

  async forgeClearToken(repoId: string): Promise<void> {
    await delay(120);
    requireRepo(repoId);
    offGuard();
    authenticated = false;
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
} satisfies Partial<IpcApi>;
