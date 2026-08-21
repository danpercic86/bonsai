/** AI conflict-resolution autonomy (P13). proposeReview = user accepts before
 *  anything is written/staged (default); autoResolve = write+stage immediately,
 *  user reviews the staged diff before commitMerge. */
export type AiAutonomy = 'proposeReview' | 'autoResolve';

/** Cheap Claude Code CLI health status (P13). A missing/broken CLI yields
 *  `installed:false` — never an error. Mirrors the Rust `AiAvailability`. */
export interface AiAvailability {
  installed: boolean;
  loggedIn: boolean;
  version: string | null;
  detail: string;
}

/** The model's proposed fully-merged file body for one conflicted path (P13).
 *  Mirrors the Rust `AiResolveProposal`; the proposal writes nothing. */
export interface AiResolveProposal {
  path: string;
  proposedText: string;
  costUsd: number | null;
}

/** P68 §F: one push event on the `ai_resolve_conflict_stream` channel. Mirrors the
 *  Rust `AiRunEvent` exactly (camelCase serde).
 *
 *  `runId` arrives on the FIRST (`started`) event — the command promise settles only
 *  when the whole run ends, so this is the ONLY way the UI learns the id in time to
 *  cancel or reply (D8). `seq` is monotonic from 0 per run; drop any event whose
 *  `seq` is <= the last seen (stale/duplicate guard). */
export type AiRunEventKind =
  /** Always first, seq 0, emitted BEFORE the child is spawned. */
  | 'started'
  /** One human-readable log line for the dock. High frequency ⇒ batch it (D5). */
  | 'log'
  /** A `result` line parsed; the run may continue with another turn. */
  | 'turnEnd'
  /** Blocked on `aiReplyRun`; the idle watchdog is paused (D3). */
  | 'awaitingInput'
  /** Terminal: success (the command promise resolves right after). */
  | 'done'
  /** Terminal: `text` is the same message as the `aiFailed` rejection. */
  | 'failed'
  /** Terminal: user cancel (the command rejects `aiCancelled`). */
  | 'cancelled';

export interface AiRunEvent {
  /** Stable for the whole run, even across sequential bulk batches. */
  runId: string;
  seq: number;
  kind: AiRunEventKind;
  /** One log line, the question, or the terminal message; never a whole payload. */
  text: string | null;
  /** Cost of the turn that just ended / of the run. LAST value wins within a run —
   *  never summed (spike §1.8). */
  costUsd: number | null;
  /** Since the run started, not since the turn. */
  elapsedMs: number;
  /** The file this event is about when known (bulk attribution); null run-level. */
  path: string | null;
  /** 1-based turn counter; 0 on `started`. */
  turn: number;
  /** Only on `cancelled`/`failed`: the assistant text accumulated so far (D2).
   *  DISPLAY-ONLY and lossy by construction — never offer it as a proposal. */
  partialText: string | null;
  /** P68d: the CLI's CUMULATIVE `estimated_tokens` from a `thinking_tokens`
   *  heartbeat — the run's only LIVE spend signal, since `costUsd` exists only at a
   *  turn boundary and a long single-turn run would otherwise read `$—` for minutes.
   *
   *  A `kind: 'log'` event with `text === null` and this set is a METRICS-ONLY
   *  heartbeat: record the number, do NOT append a log line (A4 — one heartbeat per
   *  second would drown the dock). The two fields are mutually exclusive on a
   *  `log` event.
   *
   *  Scope, verified against `claude` v2.1.233: THINKING tokens only, and estimated
   *  (600 reported vs 679 actual at the end of one run); a run that never enters
   *  extended thinking emits no heartbeats and this stays null throughout. Never
   *  convert it to a dollar figure — there is no price table anywhere in Bonsai. */
  thinkingTokens: number | null;
}

/** One path a streaming resolve could not handle. NEVER fatal to the batch (D11). */
export interface AiResolveFailure {
  path: string;
  reason: string;
}

/** P68 §D: the outcome of ONE streaming resolve run over 1..n paths. The promise —
 *  not the event stream — is authoritative for this data.
 *
 *  A `proposedText` here is a REVIEWABLE proposal, not a verified-clean merge: the
 *  single-path stream returns the model's body verbatim (P13 parity), so callers
 *  MUST keep applying `hasUnresolvedMarkers` before staging anything (D4). */
export interface AiResolveBatch {
  runId: string;
  proposals: AiResolveProposal[];
  failed: AiResolveFailure[];
  /** Last value within a run, summed across sequential bulk batches (A10). */
  costUsd: number | null;
  /** Max turns used across batches (1 when no question was asked). */
  turns: number;
}

/** P68 §B/D10: repo access granted to a conflict-resolution run. `readOnly` ⇒
 *  `--tools "Read,Grep,Glob"`; `none` ⇒ the old blind `--tools ""`. There is
 *  deliberately no write/edit/bash option. */
export type AiConflictTools = 'readOnly' | 'none';

/** The model's proposed commit message from the staged diff (P15a).
 *  Mirrors the Rust `CommitMessageProposal`; generation writes nothing. */
export interface CommitMessageProposal {
  /** Trimmed; may contain newlines (summary + body). */
  message: string;
  costUsd: number | null;
}

/** Explain (teammate-friendly summary) vs Review (risks/bugs/style) (P15b). */
export type AiAnalysisMode = 'explain' | 'review';

/** Diff source for aiAnalyzeDiff — discriminated on `kind` (P15b; P25 B1 adds
 *  the `worktree` + `branch` review scopes). */
export type AiDiffTarget =
  | { kind: 'commit'; oid: string }
  | { kind: 'workdirFile'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'staged' }
  | { kind: 'worktree' } // P25 B1: whole working-tree change set
  | { kind: 'branch'; name: string; base?: string | null }; // P25 B1: branch vs merge-base

/** Which range to digest for aiDigest — discriminated on `kind` (P28).
 *  betweenRefs = merge-base range (`from...to` narrative); lastDays =
 *  first-parent commits on HEAD within the window (days >= 1); sinceCommit =
 *  sugar for betweenRefs{from: oid, to: 'HEAD'}. */
export type AiDigestRange =
  | { kind: 'betweenRefs'; from: string; to: string }
  | { kind: 'lastDays'; days: number }
  | { kind: 'sinceCommit'; oid: string };

/** Read-only prose result of aiAnalyzeDiff (P15b). Mirrors the Rust
 *  `AiAnalysis`; analysis writes nothing. */
export interface AiAnalysis {
  text: string;
  costUsd: number | null;
}

/** Read-only branch/range summary result of aiSummarizeRange (P15c). Mirrors the
 *  Rust `AiSummary`; summarizing writes nothing. `base`/`target` are echoed for
 *  the panel header; `commitCount` is the number of commits listed (capped). */
export interface AiSummary {
  text: string;
  base: string;
  target: string;
  commitCount: number;
  costUsd: number | null;
}

/** Which range to write release notes for with aiChangelog — discriminated on
 *  `kind` (P56). betweenRefs = notes for commits in `to` but not `from` (any
 *  revparse-able refs; tags are the common case); sinceLastTag = notes since the
 *  most recent tag reachable from `target` (default HEAD), EXCLUDING `target`'s
 *  own tip. Mirrors the Rust `ChangelogRange`. */
export type ChangelogRange =
  | { kind: 'betweenRefs'; from: string; to: string }
  | { kind: 'sinceLastTag'; target?: string | null };

/** Grouped Markdown release notes + the RESOLVED range echoed for the panel
 *  header (crucially the resolved previous-tag name for sinceLastTag). Mirrors
 *  the Rust `AiChangelog`; generating writes nothing. `commitCount` is the number
 *  of commits listed (capped). */
export interface AiChangelog {
  text: string;
  fromRef: string;
  toRef: string;
  commitCount: number;
  costUsd: number | null;
}

/** Grounding source for aiSuggestBranchName — discriminated on `kind` (P53c).
 *  working = the index-aware working-tree change set (the common "about to start
 *  work" case); commitRange = name a branch that will carry `from..to`. */
export type BranchNameSource =
  | { kind: 'working' }
  | { kind: 'commitRange'; from: string; to: string };

/** Ranked branch-name candidates (best first); each is a valid git branch name
 *  (backend-sanitized). Mirrors the Rust `BranchNameProposal`. Naming writes
 *  nothing — the user picks/edits a candidate and the existing create path runs. */
export interface BranchNameProposal {
  names: string[];
  costUsd: number | null;
}

/** One proposed logical commit (P54). v1 is file-level: each changed file is in
 *  exactly one group across the plan. Round-trips as both proposal and plan. */
export interface ComposeGroup {
  files: string[];
  message: string;
}

/** Normalized composer proposal — always an apply-able partition of the change
 *  set (backend-enforced). Mirrors the Rust `ComposeProposal`. */
export interface ComposeProposal {
  groups: ComposeGroup[];
  /** Changed files the AI did not place (or overflow past the group cap). */
  unassigned: string[];
  /** Normalizer notes (informational; never an error). */
  notes: string[];
  costUsd: number | null;
}

/** User-finalized plan to apply (P54b). ORDERED — the first group becomes the
 *  oldest commit. A changed file absent from every group is intentionally left
 *  uncommitted in the working tree. Mirrors the Rust `ComposePlan`. */
export interface ComposePlan {
  groups: ComposeGroup[];
}

/** One created commit (P54b). `oid` is the full 40-hex id; `summary` is the first
 *  message line. */
export interface ComposeCommit {
  oid: string;
  summary: string;
}

/** Result of IpcApi.applyComposedCommits (P54b): created commits, oldest→newest. */
export interface ComposeApplyResult {
  commits: ComposeCommit[];
}
