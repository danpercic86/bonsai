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

export interface CommitResult {
  /** Full 40-char hex oid of the new commit. */
  oid: string;
  /** First line of the cleaned message. */
  summary: string;
  /** Branch HEAD points at after the commit ("main"); null when detached. */
  branch: string | null;
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
  | { kind: 'wouldNotFastForward'; branch: string; ahead: number; behind: number };

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

export type MergeOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string; stashed: boolean }
  | { kind: 'merged'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

export type RebaseOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string }
  | { kind: 'rebased'; branch: string; head: string; steps: number }
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
export interface GraphPrefs {
  dotRadius: number;
  avatarRadius: number;
  rowHeight: number;
  laneWidth: number;
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
}

export interface UiSettingsPatch {
  theme?: Theme;
  paneWidths?: PaneWidths;
  listView?: ListView;
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
    | 'updateFailed';
  message: string;
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
  /** Stage paths (worktree-relative, forward slashes — StatusEntry.path strings). Atomic. */
  stage(repoId: string, paths: string[]): Promise<void>;
  /** Unstage paths. Atomic. Safe (worktree never touched). */
  unstage(repoId: string, paths: string[]): Promise<void>;
  /** Create a commit from the index. Rejects with AppError kinds
   *  emptyMessage | configMissing | nothingToCommit | git | noRepo. */
  commit(repoId: string, message: string): Promise<CommitResult>;
  /** Diff of one working-dir file. staged=false: index vs workdir; staged=true: HEAD vs index.
   *  origPath: pass StatusEntry.origPath (renames). Rejects AppError ('noRepo', 'git'). */
  getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
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
  ): Promise<FileDiff>;
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
  /** Push current branch (sets upstream to origin/<branch> when none). Rejects
   *  noRemote | authFailed | networkError | pushRejected | git | noRepo. */
  push(repoId: string): Promise<PushResult>;
  /** Force-push the current branch to its upstream WITH A LEASE (P37). Refuses
   *  (pushRejected) if the remote moved since the last fetch. Rejects
   *  noUpstream | noRemote | authFailed | networkError | pushRejected | git | noRepo. */
  forcePush(repoId: string): Promise<PushResult>;
  /** Current operation state (merge/rebase/...). Part of the refresh batch.
   *  Rejects noRepo | git. */
  getOpState(repoId: string): Promise<RepoOpState>;
  /** Merge a local or remote-tracking branch into the current branch. Rejects
   *  operationInProgress | branchNotFound | checkoutConflict | configMissing
   *  | git | noRepo. */
  mergeBranch(repoId: string, name: string): Promise<MergeOutcome>;
  /** Finalize a paused merge. Rejects noOperationInProgress
   *  | unresolvedConflicts | emptyMessage | configMissing | git | noRepo. */
  commitMerge(repoId: string, message: string): Promise<CommitResult>;
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
   *  everything except those (`appliedSkippingReserved`). */
  applyStash(repoId: string, index: number, skipReserved: boolean): Promise<ApplyStashOutcome>;
  /** Apply + drop on clean success (retained on conflict). Rejects operationInProgress | git | noRepo.
   *  `skipReserved`: as for `applyStash`; when any reserved path is skipped the
   *  stash is KEPT (not dropped) so the reserved blobs are not lost. */
  popStash(repoId: string, index: number, skipReserved: boolean): Promise<ApplyStashOutcome>;
  /** Permanently discard stash `index` (UI confirms). Rejects git | noRepo. */
  dropStash(repoId: string, index: number): Promise<void>;
  /** Amend HEAD with a new message + the current index (P20). Preserves HEAD's
   *  parents + original author. Rejects operationInProgress | emptyMessage |
   *  configMissing | git | noRepo. */
  commitAmend(repoId: string, message: string): Promise<CommitResult>;
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
  /** Cheap Claude Code CLI health probe (P13). Never rejects for CLI state. */
  checkAiAvailability(): Promise<AiAvailability>;
  /** Propose an AI merge resolution for one conflicted path (P13). Writes nothing.
   *  Rejects aiUnavailable | aiFailed | git | invalidName | noRepo. */
  aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal>;
  /** P15a. Generate a commit message from the staged diff. Never auto-commits.
   *  Rejects aiUnavailable | aiFailed | nothingToCommit | git | noRepo. */
  generateCommitMessage(repoId: string): Promise<CommitMessageProposal>;
  /** P15b. Explain or review a diff target (read-only prose). Writes nothing.
   *  Rejects aiUnavailable | aiFailed | nothingToCommit | git | invalidName | noRepo. */
  aiAnalyzeDiff(repoId: string, target: AiDiffTarget, mode: AiAnalysisMode): Promise<AiAnalysis>;
  /** P28. AI "what changed" digest over a selectable range (read-only prose).
   *  Writes nothing. Rejects aiUnavailable | aiFailed | git | invalidName | noRepo. */
  aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis>;
  /** P15c. Summarize commits/diff unique to `target` vs `base` (read-only prose).
   *  Rejects aiUnavailable | aiFailed | git | noRepo. */
  aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary>;
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
}
