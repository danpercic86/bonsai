// P62b: forge / PR integration mock — offline, sentinel-aware (overview F6).
// Data is canned in ../../fixtures/forge; ZERO network. Sentinels:
//   (default)     → unauthenticated: forgeRepoContext reports authenticated:false
//                   so the panel shows <ForgeConnect>; forgeSetToken then flips it.
//   ?forge=auth   → starts authenticated (viewer warm) → the PR list renders at once.
//   ?forge=off    → every command throws {kind:'networkError'} (offline path).
//   token incl. 'bad' in forgeSetToken → throws {kind:'authFailed'} (mirrors
//                   compose's '#fail'), storing/flipping nothing.
// Spread into mockIpc via forgeHandlers.
import { delay, query as urlParam, requireRepo } from '../repoState';
import {
  FORGE_PR_DETAIL,
  FORGE_PR_LIST,
  FORGE_REPO_CONTEXT,
  FORGE_REVIEW_COMMENTS,
  FORGE_VIEWER,
} from '../../fixtures/forge';
import type {
  AppError,
  CreatePrInput,
  ForgeRepoContext,
  ForgeViewer,
  IpcApi,
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
    // Retarget the canned detail to the requested PR when it is a known row.
    const summary = FORGE_PR_LIST.find((pr) => pr.number === number) ?? FORGE_PR_DETAIL.summary;
    return { ...FORGE_PR_DETAIL, summary };
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
} satisfies Partial<IpcApi>;
