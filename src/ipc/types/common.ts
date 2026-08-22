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

/** Reset mode (P20). Mirrors the Rust `ResetMode` serde enum (camelCase). */
export type ResetMode = 'soft' | 'mixed' | 'hard';

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
    /** P68 #7 / H1: the novel-content gate refused to auto-stage an AI body (it has
     *  a line present in no version of base/ours/theirs). Distinct from `aiFailed`
     *  so the frontend routes it to review instead of a raw "failed" toast. */
    | 'aiNeedsReview'
    /** P68 §B: the user cancelled a streaming AI run. NOT a failure — show a
     *  `cancelled` run state, no error toast. */
    | 'aiCancelled'
    | 'updateFailed'
    | 'externalToolFailed'
    | 'hookRejected'
    | 'forgeUnsupported'
    | 'forgeAuthRequired'
    | 'forgeRateLimited'
    | 'forgeApi'
    /** P70: no runnable `git` executable could be resolved. NOT an auth failure
     *  and NOT an ordinary git error: the frontend routes it to the ONE
     *  persistent `GitMissingBanner` (plus a single coalesced toast for a
     *  user-pressed remote op) instead of N repeated toasts. Raised only by
     *  paths that shell out — SSH-agent authentication never produces it. */
    | 'gitNotFound';
  message: string;
}

/** P70: which rung of the resolver ladder produced the git path. Mirrors the
 *  Rust `GitBinSource`. */
export type GitBinSource = 'override' | 'path' | 'registry' | 'wellKnown' | 'fallback';

/** P70: startup git preflight. `found: false` is a NORMAL result, never a
 *  rejection. Mirrors the Rust `GitAvailability`. */
export interface GitAvailability {
  found: boolean;
  /** The path actually tried — populated whenever a candidate resolved, even
   *  when it turned out to be unrunnable; `null` only when the ladder fell back
   *  to the bare name. The banner keys its "found but unrunnable" variant on it. */
  path: string | null;
  /** e.g. `'2.47.1.windows.1'`; `null` when not found or unparseable. */
  version: string | null;
  source: GitBinSource;
  /** Human one-liner: the diagnostic when found, the full not-found copy otherwise. */
  detail: string;
}
