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
  | { kind: 'revert' };

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

export type ApplyStashOutcome =
  | { kind: 'applied' }
  | { kind: 'conflicts'; paths: string[] };

export interface CreateStashResult {
  created: boolean;
}

/** Reset mode (P20). Mirrors the Rust `ResetMode` serde enum (camelCase). */
export type ResetMode = 'soft' | 'mixed' | 'hard';

export interface CreateBranchHereResult {
  /** true when uncommitted work was auto-stashed and carried across. */
  stashed: boolean;
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

/** Cherry-pick outcome (P20). Mirrors the Rust `CherrypickOutcome` serde enum
 *  (tagged "kind", camelCase). `conflicts` pauses into RepoOpState.cherryPick. */
export type CherrypickOutcome =
  | { kind: 'committed'; oid: string }
  | { kind: 'conflicts'; paths: string[] };

/** Revert outcome (P20). Mirrors the Rust `RevertOutcome` serde enum (tagged
 *  "kind", camelCase). `conflicts` pauses into RepoOpState.revert. */
export type RevertOutcome =
  | { kind: 'committed'; oid: string }
  | { kind: 'conflicts'; paths: string[] };

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

/** Diff source for aiAnalyzeDiff — discriminated on `kind` (P15b). */
export type AiDiffTarget =
  | { kind: 'commit'; oid: string }
  | { kind: 'workdirFile'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'staged' };

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

export interface UiSettings {
  theme: Theme;
  paneWidths: PaneWidths;
  listView: ListView;
  autoFetch: AutoFetchSettings;
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
}

export interface UiSettingsPatch {
  theme?: Theme;
  paneWidths?: PaneWidths;
  listView?: ListView;
  autoFetch?: AutoFetchSettings;
  graph?: GraphPrefs;
  // AI assistance (P13).
  aiEnabled?: boolean;
  aiConflictAutonomy?: AiAutonomy;
  aiConsented?: boolean;
  // Embedded MCP server (P16).
  mcpConsented?: boolean;
  // MCP write consent (P16c).
  mcpWriteConsented?: boolean;
}

/** Embedded MCP server status for the Settings panel (P16). Mirrors the Rust
 *  `McpStatus`. `enabled` is the live runtime state; `port`/`url`/`token`/
 *  `claudeAddCommand` are populated only while running. */
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
  /** Ready-to-paste `claude mcp add` line; `null` when stopped. */
  claudeAddCommand: string | null;
  /** 14 (read-only) or 34 (write enabled). */
  toolCount: number;
}

/** Persisted multi-tab session: open tabs (in display order) + the active tab.
 *  `repoId`s are canonical workdir path strings. */
export interface SessionState {
  openRepos: string[];
  activeRepo: string | null;
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
    | 'aiFailed';
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
  /** Safe checkout of a LOCAL branch. Rejects
   *  branchNotFound | checkoutConflict | git | noRepo. */
  checkoutBranch(repoId: string, name: string): Promise<void>;
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
  /** Fetch ALL remotes. Rejects noRemote | authFailed | networkError | git | noRepo. */
  fetch(repoId: string): Promise<FetchResult>;
  /** Fetch upstream remote + fast-forward only. Rejects noUpstream | authFailed
   *  | networkError | checkoutConflict | git | noRepo. */
  pull(repoId: string): Promise<PullResult>;
  /** Push current branch (sets upstream to origin/<branch> when none). Rejects
   *  noRemote | authFailed | networkError | pushRejected | git | noRepo. */
  push(repoId: string): Promise<PushResult>;
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
  /** Per-line blame of `path` as of `atOid` (null → HEAD). Read-only. Rejects
   *  other (bad path) | git (binary/unknown/too large/invalid oid) | noRepo. */
  blameFile(repoId: string, path: string, atOid: string | null): Promise<BlameLine[]>;
  /** Commits that touched `path`, newest-first, capped at `limit`. An unknown
   *  path yields `[]` (not an error). Rejects other | git | noRepo. */
  fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]>;
  /** Stash stack, index 0 (most recent) first. Rejects noRepo | git. */
  listStashes(repoId: string): Promise<StashEntry[]>;
  /** Stash the dirty worktree. message=null → git default. Rejects
   *  operationInProgress | configMissing | git | noRepo. created:false == nothing to stash. */
  createStash(
    repoId: string,
    message: string | null,
    includeUntracked: boolean,
  ): Promise<CreateStashResult>;
  /** Apply stash `index` WITHOUT dropping. Rejects operationInProgress | git | noRepo. */
  applyStash(repoId: string, index: number): Promise<ApplyStashOutcome>;
  /** Apply + drop on clean success (retained on conflict). Rejects operationInProgress | git | noRepo. */
  popStash(repoId: string, index: number): Promise<ApplyStashOutcome>;
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
  /** Cherry-pick a single commit onto the current branch (P20). Clean →
   *  committed; conflict → pauses into RepoOpState.cherryPick. Rejects
   *  operationInProgress | git | checkoutConflict | configMissing |
   *  nothingToCommit | noRepo. */
  cherrypickCommit(repoId: string, oid: string): Promise<CherrypickOutcome>;
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
}
