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
