export interface CommitResult {
  /** Full 40-char hex oid of the new commit. */
  oid: string;
  /** First line of the cleaned message. */
  summary: string;
  /** Branch HEAD points at after the commit ("main"); null when detached. */
  branch: string | null;
  /** Non-blocking post-commit hook trouble (spawn failure or non-zero exit).
   *  The commit itself landed; shown as a warning toast. Null when hooks are
   *  disabled, absent, or succeeded. Audit #2 §3.3. */
  hookWarning: string | null;
}

/** Cherry-pick outcome (P20, extended P47). Mirrors the Rust `CherrypickOutcome`
 *  serde enum (tagged "kind", camelCase). `stashed` reports an autostash that was
 *  created for the operation (and restored on `committed`, retained otherwise);
 *  `conflicts` pauses into RepoOpState.cherryPick; `stashPopConflicts` = the pick
 *  committed cleanly but re-applying the retained autostash conflicted. */
export type CherrypickOutcome =
  | { kind: 'committed'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

/** Revert outcome (P20, extended P47). Mirrors the Rust `RevertOutcome` serde enum
 *  (tagged "kind", camelCase). `stashed`/`stashPopConflicts` mirror
 *  `CherrypickOutcome`; `conflicts` pauses into RepoOpState.revert. */
export type RevertOutcome =
  | { kind: 'committed'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };
