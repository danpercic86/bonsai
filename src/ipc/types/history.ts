import type { ResetMode } from './common';

/** One blamed line (P23). Mirrors the Rust `BlameLine` (camelCase) EXACTLY.
 *  `oid` is the 40-hex of the commit that last touched the line (resolves to a
 *  graph node for reveal-in-graph); `authorTs` is seconds since epoch (UTC). */
export interface BlameLine {
  oid: string;
  authorName: string;
  authorEmail: string;
  authorTs: number;
  summary: string;
  origLineNo: number;
  finalLineNo: number;
  lineText: string;
}

/** One commit that touched a file (P23). Mirrors the Rust `FileHistoryEntry`
 *  (camelCase) EXACTLY. `authorTs` is seconds since epoch (UTC). */
export interface FileHistoryEntry {
  oid: string;
  summary: string;
  authorName: string;
  authorEmail: string;
  authorTs: number;
}

/** One reflog entry (P38 §4.2). Mirrors the Rust `ReflogEntry` (camelCase)
 *  EXACTLY. `index` is the N in `<ref>@{N}` (0 == newest). `oldOid`/`newOid`
 *  are full 40-hex (the UI shortens); a 40-zero `oldOid` marks the ref root.
 *  `committerTs` is seconds since epoch (UTC). */
export interface ReflogEntry {
  index: number;
  oldOid: string;
  newOid: string;
  committerName: string;
  committerEmail: string;
  committerTs: number;
  message: string;
}

/** Classified last-operation kind (P60c). Mirrors the Rust `UndoKind` serde
 *  enum (camelCase) EXACTLY. Drives the undo verb + reset mode. */
export type UndoKind =
  | 'commit'
  | 'amend'
  | 'merge'
  | 'rebase'
  | 'fastForward'
  | 'cherryPick'
  | 'revert'
  | 'reset'
  | 'branchSwitch'
  | 'unknown';

/** Plan for reversing the last HEAD-moving operation (P60c). Mirrors the Rust
 *  `UndoPlan` (camelCase) EXACTLY. `targetOid`/`targetShort` are "" when there
 *  is nothing to undo or the target is the 40-zero root. `resetMode` is null
 *  when `!undoable`. `worktreeDirty` is TRACKED dirtiness (staged + unstaged) —
 *  a hard reset preserves untracked files. When `requiresCleanWorktree &&
 *  worktreeDirty` the UI SHOWS the plan but BLOCKS the button (stash first). */
export interface UndoPlan {
  kind: UndoKind;
  summary: string;
  targetOid: string;
  targetShort: string;
  resetMode: ResetMode | null;
  requiresCleanWorktree: boolean;
  worktreeDirty: boolean;
  undoable: boolean;
  reason: string | null;
}
