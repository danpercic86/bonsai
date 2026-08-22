import type { AiAnalysis, AiAnalysisMode, AiAvailability, AiChangelog, AiDiffTarget, AiDigestRange, AiResolveBatch, AiResolveProposal, AiRunEvent, AiSummary, BranchNameProposal, BranchNameSource, ChangelogRange, CommitMessageProposal, ComposeApplyResult, ComposePlan, ComposeProposal } from './ai';
import type { AgentAsset, AgentAssetInput, AgentAssetInventory, AgentAssetKind, AiAssetInventory, AiGeneratedAsset, AssetContent, ContextProfile, ProfileActivation, ProfilePreviewEntry, ProfileStore, WorktreeContextStatus } from './ai-assets';
import type { BranchDeleteResult, BranchesSnapshot, CheckoutResult, CreateBranchHereResult, MergeOutcome, RebaseOutcome, RebaseTodoOp, RemoteInfo, RenameBranchResult, StaleReport, TagSyncReport } from './branches';
import type { CherrypickOutcome, CommitResult, RevertOutcome } from './commit';
import type { BisectOutcome, CloneProgress, GitAvailability, OpenRepoResult, RecentRepo, RepoChangedPayload, RepoOpState, ResetMode, Unsubscribe } from './common';
import type { ConfigLevelArg, ConfigView } from './config';
import type { ConflictEntry, ConflictFile, ConflictResolution } from './conflict';
import type { CommitDiff, CompareDiff, FileDiff, ImageDiff, ImageDiffRequest, LineSelection } from './diff';
import type { CommitStatus, CreatePrInput, ForgeAccount, ForgeKind, ForgeRepoContext, ForgeViewer, MergePrInput, PrDescription, PrDetail, PrListQuery, PrPage, ReviewComment } from './forge';
import type { GraphChunk, GraphLayout } from './graph';
import type { RepoHealth } from './health';
import type { RepoHooksDisclosure } from './hooks';
import type { BlameLine, FileHistoryEntry, ReflogEntry, UndoPlan } from './history';
import type { JobKind, JobStatus, JobStatusChangedPayload } from './jobs';
import type { McpStatus, SessionState } from './mcp';
import type { FetchResult, PullResult, PushResult } from './remotes';
import type { OperationPlan } from './safe-op';
import type { HistoryAnswer, HistoryQuery, HistorySearchResults, IndexProgress, IndexStatus, SearchQuery, SearchResults } from './search';
import type { UiSettings, UiSettingsPatch } from './settings';
import type { SigningStatus, VerifyResults } from './signing';
import type { ApplyStashOutcome, CreateStashResult, StashEntry, StashScope } from './stash';
import type { StatusSnapshot } from './status';
import type { SubmoduleDeinitOutcome, SubmoduleInfo, SubmoduleRemoveOutcome } from './submodule';
import type { UpdateCheckResult, UpdateProgress } from './update';
import type { CopyCandidate, CopyPlanEntry, CopySelection, WorktreeInfo } from './worktree';

export interface IpcApi {
  /** Open (or focus) a repo. Returns the canonical `repoId` + info. A usable
   *  repo (isRepo && !bare) creates/refreshes a keyed entry; re-opening an
   *  already-open path focuses it (same `repoId`, no reset). Rejects {@link AppError}. */
  openRepo(path: string): Promise<OpenRepoResult>;
  /** Clone `url` into `dest`, streaming progress via `onProgress`. Resolves to the
   *  absolute workdir path of the clone (caller then opens it as a tab). The frontend
   *  passes a plain callback; the Tauri impl bridges it through a `Channel`, the mock
   *  invokes it directly. Rejects io | authFailed | networkError | git. */
  cloneRepo(url: string, dest: string, onProgress: (p: CloneProgress) => void): Promise<string>;
  /** Initialize (or open, if already a repo) a repository at `path`. Resolves to the
   *  absolute workdir path. Rejects io | git. */
  initRepo(path: string): Promise<string>;
  /** Close a repo and tear down its watcher. Idempotent (unknown id ⇒ resolves). */
  closeRepo(repoId: string): Promise<void>;
  /** Resolves to `null` when the user cancels the dialog. */
  pickFolder(): Promise<string | null>;
  /** Rejects with {@link AppError} (`noRepo` when the id is not open). */
  getStatus(repoId: string): Promise<StatusSnapshot>;
  /** Full graph layout for a repo. Rejects with {@link AppError} (`noRepo` when the id is not open). */
  getGraph(repoId: string): Promise<GraphLayout>;
  /** P65: stream the graph layout for a repo as ordered chunks (meta -> batch* ->
   *  done). The frontend passes a plain callback; the Tauri impl bridges it
   *  through a `Channel`, the mock invokes it directly. Resolves when the stream
   *  completes (after the `done` chunk). Rejects with {@link AppError} (`noRepo`
   *  when the id is not open, `git`). `getGraph` is retained (small-repo/tests). */
  streamGraph(repoId: string, onChunk: (c: GraphChunk) => void): Promise<void>;
  /** Stage paths (worktree-relative, forward slashes — StatusEntry.path strings). Atomic. */
  stage(repoId: string, paths: string[]): Promise<void>;
  /** Unstage paths. Atomic. Safe (worktree never touched). */
  unstage(repoId: string, paths: string[]): Promise<void>;
  /** Create a commit from the index. `sign` (P58): null/undefined ⇒ follow
   *  `commit.gpgsign`; true ⇒ force sign; false ⇒ force unsigned. `skipHooks`
   *  (P59a): true ≡ `--no-verify`; null/undefined/false ⇒ run hooks per
   *  `bonsai.runHooks` (default true). Rejects with AppError kinds emptyMessage |
   *  configMissing | nothingToCommit | hookRejected | git | noRepo. */
  commit(
    repoId: string,
    message: string,
    sign?: boolean | null,
    skipHooks?: boolean,
  ): Promise<CommitResult>;
  /** Diff of one working-dir file. staged=false: index vs workdir; staged=true: HEAD vs index.
   *  origPath: pass StatusEntry.origPath (renames). Rejects AppError ('noRepo', 'git'). */
  getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
    /** P61a: when true, paired add/del lines carry `spans` (word-level ranges). */
    intraline: boolean,
  ): Promise<FileDiff>;
  /** Commit details + per-file headers vs first parent. Rejects AppError ('noRepo', 'git'). */
  getCommitDiff(repoId: string, oid: string): Promise<CommitDiff>;
  /** Hunks for one file of a commit's first-parent diff. `fullContext` true ->
   *  one whole-file hunk (File View). */
  getCommitFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    /** P61a: when true, paired add/del lines carry `spans` (word-level ranges). */
    intraline: boolean,
  ): Promise<FileDiff>;
  /** Stage only the selected changed lines of one working-dir file (index moves
   *  toward the workdir). Empty selection is a no-op. Rejects AppError
   *  ('noRepo' | 'git' | 'other'[stale/unsupported/invalid path]). */
  stagePartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void>;
  /** Unstage only the selected changed lines of one staged file (index moves
   *  toward HEAD). Empty selection is a no-op. Same rejections. */
  unstagePartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void>;
  /** Discard the selected changed lines of one tracked working-dir file: the
   *  WORKTREE moves toward the INDEX; the index is never modified. DESTRUCTIVE —
   *  callers must confirm first. Empty selection is a no-op. Rejects AppError
   *  ('noRepo' | 'git'[untracked] | 'other'[stale/unsupported/invalid path]). */
  discardPartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void>;
  /** Tree-vs-tree diff between HEAD (old) and `oid` (new): `git diff HEAD <oid>`.
   *  HEAD is resolved server-side (detached ok; unborn -> empty old tree). Empty
   *  `files` when `oid` IS HEAD. Rejects {@link AppError} (`noRepo`, `git`). */
  compareWithHead(repoId: string, oid: string): Promise<CompareDiff>;
  /** Hunks for one file of the HEAD → `oid` comparison. `origPath`: pass the
   *  FileDiffHeader.origPath for renames. Rejects AppError (`noRepo`, `git`). */
  compareWithHeadFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    /** P61a: when true, paired add/del lines carry `spans` (word-level ranges). */
    intraline: boolean,
  ): Promise<FileDiff>;
  /** P61b: both sides of an image comparison as base64 (D2). `request` picks the
   *  context (workdir/commit/compare). Rejects AppError (`noRepo`, `git`). */
  getImageDiff(repoId: string, request: ImageDiffRequest): Promise<ImageDiff>;
  /** Local branches + remotes + tags + HEAD in one snapshot. Rejects noRepo | git. */
  listBranches(repoId: string): Promise<BranchesSnapshot>;
  /** Create branch at current HEAD (no checkout). Rejects
   *  invalidName | branchExists | git | noRepo. */
  createBranch(repoId: string, name: string): Promise<void>;
  /** Create local branch `name` at commit `oid`, auto-stashing/re-applying
   *  uncommitted work across the checkout. Rejects invalidName | branchExists
   *  | operationInProgress | configMissing | checkoutConflict | git | noRepo. */
  createBranchHere(repoId: string, name: string, oid: string): Promise<CreateBranchHereResult>;
  /** Dirty-safe checkout of a LOCAL branch (P33): auto-stash → switch → auto
   *  fast-forward to upstream (no fetch) → re-apply stash. A conflicted re-apply
   *  is a SUCCESS carrying `apply: {kind:'conflicts'}` (stash retained). Rejects
   *  branchNotFound | operationInProgress | configMissing | checkoutConflict |
   *  git | noRepo. */
  checkoutBranch(repoId: string, name: string): Promise<CheckoutResult>;
  /** Delete a LOCAL, fully merged, non-current branch. Rejects
   *  branchNotFound | unmergedBranch | git | noRepo. */
  deleteBranch(repoId: string, name: string): Promise<void>;
  /** Rename a local branch (git branch -m). Preserves upstream + reflog; rewrites
   *  HEAD when the renamed branch is checked out. Rejects
   *  invalidName | branchNotFound | branchExists | git | noRepo. */
  renameBranch(repoId: string, oldName: string, newName: string): Promise<RenameBranchResult>;
  /** GitKraken-style remote checkout: create/reuse a local tracking branch for
   *  `name` ("<remote>/<branch>") and switch to it. Rejects
   *  invalidName | branchNotFound | checkoutConflict | git | noRepo. */
  checkoutRemoteBranch(repoId: string, name: string): Promise<void>;
  /** Delete the LOCAL remote-tracking ref `name` (does NOT touch the server).
   *  Rejects branchNotFound | git | noRepo. */
  deleteRemoteBranch(repoId: string, name: string): Promise<void>;
  /** Classify local branches safe to delete (merged into `base` OR upstream-gone).
   *  Read-only; `base` auto-resolves when omitted. Rejects git | noRepo. */
  listStaleBranches(repoId: string, base?: string): Promise<StaleReport>;
  /** Batch-delete the given branch names that are STILL safe (server re-verifies
   *  against a fresh stale set + not-current + not-base). Per-branch outcomes are
   *  DATA, never thrown. Rejects git (bad base) | noRepo. */
  deleteBranches(repoId: string, names: string[], base?: string): Promise<BranchDeleteResult[]>;
  /** Fetch ALL remotes. Rejects noRemote | authFailed | networkError | git | noRepo. */
  fetch(repoId: string): Promise<FetchResult>;
  /** Fetch upstream remote + fast-forward only. Rejects noUpstream | authFailed
   *  | networkError | checkoutConflict | git | noRepo. */
  pull(repoId: string): Promise<PullResult>;
  /** Push current branch (sets upstream to origin/<branch> when none). `skipHooks`
   *  (P59a-2): true ≡ `git push --no-verify`; otherwise the `pre-push` hook runs
   *  first and a non-zero exit rejects with `hookRejected`. Rejects
   *  noRemote | authFailed | networkError | pushRejected | hookRejected | git | noRepo. */
  push(repoId: string, skipHooks?: boolean): Promise<PushResult>;
  /** Force-push the current branch to its upstream WITH A LEASE (P37). Refuses
   *  (pushRejected) if the remote moved since the last fetch. `skipHooks` (P59a-2)
   *  as {@link push}. Rejects noUpstream | noRemote | authFailed | networkError
   *  | pushRejected | hookRejected | git | noRepo. */
  forcePush(repoId: string, skipHooks?: boolean): Promise<PushResult>;
  /** Current operation state (merge/rebase/...). Part of the refresh batch.
   *  Rejects noRepo | git. */
  getOpState(repoId: string): Promise<RepoOpState>;
  /** Merge a local or remote-tracking branch into the current branch. Rejects
   *  operationInProgress | branchNotFound | checkoutConflict | configMissing
   *  | git | noRepo. */
  mergeBranch(repoId: string, name: string): Promise<MergeOutcome>;
  /** Finalize a paused merge. `skipHooks` (P59a) as {@link commit}. Rejects
   *  noOperationInProgress | unresolvedConflicts | emptyMessage | configMissing
   *  | hookRejected | git | noRepo. */
  commitMerge(repoId: string, message: string, skipHooks?: boolean): Promise<CommitResult>;
  /** Abort a paused merge (worktree-destructive for merge-touched files).
   *  Rejects noOperationInProgress | git | noRepo. */
  abortMerge(repoId: string): Promise<void>;
  /** Whether this repo has runnable git hooks and whether the user has been shown
   *  the one-time execution disclosure. Drives the frontend hook-disclosure gate
   *  (commit/amend/merge-commit/push). Rejects noRepo | git. */
  getRepoHooksDisclosure(repoId: string): Promise<RepoHooksDisclosure>;
  /** Record that the user acknowledged this repo's hook disclosure (persisted,
   *  per-repo). Idempotent. Rejects noRepo | git. */
  ackRepoHooks(repoId: string): Promise<void>;
  /** All current index conflicts, path-ascending. Rejects noRepo | git. */
  listConflicts(repoId: string): Promise<ConflictEntry[]>;
  /** Read-only marker view of one conflicted file. Rejects noRepo | git. */
  getConflict(repoId: string, path: string): Promise<ConflictFile>;
  /** Resolve one conflicted path. Rejects noRepo | git | invalidName. */
  resolveConflict(repoId: string, path: string, resolution: ConflictResolution): Promise<void>;
  /** Stage user-authored resolved text for one conflicted path (P12).
   *  Rejects noRepo | git | invalidName. */
  resolveConflictText(repoId: string, path: string, content: string): Promise<void>;
  /** P68 #7 / H1: stage an AI-proposed resolution, gated server-side by the novel-content check.
   *  Rejects aiNeedsReview (body has lines in no version) | aiFailed | git | invalidName | noRepo. */
  aiApplyResolution(repoId: string, path: string, content: string): Promise<void>;
  /** Start a rebase of the current branch onto `onto` (local or remote-tracking
   *  shorthand). Rejects operationInProgress | branchNotFound | checkoutConflict
   *  | configMissing | git | noRepo. */
  rebaseBranch(repoId: string, onto: string): Promise<RebaseOutcome>;
  /** Resume a paused rebase. Rejects noOperationInProgress | unresolvedConflicts
   *  | configMissing | git | noRepo. */
  rebaseContinue(repoId: string): Promise<RebaseOutcome>;
  /** Skip the current operation and resume. Rejects noOperationInProgress
   *  | configMissing | git | noRepo. */
  rebaseSkip(repoId: string): Promise<RebaseOutcome>;
  /** Abort a paused rebase (worktree-destructive). Rejects noOperationInProgress
   *  | git | noRepo. */
  rebaseAbort(repoId: string): Promise<void>;
  /** Default interactive-rebase todo list (all `pick`, oldest-first) for the
   *  first-parent range `baseOid..HEAD`, seeding the plan editor. Rejects
   *  git | noRepo. */
  getInteractivePlan(repoId: string, baseOid: string): Promise<RebaseTodoOp[]>;
  /** Start an interactive rebase of the current branch onto `ontoOid`, replaying
   *  `todos` in the given order. Clean → `rebased`; conflict → `conflicts`
   *  (pauses into RepoOpState.rebase, driven by the existing OpBanner +
   *  rebaseContinue/Skip/Abort). Rejects operationInProgress | checkoutConflict
   *  | configMissing | git | noRepo. */
  startInteractiveRebase(
    repoId: string,
    ontoOid: string,
    todos: RebaseTodoOp[],
  ): Promise<RebaseOutcome>;
  /** Start a git bisect: `bad` = known-bad commit, `good` = one or more
   *  known-good ancestors. Detaches HEAD onto the first midpoint; progress
   *  surfaces via getOpState (RepoOpState.bisect). Rejects operationInProgress
   *  | git | noRepo. */
  startBisect(repoId: string, bad: string, good: string[]): Promise<BisectOutcome>;
  /** Mark the current bisect midpoint good (`isGood: true`) or bad, then pick
   *  the next midpoint or converge. Rejects noOperationInProgress | git | noRepo. */
  bisectMark(repoId: string, isGood: boolean): Promise<BisectOutcome>;
  /** Skip the current (untestable) bisect midpoint. Rejects
   *  noOperationInProgress | git | noRepo. */
  bisectSkip(repoId: string): Promise<BisectOutcome>;
  /** Abort/finish a bisect: restore the original HEAD/branch + worktree
   *  (destructive — confirm first). Rejects noOperationInProgress | git | noRepo. */
  bisectReset(repoId: string): Promise<void>;
  /** Per-line blame of `path` as of `atOid` (null → HEAD). Read-only. Rejects
   *  other (bad path) | git (binary/unknown/too large/invalid oid) | noRepo. */
  blameFile(repoId: string, path: string, atOid: string | null): Promise<BlameLine[]>;
  /** Commits that touched `path`, newest-first, capped at `limit`. An unknown
   *  path yields `[]` (not an error). Rejects other | git | noRepo. */
  fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]>;
  /** Reflog for `refName` ("HEAD" or a local branch name), newest-first, capped.
   *  A never-updated ref yields `[]` (not an error). Read-only. Rejects git | noRepo. */
  readReflog(repoId: string, refName: string): Promise<ReflogEntry[]>;
  /** Describe how to reverse the last HEAD-moving op (P60c). READ-ONLY: reads
   *  HEAD reflog[0], classifies it, and returns an `UndoPlan` (target + reset
   *  mode + safety flags). Execution reuses `resetBranch`. Rejects git | noRepo. */
  describeLastUndo(repoId: string): Promise<UndoPlan>;
  /** Commit/content search (P50a). Dispatches by `query.field`: message/author/
   *  all via a header-only git2 revwalk; path/content via `git log`. Capped
   *  (`truncated` when more may exist). Empty/whitespace `text` resolves to
   *  `{ matches: [], truncated: false }`. Read-only, does NOT emit repo-changed.
   *  Rejects git (bad pathspec / invalid `-G` regex) | noRepo. */
  searchCommits(repoId: string, query: SearchQuery): Promise<SearchResults>;
  /** Effective signing config for the commit-box indicator/toggle (P58a D6).
   *  Read-only; does NOT emit repo-changed. Rejects noRepo | git. */
  signingStatus(repoId: string): Promise<SigningStatus>;
  /** Verify signatures for a bounded set of commit oids (P58b) — the visible
   *  graph rows. Read-only; does NOT emit repo-changed. ONE git subprocess per
   *  call, capped at MAX_VERIFY_BATCH. Non-hex oids are dropped and unresolvable
   *  ones omitted; a missing gpg/ssh toolchain degrades to `cannotCheck` rather
   *  than rejecting. Rejects git | noRepo. */
  verifyCommits(repoId: string, oids: string[]): Promise<VerifyResults>;
  /** Build/refresh the per-commit semantic-search INDEX (BM25 over message+diff),
   *  streaming `IndexProgress`. Incremental: only commits absent from the store are
   *  (re)documented. Writes to the app data dir keyed by repo — NOT the repo; does
   *  NOT emit repo-changed. NOT AI-gated. The frontend passes a plain callback; the
   *  Tauri impl bridges it through a `Channel`, the mock invokes it directly.
   *  Rejects git | io | noRepo. */
  historyIndexBuild(repoId: string, onProgress: (p: IndexProgress) => void): Promise<IndexStatus>;
  /** Cheap status of the persisted index (built?, count, staleness vs current
   *  refs). Read-only, NOT AI-gated, does NOT emit repo-changed. Rejects git | noRepo. */
  historyIndexStatus(repoId: string): Promise<IndexStatus>;
  /** Relevance-ranked retrieval over the persisted index (pure IR; NOT AI-gated).
   *  Empty/whitespace `text` ⇒ { hits: [], ... }. No index ⇒ { hits: [],
   *  indexStale: true, indexedCommits: 0 } (UI offers Build). Read-only, does NOT
   *  emit repo-changed. Rejects io | noRepo. */
  historySearch(repoId: string, query: HistoryQuery): Promise<HistorySearchResults>;
  /** Retrieve the top-`topK` relevant commits from the persisted index, then
   *  synthesize an NL answer grounded in their REAL diffs via the local `claude`
   *  CLI (P57c). Read-only; WRITES NOTHING; does NOT emit repo-changed. AI-gated.
   *  `topK` 0 ⇒ backend default. Rejects aiUnavailable (CLI off / consent off) |
   *  aiFailed (no index / no relevant commits / CLI error) | git | noRepo. */
  aiSearchHistory(repoId: string, question: string, topK: number): Promise<HistoryAnswer>;
  /** Config view for `level` of `repoId`: curated keys (effective value + level
   *  + target-level value) + advanced entries. Read-only. Rejects git | noRepo. */
  getConfig(repoId: string, level: ConfigLevelArg): Promise<ConfigView>;
  /** Write `value` to `key` at `level`. Validated server-side (key shape, enum
   *  value). Rejects invalidName | git | noRepo. Does NOT emit repo-changed. */
  setConfig(repoId: string, level: ConfigLevelArg, key: string, value: string): Promise<void>;
  /** Remove `key` at `level` (idempotent). Rejects invalidName | git | noRepo. */
  unsetConfig(repoId: string, level: ConfigLevelArg, key: string): Promise<void>;
  /** Apply an identity (live in-memory profile fields, NOT a persisted id) to
   *  `repoId`'s Local git config; returns the refreshed Local ConfigView.
   *  Rejects noRepo | invalidName | git. */
  applyIdentityProfile(
    repoId: string,
    userName: string,
    userEmail: string,
    signingKey: string | null,
  ): Promise<ConfigView>;
  /** Stash stack, index 0 (most recent) first. Rejects noRepo | git. */
  listStashes(repoId: string): Promise<StashEntry[]>;
  /** Stash the worktree per `scope`. message=null → git default. created:false ==
   *  nothing in that scope to stash (NOT an error). `scope: 'staged'` captures only
   *  index-vs-HEAD paths (mixed files folded whole), leaving unstaged-only edits and
   *  untracked files in the worktree. Rejects operationInProgress | configMissing |
   *  git | noRepo. */
  createStash(
    repoId: string,
    message: string | null,
    scope: StashScope,
  ): Promise<CreateStashResult>;
  /** Apply stash `index` WITHOUT dropping. Rejects operationInProgress | git | noRepo.
   *  `skipReserved`: on first attempt (false) a stash containing Windows-reserved
   *  paths returns `reservedPaths` and applies nothing; retry with true to apply
   *  everything except those (`appliedSkippingReserved`).
   *  `expectedOid` (F-A6-B): the oid the UI rendered for this stack index. When
   *  provided and it no longer matches the entry at `index`, the backend rejects
   *  with git "stash list changed; refresh and retry" BEFORE touching anything,
   *  guarding against a stack shift between render and confirm. */
  applyStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
    expectedOid?: string,
  ): Promise<ApplyStashOutcome>;
  /** Apply + drop on clean success (retained on conflict). Rejects operationInProgress | git | noRepo.
   *  `skipReserved`: as for `applyStash`; when any reserved path is skipped the
   *  stash is KEPT (not dropped) so the reserved blobs are not lost.
   *  `expectedOid`: as for {@link applyStash} — wrong-target guard (F-A6-B). */
  popStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
    expectedOid?: string,
  ): Promise<ApplyStashOutcome>;
  /** Permanently discard stash `index` (UI confirms). Rejects git | noRepo.
   *  `expectedOid`: as for {@link applyStash} — wrong-target guard (F-A6-B). */
  dropStash(repoId: string, index: number, expectedOid?: string): Promise<void>;
  /** Amend HEAD with a new message + the current index (P20). Preserves HEAD's
   *  parents + original author. `sign` (P58) + `skipHooks` (P59a): as
   *  {@link commit}. Rejects operationInProgress | emptyMessage | configMissing
   *  | hookRejected | git | noRepo. */
  commitAmend(
    repoId: string,
    message: string,
    sign?: boolean | null,
    skipHooks?: boolean,
  ): Promise<CommitResult>;
  /** Move the current branch (HEAD) to `oid` in `mode` (P20). Hard is
   *  destructive — the UI confirms first. Rejects operationInProgress | git | noRepo. */
  resetBranch(repoId: string, oid: string, mode: ResetMode): Promise<void>;
  /** Restore tracked worktree files to the index version, discarding unstaged
   *  edits (P20). Destructive — the UI confirms first. Rejects other | git | noRepo. */
  discardPaths(repoId: string, paths: string[]): Promise<void>;
  /** Force-discard a mixed set: tracked paths restored to the index version,
   *  untracked paths deleted from disk (P36). Destructive — the UI confirms
   *  first. Rejects other (invalid path) | io | git | noRepo. */
  discardPathsForce(repoId: string, paths: string[]): Promise<void>;
  /** Cherry-pick a single commit onto the current branch (P20, P47). Clean →
   *  committed; conflict → pauses into RepoOpState.cherryPick. `message` (P47):
   *  omit/null → reuse the picked commit's message; a string overrides it. A
   *  dirty tracked worktree is autostashed first. Rejects operationInProgress |
   *  git | checkoutConflict | configMissing | nothingToCommit | noRepo. */
  cherrypickCommit(
    repoId: string,
    oid: string,
    message?: string | null,
  ): Promise<CherrypickOutcome>;
  /** Finalize a paused (resolved) cherry-pick (P20). Rejects
   *  noOperationInProgress | unresolvedConflicts | configMissing |
   *  nothingToCommit | git | noRepo. */
  cherrypickContinue(repoId: string): Promise<CherrypickOutcome>;
  /** Abort a paused cherry-pick (reset --hard; UI confirms). Rejects
   *  noOperationInProgress | git | noRepo. */
  cherrypickAbort(repoId: string): Promise<void>;
  /** Revert a single commit on the current branch (P20). Clean → committed;
   *  conflict → pauses into RepoOpState.revert. Rejects operationInProgress |
   *  git | checkoutConflict | configMissing | nothingToCommit | noRepo. */
  revertCommit(repoId: string, oid: string): Promise<RevertOutcome>;
  /** Finalize a paused (resolved) revert (P20). Rejects noOperationInProgress |
   *  unresolvedConflicts | configMissing | nothingToCommit | git | noRepo. */
  revertContinue(repoId: string): Promise<RevertOutcome>;
  /** Abort a paused revert (reset --hard; UI confirms). Rejects
   *  noOperationInProgress | git | noRepo. */
  revertAbort(repoId: string): Promise<void>;
  /** All submodules with classified status. Rejects noRepo | git. */
  listSubmodules(repoId: string): Promise<SubmoduleInfo[]>;
  /** Register `name` in .git/config (no worktree change). Rejects noRepo | invalidName | git. */
  initSubmodule(repoId: string, name: string): Promise<void>;
  /** Init-if-needed + fetch + checkout the pinned commit. Rejects
   *  noRepo | invalidName | authFailed | networkError | git. */
  updateSubmodule(repoId: string, name: string): Promise<void>;
  /** Copy the .gitmodules URL into config + the submodule remote. Rejects noRepo | invalidName | git. */
  syncSubmodule(repoId: string, name: string): Promise<void>;
  /** P60d: add a submodule from `url` at repo-relative `path` (clones it).
   *  Rejects noRepo | invalidName | git. */
  addSubmodule(repoId: string, url: string, path: string): Promise<SubmoduleInfo>;
  /** P60d/P82: deinit — clear config + empty worktree; keep .gitmodules.
   *  `force=false` refuses (`dirtyNeedsForce`) when the submodule worktree is
   *  dirty, mutating nothing; re-invoke with `force=true` to discard.
   *  Rejects noRepo | invalidName | git. */
  deinitSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleDeinitOutcome>;
  /** P60d/P82: remove entirely (deinit + git rm + drop .git/modules). DESTRUCTIVE.
   *  `force` semantics as `deinitSubmodule`. Rejects noRepo | invalidName | git. */
  removeSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleRemoveOutcome>;
  // --- P27: worktrees ---
  /** All worktrees (main first) with resolved branch/oid/badges. Rejects noRepo | git. */
  listWorktrees(repoId: string): Promise<WorktreeInfo[]>;
  /** Create a worktree checking out `branch`, at a derived
   *  `<parent>/.worktrees/<repo>/<name-slug>` path. `name` is the user-editable
   *  on-disk label (defaults to the branch in the UI, decoupled from it —
   *  P32 Part A; a blank `name` defaults to `branch`). Returns the created row.
   *  Rejects noRepo | invalidName | branchNotFound | git | io. */
  addWorktree(repoId: string, branch: string, name: string): Promise<WorktreeInfo>;
  /** Remove worktree `name` (refuses main/current/locked/dirty; deletes the
   *  directory from disk). Rejects noRepo | invalidName | git | io. */
  removeWorktree(repoId: string, name: string): Promise<void>;
  /** Lock worktree `name` with an optional reason. Rejects noRepo | invalidName | git. */
  lockWorktree(repoId: string, name: string, reason?: string): Promise<void>;
  /** Unlock worktree `name`. Rejects noRepo | invalidName | git. */
  unlockWorktree(repoId: string, name: string): Promise<void>;
  /** Uncommitted + gitignored files eligible to copy into a new worktree
   *  (deletions excluded), grouped staged/unstaged/untracked/ignored.
   *  Rejects noRepo | git. */
  listCopyCandidates(repoId: string): Promise<CopyCandidate[]>;
  /** Classify `paths` against `branch` (clean/conflict) BEFORE creating the
   *  worktree. Rejects noRepo | branchNotFound | git. */
  previewWorktreeCopy(repoId: string, branch: string, paths: string[]): Promise<CopyPlanEntry[]>;
  /** Create the worktree (branch/name per Part A) then copy each `copy`
   *  selection in; `skip` selections are not written; empty == plain create.
   *  Rejects noRepo | invalidName | branchNotFound | git | io. */
  addWorktreeWithChanges(
    repoId: string,
    branch: string,
    name: string,
    selections: CopySelection[],
  ): Promise<WorktreeInfo>;
  // --- P29: repo health ---
  /** All four repo-health sections in one round-trip (READ-ONLY). Per-section
   *  failures land in `Section.error` inside the payload; the call itself
   *  rejects only noRepo | other (join). */
  getRepoHealth(repoId: string): Promise<RepoHealth>;
  // --- P22: tags ---
  /** Create a tag at `targetOid`. `message` non-null ⇒ annotated (needs git identity),
   *  null ⇒ lightweight. `force` overwrites (v1 UI passes false). Rejects
   *  noRepo | invalidName | configMissing | git. */
  createTag(
    repoId: string,
    name: string,
    targetOid: string,
    message: string | null,
    force: boolean,
  ): Promise<void>;
  /** Delete a LOCAL tag (does not touch any remote). Rejects noRepo | invalidName | git. */
  deleteTag(repoId: string, name: string): Promise<void>;
  /** Push refs/tags/<tagName> to `remote`. `force` false in v1. Rejects
   *  noRepo | noRemote | authFailed | networkError | pushRejected | git. */
  pushTag(repoId: string, remote: string, tagName: string, force: boolean): Promise<void>;
  // --- P77: tag sync ---
  /** Live tag reconciliation vs `remote` (null => default remote). One ls-remote
   *  round-trip; best-effort — callers must render the plain tags list even when
   *  this rejects. Rejects noRepo | noRemote | authFailed | networkError | git. */
  listTagSync(repoId: string, remote: string | null): Promise<TagSyncReport>;
  /** Force-update one local tag from `remote`. Rejects noRepo | invalidName |
   *  noRemote | authFailed | networkError | git. */
  forceRefreshTag(repoId: string, remote: string, tagName: string): Promise<void>;
  /** Delete a tag on `remote` (destructive — confirm first). Rejects noRepo |
   *  invalidName | noRemote | authFailed | networkError | pushRejected | git. */
  deleteRemoteTag(repoId: string, remote: string, tagName: string): Promise<void>;
  // --- P22: remotes ---
  /** Configured remotes (name + fetch URL). Rejects noRepo | git. */
  listRemotes(repoId: string): Promise<RemoteInfo[]>;
  /** Add a remote. Rejects noRepo | invalidName | git. */
  addRemote(repoId: string, name: string, url: string): Promise<void>;
  /** Remove a remote (drops its tracking refs). Rejects noRepo | noRemote | git. */
  removeRemote(repoId: string, name: string): Promise<void>;
  /** Rename a remote. Rejects noRepo | noRemote | invalidName | git. */
  renameRemote(repoId: string, name: string, newName: string): Promise<void>;
  /** Set a remote's fetch URL. Rejects noRepo | noRemote | git. */
  setRemoteUrl(repoId: string, name: string, url: string): Promise<void>;
  /** Recent successfully-opened repos, most recent first, max 10. Never rejects
   *  for a missing/corrupt settings file (returns []). */
  getRecentRepos(): Promise<RecentRepo[]>;
  /** Removes one entry; returns the updated list. */
  removeRecentRepo(path: string): Promise<RecentRepo[]>;
  /** Fires after debounced filesystem changes; payload carries the `repoId`. */
  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe>;
  /** Fires when the app window regains focus. */
  onWindowFocus(cb: () => void): Promise<Unsubscribe>;
  /** P30. Background-job status for one open repo — exactly 2 entries
   *  (autoFetch, healthRefresh). Rejects noRepo. */
  getJobStatus(repoId: string): Promise<JobStatus[]>;
  /** P30 D10. Fire-and-forget manual run: resolves once the job STARTS; the
   *  result arrives via onJobStatusChanged. Ignores backoff delay. Rejects
   *  noRepo | other("job already running"). */
  runJobNow(repoId: string, job: JobKind): Promise<void>;
  /** P30. Fires on every job completion/skip; small push signal. */
  onJobStatusChanged(cb: (p: JobStatusChangedPayload) => void): Promise<Unsubscribe>;
  /** Current UI settings (theme + pane widths). Never rejects for a missing/corrupt file. */
  getUiSettings(): Promise<UiSettings>;
  /** Applies a partial patch (only defined fields) and returns the resulting settings. */
  setUiSettings(patch: UiSettingsPatch): Promise<UiSettings>;
  /** P49: launch the OS terminal at `path` (a repo/worktree/submodule dir). Uses
   *  the configured terminalCommand template (empty ⇒ auto-detect). Rejects
   *  AppError('externalToolFailed' | 'io'). */
  openInTerminal(path: string): Promise<void>;
  /** P49: reveal `path` in the OS file manager. Rejects AppError('externalToolFailed' | 'io'). */
  revealInFileManager(path: string): Promise<void>;
  /** P49: open `path` in the configured editor (empty ⇒ auto-detect VS Code).
   *  Rejects AppError('externalToolFailed' | 'io'). */
  openInEditor(path: string): Promise<void>;
  /** P72: open `url` in the user's default browser. Web URLs only — a non-http(s)
   *  scheme, a hostless URL, or a leading `-` is refused before anything spawns.
   *  Rejects AppError('externalToolFailed'). */
  openUrl(url: string): Promise<void>;
  /** P70: resolve the `git` executable and report availability. Cheap, one-shot
   *  at startup, re-invocable from the banner's Re-check. Never rejects for git
   *  state — a missing git is `{ found: false, ... }`. */
  checkGitAvailability(): Promise<GitAvailability>;
  /** Cheap Claude Code CLI health probe (P13). Never rejects for CLI state. */
  checkAiAvailability(): Promise<AiAvailability>;
  /** Propose an AI merge resolution for one conflicted path (P13). Writes nothing.
   *  Rejects aiUnavailable | aiFailed | git | invalidName | noRepo. */
  aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal>;
  /** P68 §D. STREAMING AI resolution for 1..n conflicted paths — a single file is
   *  literally `paths.length === 1` (A1). Writes NOTHING (D4): the returned bodies
   *  are proposals that still have to go through `hasUnresolvedMarkers` and the
   *  explicit `resolveConflictText` call.
   *
   *  `onEvent` receives every `AiRunEvent` as it happens; the FIRST one (`started`)
   *  carries the `runId` needed by `aiCancelRun` / `aiReplyRun` (D8) — the promise
   *  settles only when the run is over, so waiting for it is too late to cancel.
   *  Rejects aiUnavailable | aiFailed (incl. "too many AI runs in progress …" when
   *  the backend concurrency cap is hit) | aiCancelled | git | invalidName | noRepo. */
  aiResolveConflictStream(
    repoId: string,
    paths: string[],
    onEvent: (e: AiRunEvent) => void,
  ): Promise<AiResolveBatch>;
  /** P68 §B/D7. Cancel a streaming run. IDEMPOTENT: an unknown or already-finished
   *  id resolves — a cancel racing a completion is normal and must not error. */
  aiCancelRun(runId: string): Promise<void>;
  /** P68 §B/D9. Answer a mid-run question. Rejects aiFailed when the run is unknown
   *  or is not awaiting input (a stray reply is never silently swallowed). */
  aiReplyRun(runId: string, text: string): Promise<void>;
  /** P15a. Generate a commit message from the staged diff. Never auto-commits.
   *  Rejects aiUnavailable | aiFailed | nothingToCommit | git | noRepo. */
  generateCommitMessage(repoId: string): Promise<CommitMessageProposal>;
  /** P15b. Explain or review a diff target (read-only prose). Writes nothing.
   *  Rejects aiUnavailable | aiFailed | nothingToCommit | git | invalidName | noRepo. */
  aiAnalyzeDiff(repoId: string, target: AiDiffTarget, mode: AiAnalysisMode): Promise<AiAnalysis>;
  /** P28. AI "what changed" digest over a selectable range (read-only prose).
   *  Writes nothing. Rejects aiUnavailable | aiFailed | git | invalidName | noRepo. */
  aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis>;
  /** P56. Generate grouped Markdown release notes for a tag/ref range (or since
   *  the last tag). Read-only; WRITES NOTHING; does NOT emit repo-changed. Fully
   *  local. Rejects aiUnavailable | aiFailed (empty range / no earlier tag / CLI)
   *  | git (bad ref) | noRepo. */
  aiChangelog(repoId: string, range: ChangelogRange): Promise<AiChangelog>;
  /** P64. Generate a pull-request title + Markdown body grounded in the commits
   *  unique to `head` vs `base` + the net diffstat. Read-only; WRITES NOTHING;
   *  never posts to a forge; does NOT emit repo-changed. The proposal fills the
   *  create-PR form for the user to review/edit before Create. Rejects
   *  aiUnavailable | aiFailed (empty range / no usable title / CLI) | git (bad
   *  ref) | noRepo. */
  aiGeneratePrDescription(repoId: string, base: string, head: string): Promise<PrDescription>;
  /** P53a. AI "why does this line exist" — blames `lineNo` (as of `atOid`, null →
   *  HEAD) to find the introducing commit, then explains that commit's change to
   *  the file focused on that line. Read-only; writes nothing; does NOT emit
   *  repo-changed. Rejects aiUnavailable | aiFailed (line out of range / no
   *  content) | git | invalidName | noRepo. */
  aiExplainLine(repoId: string, path: string, lineNo: number, atOid: string | null): Promise<AiAnalysis>;
  /** P15c. Summarize commits/diff unique to `target` vs `base` (read-only prose).
   *  Rejects aiUnavailable | aiFailed | git | noRepo. */
  aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary>;
  /** P53c. AI branch-name suggestions from `source`. Read-only; WRITES NOTHING.
   *  Returns 1..5 sanitized, valid candidates the user picks/edits in the
   *  branch-create dialog. Rejects aiUnavailable | aiFailed (empty grounding /
   *  no usable name) | git (bad ref) | noRepo. */
  aiSuggestBranchName(repoId: string, source: BranchNameSource): Promise<BranchNameProposal>;
  /** P54a. Propose grouping the working-tree changes (HEAD vs working tree, incl.
   *  untracked) into logical commits. Read-only; WRITES NOTHING. `guidance` = an
   *  optional free-text hint (e.g. "keep tests separate"). The result is ALWAYS an
   *  apply-able partition (unknown paths dropped, overlaps first-wins, uncovered
   *  files in `unassigned`). Unparseable model output is NOT an error — it resolves
   *  with groups:[] + all files unassigned. Rejects aiUnavailable | aiFailed (CLI
   *  fail/empty) | nothingToCommit (clean tree) | git | noRepo. */
  aiComposeCommits(repoId: string, guidance: string | null): Promise<ComposeProposal>;
  /** P55. Map a natural-language `request` to ONE allowlisted, previewable git
   *  operation. READ-ONLY: WRITES NOTHING, does NOT emit repo-changed — the caller
   *  must show the preview and, on explicit confirm, dispatch the resolved op via
   *  its EXISTING typed command (safeOpDispatch, P55c). An unmappable / adversarial
   *  request resolves to `unsupported` (a normal outcome, never a mutation, never a
   *  shell command). Rejects aiUnavailable | aiFailed | git | noRepo. */
  aiPlanOperation(repoId: string, request: string): Promise<OperationPlan>;
  /** Apply a reviewed plan as an ORDERED stage+commit sequence. ATOMIC: validates
   *  fully, resets the index to HEAD (working tree UNTOUCHED), commits each group;
   *  ANY mid-sequence failure rolls HEAD+index back so NOTHING is committed. Files
   *  in no group are left uncommitted. Called ONLY on the user's explicit final
   *  confirm. Does NOT emit repo-changed (caller refetches). Not AI-gated. Rejects
   *  noRepo | operationInProgress | git | emptyMessage | configMissing |
   *  nothingToCommit | other (unknown/duplicate path, no-op group, drift). */
  applyComposedCommits(repoId: string, plan: ComposePlan): Promise<ComposeApplyResult>;
  /** Persisted multi-tab session. Never rejects for a missing/corrupt file (empty). */
  getSession(): Promise<SessionState>;
  /** Writes the whole session (tabs change as a unit). Rejects io on save failure. */
  setSession(session: SessionState): Promise<void>;
  /** P16. Tell the backend the focused-tab repoId (or null when none). Seeds new
   *  embedded-MCP sessions; never disturbs an already-connected AI session. */
  setActiveRepo(repoId: string | null): Promise<void>;
  /** P16. Current embedded MCP server status for the Settings panel. */
  getMcpStatus(): Promise<McpStatus>;
  /** P16. Start/stop the embedded MCP server (read-only in P16b). Returns the
   *  resulting status; also fires `onMcpServerChanged`. */
  setMcpEnabled(enabled: boolean): Promise<McpStatus>;
  /** P16c. Flip the write-gate; bounces the running server (stop+restart on the
   *  same token/port) so the 20 mutation tools (de)register and live sessions
   *  re-negotiate. Returns the resulting status; also fires `onMcpServerChanged`. */
  setMcpAllowWrite(allowWrite: boolean): Promise<McpStatus>;
  /** P16. Fires on server start/stop/bounce; payload is the new status. */
  onMcpServerChanged(cb: (s: McpStatus) => void): Promise<Unsubscribe>;
  /** P16. Registers the running embedded MCP server with the local `claude` CLI
   *  via `claude mcp add`. `scope` is `'user'` (global) or `'local'` (the open
   *  repo, private). `repoPath` sets the child cwd (required for a meaningful
   *  `local` registration; may be `null` for `user`). Rejects `aiUnavailable`
   *  (CLI not on PATH) | `aiFailed` (non-zero exit / timeout) | `other` (server
   *  not running). */
  registerMcpWithClaude(scope: 'user' | 'local', repoPath: string | null): Promise<void>;
  /** P24. Full AI-asset inventory + drift for a repo. `canonical` optionally
   *  overrides the drift reference asset id. Rejects io | noRepo. */
  listAiAssets(repoId: string, canonical?: string): Promise<AiAssetInventory>;
  /** P24. Raw content of one AI-asset file (repo-relative path, validated inside
   *  the workdir). A missing file resolves `exists:false`. Rejects other | io | noRepo. */
  readAiAsset(repoId: string, path: string): Promise<AssetContent>;
  /** P26. Managed inventory of the three `.claude/` agent-asset kinds (skills /
   *  subagents / slash commands), parsed + validated. Empty when `.claude/` is
   *  absent. Rejects io | noRepo. */
  listAgentAssets(repoId: string): Promise<AgentAssetInventory>;
  /** P26. One parsed agent asset by (kind, name); a missing file resolves to an
   *  `exists:false` shell. Rejects invalidName | io | noRepo. */
  readAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAsset>;
  /** P26b. Create or overwrite an agent asset (atomic temp+rename, parent dirs
   *  incl. the skill `<name>/` dir). Missing required fields don't block the
   *  write — the returned inventory flags them `valid:false`. Returns the
   *  refreshed inventory. Rejects invalidName | other | io | noRepo. */
  saveAgentAsset(repoId: string, asset: AgentAssetInput): Promise<AgentAssetInventory>;
  /** P26b. Delete one agent asset. A skill removes the whole
   *  `.claude/skills/<name>/` directory; agent/command removes the single file.
   *  A missing target is a no-op. Returns the refreshed inventory. Rejects
   *  invalidName | io | noRepo. */
  deleteAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAssetInventory>;
  /** P24. The context-profile store (lazy empty default when absent). Rejects
   *  other | io | noRepo. */
  listProfiles(repoId: string): Promise<ProfileStore>;
  /** P24. Insert-or-replace a profile keyed by name, then persist. Rejects
   *  invalidName (bad name / non-single-file target) | other | io | noRepo. */
  saveProfile(repoId: string, profile: ContextProfile): Promise<ProfileStore>;
  /** P24. Remove a profile (no-op if absent); clears `activeProfile` if matched.
   *  Rejects other | io | noRepo. */
  deleteProfile(repoId: string, name: string): Promise<ProfileStore>;
  /** P24. Per-target before/after preview for an activation. Writes nothing.
   *  Rejects other | io | noRepo. */
  previewProfile(repoId: string, name: string): Promise<ProfilePreviewEntry[]>;
  /** P24. Activate a profile: write each target's content to its mapped file,
   *  set `activeProfile`. The one write path. Rejects invalidName | other | io | noRepo. */
  activateProfile(repoId: string, name: string): Promise<ProfileActivation>;
  /** P31. The worktree × AI-context matrix: every worktree row with its active
   *  profile + drift/missing counts. Read-only. Rejects git | other | io | noRepo. */
  listWorktreeContexts(repoId: string): Promise<WorktreeContextStatus[]>;
  /** P31. Per-target preview for activating `name` onto worktree `worktreeKey`.
   *  Writes nothing; enforces D6 eligibility (locked/invalid/prunable → git).
   *  Rejects git | other | io | noRepo. */
  previewWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfilePreviewEntry[]>;
  /** P31. Activate `name` onto worktree `worktreeKey` — the one write path,
   *  UI-gated behind confirm + preview. D6 eligibility + D7 dirty-target guard.
   *  Rejects invalidName | git | other | io | noRepo. */
  activateWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfileActivation>;
  /** P24e. Translate the `sourceAssetId` instruction file into `targetAgent`'s
   *  flavor via the local `claude` CLI. Consent-gated. WRITES NOTHING — returns
   *  proposed text the user reviews and saves into a profile target. Rejects
   *  aiUnavailable | aiFailed | other | io | noRepo. */
  aiGenerateAsset(
    repoId: string,
    sourceAssetId: string,
    targetAgent: string,
    guidance?: string,
  ): Promise<AiGeneratedAsset>;
  /** P42. Check the configured endpoint for a newer release. Resolves with
   *  availability + version metadata. Rejects AppError (`networkError`
   *  offline/unreachable, `updateFailed` bad signature/manifest). No-op safe to
   *  call repeatedly. */
  checkForUpdate(): Promise<UpdateCheckResult>;
  /** P42. Download + install the update discovered by the most recent
   *  checkForUpdate, streaming byte progress via `onProgress`. Resolves when the
   *  installer has applied the update; the app must then call relaunchApp() to
   *  restart. Rejects `noOperationInProgress` if no update was found first,
   *  `networkError`/`updateFailed` on transfer/verify failure. */
  downloadAndInstallUpdate(onProgress: (p: UpdateProgress) => void): Promise<void>;
  /** P42. Restart the app to complete a finished update (tauri-plugin-process).
   *  Never resolves in practice (process exits). In the mock it is a logged
   *  no-op. */
  relaunchApp(): Promise<void>;
  // --- P62: forge / PR integration ---
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
  // --- P79/P80: global forge account management (repo-independent) ---
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
