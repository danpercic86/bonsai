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
