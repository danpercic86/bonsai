// P87 git-activity observability — TS mirror of `bonsai_core::git::activity`
// (camelCase; an ABSENT field means the optional is unset, never null).
//
// ONE fire-and-forget event stream for every git op that runs hooks or does
// network I/O. The frontend log panel + toolbar phase readout + store are P87b;
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
  | 'commit'
  | 'amend'
  | 'mergeCommit'
  | 'push'
  | 'forcePush'
  | 'fetch'
  | 'pull';

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
}
