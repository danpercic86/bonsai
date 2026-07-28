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

export interface CommitResult {
  /** Full 40-char hex oid of the new commit. */
  oid: string;
  /** First line of the cleaned message. */
  summary: string;
  /** Branch HEAD points at after the commit ("main"); null when detached. */
  branch: string | null;
}

export interface RepoChangedPayload {
  reason: string;
}

export type Unsubscribe = () => void;

export interface AppError {
  kind: 'git' | 'io' | 'other' | 'noRepo' | 'emptyMessage' | 'configMissing' | 'nothingToCommit';
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
  /** Fires after debounced filesystem changes in the open repo. */
  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe>;
  /** Fires when the app window regains focus. */
  onWindowFocus(cb: () => void): Promise<Unsubscribe>;
}
