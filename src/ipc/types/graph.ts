export type RefKind = 'localBranch' | 'remoteBranch' | 'tag' | 'head' | 'stash';

export interface RefLabel {
  /** Shorthand: "main", "origin/main", "v1.0", "HEAD". */
  name: string;
  kind: RefKind;
  /** true on the local branch HEAD points at (attached), or on the head label (detached). */
  isHead: boolean;
}

export interface GraphNode {
  /** Full 40-char hex oid. */
  id: string;
  lane: number;
  /** Indices into GraphLayout.nodes; parents always at a HIGHER index. First entry = first parent. */
  parents: number[];
  /** Absent when empty (serde skip_serializing_if). */
  refs?: RefLabel[];
  summary: string;
  author: string;
  /** Author commit time, seconds since epoch (UTC). */
  ts: number;
  /** Committer commit time, seconds since epoch (UTC). P51: powers the
   *  author-vs-committer date basis toggle. Often == `ts`. */
  committerTs: number;
}

export interface GraphEdge {
  /** Child row/index. */
  from: number;
  /** Parent row/index; to > from. */
  to: number;
  /** Lane of the vertical run between the rows. */
  lane: number;
}

export interface GraphLayout {
  /** Row number == index in this array (no row field on the wire). */
  nodes: GraphNode[];
  /** Sorted ascending by (from, to). */
  edges: GraphEdge[];
  laneCount: number;
  headIndex: number | null;
  truncated: boolean;
}

/** P65 (streamed graph): one commit row as delivered by `streamGraph`. Identical
 *  to {@link GraphNode} MINUS `parents` — a child's parent rows are not known
 *  when it is emitted (parents are always at HIGHER, not-yet-walked rows), so the
 *  frontend reconstructs `parents` from the edge ordinals ({@link StreamEdge.ord}).
 *  Mirrors the Rust `StreamNode` (camelCase; `refs` omitted when empty). */
export interface StreamNode {
  id: string;
  lane: number;
  refs?: RefLabel[];
  summary: string;
  author: string;
  ts: number;
  committerTs: number;
}

/** P65 (streamed graph): a {@link GraphEdge} PLUS the child's parent ordinal
 *  (`ord`) so the frontend can rebuild each node's ordered `parents`. `ord === 0`
 *  is the first parent (the lane-inheriting edge). Mirrors the Rust
 *  `GraphStreamEdge`. `from` (child) < `to` (parent). */
export interface StreamEdge {
  from: number;
  to: number;
  lane: number;
  ord: number;
}

/** P65 (streamed graph): one `streamGraph` channel message. Wire order: exactly
 *  one `meta`, then N `batch`, then exactly one `done`. On any error the command
 *  REJECTS (AppError) instead of sending `done`. Mirrors the Rust `GraphChunk`
 *  serde enum (tagged `kind`, camelCase) byte-for-byte.
 *  - `meta`: first message. `total` = exact reachable-commit count if cheaply
 *    known, else null (frontend grows the scroll extent as rows arrive).
 *    `headOid` lets the frontend resolve `headIndex` the moment HEAD's row lands.
 *  - `batch`: a run of consecutive rows `[startRow, startRow + nodes.length)`
 *    plus the edges FINALIZED within them (parent `to` in this batch).
 *    `laneCountSoFar` is the running max (monotonic) — drives the graph-area
 *    width without ever shrinking.
 *  - `done`: terminal authoritative scalars. `totalRows` == nodes emitted;
 *    `headIndex` resolved; `truncated` set at the streaming cap. */
export type GraphChunk =
  | { kind: 'meta'; total: number | null; headOid: string | null }
  | { kind: 'batch'; startRow: number; laneCountSoFar: number; nodes: StreamNode[]; edges: StreamEdge[] }
  | { kind: 'done'; totalRows: number; laneCount: number; headIndex: number | null; truncated: boolean };

/** Graph geometry knobs (P11 §2.3) — pure render geometry, not layout math. */
/** Which timestamp the graph's date column + relative/absolute date use (P51).
 *  Mirrors the Rust `GraphDateBasis` enum (lowercase wire values). */
export type GraphDateBasis = 'author' | 'committer';

export interface GraphPrefs {
  avatarRadius: number;
  rowHeight: number;
  laneWidth: number;
  /** P51: short-SHA column (+ verified-badge slot). Default true. */
  showSha: boolean;
  /** P51: optional full author-name text column. Default false. */
  showAuthor: boolean;
  /** P51: date column. Default true. */
  showDate: boolean;
  /** P51: which timestamp the date column/tooltip use. Default 'author'. */
  dateBasis: GraphDateBasis;
  /** P51: ahead/behind chip on branch-tip pills. Default true. */
  showAheadBehind: boolean;
  /** P51: compact (denser) rows. Default false. */
  compact: boolean;
  /** P58c: light the per-row signature badge from `verifyCommits`. Default true.
   *  When false the P51 faint stub renders unchanged and NO verification is
   *  requested (individually toggleable, like the other detail columns). */
  showSignatureBadge: boolean;
  /** P63: PR-state badge on branch-tip pills. Default false (network+auth-gated
   *  — inert without a connected forge, so opt-in). */
  showPrBadge: boolean;
  /** P63: CI/build-status dot on branch-tip pills. Default false (same
   *  network+auth gating as showPrBadge). */
  showCiStatus: boolean;
}
