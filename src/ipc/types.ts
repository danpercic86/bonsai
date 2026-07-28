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

export type RefKind = 'localBranch' | 'remoteBranch' | 'tag' | 'head';

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
}

export interface RemoteBranchInfo {
  /** Shorthand incl. remote, e.g. "origin/main". */
  name: string;
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
  reason: string;
}

export type Theme = 'dark' | 'light';

export interface PaneWidths {
  sidebar: number;
  rightPanel: number;
}

export interface UiSettings {
  theme: Theme;
  paneWidths: PaneWidths;
}

export interface UiSettingsPatch {
  theme?: Theme;
  paneWidths?: PaneWidths;
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
    | 'pushRejected';
  message: string;
}

export interface IpcApi {
  /** Rejects with {@link AppError}. */
  openRepo(path: string): Promise<RepoInfo>;
  /** Resolves to `null` when the user cancels the dialog. */
  pickFolder(): Promise<string | null>;
  /** Rejects with {@link AppError} (`noRepo` when nothing is open). */
  getStatus(): Promise<StatusSnapshot>;
  /** Full graph layout for the open repo. Rejects with {@link AppError} (`noRepo` when nothing open). */
  getGraph(): Promise<GraphLayout>;
  /** Stage paths (worktree-relative, forward slashes — StatusEntry.path strings). Atomic. */
  stage(paths: string[]): Promise<void>;
  /** Unstage paths. Atomic. Safe (worktree never touched). */
  unstage(paths: string[]): Promise<void>;
  /** Create a commit from the index. Rejects with AppError kinds
   *  emptyMessage | configMissing | nothingToCommit | git | noRepo. */
  commit(message: string): Promise<CommitResult>;
  /** Diff of one working-dir file. staged=false: index vs workdir; staged=true: HEAD vs index.
   *  origPath: pass StatusEntry.origPath (renames). Rejects AppError ('noRepo', 'git'). */
  getWorkdirFileDiff(path: string, origPath: string | null, staged: boolean): Promise<FileDiff>;
  /** Commit details + per-file headers vs first parent. Rejects AppError ('noRepo', 'git'). */
  getCommitDiff(oid: string): Promise<CommitDiff>;
  /** Hunks for one file of a commit's first-parent diff. */
  getCommitFileDiff(oid: string, path: string, origPath: string | null): Promise<FileDiff>;
  /** Local branches + remotes + tags + HEAD in one snapshot. Rejects noRepo | git. */
  listBranches(): Promise<BranchesSnapshot>;
  /** Create branch at current HEAD (no checkout). Rejects
   *  invalidName | branchExists | git | noRepo. */
  createBranch(name: string): Promise<void>;
  /** Safe checkout of a LOCAL branch. Rejects
   *  branchNotFound | checkoutConflict | git | noRepo. */
  checkoutBranch(name: string): Promise<void>;
  /** Delete a LOCAL, fully merged, non-current branch. Rejects
   *  branchNotFound | unmergedBranch | git | noRepo. */
  deleteBranch(name: string): Promise<void>;
  /** Fetch ALL remotes. Rejects noRemote | authFailed | networkError | git | noRepo. */
  fetch(): Promise<FetchResult>;
  /** Fetch upstream remote + fast-forward only. Rejects noUpstream | authFailed
   *  | networkError | checkoutConflict | git | noRepo. */
  pull(): Promise<PullResult>;
  /** Push current branch (sets upstream to origin/<branch> when none). Rejects
   *  noRemote | authFailed | networkError | pushRejected | git | noRepo. */
  push(): Promise<PushResult>;
  /** Recent successfully-opened repos, most recent first, max 10. Never rejects
   *  for a missing/corrupt settings file (returns []). */
  getRecentRepos(): Promise<RecentRepo[]>;
  /** Removes one entry; returns the updated list. */
  removeRecentRepo(path: string): Promise<RecentRepo[]>;
  /** Fires after debounced filesystem changes in the open repo. */
  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe>;
  /** Fires when the app window regains focus. */
  onWindowFocus(cb: () => void): Promise<Unsubscribe>;
  /** Current UI settings (theme + pane widths). Never rejects for a missing/corrupt file. */
  getUiSettings(): Promise<UiSettings>;
  /** Applies a partial patch (only defined fields) and returns the resulting settings. */
  setUiSettings(patch: UiSettingsPatch): Promise<UiSettings>;
}
