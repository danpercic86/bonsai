import type {
  CommitStatus,
  CreatePrInput,
  ForgeAccount,
  ForgeKind,
  ForgeRepoContext,
  ForgeViewer,
  MergePrInput,
  PrDetail,
  PrListQuery,
  PrPage,
  ReviewComment,
} from './forge';

/** P62+: forge / PR integration + global forge account management, split out of
 *  `IpcApi` (which extends this) to keep whole-file reads cheap. Behaviour is
 *  identical — this is the same method surface, unchanged. */
export interface IpcApiForge {
  /** Repo identity from `origin` + keychain presence (no network). An
   *  unrecognized/unparseable origin returns a friendly `unknown`-provider
   *  context, NOT an error. Rejects AppError (`noRepo` | `noRemote` | `git`). */
  forgeRepoContext(repoId: string): Promise<ForgeRepoContext>;
  /** One page of PR summaries for the state filter (`perPage` capped at 50).
   *  Rejects AppError (`noRepo` | `forgeUnsupported` | `noRemote` |
   *  `forgeRateLimited` | `forgeApi` | `networkError` | `git`). */
  forgeListPrs(repoId: string, query: PrListQuery): Promise<PrPage>;
  /** A single PR (body, diff stats, mergeable, labels). Rejects AppError
   *  (`noRepo` | `forgeUnsupported` | `forgeApi` | `forgeRateLimited` |
   *  `networkError` | `git`). */
  forgeGetPr(repoId: string, number: number): Promise<PrDetail>;
  /** Open a new PR; REQUIRES a stored token. Rejects AppError (`noRepo` |
   *  `forgeAuthRequired` | `forgeUnsupported` | `forgeApi` | `forgeRateLimited`
   *  | `networkError` | `git`). */
  forgeCreatePr(repoId: string, input: CreatePrInput): Promise<PrDetail>;
  /** Merge a PR; REQUIRES a stored token. Never force-merges — a not-mergeable
   *  PR rejects with a clear `forgeApi` message and changes nothing. Rejects
   *  AppError (`noRepo` | `forgeAuthRequired` | `forgeUnsupported` | `forgeApi`
   *  | `forgeRateLimited` | `authFailed` | `networkError` | `git`). */
  forgeMergePr(repoId: string, number: number, input: MergePrInput): Promise<PrDetail>;
  /** Close/decline/abandon a PR WITHOUT merging; REQUIRES a stored token.
   *  Rejects AppError (`noRepo` | `forgeAuthRequired` | `forgeUnsupported` |
   *  `forgeApi` | `forgeRateLimited` | `authFailed` | `networkError` | `git`). */
  forgeClosePr(repoId: string, number: number): Promise<PrDetail>;
  /** Merged review + conversation comments, sorted by creation time. Rejects
   *  AppError (`noRepo` | `forgeUnsupported` | `forgeApi` | `forgeRateLimited`
   *  | `networkError` | `git`). */
  forgeListReviewComments(repoId: string, number: number): Promise<ReviewComment[]>;
  /** Validate a pasted PAT (`GET /user`) and store it in the OS keychain keyed
   *  by host; resolves with the authenticated viewer. A rejected token stores
   *  NOTHING and the token is never logged/echoed. Rejects AppError (`noRepo` |
   *  `authFailed` | `forgeUnsupported` | `noRemote` | `forgeRateLimited` |
   *  `networkError`). */
  forgeSetToken(repoId: string, token: string): Promise<ForgeViewer>;
  /** Sign out: delete the host's PAT from the keychain + evict the cached
   *  viewer. Idempotent. Rejects AppError (`noRepo` | `noRemote`). */
  forgeClearToken(repoId: string): Promise<void>;
  /** P63: batch commit/CI statuses for graph badges — one CommitStatus per
   *  requested sha, in the SAME order (one round-trip / one spawn_blocking).
   *  Rejects AppError (`noRepo` | `forgeUnsupported` | `noRemote` | `forgeApi`
   *  | `forgeRateLimited` | `authFailed` | `networkError` | `git`). */
  forgeCommitStatuses(repoId: string, shas: string[]): Promise<CommitStatus[]>;
  /** P80: all forge accounts across all hosts (the settings index), each with
   *  live `connected` + `isHostDefault` + best-effort login/avatar. No network.
   *  Rejects AppError (`other`). */
  forgeListAccounts(): Promise<ForgeAccount[]>;
  /** P80: validate + store a PAT for `host`/`kind` directly (no repo), learn the
   *  login, store under a three-part keychain key, upsert the account, and set it
   *  as the host default if none exists. Rejects AppError (`authFailed` |
   *  `forgeUnsupported` | `forgeRateLimited` | `networkError` | `other`). */
  forgeAddAccount(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer>;
  /** P79 back-compat alias for {@link forgeAddAccount} (same behavior). */
  forgeSetTokenForHost(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer>;
  /** P80: delete an account's token (by its keychain key), remove the record, and
   *  clean references (host default, repo overrides). Idempotent. Rejects
   *  AppError (`other`). */
  forgeRemoveAccount(accountId: string): Promise<void>;
  /** P80: set/replace the default account for `host`. Rejects AppError (`other`)
   *  if `accountId` isn't on the host. */
  forgeSetHostDefault(host: string, accountId: string): Promise<void>;
  /** P80: pin (`accountId`) or clear (`null` ⇒ inherit) a repo's account
   *  override. Rejects AppError (`noRepo` | `other`). */
  forgeSetRepoAccount(repoId: string, accountId: string | null): Promise<void>;
  /** P79: sign out ALL accounts on a host — delete their tokens + records +
   *  defaults + overrides. Idempotent. Rejects AppError (`other`). */
  forgeClearTokenForHost(host: string): Promise<void>;
  /** P79: evict a host's cached viewer WITHOUT deleting the token (expiry flow).
   *  Infallible. */
  forgeInvalidateViewer(host: string): Promise<void>;
}
