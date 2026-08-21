export interface HeadInfo {
  branchName: string | null;
  oid: string;
  detached: boolean;
  unborn: boolean;
}

export interface RepoInfo {
  path: string;
  isRepo: boolean;
  /** `true` for bare repositories (rejected at open); always `false` when `isRepo` is `false`. */
  bare: boolean;
  head: HeadInfo | null;
}

/** Result of `openRepo`: the canonical `repoId` (workdir path string) + repo info.
 *  `repoId` is meaningful (a map entry exists) only for a usable repo; it is still
 *  returned for non-usable opens so the frontend can key its error UI. */
export interface OpenRepoResult {
  repoId: string;
  info: RepoInfo;
}

/** Streamed clone transfer progress (P21). Mirrors the Rust `CloneProgress`
 *  EXACTLY (camelCase). One per git2 `transfer_progress` tick. The UI treats the
 *  fraction as `receivedObjects/totalObjects` while `totalDeltas === 0`, else
 *  `indexedDeltas/totalDeltas` (the resolving-deltas phase). */
export interface CloneProgress {
  receivedObjects: number;
  totalObjects: number;
  indexedDeltas: number;
  totalDeltas: number;
  /** u64 on the wire; safe as a JS number for realistic repos. */
  receivedBytes: number;
}

export type FileStatus =
  | 'added'
  | 'modified'
  | 'deleted'
  | 'renamed'
  | 'typechange'
  | 'conflicted'
  | 'untracked';

export interface StatusEntry {
  /** Repo-relative path, forward slashes. For renames: the NEW path. */
  path: string;
  /** For renames: the OLD path. `null` otherwise. */
  origPath: string | null;
  status: FileStatus;
}

export interface StatusSnapshot {
  staged: StatusEntry[];
  unstaged: StatusEntry[];
  untracked: StatusEntry[];
  conflicted: StatusEntry[];
}

export type RefKind = 'localBranch' | 'remoteBranch' | 'tag' | 'head' | 'stash';

export interface RefLabel {
  /** Shorthand: "main", "origin/main", "v1.0", "HEAD". */
  name: string;
  kind: RefKind;
  /** true on the local branch HEAD points at (attached), or on the head label (detached). */
  isHead: boolean;
}

export interface GraphNode {
  /** Full 40-char hex oid. */
  id: string;
  lane: number;
  /** Indices into GraphLayout.nodes; parents always at a HIGHER index. First entry = first parent. */
  parents: number[];
  /** Absent when empty (serde skip_serializing_if). */
  refs?: RefLabel[];
  summary: string;
  author: string;
  /** Author commit time, seconds since epoch (UTC). */
  ts: number;
  /** Committer commit time, seconds since epoch (UTC). P51: powers the
   *  author-vs-committer date basis toggle. Often == `ts`. */
  committerTs: number;
}

export interface GraphEdge {
  /** Child row/index. */
  from: number;
  /** Parent row/index; to > from. */
  to: number;
  /** Lane of the vertical run between the rows. */
  lane: number;
}

export interface GraphLayout {
  /** Row number == index in this array (no row field on the wire). */
  nodes: GraphNode[];
  /** Sorted ascending by (from, to). */
  edges: GraphEdge[];
  laneCount: number;
  headIndex: number | null;
  truncated: boolean;
}

/** P65 (streamed graph): one commit row as delivered by `streamGraph`. Identical
 *  to {@link GraphNode} MINUS `parents` — a child's parent rows are not known
 *  when it is emitted (parents are always at HIGHER, not-yet-walked rows), so the
 *  frontend reconstructs `parents` from the edge ordinals ({@link StreamEdge.ord}).
 *  Mirrors the Rust `StreamNode` (camelCase; `refs` omitted when empty). */
export interface StreamNode {
  id: string;
  lane: number;
  refs?: RefLabel[];
  summary: string;
  author: string;
  ts: number;
  committerTs: number;
}

/** P65 (streamed graph): a {@link GraphEdge} PLUS the child's parent ordinal
 *  (`ord`) so the frontend can rebuild each node's ordered `parents`. `ord === 0`
 *  is the first parent (the lane-inheriting edge). Mirrors the Rust
 *  `GraphStreamEdge`. `from` (child) < `to` (parent). */
export interface StreamEdge {
  from: number;
  to: number;
  lane: number;
  ord: number;
}

/** P65 (streamed graph): one `streamGraph` channel message. Wire order: exactly
 *  one `meta`, then N `batch`, then exactly one `done`. On any error the command
 *  REJECTS (AppError) instead of sending `done`. Mirrors the Rust `GraphChunk`
 *  serde enum (tagged `kind`, camelCase) byte-for-byte.
 *  - `meta`: first message. `total` = exact reachable-commit count if cheaply
 *    known, else null (frontend grows the scroll extent as rows arrive).
 *    `headOid` lets the frontend resolve `headIndex` the moment HEAD's row lands.
 *  - `batch`: a run of consecutive rows `[startRow, startRow + nodes.length)`
 *    plus the edges FINALIZED within them (parent `to` in this batch).
 *    `laneCountSoFar` is the running max (monotonic) — drives the graph-area
 *    width without ever shrinking.
 *  - `done`: terminal authoritative scalars. `totalRows` == nodes emitted;
 *    `headIndex` resolved; `truncated` set at the streaming cap. */
export type GraphChunk =
  | { kind: 'meta'; total: number | null; headOid: string | null }
  | { kind: 'batch'; startRow: number; laneCountSoFar: number; nodes: StreamNode[]; edges: StreamEdge[] }
  | { kind: 'done'; totalRows: number; laneCount: number; headIndex: number | null; truncated: boolean };

export type LineKind = 'context' | 'add' | 'del';

/** One selected changed line for partial staging (P17). Context lines are
 *  dropped before sending; the backend identifies an Add by `newNo` and a Del
 *  by `oldNo`. Mirrors the Rust `LineSelection`. */
export interface LineSelection {
  kind: LineKind; // 'add' | 'del' (context dropped before sending)
  oldNo: number | null;
  newNo: number | null;
}

export interface DiffLine {
  kind: LineKind;
  /** Line number in the OLD file; `null` for add lines. */
  oldNo: number | null;
  /** Line number in the NEW file; `null` for del lines. */
  newNo: number | null;
  /** Content without the leading +/-/space and without the trailing newline. */
  content: string;
  /** Present (true) only on the last line of a file lacking a trailing newline. */
  noNewline?: boolean;
  /** P61a: CHANGED sub-ranges within `content` as `[startCodePoint, lenCodePoints]`,
   *  ascending + non-overlapping. Present only on paired add/del lines when the
   *  diff was fetched with `intraline=true`; absent/empty => render plain. Slice
   *  via `Array.from(content)` (code-point aware — offsets are NOT UTF-16 units). */
  spans?: [number, number][];
}

export interface Hunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

export interface FileDiff {
  /** NEW path for renames; repo-relative, forward slashes. */
  path: string;
  /** OLD path for renames; `null` otherwise. */
  origPath: string | null;
  status: FileStatus;
  binary: boolean; // true -> hunks empty
  tooLarge: boolean; // true -> hunks empty
  hunks: Hunk[];
}

export interface FileDiffHeader {
  path: string;
  origPath: string | null;
  status: FileStatus;
  additions: number;
  deletions: number;
  binary: boolean;
}

export interface CommitDetails {
  oid: string;
  summary: string;
  /** Full message, trailing whitespace trimmed. Includes the summary line. */
  message: string;
  authorName: string;
  authorEmail: string;
  /** Seconds since epoch (UTC). */
  authorTs: number;
  committerTs: number;
  /** Full oids, first parent first. length > 1 => merge commit. */
  parents: string[];
}

export interface CommitDiff {
  details: CommitDetails;
  /** Sorted by path ascending. Headers only — hunks are fetched per file. */
  files: FileDiffHeader[];
}

export interface CompareEndpoint {
  /** Full 40-char hex; "" when HEAD is unborn (old side). */
  oid: string;
  /** First line of that commit's message; "" when unborn. */
  summary: string;
}

export interface CompareDiff {
  /** OLD side = HEAD. */
  from: CompareEndpoint;
  /** NEW side = the right-clicked commit. */
  to: CompareEndpoint;
  /** Sorted path-ascending. Empty when from.oid === to.oid. Headers only. */
  files: FileDiffHeader[];
}

/** P61b: one resolved side of an image comparison (base64 over IPC — D2).
 *  The frontend builds `data:${mime};base64,${base64}` for a plain `<img>`. */
export interface ImageSide {
  /** Raw blob bytes, standard base64 (NO `data:` prefix). */
  base64: string;
  /** MIME from the path extension, e.g. "image/png". */
  mime: string;
  /** Raw byte length pre-base64 (for the "N KB" label). */
  byteLen: number;
}

/** P61b: both sides of an image comparison. A `null` side is either absent
 *  (add/delete) or over the 8 MiB cap; the `*TooLarge` flags disambiguate. */
export interface ImageDiff {
  path: string;
  /** OLD side (index / HEAD / parent tree). null when added, missing, or over-cap. */
  old: ImageSide | null;
  /** NEW side (workdir / index / commit tree). null when deleted, missing, or over-cap. */
  new: ImageSide | null;
  oldTooLarge: boolean;
  newTooLarge: boolean;
}

/** P61b: which pair to load — mirrors the three file-diff contexts. Tagged on
 *  `kind`; matches the Rust `ImageDiffRequest` (camelCase keys + fields). */
export type ImageDiffRequest =
  | { kind: 'workdir'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'commit'; oid: string; path: string; origPath: string | null }
  | { kind: 'compare'; toOid: string; path: string; origPath: string | null };

export interface CommitResult {
  /** Full 40-char hex oid of the new commit. */
  oid: string;
  /** First line of the cleaned message. */
  summary: string;
  /** Branch HEAD points at after the commit ("main"); null when detached. */
  branch: string | null;
  /** Non-blocking post-commit hook trouble (spawn failure or non-zero exit).
   *  The commit itself landed; shown as a warning toast. Null when hooks are
   *  disabled, absent, or succeeded. Audit #2 §3.3. */
  hookWarning: string | null;
}

export interface BranchInfo {
  /** Shorthand, e.g. "main", "feature/sidebar". */
  name: string;
  /** True for the branch HEAD points at (always false when detached/unborn). */
  isHead: boolean;
  /** Upstream shorthand, e.g. "origin/main"; null when none configured or the ref is gone. */
  upstream: string | null;
  /** Commits ahead of / behind upstream. null whenever `upstream` is null. */
  ahead: number | null;
  behind: number | null;
  /** Full 40-char hex oid of the branch tip. */
  tip: string;
}

export interface RemoteBranchInfo {
  /** Shorthand incl. remote, e.g. "origin/main". */
  name: string;
  /** Full 40-char hex oid of the remote-tracking branch tip. */
  tip: string;
}

/** One configured remote (P22 §3.1). Mirrors the Rust `RemoteInfo` (camelCase). */
export interface RemoteInfo {
  /** Remote name, e.g. "origin". */
  name: string;
  /** Fetch URL; null if unreadable/non-UTF-8. */
  url: string | null;
}

// --- P77: tag sync (mirrors bonsai-core `git/tag_sync.rs`; serde camelCase). ---

/**
 * One tag's reconciliation state against a chosen remote.
 * `deleted-on-remote` is RESERVED for a future pushed-set upgrade and is never
 * emitted by the v1 backend (folded into `local-only`); kept for forward-compat.
 */
export type TagSyncStatus =
  | 'in-sync'
  | 'local-only'
  | 'stale'
  | 'remote-only'
  | 'deleted-on-remote';

/** One tag row in a {@link TagSyncReport}; rendered verbatim (fully precomputed). */
export interface TagSyncEntry {
  /** Short tag name (no `refs/tags/` prefix). */
  name: string;
  status: TagSyncStatus;
  /** Peeled committish the LOCAL tag resolves to (40-hex); null => remote-only. */
  localOid: string | null;
  /** Peeled committish the REMOTE tag resolves to (40-hex); null => local-only. */
  remoteOid: string | null;
  /** True if the tag is an annotated tag object on EITHER side (display flag). */
  annotated: boolean;
}

/** Result of one live ls-remote reconciliation pass. */
export interface TagSyncReport {
  /** The remote actually queried (resolved default when the caller passed null). */
  remote: string;
  /** One row per tag in the local∪remote union, sorted case-insensitively. */
  entries: TagSyncEntry[];
}

export interface BranchesSnapshot {
  /** Sorted case-insensitively by name. */
  local: BranchInfo[];
  /** Sorted case-insensitively; symbolic "<remote>/HEAD" entries excluded. */
  remote: RemoteBranchInfo[];
  /** Tag names (lightweight + annotated), sorted case-insensitively. */
  tags: string[];
  /** One source of truth for attached/detached/unborn in the sidebar. */
  head: HeadInfo;
}

/** Why a branch is safe to delete (P25 §4.1). Bare-string enum. */
export type StaleReason = 'merged' | 'goneUpstream';

/** One local branch classified as stale (P25 §4.1). Mirrors Rust `StaleBranch`. */
export interface StaleBranch {
  name: string;
  /** Full 40-hex tip oid. */
  tip: string;
  /** First line of the tip commit's message. */
  lastCommitSummary: string;
  /** Tip commit author name. */
  lastCommitAuthor: string;
  /** Tip committer time, epoch seconds. */
  lastCommitTime: number;
  /** Primary reason: 'merged' when merged (even if also gone), else 'goneUpstream'. */
  reason: StaleReason;
  /** Raw flags (a branch may be both). */
  merged: boolean;
  goneUpstream: boolean;
  /** Configured upstream shorthand (e.g. "origin/feature"), if any — present even when gone. */
  upstream: string | null;
  /** Ahead/behind the BASE (best-effort; null on lookup error). ahead=0 when merged. */
  ahead: number | null;
  behind: number | null;
  /** Always false in returned entries (the current branch is excluded); defensive wire field. */
  isCurrent: boolean;
}

/** Read-only stale classification result (P25 §4.1). Mirrors Rust `StaleReport`. */
export interface StaleReport {
  /** Resolved base shorthand (e.g. "main" / "origin/main"). */
  base: string;
  /** Full 40-hex base commit oid. */
  baseOid: string;
  /** Stale candidates, case-insensitively sorted by name. Excludes base + current HEAD. */
  branches: StaleBranch[];
}

/** Per-branch outcome of a batch delete (P25 §4.3). Bare-string enum. */
export type BranchDeleteStatus =
  | 'deleted'
  | 'skippedCurrent'
  | 'skippedBase'
  | 'skippedNotStale'
  | 'skippedNotFound'
  | 'failed';

/** One result row from `deleteBranches` (P25 §4.3). Mirrors Rust `BranchDeleteResult`. */
export interface BranchDeleteResult {
  name: string;
  status: BranchDeleteStatus;
  /** Human detail for skipped/failed rows; null when deleted. */
  message: string | null;
}

export interface RemoteFetchResult {
  /** Remote name, e.g. "origin". */
  remote: string;
  receivedObjects: number;
  /** update_tips invocations where old != new (incl. newly created refs). */
  updatedRefs: number;
}

export interface FetchResult {
  /** One entry per configured remote, in remote-list order. */
  remotes: RemoteFetchResult[];
}

export type PullResult =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; from: string; to: string }
  | {
      kind: 'wouldNotFastForward';
      branch: string;
      ahead: number;
      behind: number;
      /** P60b: resolved upstream shorthand ("origin/main") — the exact name the
       *  frontend hands to mergeBranch/rebaseBranch when reconciling. */
      upstream: string;
    };

export type RepoOpState =
  | { kind: 'none' }
  | { kind: 'merge'; incoming: string; message: string }
  | {
      kind: 'rebase';
      headName: string | null;
      onto: string | null;
      currentStep: number;
      totalSteps: number;
    }
  | { kind: 'cherryPick' }
  | { kind: 'revert' }
  | {
      kind: 'bisect';
      /** oid under test now; null in the terminal `found` phase. */
      current: string | null;
      /** the bounding known-bad commit. */
      bad: string;
      /** known-good boundary commits. */
      good: string[];
      /** skipped (untestable) commits. */
      skipped: string[];
      /** culprit once converged; null while still searching. */
      firstBad: string | null;
      /** testable candidates left. */
      revisionsRemaining: number;
      /** ~log2(revisionsRemaining). */
      estimatedSteps: number;
    };

/** Outcome of startBisect / bisectMark / bisectSkip (P39). Mirrors the Rust
 *  `BisectOutcome` serde enum (tagged "kind", camelCase). */
export type BisectOutcome =
  | { kind: 'testing'; current: string; revisionsRemaining: number; estimatedSteps: number }
  | { kind: 'found'; firstBad: string }
  | { kind: 'cannotDetermine'; skipped: string[] };

export type ConflictKind =
  | 'bothModified'
  | 'bothAdded'
  | 'deletedByUs'
  | 'deletedByThem'
  | 'addedByUs'
  | 'addedByThem'
  | 'bothDeleted';

export interface ConflictEntry {
  path: string;
  kind: ConflictKind;
  hasBase: boolean;
  hasOurs: boolean;
  hasTheirs: boolean;
}

export interface ConflictFile {
  path: string;
  kind: ConflictKind;
  binary: boolean;
  tooLarge: boolean;
  /** Worktree file missing (deletion conflicts). text is '' when true. */
  missing: boolean;
  /** Worktree contents INCLUDING <<<<<<< ======= >>>>>>> markers. */
  text: string;
  /** Stage-2 (OURS) blob text. '' when the ours side is absent or text is suppressed. */
  ours: string;
  /** Stage-3 (THEIRS) blob text. '' when the theirs side is absent or text is suppressed. */
  theirs: string;
}

export type ConflictResolution = 'ours' | 'theirs' | 'markResolved';

export interface StashEntry {
  index: number;      // 0 == stash@{0}; SHIFTS after drop/pop — always refetch
  message: string;
  oid: string;        // stash commit oid
  baseOid: string;    // first-parent = base commit the pill attaches to
  ts: number;         // seconds since epoch (UTC)
}

export type SubmoduleStatus = 'uninitialized' | 'upToDate' | 'outOfSync' | 'modifiedWorkdir';

export interface SubmoduleInfo {
  name: string;              // stable key for init/update/sync
  path: string;              // repo-relative, forward slashes
  absPath: string;           // absolute workdir path — feed to open-in-tab
  url: string | null;
  headOid: string | null;    // commit in superproject HEAD
  indexOid: string | null;   // commit in superproject index
  wtOid: string | null;      // commit checked out in the submodule (null if uninitialized)
  status: SubmoduleStatus;
}

/** Result of `deinitSubmodule` (P82). Mirrors Rust `SubmoduleDeinitOutcome`
 *  (serde tagged "kind", camelCase). `dirtyNeedsForce` = the plain op refused
 *  because the submodule worktree is dirty; re-invoke with `force=true`. */
export type SubmoduleDeinitOutcome =
  | { kind: 'deinitialized' }
  | { kind: 'dirtyNeedsForce' };

/** Result of `removeSubmodule` (P82). Mirrors Rust `SubmoduleRemoveOutcome`. */
export type SubmoduleRemoveOutcome =
  | { kind: 'removed' }
  | { kind: 'dirtyNeedsForce' };

/** One worktree row (main or linked) — P27. Wire mirror of the Rust
 *  `WorktreeInfo`. `headOid` is full 40-hex; the UI shortens to 7. */
export interface WorktreeInfo {
  name: string;               // stable key for remove/lock/unlock
  absPath: string;            // absolute workdir path — feed to open-in-tab
  relPath: string | null;     // repo-relative if under the main workdir, else null
  branch: string | null;      // short branch name; null if detached/invalid
  headOid: string | null;     // full 40-hex; UI shortens to 7
  locked: boolean;
  lockReason: string | null;
  isMain: boolean;
  isCurrent: boolean;
  prunable: boolean;          // stale
  valid: boolean;
}

// --- P32 Part B: copy uncommitted changes into a new worktree ---------------

/** Which status list a copy candidate came from (wire mirror of Rust
 *  `CopyGroup`). */
export type CopyGroup = 'staged' | 'unstaged' | 'untracked' | 'ignored';
/** Conflict verdict for a selected path against the target branch. */
export type CopyVerdict = 'clean' | 'conflict';
/** What to do with one selected path at create time ("Overwrite" in the UI ==
 *  `copy` on a conflict). */
export type CopyAction = 'copy' | 'skip';

/** One file the user may copy into the new worktree. */
export interface CopyCandidate {
  path: string; // repo-relative, forward slashes
  group: CopyGroup;
}
/** Result of `previewWorktreeCopy` for one path. */
export interface CopyPlanEntry {
  path: string;
  verdict: CopyVerdict;
}
/** One user decision, sent to `addWorktreeWithChanges`. */
export interface CopySelection {
  path: string;
  action: CopyAction;
}

// --- P29: repo health -------------------------------------------------------

/** P29. Per-section envelope: exactly one of `data`/`error` is set. Section
 *  failures never reject the whole getRepoHealth call (contract §D4). */
export interface Section<T> {
  data: T | null;
  error: string | null;
  elapsedMs: number;
}

/** P29. All four health sections in one payload. Wire mirror of the Rust
 *  `RepoHealth` (camelCase). */
export interface RepoHealth {
  stats: Section<StatsSection>;
  branches: Section<BranchesSection>;
  workingState: Section<WorkingStateSection>;
  structure: Section<StructureSection>;
  /** Epoch seconds. */
  generatedAt: number;
}

/** P29. History/ODB/disk metrics. Every `*Capped` flag means the number is a
 *  floor — the UI renders `≥`. */
export interface StatsSection {
  commitCount: number;
  commitCountCapped: boolean;
  commitsLast30d: number;
  authorsLast30d: number;
  authorsTotal: number;
  objectCount: number;
  objectScanCapped: boolean;
  /** Top 10 desc by size; blob→path mapping is out of scope (label "blob <shortOid>"). */
  largestBlobs: BlobStat[];
  workdirFileCount: number;
  workdirBytes: number;
  workdirScanCapped: boolean;
  /** Top 10 desc; forward-slash repo-relative paths. */
  largestFiles: FileStat[];
  /** Worktree files >= 10 MiB (warn when > 0). */
  largeFileCount: number;
  gitDirBytes: number;
  gitDirScanCapped: boolean;
}

export interface BlobStat {
  /** Full 40-hex; UI shortens to 7. */
  oid: string;
  size: number;
}

export interface FileStat {
  path: string;
  size: number;
}

/** P29. Ref counts + HEAD shape + stale rollup. */
export interface BranchesSection {
  localCount: number;
  remoteCount: number;
  tagCount: number;
  /** null = detached/unborn. */
  currentBranch: string | null;
  detached: boolean;
  unborn: boolean;
  /** vs upstream, best-effort; null when no upstream / lookup failed. */
  ahead: number | null;
  behind: number | null;
  upstream: string | null;
  /** null when the stale scan failed — see `staleError` (sub-metric, D9). */
  stale: StaleRollup | null;
  staleError: string | null;
}

export interface StaleRollup {
  base: string;
  mergedCount: number;
  goneUpstreamCount: number;
}

/** P29. Working-tree facts; reuses `RepoOpState` verbatim. */
export interface WorkingStateSection {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  opState: RepoOpState;
  stashCount: number;
  hasGitignore: boolean;
}

/** P29. Submodule/worktree/AI-asset rollups. */
export interface StructureSection {
  submoduleCount: number;
  submodulesUninitialized: number;
  submodulesOutOfSync: number;
  submodulesModified: number;
  /** Includes the synthesized main row. */
  worktreeCount: number;
  worktreesLocked: number;
  worktreesPrunable: number;
  worktreesInvalid: number;
  assetDriftedCount: number;
  assetsInSync: boolean;
}

export type ApplyStashOutcome =
  | { kind: 'applied' }
  | { kind: 'conflicts'; paths: string[] }
  /** Blocked pre-apply: the stash contains Windows-reserved paths (e.g. `NUL`)
   *  that cannot be checked out. Nothing was applied and the stash is retained.
   *  Retry with `skipReserved: true` to apply everything except these. */
  | { kind: 'reservedPaths'; paths: string[] }
  /** Applied everything except the listed Windows-reserved paths, which could
   *  not be restored. For pop, the stash is KEPT (not dropped) so the reserved
   *  blobs are not lost. */
  | { kind: 'appliedSkippingReserved'; skipped: string[] };

export interface CreateStashResult {
  created: boolean;
}

/** Which changes a createStash call captures. Mirrors Rust `StashScope` (camelCase).
 *  - `all`: staged + unstaged tracked changes (untracked left in place).
 *  - `allWithUntracked`: adds untracked files.
 *  - `staged`: only the staged (index-vs-HEAD) paths; mixed files are folded whole,
 *    unstaged-only paths and untracked files are left untouched. */
export type StashScope = 'all' | 'allWithUntracked' | 'staged';

/** Reset mode (P20). Mirrors the Rust `ResetMode` serde enum (camelCase). */
export type ResetMode = 'soft' | 'mixed' | 'hard';

export interface CreateBranchHereResult {
  /** true when uncommitted work was auto-stashed and carried across. */
  stashed: boolean;
  /** Present only when `stashed`; null otherwise (serde None → null). */
  apply: ApplyStashOutcome | null;
}

/** Result of a dirty-safe branch switch (P33). */
export interface CheckoutResult {
  /** true when uncommitted work was auto-stashed and carried across. */
  stashed: boolean;
  /** true when the switched-to branch was fast-forwarded to its upstream. */
  fastForwarded: boolean;
  /** Present only when `stashed`; null otherwise (serde None → null). */
  apply: ApplyStashOutcome | null;
}

/** Result of a branch rename (P60a). Mirrors Rust `RenameBranchResult`. */
export interface RenameBranchResult {
  /** true when the renamed branch was the checked-out branch (HEAD followed the
   *  rename) — the frontend then refetches HEAD/status, not just the list. */
  wasHead: boolean;
  /** The upstream shorthand still configured after the rename (e.g. "origin/main"),
   *  or null. libgit2 renames the `branch.<name>.*` config section, so tracking
   *  is preserved. */
  upstream: string | null;
}

export type MergeOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string; stashed: boolean }
  | { kind: 'merged'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

export type RebaseOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string }
  | { kind: 'rebased'; branch: string; head: string; steps: number; warnings?: string[] }
  | { kind: 'conflicts'; paths: string[]; currentStep: number; totalSteps: number };

/** Interactive-rebase per-op action (P23). Mirrors the Rust `RebaseAction`
 *  serde enum EXACTLY. Wire: "pick" | "reword" | "squash" | "fixup" | "drop". */
export type RebaseAction = 'pick' | 'reword' | 'squash' | 'fixup' | 'drop';

/** One interactive-rebase todo-list entry (P23). Mirrors the Rust
 *  `RebaseTodoOp` (camelCase). `oid` = the commit being replayed. `newMessage`
 *  is REQUIRED for `reword`, OPTIONAL for `squash` (null → default concat),
 *  null otherwise. */
export interface RebaseTodoOp {
  oid: string;
  action: RebaseAction;
  newMessage: string | null;
}

/** One blamed line (P23). Mirrors the Rust `BlameLine` (camelCase) EXACTLY.
 *  `oid` is the 40-hex of the commit that last touched the line (resolves to a
 *  graph node for reveal-in-graph); `authorTs` is seconds since epoch (UTC). */
export interface BlameLine {
  oid: string;
  authorName: string;
  authorEmail: string;
  authorTs: number;
  summary: string;
  origLineNo: number;
  finalLineNo: number;
  lineText: string;
}

/** One commit that touched a file (P23). Mirrors the Rust `FileHistoryEntry`
 *  (camelCase) EXACTLY. `authorTs` is seconds since epoch (UTC). */
export interface FileHistoryEntry {
  oid: string;
  summary: string;
  authorName: string;
  authorEmail: string;
  authorTs: number;
}

/** One reflog entry (P38 §4.2). Mirrors the Rust `ReflogEntry` (camelCase)
 *  EXACTLY. `index` is the N in `<ref>@{N}` (0 == newest). `oldOid`/`newOid`
 *  are full 40-hex (the UI shortens); a 40-zero `oldOid` marks the ref root.
 *  `committerTs` is seconds since epoch (UTC). */
export interface ReflogEntry {
  index: number;
  oldOid: string;
  newOid: string;
  committerName: string;
  committerEmail: string;
  committerTs: number;
  message: string;
}

/** Classified last-operation kind (P60c). Mirrors the Rust `UndoKind` serde
 *  enum (camelCase) EXACTLY. Drives the undo verb + reset mode. */
export type UndoKind =
  | 'commit'
  | 'amend'
  | 'merge'
  | 'rebase'
  | 'fastForward'
  | 'cherryPick'
  | 'revert'
  | 'reset'
  | 'branchSwitch'
  | 'unknown';

/** Plan for reversing the last HEAD-moving operation (P60c). Mirrors the Rust
 *  `UndoPlan` (camelCase) EXACTLY. `targetOid`/`targetShort` are "" when there
 *  is nothing to undo or the target is the 40-zero root. `resetMode` is null
 *  when `!undoable`. `worktreeDirty` is TRACKED dirtiness (staged + unstaged) —
 *  a hard reset preserves untracked files. When `requiresCleanWorktree &&
 *  worktreeDirty` the UI SHOWS the plan but BLOCKS the button (stash first). */
export interface UndoPlan {
  kind: UndoKind;
  summary: string;
  targetOid: string;
  targetShort: string;
  resetMode: ResetMode | null;
  requiresCleanWorktree: boolean;
  worktreeDirty: boolean;
  undoable: boolean;
  reason: string | null;
}

/** Which field(s) commit search examines (P50). `all` = message OR author. */
export type SearchField = 'all' | 'message' | 'author' | 'path' | 'content';
/** Which field actually matched a result row. */
export type MatchedField = 'message' | 'author' | 'path' | 'content';

/** A commit/content search request (P50a). Mirrors the Rust `SearchQuery`
 *  (camelCase) EXACTLY. `regex` applies to CONTENT only (v1): false = `-S`
 *  literal, true = `-G` regex; ignored for message/author/path. `caseSensitive`
 *  false ⇒ case-insensitive. `maxResults` 0 ⇒ backend default cap (1000),
 *  clamped to it. `scopeRef` null ⇒ all refs; a ref/oid ⇒ walk only that scope.
 *  (Date scope `since`/`until` is deferred — not part of the v1 wire type.) */
export interface SearchQuery {
  text: string;
  field: SearchField;
  regex: boolean;
  caseSensitive: boolean;
  maxResults: number;
  scopeRef: string | null;
}

/** One matched commit (P50a). Mirrors the Rust `SearchMatch` (camelCase)
 *  EXACTLY. `oid` is full 40-hex (feeds revealCommitByOid). `snippet` is the
 *  matched pathspec for Path mode, absent otherwise (serde skip when None). */
export interface SearchMatch {
  oid: string;
  summary: string;
  authorName: string;
  authorTs: number;
  matched: MatchedField;
  snippet?: string;
}

/** Commit-search response (P50a): capped, newest-first matches + a `truncated`
 *  flag when a cap or scan bound was hit ("there may be more"). */
export interface SearchResults {
  matches: SearchMatch[];
  truncated: boolean;
}

// ---- P58: commit signing ---------------------------------------------------

/** `gpg.format` — how commits are signed. Mirrors the Rust `SignFormat`
 *  (lowercase). */
export type SignFormat = 'ssh' | 'openpgp';

/** Effective signing config for the commit-box indicator/toggle (P58a D6).
 *  Mirrors the Rust `SigningStatus` (camelCase). `enabled` = effective
 *  `commit.gpgsign`; `format` is null when `gpg.format` is unset (git default =
 *  openpgp); `hasKey` = `user.signingkey` set + non-empty; `key` (path or id) is
 *  omitted when unset. */
export interface SigningStatus {
  enabled: boolean;
  format: SignFormat | null;
  hasKey: boolean;
  key?: string;
}

/** `git log --format=%G?` verdict for one commit (P58b). Mirrors the Rust
 *  `VerifyStatus` (camelCase). Authoritative for BOTH ssh and openpgp — git owns
 *  the trust check. `unsigned` ⇒ no signature (badge stays blank). */
export type VerifyStatus =
  | 'good'
  | 'goodUnknown'
  | 'bad'
  | 'expired'
  | 'expiredKey'
  | 'revoked'
  | 'cannotCheck'
  | 'unsigned';

/** One commit's verification verdict (P58b). Mirrors the Rust
 *  `CommitVerification` (camelCase). `signer` (%GS) / `key` (%GK) are omitted
 *  when git reported them empty. */
export interface CommitVerification {
  oid: string;
  status: VerifyStatus;
  signer?: string;
  key?: string;
}

/** Result of `verifyCommits` (P58b): one entry per RESOLVABLE requested oid, in
 *  request order. Non-hex / unresolvable oids are omitted (kept "unchecked" by
 *  the frontend). Mirrors the Rust `VerifyResults`. */
export interface VerifyResults {
  verifications: CommitVerification[];
}

// ---- P57: semantic commit-history search (BM25 index) ----------------------

/** Build phase of `historyIndexBuild` (P57a). Mirrors the Rust `IndexPhase`
 *  (lowercase camelCase wire values). */
export type IndexPhase = 'counting' | 'extracting' | 'writing' | 'done';

/** One streamed build-progress tick (P57a). Mirrors the Rust `IndexProgress`
 *  (camelCase). `total`/`newCommits` are 0 until the counting phase completes;
 *  `processed` climbs during extraction. */
export interface IndexProgress {
  phase: IndexPhase;
  /** Commits documented so far THIS build. */
  processed: number;
  /** Commits to document THIS build (0 until counted). */
  total: number;
  /** Of `total`, how many were newly-added (incremental). */
  newCommits: number;
}

/** Cheap status of the persisted history index (P57a). Mirrors the Rust
 *  `IndexStatus` (camelCase). `built` is true iff an index file exists AND parsed
 *  at the current schema; `stale` means the current ref tips differ from the last
 *  build's; `newCommits` counts reachable commits not yet indexed (0 when fresh).
 *  `headOid`/`builtAt` are null before the first successful build. */
export interface IndexStatus {
  built: boolean;
  indexedCommits: number;
  headOid: string | null;
  stale: boolean;
  newCommits: number;
  schema: number;
  builtAt: number | null;
  /** Commits skipped as UNREADABLE (corrupt/missing objects) by the build that
   *  returned this status; always 0 from `historyIndexStatus` (only a build can
   *  skip — skipped oids are retried next build). Audit #2 §3.3. */
  skippedCommits: number;
}

/** Retrieval query for `historySearch` (P57b). Mirrors the Rust `HistoryQuery`
 *  (camelCase). `topK` 0 ⇒ the backend default (DEFAULT_TOP_K = 20), clamped to
 *  MAX_TOP_K = 50. */
export interface HistoryQuery {
  text: string;
  topK: number;
}

/** One relevance-ranked commit from `historySearch` (P57b). Mirrors the Rust
 *  `HistoryHit` (camelCase). Overlaps P50's `SearchMatch` so the results UI reuses
 *  `revealCommitByOid` + the graph match rings. `score` is BM25 relevance,
 *  descending. */
export interface HistoryHit {
  oid: string;
  summary: string;
  authorName: string;
  authorTs: number;
  score: number;
}

/** Ranked retrieval results (P57b). Mirrors the Rust `HistorySearchResults`
 *  (camelCase). `indexStale` is true when no usable index exists yet (UI offers
 *  Build). */
export interface HistorySearchResults {
  hits: HistoryHit[];
  indexStale: boolean;
  indexedCommits: number;
}

/** AI answer grounded in retrieved commits (P57c). Mirrors the Rust
 *  `HistoryAnswer` (camelCase). `text` is fence-stripped prose; `cited` are the
 *  short-oids the answer references (best-effort, for UI emphasis); `retrieved`
 *  is the commit set fed to the model (drives the results list + reveal). */
export interface HistoryAnswer {
  text: string;
  cited: string[];
  retrieved: HistoryHit[];
  costUsd: number | null;
}

/** Write-target level (P40). System is never a write target. */
export type ConfigLevelArg = 'local' | 'global';
/** Where a value actually lives (read result). */
export type ConfigLevelName = 'local' | 'global' | 'system' | 'other';
export type ConfigValueKind = 'text' | 'bool' | 'enum';

/** A curated key with effective value + the value set at the target level (P40
 *  §4.2). Mirrors the Rust `CuratedEntry` (camelCase). `targetValue == null` +
 *  `effectiveValue != null` => the value is inherited from `effectiveLevel`. */
export interface CuratedConfigEntry {
  key: string;
  kind: ConfigValueKind;
  enumValues: string[];
  effectiveValue: string | null;
  effectiveLevel: ConfigLevelName | null;
  targetValue: string | null;
}

/** An arbitrary section.key entry at the target level (P40 Advanced list). */
export interface ConfigEntry {
  name: string;
  value: string;
  level: ConfigLevelName;
}

/** Result of getConfig for one target level (P40 §4.2). */
export interface ConfigView {
  targetLevel: ConfigLevelArg;
  curated: CuratedConfigEntry[];
  advanced: ConfigEntry[];
}

/** One named identity profile (P44). `id` is a stable crypto.randomUUID(). */
export interface IdentityProfile {
  id: string;
  label: string;
  userName: string;
  userEmail: string;
  /** Optional user.signingkey; null/empty ⇒ not written on apply. */
  signingKey: string | null;
}

/** Cherry-pick outcome (P20, extended P47). Mirrors the Rust `CherrypickOutcome`
 *  serde enum (tagged "kind", camelCase). `stashed` reports an autostash that was
 *  created for the operation (and restored on `committed`, retained otherwise);
 *  `conflicts` pauses into RepoOpState.cherryPick; `stashPopConflicts` = the pick
 *  committed cleanly but re-applying the retained autostash conflicted. */
export type CherrypickOutcome =
  | { kind: 'committed'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

/** Revert outcome (P20, extended P47). Mirrors the Rust `RevertOutcome` serde enum
 *  (tagged "kind", camelCase). `stashed`/`stashPopConflicts` mirror
 *  `CherrypickOutcome`; `conflicts` pauses into RepoOpState.revert. */
export type RevertOutcome =
  | { kind: 'committed'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

export type PushResult =
  | { kind: 'upToDate'; remote: string; branch: string }
  | { kind: 'pushed'; remote: string; branch: string; setUpstream: boolean };

export interface RecentRepo {
  /** Absolute workdir path as passed to openRepo. */
  path: string;
  /** Seconds since epoch (UTC) of the last successful open. */
  lastOpened: number;
}

export interface RepoChangedPayload {
  /** Which open repo the debounced filesystem change belongs to. */
  repoId: string;
  reason: string;
}

export type Theme = 'dark' | 'light';

/** Flat vs tree-grouped list rendering (P3b §2) — pure display preference. */
export type ListView = 'tree' | 'flat';

/** P67 §4: right-panel vertical density. Independent of `GraphPrefs.compact`
 *  (graph row geometry). 'cozy' is the P67b tightened default. */
export type PanelDensity = 'cozy' | 'compact';

/** P80 D1: which commit button is emphasized in the Working tab footer. */
export type PrimaryCommitAction = 'commit' | 'commitPush';

export interface PaneWidths {
  sidebar: number;
  rightPanel: number;
}

/** Auto-fetch preference (P11 §2.3). OFF by default; interval in minutes. */
export interface AutoFetchSettings {
  enabled: boolean;
  intervalMinutes: number;
}

/** Periodic read-only refresh signal (P30 §5). OFF by default; interval in
 *  minutes. Mirrors the Rust `HealthRefresh`. */
export interface HealthRefreshSettings {
  enabled: boolean;
  intervalMinutes: number;
}

// ---- P30: background-job scheduler (mirrors scheduler.rs / commands.rs) ----

/** The two Rust-side background jobs (P30 §5). */
export type JobKind = 'autoFetch' | 'healthRefresh';

/** Outcome of one job run. `skipped` = overlap guard; `suppressed` = an
 *  operation (merge/rebase/…) was in progress. */
export type JobOutcome = 'success' | 'failed' | 'suppressed' | 'skipped';

/** One background job's status for the UI readout (P30 §3). */
export interface JobStatus {
  job: JobKind;
  enabled: boolean;
  lastRunMs: number | null;
  lastOutcome: JobOutcome | null;
  lastError: string | null;
  consecutiveFailures: number;
  inBackoff: boolean;
  /** Estimate; null when disabled (or never seen by the loop yet). */
  nextRunMs: number | null;
}

/** Payload of the `job-status-changed` event (P30 §4). */
export interface JobStatusChangedPayload {
  repoId: string;
  job: JobKind;
  outcome: JobOutcome;
  /** autoFetch success only. */
  updatedRefs?: number;
  /** failed only. */
  error?: string;
  consecutiveFailures: number;
  inBackoff: boolean;
  /** true exactly on the 2→3 failure transition — toast ONLY then (D6). */
  enteredBackoff: boolean;
  tsMs: number;
  nextRunMs: number | null;
}

/** Graph geometry knobs (P11 §2.3) — pure render geometry, not layout math. */
/** Which timestamp the graph's date column + relative/absolute date use (P51).
 *  Mirrors the Rust `GraphDateBasis` enum (lowercase wire values). */
export type GraphDateBasis = 'author' | 'committer';

export interface GraphPrefs {
  avatarRadius: number;
  rowHeight: number;
  laneWidth: number;
  /** P51: short-SHA column (+ verified-badge slot). Default true. */
  showSha: boolean;
  /** P51: optional full author-name text column. Default false. */
  showAuthor: boolean;
  /** P51: date column. Default true. */
  showDate: boolean;
  /** P51: which timestamp the date column/tooltip use. Default 'author'. */
  dateBasis: GraphDateBasis;
  /** P51: ahead/behind chip on branch-tip pills. Default true. */
  showAheadBehind: boolean;
  /** P51: compact (denser) rows. Default false. */
  compact: boolean;
  /** P58c: light the per-row signature badge from `verifyCommits`. Default true.
   *  When false the P51 faint stub renders unchanged and NO verification is
   *  requested (individually toggleable, like the other detail columns). */
  showSignatureBadge: boolean;
  /** P63: PR-state badge on branch-tip pills. Default false (network+auth-gated
   *  — inert without a connected forge, so opt-in). */
  showPrBadge: boolean;
  /** P63: CI/build-status dot on branch-tip pills. Default false (same
   *  network+auth gating as showPrBadge). */
  showCiStatus: boolean;
}

/** AI conflict-resolution autonomy (P13). proposeReview = user accepts before
 *  anything is written/staged (default); autoResolve = write+stage immediately,
 *  user reviews the staged diff before commitMerge. */
export type AiAutonomy = 'proposeReview' | 'autoResolve';

/** Cheap Claude Code CLI health status (P13). A missing/broken CLI yields
 *  `installed:false` — never an error. Mirrors the Rust `AiAvailability`. */
export interface AiAvailability {
  installed: boolean;
  loggedIn: boolean;
  version: string | null;
  detail: string;
}

/** The model's proposed fully-merged file body for one conflicted path (P13).
 *  Mirrors the Rust `AiResolveProposal`; the proposal writes nothing. */
export interface AiResolveProposal {
  path: string;
  proposedText: string;
  costUsd: number | null;
}

/** P68 §F: one push event on the `ai_resolve_conflict_stream` channel. Mirrors the
 *  Rust `AiRunEvent` exactly (camelCase serde).
 *
 *  `runId` arrives on the FIRST (`started`) event — the command promise settles only
 *  when the whole run ends, so this is the ONLY way the UI learns the id in time to
 *  cancel or reply (D8). `seq` is monotonic from 0 per run; drop any event whose
 *  `seq` is <= the last seen (stale/duplicate guard). */
export type AiRunEventKind =
  /** Always first, seq 0, emitted BEFORE the child is spawned. */
  | 'started'
  /** One human-readable log line for the dock. High frequency ⇒ batch it (D5). */
  | 'log'
  /** A `result` line parsed; the run may continue with another turn. */
  | 'turnEnd'
  /** Blocked on `aiReplyRun`; the idle watchdog is paused (D3). */
  | 'awaitingInput'
  /** Terminal: success (the command promise resolves right after). */
  | 'done'
  /** Terminal: `text` is the same message as the `aiFailed` rejection. */
  | 'failed'
  /** Terminal: user cancel (the command rejects `aiCancelled`). */
  | 'cancelled';

export interface AiRunEvent {
  /** Stable for the whole run, even across sequential bulk batches. */
  runId: string;
  seq: number;
  kind: AiRunEventKind;
  /** One log line, the question, or the terminal message; never a whole payload. */
  text: string | null;
  /** Cost of the turn that just ended / of the run. LAST value wins within a run —
   *  never summed (spike §1.8). */
  costUsd: number | null;
  /** Since the run started, not since the turn. */
  elapsedMs: number;
  /** The file this event is about when known (bulk attribution); null run-level. */
  path: string | null;
  /** 1-based turn counter; 0 on `started`. */
  turn: number;
  /** Only on `cancelled`/`failed`: the assistant text accumulated so far (D2).
   *  DISPLAY-ONLY and lossy by construction — never offer it as a proposal. */
  partialText: string | null;
  /** P68d: the CLI's CUMULATIVE `estimated_tokens` from a `thinking_tokens`
   *  heartbeat — the run's only LIVE spend signal, since `costUsd` exists only at a
   *  turn boundary and a long single-turn run would otherwise read `$—` for minutes.
   *
   *  A `kind: 'log'` event with `text === null` and this set is a METRICS-ONLY
   *  heartbeat: record the number, do NOT append a log line (A4 — one heartbeat per
   *  second would drown the dock). The two fields are mutually exclusive on a
   *  `log` event.
   *
   *  Scope, verified against `claude` v2.1.233: THINKING tokens only, and estimated
   *  (600 reported vs 679 actual at the end of one run); a run that never enters
   *  extended thinking emits no heartbeats and this stays null throughout. Never
   *  convert it to a dollar figure — there is no price table anywhere in Bonsai. */
  thinkingTokens: number | null;
}

/** One path a streaming resolve could not handle. NEVER fatal to the batch (D11). */
export interface AiResolveFailure {
  path: string;
  reason: string;
}

/** P68 §D: the outcome of ONE streaming resolve run over 1..n paths. The promise —
 *  not the event stream — is authoritative for this data.
 *
 *  A `proposedText` here is a REVIEWABLE proposal, not a verified-clean merge: the
 *  single-path stream returns the model's body verbatim (P13 parity), so callers
 *  MUST keep applying `hasUnresolvedMarkers` before staging anything (D4). */
export interface AiResolveBatch {
  runId: string;
  proposals: AiResolveProposal[];
  failed: AiResolveFailure[];
  /** Last value within a run, summed across sequential bulk batches (A10). */
  costUsd: number | null;
  /** Max turns used across batches (1 when no question was asked). */
  turns: number;
}

/** P68 §B/D10: repo access granted to a conflict-resolution run. `readOnly` ⇒
 *  `--tools "Read,Grep,Glob"`; `none` ⇒ the old blind `--tools ""`. There is
 *  deliberately no write/edit/bash option. */
export type AiConflictTools = 'readOnly' | 'none';

/** The model's proposed commit message from the staged diff (P15a).
 *  Mirrors the Rust `CommitMessageProposal`; generation writes nothing. */
export interface CommitMessageProposal {
  /** Trimmed; may contain newlines (summary + body). */
  message: string;
  costUsd: number | null;
}

/** Explain (teammate-friendly summary) vs Review (risks/bugs/style) (P15b). */
export type AiAnalysisMode = 'explain' | 'review';

/** Diff source for aiAnalyzeDiff — discriminated on `kind` (P15b; P25 B1 adds
 *  the `worktree` + `branch` review scopes). */
export type AiDiffTarget =
  | { kind: 'commit'; oid: string }
  | { kind: 'workdirFile'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'staged' }
  | { kind: 'worktree' } // P25 B1: whole working-tree change set
  | { kind: 'branch'; name: string; base?: string | null }; // P25 B1: branch vs merge-base

/** Which range to digest for aiDigest — discriminated on `kind` (P28).
 *  betweenRefs = merge-base range (`from...to` narrative); lastDays =
 *  first-parent commits on HEAD within the window (days >= 1); sinceCommit =
 *  sugar for betweenRefs{from: oid, to: 'HEAD'}. */
export type AiDigestRange =
  | { kind: 'betweenRefs'; from: string; to: string }
  | { kind: 'lastDays'; days: number }
  | { kind: 'sinceCommit'; oid: string };

/** Read-only prose result of aiAnalyzeDiff (P15b). Mirrors the Rust
 *  `AiAnalysis`; analysis writes nothing. */
export interface AiAnalysis {
  text: string;
  costUsd: number | null;
}

/** Read-only branch/range summary result of aiSummarizeRange (P15c). Mirrors the
 *  Rust `AiSummary`; summarizing writes nothing. `base`/`target` are echoed for
 *  the panel header; `commitCount` is the number of commits listed (capped). */
export interface AiSummary {
  text: string;
  base: string;
  target: string;
  commitCount: number;
  costUsd: number | null;
}

/** Which range to write release notes for with aiChangelog — discriminated on
 *  `kind` (P56). betweenRefs = notes for commits in `to` but not `from` (any
 *  revparse-able refs; tags are the common case); sinceLastTag = notes since the
 *  most recent tag reachable from `target` (default HEAD), EXCLUDING `target`'s
 *  own tip. Mirrors the Rust `ChangelogRange`. */
export type ChangelogRange =
  | { kind: 'betweenRefs'; from: string; to: string }
  | { kind: 'sinceLastTag'; target?: string | null };

/** Grouped Markdown release notes + the RESOLVED range echoed for the panel
 *  header (crucially the resolved previous-tag name for sinceLastTag). Mirrors
 *  the Rust `AiChangelog`; generating writes nothing. `commitCount` is the number
 *  of commits listed (capped). */
export interface AiChangelog {
  text: string;
  fromRef: string;
  toRef: string;
  commitCount: number;
  costUsd: number | null;
}

/** Grounding source for aiSuggestBranchName — discriminated on `kind` (P53c).
 *  working = the index-aware working-tree change set (the common "about to start
 *  work" case); commitRange = name a branch that will carry `from..to`. */
export type BranchNameSource =
  | { kind: 'working' }
  | { kind: 'commitRange'; from: string; to: string };

/** Ranked branch-name candidates (best first); each is a valid git branch name
 *  (backend-sanitized). Mirrors the Rust `BranchNameProposal`. Naming writes
 *  nothing — the user picks/edits a candidate and the existing create path runs. */
export interface BranchNameProposal {
  names: string[];
  costUsd: number | null;
}

/** One proposed logical commit (P54). v1 is file-level: each changed file is in
 *  exactly one group across the plan. Round-trips as both proposal and plan. */
export interface ComposeGroup {
  files: string[];
  message: string;
}

/** Normalized composer proposal — always an apply-able partition of the change
 *  set (backend-enforced). Mirrors the Rust `ComposeProposal`. */
export interface ComposeProposal {
  groups: ComposeGroup[];
  /** Changed files the AI did not place (or overflow past the group cap). */
  unassigned: string[];
  /** Normalizer notes (informational; never an error). */
  notes: string[];
  costUsd: number | null;
}

/** User-finalized plan to apply (P54b). ORDERED — the first group becomes the
 *  oldest commit. A changed file absent from every group is intentionally left
 *  uncommitted in the working tree. Mirrors the Rust `ComposePlan`. */
export interface ComposePlan {
  groups: ComposeGroup[];
}

/** One created commit (P54b). `oid` is the full 40-hex id; `summary` is the first
 *  message line. */
export interface ComposeCommit {
  oid: string;
  summary: string;
}

/** Result of IpcApi.applyComposedCommits (P54b): created commits, oldest→newest. */
export interface ComposeApplyResult {
  commits: ComposeCommit[];
}

// ---------------------------------------------------------------- P55 NL→safe-op

/** The resolved-op kinds a plan can propose (P55). Mirrors the Rust `SafeOp` tag
 *  union; each maps 1:1 to an EXISTING typed command on confirm (safeOpDispatch). */
export type SafeOpKind =
  | 'reset'
  | 'revert'
  | 'switchBranch'
  | 'createBranch'
  | 'deleteBranch'
  | 'stash'
  | 'discard'
  | 'merge';

/** A fully-RESOLVED typed operation (P55). Rust resolved every ref/oid; the model
 *  never yields an oid. Discriminated on `kind`. Mirrors the Rust `SafeOp`. */
export type SafeOp =
  | { kind: 'reset'; targetOid: string; targetShort: string; mode: ResetMode }
  | { kind: 'revert'; oid: string; short: string }
  | { kind: 'switchBranch'; name: string; remote: boolean }
  | { kind: 'createBranch'; name: string; atOid: string | null }
  | { kind: 'deleteBranch'; name: string }
  | { kind: 'stash'; message: string | null; includeUntracked: boolean }
  | { kind: 'discard'; paths: string[] }
  | { kind: 'merge'; name: string };

/** Danger tier for the preview badge / confirm variant (P55). */
export type DangerLevel = 'safe' | 'caution' | 'destructive';

/** A ref that moves as part of an op, displayed `fromShort → toShort` (P55). */
export interface RefChange {
  name: string;
  fromShort: string;
  toShort: string;
}

/** One commit line in a preview's dropped list (P55). */
export interface CommitRef {
  short: string;
  summary: string;
}

/** Read-only description of what confirming a `SafeOp` will do (P55). All fields
 *  are display-ready; React only renders. Mirrors the Rust `OperationPreview`. */
export interface OperationPreview {
  title: string;
  summary: string;
  danger: DangerLevel;
  refChanges: RefChange[];
  droppedCommits: CommitRef[];
  addedCommits: number;
  worktreeWarning: string | null;
  confirmLabel: string;
}

/** A resolved, previewable proposal (P55). `rationale` is a one-line "why this
 *  maps to your ask" (Rust-generated). Mirrors the Rust `ProposedOperation`. */
export interface ProposedOperation {
  op: SafeOp;
  preview: OperationPreview;
  rationale: string;
  costUsd: number | null;
}

/** Result of aiPlanOperation (P55). `unsupported` is a NORMAL (non-error) outcome
 *  rendered as a calm "I can't do that safely" message. Mirrors the Rust
 *  `PlanOutcome`. */
export type OperationPlan =
  | { kind: 'proposed'; operation: ProposedOperation }
  | { kind: 'unsupported'; reason: string; costUsd: number | null };

/** Result of IpcApi.checkForUpdate (P42). `available` false ⇒ up to date;
 *  version/notes/date populated only when available. currentVersion is always set. */
export interface UpdateCheckResult {
  available: boolean;
  currentVersion: string;
  /** Target version when available, else null. */
  version: string | null;
  /** Release notes (may be markdown/plain), else null. */
  notes: string | null;
  /** Publish date string from the manifest, else null. */
  date: string | null;
}

/** Streamed progress of downloadAndInstallUpdate (P42). Bytes are cumulative. */
export interface UpdateProgress {
  phase: 'started' | 'downloading' | 'finished';
  downloadedBytes: number;
  /** Total size when the manifest/server provides it, else null. */
  contentLength: number | null;
}

export interface UiSettings {
  theme: Theme;
  paneWidths: PaneWidths;
  listView: ListView;
  /** P67 §4: right-panel density; display-only, patches independently. */
  panelDensity: PanelDensity;
  /** P80 D1: which commit button is emphasized in the Working tab footer. */
  primaryCommitAction: PrimaryCommitAction;
  autoFetch: AutoFetchSettings;
  /** P30: periodic read-only refresh signal (backend scheduler). */
  healthRefresh: HealthRefreshSettings;
  graph: GraphPrefs;
  // AI assistance (P13).
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  /** One-time consent to expose open repos to an external MCP client for
   *  reading (P16). */
  mcpConsented: boolean;
  /** One-time consent to let an external MCP client modify open repos (P16c). */
  mcpWriteConsented: boolean;
  /** P43: first-run onboarding has been shown+dismissed. Defaults false. */
  onboardingSeen: boolean;
  /** P42: auto-check for updates on launch. Defaults false. */
  autoCheckUpdates: boolean;
  /** P44: named identity profiles (global). */
  profiles: IdentityProfile[];
  /** P49: terminal launch command template ("{path}" placeholder). Empty ⇒
   *  per-OS auto-detect. */
  terminalCommand: string;
  /** P49: editor launch command template. Empty ⇒ auto-detect VS Code. */
  editorCommand: string;
  // ---- P68 §8.3: streaming AI-run knobs. Each patches independently; the two
  // LOCKED defaults are `aiHardCapSecs = 0` (unbounded — the user cancels instead)
  // and `aiMaxBudgetUsd = 0` (the `--max-budget-usd` flag is omitted entirely).
  /** Kill a run after this long with NO output from the CLI. `0` = disabled.
   *  PAUSED while the run awaits an answer (D3). Default 300. */
  aiIdleTimeoutSecs: number;
  /** Absolute wall-clock cap. `0` = unbounded (the default). Also paused while
   *  awaiting input. */
  aiHardCapSecs: number;
  /** Max turns before a still-questioning model is failed. Default 6. */
  aiMaxTurns: number;
  /** Stream `log` events at all. `false` suppresses them in RUST (no IPC cost);
   *  status-changing events always flow. Default true. */
  aiStreamLog: boolean;
  /** Pass `--include-partial-messages`. Default false (unverified line shape). */
  aiIncludePartialMessages: boolean;
  /** Repo access for a conflict run (D10). Default `readOnly`. */
  aiConflictTools: AiConflictTools;
  /** Bulk payload cap in bytes; over it the run SPLITS into sequential batches,
   *  never truncates. Default 400000. */
  aiBulkMaxBytes: number;
  /** `--max-budget-usd` when > 0; `0` ⇒ the flag is not passed. Default 0. */
  aiMaxBudgetUsd: number;
  /** Height of the AI activity dock in px. Default 180. */
  aiDockHeight: number;
  /** Dock starts collapsed (header only). Default false. */
  aiDockCollapsed: boolean;
}

export interface UiSettingsPatch {
  theme?: Theme;
  paneWidths?: PaneWidths;
  listView?: ListView;
  /** P67 §4: right-panel density (P67c). */
  panelDensity?: PanelDensity;
  /** P80 D1: primary commit action; patches independently. */
  primaryCommitAction?: PrimaryCommitAction;
  autoFetch?: AutoFetchSettings;
  /** Whole-struct patch, like autoFetch (P30 D7). */
  healthRefresh?: HealthRefreshSettings;
  graph?: GraphPrefs;
  // AI assistance (P13).
  aiEnabled?: boolean;
  aiConflictAutonomy?: AiAutonomy;
  aiConsented?: boolean;
  // Embedded MCP server (P16).
  mcpConsented?: boolean;
  // MCP write consent (P16c).
  mcpWriteConsented?: boolean;
  // First-run onboarding (P43).
  onboardingSeen?: boolean;
  // Auto-check-updates-on-launch (P42).
  autoCheckUpdates?: boolean;
  /** P44: identity profiles — whole-array replace (like paneWidths). */
  profiles?: IdentityProfile[];
  /** P49: terminal launch command template; patches independently. */
  terminalCommand?: string;
  /** P49: editor launch command template; patches independently. */
  editorCommand?: string;
  // P68 §8.3: the ten streaming AI-run knobs; each patches independently of
  // `graph` / `listView` / `panelDensity` and is clamped on write in Rust.
  aiIdleTimeoutSecs?: number;
  aiHardCapSecs?: number;
  aiMaxTurns?: number;
  aiStreamLog?: boolean;
  aiIncludePartialMessages?: boolean;
  aiConflictTools?: AiConflictTools;
  aiBulkMaxBytes?: number;
  aiMaxBudgetUsd?: number;
  aiDockHeight?: number;
  aiDockCollapsed?: boolean;
}

/** Embedded MCP server status for the Settings panel (P16). Mirrors the Rust
 *  `McpStatus`. `enabled` is the live runtime state; `port`/`url`/`token` are
 *  populated only while running. */
export interface McpStatus {
  /** Server running? */
  enabled: boolean;
  /** Write tools registered? Reflects the running server's live gate (P16c). */
  allowWrite: boolean;
  /** Bound port when running, else `null`. */
  port: number | null;
  /** e.g. "http://127.0.0.1:8765/mcp"; `null` when stopped. */
  url: string | null;
  /** Persisted bearer token; `null` when stopped. */
  token: string | null;
  /** 14 (read-only) or 34 (write enabled). */
  toolCount: number;
}

/** Persisted multi-tab session: open tabs (in display order) + the active tab.
 *  `repoId`s are canonical workdir path strings. */
export interface SessionState {
  openRepos: string[];
  activeRepo: string | null;
}

// P24 — AI-asset management (inventory + drift). Mirrors the Rust wire types
// in `crates/bonsai-core/src/assets/` exactly (camelCase).

/** Kind of an AI-asset target. Bare-string serde enum on the Rust side. */
export type AssetKind = 'singleFile' | 'rulesDir' | 'config';

export interface AssetFile {
  path: string;
  /** u64 on the wire; safe as a JS number here. */
  size: number;
  contentHash: string;
  normalizedHash: string;
  /** epoch seconds, or null when unavailable. */
  modified: number | null;
}

export interface AiAsset {
  id: string;
  agent: string;
  label: string;
  kind: AssetKind;
  path: string;
  managed: boolean;
  exists: boolean;
  files: AssetFile[];
}

export interface DriftEntry {
  assetId: string;
  exists: boolean;
  comparable: boolean;
  normalizedHash: string | null;
  inSync: boolean;
}

export interface DriftReport {
  canonicalId: string | null;
  canonicalHash: string | null;
  entries: DriftEntry[];
  inSync: boolean;
}

export interface AiAssetInventory {
  assets: AiAsset[];
  drift: DriftReport;
}

export interface AssetContent {
  path: string;
  exists: boolean;
  content: string | null;
}

// P26 — Agent-asset (skills / subagents / slash commands) manager. Mirrors the
// Rust wire types in `crates/bonsai-core/src/assets/bundle.rs` exactly (camelCase;
// bare-string enums).

/** Which `.claude/` agent-asset kind. Bare-string serde enum on the Rust side. */
export type AgentAssetKind = 'skill' | 'agent' | 'command';

/** Severity of a validation finding. Bare-string serde enum on the Rust side. */
export type IssueSeverity = 'error' | 'warning';

/** One frontmatter entry; `value` is the verbatim opaque scalar after `key: `. */
export interface FrontmatterField {
  key: string;
  value: string;
}

export interface AssetIssue {
  severity: IssueSeverity;
  message: string;
}

/** Validation verdict for one asset. `valid` iff no Error-severity issue. */
export interface Validation {
  valid: boolean;
  issues: AssetIssue[];
}

export interface AgentAsset {
  kind: AgentAssetKind;
  /** Directory name (skill) or file stem (agent/command). */
  name: string;
  /** Repo-relative file path, forward slashes (e.g. `.claude/agents/foo.md`). */
  path: string;
  exists: boolean;
  /** Parsed flat frontmatter, in file order, unknown keys preserved. */
  frontmatter: FrontmatterField[];
  /** Everything after the closing `---` fence (verbatim); whole file if none. */
  body: string;
  /** `true` when the frontmatter uses multi-line/sequence/nested YAML the flat
   *  parser can't round-trip (§4.3). The structural signal the editor uses to
   *  open the asset read-only; the backend also re-guards saves on it. */
  complex: boolean;
  validation: Validation;
}

export interface AgentAssetInventory {
  assets: AgentAsset[];
}

/** Save payload for `saveAgentAsset` (P26b) — no path/exists/validation, which
 *  the backend derives/computes. */
export interface AgentAssetInput {
  kind: AgentAssetKind;
  name: string;
  frontmatter: FrontmatterField[];
  body: string;
}

/** One profile target: which single-file asset to write, and its verbatim content. */
export interface ProfileTarget {
  assetId: string;
  content: string;
}

export interface ContextProfile {
  name: string;
  description?: string | null;
  model?: string | null;
  targets: ProfileTarget[];
}

/** The on-disk store (`.bonsai/profiles.json`) and the wire shape of
 *  list/save/delete/activate. */
export interface ProfileStore {
  version: number;
  profiles: ContextProfile[];
  /** LEGACY mirror of `worktreeActivations["@main"]` (P31 D4). */
  activeProfile?: string | null;
  /** P31 D3/D4: worktree key (`"@main"` | linked worktree name) → the profile
   *  last activated INTO that worktree. Omitted by serde when empty. */
  worktreeActivations?: Record<string, string>;
}

export interface ProfilePreviewEntry {
  assetId: string;
  path: string;
  current: string | null;
  proposed: string;
  changed: boolean;
}

/** What an activation did to one target's file. Bare-string serde enum on Rust. */
export type TargetWriteAction = 'created' | 'written' | 'unchanged';

export interface TargetWriteResult {
  assetId: string;
  path: string;
  action: TargetWriteAction;
}

export interface ProfileActivation {
  profile: string;
  results: TargetWriteResult[];
  /** The store after `activeProfile` was updated (frontend refreshes from this). */
  store: ProfileStore;
}

/** P31 §4. One row of the worktree × AI-context matrix. Wire mirror of the
 *  Rust `WorktreeContextStatus`. */
export interface WorktreeContextStatus {
  /** Store key + command argument: `"@main"` | linked worktree name (D3). */
  worktreeKey: string;
  /** Display name (main basename / linked name). */
  name: string;
  /** Absolute path, forward slashes. */
  absPath: string;
  branch: string | null;
  isMain: boolean;
  isCurrent: boolean;
  locked: boolean;
  prunable: boolean;
  valid: boolean;
  /** From `worktreeActivations` (v1 legacy `activeProfile` folded in for `"@main"`). */
  activeProfile: string | null;
  /** D10: drift entries `comparable && exists && !inSync` in THIS worktree. */
  driftedCount: number;
  /** Comparable descriptors with `exists === false` in THIS worktree. */
  missingCount: number;
  /** D6: `valid && !prunable && !locked`. */
  activatable: boolean;
  /** Human-readable reason when `!activatable`, else null. */
  blockedReason: string | null;
}

/** P24e. The AI-translate helper's proposed instruction file. NOT written
 *  anywhere — the user reviews it and pastes it into a profile target. */
export interface AiGeneratedAsset {
  targetAgent: string;
  content: string;
}

export type Unsubscribe = () => void;

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

export interface AppError {
  kind:
    | 'git'
    | 'io'
    | 'other'
    | 'noRepo'
    | 'emptyMessage'
    | 'configMissing'
    | 'nothingToCommit'
    | 'branchExists'
    | 'invalidName'
    | 'checkoutConflict'
    | 'branchCheckedOutElsewhere'
    | 'unmergedBranch'
    | 'branchNotFound'
    | 'noRemote'
    | 'noUpstream'
    | 'authFailed'
    | 'networkError'
    | 'pushRejected'
    | 'operationInProgress'
    | 'noOperationInProgress'
    | 'unresolvedConflicts'
    | 'aiUnavailable'
    | 'aiFailed'
    /** P68 §B: the user cancelled a streaming AI run. NOT a failure — show a
     *  `cancelled` run state, no error toast. */
    | 'aiCancelled'
    | 'updateFailed'
    | 'externalToolFailed'
    | 'hookRejected'
    | 'forgeUnsupported'
    | 'forgeAuthRequired'
    | 'forgeRateLimited'
    | 'forgeApi'
    /** P70: no runnable `git` executable could be resolved. NOT an auth failure
     *  and NOT an ordinary git error: the frontend routes it to the ONE
     *  persistent `GitMissingBanner` (plus a single coalesced toast for a
     *  user-pressed remote op) instead of N repeated toasts. Raised only by
     *  paths that shell out — SSH-agent authentication never produces it. */
    | 'gitNotFound';
  message: string;
}

/** P70: which rung of the resolver ladder produced the git path. Mirrors the
 *  Rust `GitBinSource`. */
export type GitBinSource = 'override' | 'path' | 'registry' | 'wellKnown' | 'fallback';

/** P70: startup git preflight. `found: false` is a NORMAL result, never a
 *  rejection. Mirrors the Rust `GitAvailability`. */
export interface GitAvailability {
  found: boolean;
  /** The path actually tried — populated whenever a candidate resolved, even
   *  when it turned out to be unrunnable; `null` only when the ladder fell back
   *  to the bare name. The banner keys its "found but unrunnable" variant on it. */
  path: string | null;
  /** e.g. `'2.47.1.windows.1'`; `null` when not found or unparseable. */
  version: string | null;
  source: GitBinSource;
  /** Human one-liner: the diagnostic when found, the full not-found copy otherwise. */
  detail: string;
}

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
  /** All current index conflicts, path-ascending. Rejects noRepo | git. */
  listConflicts(repoId: string): Promise<ConflictEntry[]>;
  /** Read-only marker view of one conflicted file. Rejects noRepo | git. */
  getConflict(repoId: string, path: string): Promise<ConflictFile>;
  /** Resolve one conflicted path. Rejects noRepo | git | invalidName. */
  resolveConflict(repoId: string, path: string, resolution: ConflictResolution): Promise<void>;
  /** Stage user-authored resolved text for one conflicted path (P12).
   *  Rejects noRepo | git | invalidName. */
  resolveConflictText(repoId: string, path: string, content: string): Promise<void>;
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
