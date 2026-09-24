// P87 git-activity observability — TS mirror of `bonsai_core::git::activity`
// (camelCase; an ABSENT field means the optional is unset, never null).
//
// ONE fire-and-forget event stream for every repo-changing git op (P87 hooks +
// network ops; P119 extends it to every action). The frontend log panel + toolbar phase readout + store are P87b;
// this file is only the wire type + the `gitActivitySubscribe` IPC surface.

export type GitActivityKind =
  | 'started'
  | 'phase'
  | 'stdoutLine'
  | 'stderrLine'
  | 'hookDone'
  | 'finished'
  | 'progress';

export type GitActivityCategory =
  // P87
  | 'commit'
  | 'amend'
  | 'mergeCommit'
  | 'push'
  | 'forcePush'
  | 'fetch'
  | 'pull'
  // P119 §1 rows 1–56, table order
  | 'checkoutBranch'
  | 'checkoutCommit'
  | 'checkoutRemote'
  | 'createBranch'
  | 'deleteBranch'
  | 'deleteBranches'
  | 'renameBranch'
  | 'deleteRemoteTracking'
  | 'merge'
  | 'abortMerge'
  | 'rebase'
  | 'interactiveRebase'
  | 'rebaseContinue'
  | 'rebaseSkip'
  | 'rebaseAbort'
  | 'cherryPick'
  | 'cherryPickContinue'
  | 'cherryPickAbort'
  | 'revert'
  | 'revertContinue'
  | 'revertAbort'
  | 'resetSoft'
  | 'resetMixed'
  | 'resetHard'
  | 'stashCreate'
  | 'stashApply'
  | 'stashPop'
  | 'stashDrop'
  | 'createTag'
  | 'deleteTag'
  | 'pushTag'
  | 'deleteRemoteTag'
  | 'forceRefreshTag'
  | 'submoduleAdd'
  | 'submoduleInit'
  | 'submoduleUpdate'
  | 'submoduleSync'
  | 'submoduleDeinit'
  | 'submoduleRemove'
  | 'worktreeAdd'
  | 'worktreeRemove'
  | 'worktreeLock'
  | 'worktreeUnlock'
  | 'discard'
  | 'bisectStart'
  | 'bisectGood'
  | 'bisectBad'
  | 'bisectSkip'
  | 'bisectReset'
  | 'composeCommits'
  | 'cloneRepo'
  | 'initRepo'
  | 'addRemote'
  | 'removeRemote'
  | 'renameRemote'
  | 'setRemoteUrl';

/** How a SUCCESSFUL run ended, when that is more than "done" (P119 §2.6).
 *  `conflicts` = the command succeeded but left the repo paused on conflicts. */
export type GitRunOutcome = 'conflicts' | 'fastForwarded' | 'merged' | 'upToDate';

export type GitPhaseKind = 'preparing' | 'runningHook' | 'network' | 'finalizing';

export interface GitPhase {
  kind: GitPhaseKind;
  /** Set only for a `runningHook` phase (e.g. "pre-push"). */
  hook?: string;
}

/** Structured fetch/pull network-transfer counts (§14). Present only on a
 *  `progress` event. `totalObjects === 0` ⇒ indeterminate (guard before
 *  dividing). `totalDeltas`/`indexedDeltas` are set only during delta-resolution. */
export interface GitTransferProgress {
  receivedObjects: number;
  totalObjects: number;
  indexedObjects: number;
  receivedBytes: number;
  totalDeltas?: number;
  indexedDeltas?: number;
}

/** One event on the git-activity stream. `id` is stable per activity (first
 *  delivered on `started`); `seq` is monotonic from 0 — drop any event whose
 *  `seq <= the last seen` for its id. */
export interface GitActivityEvent {
  id: string;
  seq: number;
  kind: GitActivityKind;
  elapsedMs: number;
  /** `started` only. */
  category?: GitActivityCategory;
  /** `started` + `phase`. */
  phase?: GitPhase;
  /** `stdoutLine` / `stderrLine` only; capped + control-stripped. */
  line?: string;
  /** `hookDone` only. */
  hook?: string;
  /** `hookDone` + `finished` (absent = killed / no exit code). */
  code?: number;
  /** `hookDone` + `finished`. */
  success?: boolean;
  /** `progress` only. */
  progress?: GitTransferProgress;
  /** `started` only — the run's target ref. A raw git identifier (`origin/main`,
   *  `main`), sanitized and <=255 chars. Absent = this run has no target; the
   *  frontend derives all copy from the category (never a human phrase here). */
  target?: string;
  /** `started` only; present iff the run acts on ≥2 items (then `target` is
   *  absent). A count, not copy — the frontend owns "Delete 3 branches". P119. */
  targetCount?: number;
  /** `finished` only, success runs only. P119. */
  outcome?: GitRunOutcome;
}
