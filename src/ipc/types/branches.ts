import type { HeadInfo } from './common';
import type { ApplyStashOutcome } from './stash';

export interface BranchInfo {
  /** Shorthand, e.g. "main", "feature/sidebar". */
  name: string;
  /** True for the branch HEAD points at (always false when detached/unborn). */
  isHead: boolean;
  /** Upstream shorthand, e.g. "origin/main"; null when none configured or the ref is gone. */
  upstream: string | null;
  /** Commits ahead of / behind upstream. null whenever `upstream` is null. */
  ahead: number | null;
  behind: number | null;
  /** Full 40-char hex oid of the branch tip. */
  tip: string;
}

export interface RemoteBranchInfo {
  /** Shorthand incl. remote, e.g. "origin/main". */
  name: string;
  /** Full 40-char hex oid of the remote-tracking branch tip. */
  tip: string;
}

/** One configured remote (P22 §3.1). Mirrors the Rust `RemoteInfo` (camelCase). */
export interface RemoteInfo {
  /** Remote name, e.g. "origin". */
  name: string;
  /** Fetch URL; null if unreadable/non-UTF-8. */
  url: string | null;
}

// --- P77: tag sync (mirrors bonsai-core `git/tag_sync.rs`; serde camelCase). ---

/**
 * One tag's reconciliation state against a chosen remote.
 * `deleted-on-remote` is RESERVED for a future pushed-set upgrade and is never
 * emitted by the v1 backend (folded into `local-only`); kept for forward-compat.
 */
export type TagSyncStatus =
  | 'in-sync'
  | 'local-only'
  | 'stale'
  | 'remote-only'
  | 'deleted-on-remote';

/** One tag row in a {@link TagSyncReport}; rendered verbatim (fully precomputed). */
export interface TagSyncEntry {
  /** Short tag name (no `refs/tags/` prefix). */
  name: string;
  status: TagSyncStatus;
  /** Peeled committish the LOCAL tag resolves to (40-hex); null => remote-only. */
  localOid: string | null;
  /** Peeled committish the REMOTE tag resolves to (40-hex); null => local-only. */
  remoteOid: string | null;
  /** True if the tag is an annotated tag object on EITHER side (display flag). */
  annotated: boolean;
}

/** Result of one live ls-remote reconciliation pass. */
export interface TagSyncReport {
  /** The remote actually queried (resolved default when the caller passed null). */
  remote: string;
  /** One row per tag in the local∪remote union, sorted case-insensitively. */
  entries: TagSyncEntry[];
}

/**
 * P84: result of one non-interactive automatic tag-sync pass (run on fetch or
 * on demand). Best-effort — auth/network/no-remote yield an empty report, never
 * an error. All buckets are sorted case-insensitively.
 */
export interface TagAutoSyncReport {
  /** The remote actually reconciled ("" when none configured / skipped). */
  remote: string;
  /** Tag names newly created locally from a remote-only tag. */
  adopted: string[];
  /** Tag names whose local ref was fast-forwarded onto the remote target. */
  moved: string[];
  /** Stale tags left untouched (local ahead or diverged — not a strict FF). */
  skippedDiverged: string[];
}

export interface BranchesSnapshot {
  /** Sorted case-insensitively by name. */
  local: BranchInfo[];
  /** Sorted case-insensitively; symbolic "<remote>/HEAD" entries excluded. */
  remote: RemoteBranchInfo[];
  /** Tag names (lightweight + annotated), sorted case-insensitively. */
  tags: string[];
  /** One source of truth for attached/detached/unborn in the sidebar. */
  head: HeadInfo;
}

/** Why a branch is safe to delete (P25 §4.1). Bare-string enum. */
export type StaleReason = 'merged' | 'goneUpstream';

/** One local branch classified as stale (P25 §4.1). Mirrors Rust `StaleBranch`. */
export interface StaleBranch {
  name: string;
  /** Full 40-hex tip oid. */
  tip: string;
  /** First line of the tip commit's message. */
  lastCommitSummary: string;
  /** Tip commit author name. */
  lastCommitAuthor: string;
  /** Tip committer time, epoch seconds. */
  lastCommitTime: number;
  /** Primary reason: 'merged' when merged (even if also gone), else 'goneUpstream'. */
  reason: StaleReason;
  /** Raw flags (a branch may be both). */
  merged: boolean;
  goneUpstream: boolean;
  /** Configured upstream shorthand (e.g. "origin/feature"), if any — present even when gone. */
  upstream: string | null;
  /** Ahead/behind the BASE (best-effort; null on lookup error). ahead=0 when merged. */
  ahead: number | null;
  behind: number | null;
  /** Always false in returned entries (the current branch is excluded); defensive wire field. */
  isCurrent: boolean;
}

/** Read-only stale classification result (P25 §4.1). Mirrors Rust `StaleReport`. */
export interface StaleReport {
  /** Resolved base shorthand (e.g. "main" / "origin/main"). */
  base: string;
  /** Full 40-hex base commit oid. */
  baseOid: string;
  /** Stale candidates, case-insensitively sorted by name. Excludes base + current HEAD. */
  branches: StaleBranch[];
}

/** Per-branch outcome of a batch delete (P25 §4.3). Bare-string enum. */
export type BranchDeleteStatus =
  | 'deleted'
  | 'skippedCurrent'
  | 'skippedBase'
  | 'skippedNotStale'
  | 'skippedNotFound'
  | 'failed';

/** One result row from `deleteBranches` (P25 §4.3). Mirrors Rust `BranchDeleteResult`. */
export interface BranchDeleteResult {
  name: string;
  status: BranchDeleteStatus;
  /** Human detail for skipped/failed rows; null when deleted. */
  message: string | null;
}

export interface CreateBranchHereResult {
  /** true when uncommitted work was auto-stashed and carried across. */
  stashed: boolean;
  /** Present only when `stashed`; null otherwise (serde None → null). */
  apply: ApplyStashOutcome | null;
}

/** Result of a dirty-safe branch switch (P33). */
export interface CheckoutResult {
  /** true when uncommitted work was auto-stashed and carried across. */
  stashed: boolean;
  /** true when the switched-to branch was fast-forwarded to its upstream. */
  fastForwarded: boolean;
  /** Present only when `stashed`; null otherwise (serde None → null). */
  apply: ApplyStashOutcome | null;
}

/** Result of a branch rename (P60a). Mirrors Rust `RenameBranchResult`. */
export interface RenameBranchResult {
  /** true when the renamed branch was the checked-out branch (HEAD followed the
   *  rename) — the frontend then refetches HEAD/status, not just the list. */
  wasHead: boolean;
  /** The upstream shorthand still configured after the rename (e.g. "origin/main"),
   *  or null. libgit2 renames the `branch.<name>.*` config section, so tracking
   *  is preserved. */
  upstream: string | null;
}

export type MergeOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string; stashed: boolean }
  | { kind: 'merged'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

export type RebaseOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string }
  | { kind: 'rebased'; branch: string; head: string; steps: number; warnings?: string[] }
  | { kind: 'conflicts'; paths: string[]; currentStep: number; totalSteps: number };

/** Interactive-rebase per-op action (P23). Mirrors the Rust `RebaseAction`
 *  serde enum EXACTLY. Wire: "pick" | "reword" | "squash" | "fixup" | "drop". */
export type RebaseAction = 'pick' | 'reword' | 'squash' | 'fixup' | 'drop';

/** One interactive-rebase todo-list entry (P23). Mirrors the Rust
 *  `RebaseTodoOp` (camelCase). `oid` = the commit being replayed. `newMessage`
 *  is REQUIRED for `reword`, OPTIONAL for `squash` (null → default concat),
 *  null otherwise. */
export interface RebaseTodoOp {
  oid: string;
  action: RebaseAction;
  newMessage: string | null;
}
