export interface HeadInfo {
  branchName: string | null;
  oid: string;
  detached: boolean;
  unborn: boolean;
}

export interface RepoInfo {
  path: string;
  isRepo: boolean;
  head: HeadInfo | null;
}

export interface AppError {
  kind: 'git' | 'io' | 'other';
  message: string;
}

export interface IpcApi {
  /** Rejects with {@link AppError}. */
  openRepo(path: string): Promise<RepoInfo>;
  /** Resolves to `null` when the user cancels the dialog. */
  pickFolder(): Promise<string | null>;
}
