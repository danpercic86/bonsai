import type { FileStatus } from './status';

export type LineKind = 'context' | 'add' | 'del';

/** One selected changed line for partial staging (P17). Context lines are
 *  dropped before sending; the backend identifies an Add by `newNo` and a Del
 *  by `oldNo`. Mirrors the Rust `LineSelection`. */
export interface LineSelection {
  kind: LineKind; // 'add' | 'del' (context dropped before sending)
  oldNo: number | null;
  newNo: number | null;
}

export interface DiffLine {
  kind: LineKind;
  /** Line number in the OLD file; `null` for add lines. */
  oldNo: number | null;
  /** Line number in the NEW file; `null` for del lines. */
  newNo: number | null;
  /** Content without the leading +/-/space and without the trailing newline. */
  content: string;
  /** Present (true) only on the last line of a file lacking a trailing newline. */
  noNewline?: boolean;
  /** P61a: CHANGED sub-ranges within `content` as `[startCodePoint, lenCodePoints]`,
   *  ascending + non-overlapping. Present only on paired add/del lines when the
   *  diff was fetched with `intraline=true`; absent/empty => render plain. Slice
   *  via `Array.from(content)` (code-point aware — offsets are NOT UTF-16 units). */
  spans?: [number, number][];
}

export interface Hunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

export interface FileDiff {
  /** NEW path for renames; repo-relative, forward slashes. */
  path: string;
  /** OLD path for renames; `null` otherwise. */
  origPath: string | null;
  status: FileStatus;
  binary: boolean; // true -> hunks empty
  tooLarge: boolean; // true -> hunks empty
  hunks: Hunk[];
}

export interface FileDiffHeader {
  path: string;
  origPath: string | null;
  status: FileStatus;
  additions: number;
  deletions: number;
  binary: boolean;
}

export interface CommitDetails {
  oid: string;
  summary: string;
  /** Full message, trailing whitespace trimmed. Includes the summary line. */
  message: string;
  authorName: string;
  authorEmail: string;
  /** Seconds since epoch (UTC). */
  authorTs: number;
  committerTs: number;
  /** Full oids, first parent first. length > 1 => merge commit. */
  parents: string[];
}

export interface CommitDiff {
  details: CommitDetails;
  /** Sorted by path ascending. Headers only — hunks are fetched per file. */
  files: FileDiffHeader[];
}

export interface CompareEndpoint {
  /** Full 40-char hex; "" when HEAD is unborn (old side). */
  oid: string;
  /** First line of that commit's message; "" when unborn. */
  summary: string;
}

export interface CompareDiff {
  /** OLD side = HEAD. */
  from: CompareEndpoint;
  /** NEW side = the right-clicked commit. */
  to: CompareEndpoint;
  /** Sorted path-ascending. Empty when from.oid === to.oid. Headers only. */
  files: FileDiffHeader[];
}

/** P61b: one resolved side of an image comparison (base64 over IPC — D2).
 *  The frontend builds `data:${mime};base64,${base64}` for a plain `<img>`. */
export interface ImageSide {
  /** Raw blob bytes, standard base64 (NO `data:` prefix). */
  base64: string;
  /** MIME from the path extension, e.g. "image/png". */
  mime: string;
  /** Raw byte length pre-base64 (for the "N KB" label). */
  byteLen: number;
}

/** P61b: both sides of an image comparison. A `null` side is either absent
 *  (add/delete) or over the 8 MiB cap; the `*TooLarge` flags disambiguate. */
export interface ImageDiff {
  path: string;
  /** OLD side (index / HEAD / parent tree). null when added, missing, or over-cap. */
  old: ImageSide | null;
  /** NEW side (workdir / index / commit tree). null when deleted, missing, or over-cap. */
  new: ImageSide | null;
  oldTooLarge: boolean;
  newTooLarge: boolean;
}

/** P61b: which pair to load — mirrors the three file-diff contexts. Tagged on
 *  `kind`; matches the Rust `ImageDiffRequest` (camelCase keys + fields). */
export type ImageDiffRequest =
  | { kind: 'workdir'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'commit'; oid: string; path: string; origPath: string | null }
  | { kind: 'compare'; toOid: string; path: string; origPath: string | null };
