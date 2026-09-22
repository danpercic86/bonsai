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
  /** Why the refresh fired:
   *  - `"fs"` — a debounced watcher burst that touched `.git/HEAD`, `.git/refs/**`
   *    or `.git/packed-refs` (or a notify error): the commit graph may have moved.
   *  - `"fsWorktree"` (P110) — a debounced watcher burst of working-tree content
   *    and/or `.git/index` ONLY. Rust classified it, so the frontend can safely
   *    run the narrow `worktree` scope instead of a full graph re-stream.
   *  - `"fetch"` — a backend-confirmed remote update.
   *  - `"tags"` — P85 A3: the fire-and-forget fetch tag auto-sync adopted/moved a
   *    local tag.
   *  Any unknown reason is treated as a full refresh — always safe. */
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
    /** Audit 2026-09-11: an operation was REFUSED because it would have run
     *  repository git hooks the caller cannot disclose to the user. Raised only
     *  by `bonsai-mcp`'s standalone stdio server (no frontend to disclose
     *  through), so the app never sees it — it is in this union for Rust↔TS
     *  parity. Distinct from `hookRejected`: NO hook ran and nothing changed. */
    | 'hooksNotPermitted'
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
  /** P113a: present ONLY on `forgeRateLimited`, and only when the provider
   *  advertised a usable wait — seconds from now, parsed from `Retry-After`
   *  (Azure/Bitbucket) or the `*-RateLimit-Reset` epoch (GitHub/GitLab). The
   *  message still spells it out for humans; anything that BACKS OFF reads this
   *  instead of parsing the sentence. Absent ⇒ the caller picks its own
   *  default wait (see `forgeBackoff.ts`). */
  retryAfterSecs?: number;
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

// ---- P112: external-tool detection (mirrors `bonsai_core::tools`) ----------

/** Which of the two configurable tool slots. The file manager is deliberately
 *  NOT configurable. */
export type ExternalToolKind = 'terminal' | 'editor';

/** Where a resolution came from. Not surfaced in the UI (DEC-2); it is the
 *  provenance record that drives the launch-time recheck and the tests. */
export type ExternalToolSource =
  | 'builtIn'
  | 'path'
  | 'registry'
  | 'wellKnown'
  | 'appBundle'
  | 'custom';

/** One picker row. DISPLAY ONLY — the backend never accepts `label` / `detail` /
 *  `source` / `present` back. */
export interface DetectedTool {
  /** Opaque id — a catalog id or the pseudo-id `'custom'`. The ONLY value sent
   *  back, and only ever as a `terminalTool` / `editorTool` patch. */
  id: string;
  label: string;
  kind: ExternalToolKind;
  /** Not surfaced in the UI (DEC-2); kept for tests and provenance. */
  source: ExternalToolSource;
  /** Display-only: the resolved absolute path, or `'built in'`. */
  detail: string;
  /** `false` only for a remembered `'custom'` row whose stored path is gone
   *  (AMEND-3) — the row is still LISTED, so the UI can explain itself. */
  present: boolean;
}

/** One round trip's worth of picker data. */
export interface ExternalToolScan {
  /** Detected terminals in catalog order, then the remembered custom row. */
  terminals: DetectedTool[];
  editors: DetectedTool[];
  /** `id -> label` for every catalog entry of that kind, ANY os (AMEND-1).
   *  Without it a kept-but-undetected selection renders as the raw id — the
   *  picker would literally read `notepadpp`. */
  terminalLabels: Record<string, string>;
  editorLabels: Record<string, string>;
  /** Freshness identity the frontend compares to know a refresh landed.
   *  NOT displayed (DEC-3) — the app has no time formatter. */
  scannedAtMs: number;
}
