export interface StashEntry {
  index: number;      // 0 == stash@{0}; SHIFTS after drop/pop — always refetch
  message: string;
  oid: string;        // stash commit oid
  baseOid: string;    // first-parent = base commit the pill attaches to
  ts: number;         // seconds since epoch (UTC)
}

export type ApplyStashOutcome =
  | { kind: 'applied' }
  | { kind: 'conflicts'; paths: string[] }
  /** Blocked pre-apply: the stash contains Windows-reserved paths (e.g. `NUL`)
   *  that cannot be checked out. Nothing was applied and the stash is retained.
   *  Retry with `skipReserved: true` to apply everything except these. */
  | { kind: 'reservedPaths'; paths: string[] }
  /** Applied everything except the listed Windows-reserved paths, which could
   *  not be restored. For pop, the stash is KEPT (not dropped) so the reserved
   *  blobs are not lost. */
  | { kind: 'appliedSkippingReserved'; skipped: string[] }
  /** Applied, but these brand-new (untracked) files were NOT restored
   *  byte-identically — the target ref tracks that path, or a directory now
   *  occupies it. The stash is KEPT: it holds the only copy of that content. */
  | { kind: 'appliedPartially'; unrestored: string[] }
  /** The re-apply failed after the surrounding operation (e.g. a branch switch)
   *  had already succeeded. Nothing was dropped — the work is intact at
   *  stash@{0}. */
  | { kind: 'notApplied'; message: string };

export interface CreateStashResult {
  created: boolean;
}

/** Which changes a createStash call captures. Mirrors Rust `StashScope` (camelCase).
 *  - `all`: staged + unstaged tracked changes (untracked left in place).
 *  - `allWithUntracked`: adds untracked files.
 *  - `staged`: only the staged (index-vs-HEAD) paths; mixed files are folded whole,
 *    unstaged-only paths and untracked files are left untouched. */
export type StashScope = 'all' | 'allWithUntracked' | 'staged';
