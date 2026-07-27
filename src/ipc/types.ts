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

export interface RepoChangedPayload {
  reason: string;
}

export type Unsubscribe = () => void;

export interface AppError {
  kind: 'git' | 'io' | 'other' | 'noRepo';
  message: string;
}

export interface IpcApi {
  /** Rejects with {@link AppError}. */
  openRepo(path: string): Promise<RepoInfo>;
  /** Resolves to `null` when the user cancels the dialog. */
  pickFolder(): Promise<string | null>;
  /** Rejects with {@link AppError} (`noRepo` when nothing is open). */
  getStatus(): Promise<StatusSnapshot>;
  /** Fires after debounced filesystem changes in the open repo. */
  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe>;
  /** Fires when the app window regains focus. */
  onWindowFocus(cb: () => void): Promise<Unsubscribe>;
}
