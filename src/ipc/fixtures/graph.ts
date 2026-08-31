import type { GraphEdge, GraphLayout, GraphNode, RefLabel, StashEntry } from '../types';

import { MULTI_REF_ROW_REFS } from './multiRefRow';

const HOUR = 3600;

/** P51: rows whose committer time is 1h AFTER the author time (as after a
 *  rebase/amend) — gives the author-vs-committer date toggle a visible
 *  difference in the harness. Inert until the P51b draw layer reads `dateBasis`. */
const COMMITTER_SKEW_ROWS = new Set([1, 3, 5]);

/** Deterministic 40-hex oid per row (rows < 256). */
function oid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}

function author(row: number): string {
  return row % 2 === 0 ? 'Ada Lovelace' : 'Grace Hopper';
}

/**
 * 30-row mock layout per contract M2-graph.md §3.5. Rows 0–7 exercise the
 * E-series geometry in one graph: octopus merge (row 0), fork/merge curves,
 * criss-cross-style convergences; rows 27–29 are two disconnected roots.
 * Covers all ref-pill variants except detached HEAD — that one is exercised
 * by the `?fixture=detached` variant below.
 */
export function buildMockGraph(): GraphLayout {
  const base = Math.floor(Date.now() / 1000) - HOUR; // newest commit ~1h ago
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  const push = (
    summary: string,
    lane: number,
    parents: number[],
    refs?: RefLabel[],
    // P7 §9: optional single-token author override (exercises initials "TO").
    authorName?: string,
  ): void => {
    const row = nodes.length;
    const ts = base - row * HOUR;
    const node: GraphNode = {
      id: oid(row),
      lane,
      parents,
      summary,
      author: authorName ?? author(row),
      ts,
      committerTs: COMMITTER_SKEW_ROWS.has(row) ? ts + HOUR : ts,
    };
    if (refs !== undefined) node.refs = refs; // absent when empty, like the wire
    nodes.push(node);
  };

  // Rows 0–7 (hand-computed lanes/edges — copied from the contract).
  // P7 §9: collapse + overflow on one row. main(local head)+origin/main(remote)
  // collapse to ONE 'main' label (laptop+cloud); dev(local)+origin/dev(remote)
  // collapse to 'dev'; origin/release is remote-only ('release', cloud only);
  // v1.0/v0.9 are tags. groupRefs yields 5 entities
  // [main(L+R,head), dev(L+R), release(R), #v1.0, #v0.9] → the 180px band shows
  // main (+ maybe dev) then a +N chip. Order mirrors the Rust backend:
  // locals head-first, then remotes, then tags.
  push('Merge feat and exp', 0, [3, 1, 2], [
    { name: 'main', kind: 'localBranch', isHead: true },
    { name: 'dev', kind: 'localBranch', isHead: false },
    { name: 'origin/main', kind: 'remoteBranch', isHead: false },
    { name: 'origin/dev', kind: 'remoteBranch', isHead: false },
    { name: 'origin/release', kind: 'remoteBranch', isHead: false },
    { name: 'v1.0', kind: 'tag', isHead: false },
    { name: 'v0.9', kind: 'tag', isHead: false },
  ]);
  push('feat: polish', 1, [4], [{ name: 'feat', kind: 'localBranch', isHead: false }]);
  push('experiment', 2, [5], [{ name: 'exp', kind: 'localBranch', isHead: false }]);
  // P7 §9: single-token author → initials "TO" (other rows yield "AL"/"GH").
  // P10 §3.1: base-row stash pills removed — stashes are now their OWN nodes
  // (see withStashNodes), so `core work 4` is the plain base for stash@{0}.
  push('core work 4', 0, [5], undefined, 'torvalds');
  // P7 §9: diverged pair — local `feat` is on row 1; its remote `origin/feat`
  // lives here on row 4, so the two render as SEPARATE labels (no collapse):
  // row 1 = laptop-only `feat`, row 4 = cloud-only `feat`.
  push('feat: start', 1, [6], [{ name: 'origin/feat', kind: 'remoteBranch', isHead: false }]);
  push('core work 3', 0, [6]);
  // P10 §3.1: base-row stash pills removed — `core work 2` is the plain base for
  // stash@{1} and stash@{2}, which now render as their own offshoot nodes
  // (withStashNodes) rather than pills on this row.
  push('core work 2', 0, [7]);
  // P92 §5: the 12-hidden case — 14 refs on one row, so the "+N" chip reads
  // "+12" and its picker menu exercises the §1.3 height clamp + scroll. Table
  // lives in fixtures/multiRefRow.ts (with the matching branches entries).
  push('core work 1', 0, [8], MULTI_REF_ROW_REFS);

  edges.push(
    { from: 0, to: 1, lane: 1 },
    { from: 0, to: 2, lane: 2 },
    { from: 0, to: 3, lane: 0 },
    { from: 1, to: 4, lane: 1 },
    { from: 2, to: 5, lane: 2 },
    { from: 3, to: 5, lane: 0 },
    { from: 4, to: 6, lane: 1 },
    { from: 5, to: 6, lane: 0 },
    { from: 6, to: 7, lane: 0 },
    { from: 7, to: 8, lane: 0 },
  );

  // Rows 8–26: linear chain L19..L1 on lane 0.
  for (let row = 8; row <= 26; row++) {
    push(`chore: history ${27 - row}`, 0, [row + 1]);
    edges.push({ from: row, to: row + 1, lane: 0 });
  }

  // Row 27: main root.
  push('initial commit', 0, []);

  // Rows 28–29: disconnected second component (no edge between 27 and 28).
  push('pages: update', 0, [29], [{ name: 'gh-pages', kind: 'localBranch', isHead: false }]);
  edges.push({ from: 28, to: 29, lane: 0 });
  push('pages: init', 0, []);

  return { nodes, edges, laneCount: 3, headIndex: 0, truncated: false };
}

export interface MockCommit {
  oid: string;
  summary: string;
  /**
   * P3c mock merges: index of the SECOND parent in the BASE fixture layout
   * (pre-prepend row index, e.g. 1 = the 'feat' tip). Absent for plain
   * commits. Adds a second parent index + a curved merge edge on that
   * node's lane.
   */
  mergeParentBase?: number;
}

/**
 * Prepends `commits` (newest first) as lane-0 rows to `layout` (P1 contract
 * §3.5 — synthetic rows for mock commits):
 * - every existing node's `parents` indices and every edge's from/to shift by
 *   `commits.length`;
 * - new rows: node i = { id, lane: 0, parents: [i+1], summary, author: 'You',
 *   ts: now - i*60 }, edges (i, i+1, 0) prepended keeping (from,to) sort order
 *   (shifted old edges all have from >= commits.length);
 * - moves the `⌂`/isHead LOCAL-branch pill from the old head row to row 0
 *   (other pills — origin/main, tags — stay on the old row); headIndex = 0.
 */
export function prependCommits(layout: GraphLayout, commits: MockCommit[]): GraphLayout {
  const k = commits.length;
  if (k === 0) return layout;
  const now = Math.floor(Date.now() / 1000);

  const shiftedNodes: GraphNode[] = layout.nodes.map((n) => {
    const copy: GraphNode = { ...n, parents: n.parents.map((p) => p + k) };
    if (n.refs !== undefined) copy.refs = n.refs.map((r) => ({ ...r }));
    return copy;
  });
  const newNodes: GraphNode[] = commits.map((c, i) => ({
    id: c.oid,
    lane: 0,
    parents:
      c.mergeParentBase !== undefined ? [i + 1, c.mergeParentBase + k] : [i + 1],
    summary: c.summary,
    author: 'You',
    ts: now - i * 60,
    committerTs: now - i * 60,
  }));
  // Per row: the lane-0 first-parent edge, then (merge commits only) a second
  // edge to the base-layout parent on that parent's lane. Keeps (from, to)
  // ascending: i + 1 <= k <= mergeParentBase + k.
  const newEdges: GraphEdge[] = commits.flatMap((c, i) => {
    const edges: GraphEdge[] = [{ from: i, to: i + 1, lane: 0 }];
    if (c.mergeParentBase !== undefined) {
      edges.push({
        from: i,
        to: c.mergeParentBase + k,
        lane: layout.nodes[c.mergeParentBase]?.lane ?? 1,
      });
    }
    return edges;
  });
  const shiftedEdges: GraphEdge[] = layout.edges.map((e) => ({
    ...e,
    from: e.from + k,
    to: e.to + k,
  }));

  if (layout.headIndex !== null) {
    const oldHead = shiftedNodes[layout.headIndex];
    const refs = oldHead.refs;
    if (refs !== undefined) {
      const idx = refs.findIndex((r) => r.kind === 'localBranch' && r.isHead);
      if (idx !== -1) {
        const [pill] = refs.splice(idx, 1);
        if (refs.length === 0) delete oldHead.refs;
        newNodes[0].refs = [pill];
      }
    }
  }

  return {
    nodes: [...newNodes, ...shiftedNodes],
    edges: [...newEdges, ...shiftedEdges],
    laneCount: layout.laneCount,
    headIndex: 0,
    truncated: layout.truncated,
  };
}

/**
 * P10 §3.2: insert each stash as its OWN node at the TOP of `layout`, on a
 * fresh offshoot lane, connected by a single edge to its base row.
 *
 * For each stash (index order, stash@{0} first) whose `baseOid` matches a node
 * id in `layout`:
 *   - it becomes a new top row (like {@link prependCommits}): all existing
 *     node.parents and edge.from/to shift by k = number of INSERTED stash nodes;
 *   - new stash node i (0..k): { id: stash.oid, lane: layout.laneCount + i,
 *     parents: [baseRow + k], refs: [{ name:`stash@{n}`, kind:'stash',
 *     isHead:false }], summary: stash.message, author:'', ts: stash.ts } where
 *     baseRow = index of the base node in the ORIGINAL layout;
 *   - new edge i: { from: i, to: baseRow + k, lane: layout.laneCount + i };
 *   - laneCount += k; headIndex (if non-null) shifts by k.
 * Stashes whose baseOid is not found are skipped (orphan → not rendered in the
 * mock; the real Rust path DOES render them, but the mock has no ancestor to
 * attach to). `baseRow + k` (= to) > `i` (= from) always holds, so `to > from`
 * is preserved. Edges are NOT required to be (from,to)-sorted for the mock.
 */
export function withStashNodes(layout: GraphLayout, stashes: StashEntry[]): GraphLayout {
  const baseIndex = (s: StashEntry): number =>
    layout.nodes.findIndex((n) => n.id === s.baseOid);
  const insertable = stashes.filter((s) => baseIndex(s) !== -1);
  const k = insertable.length;
  if (k === 0) return layout;

  const shiftedNodes: GraphNode[] = layout.nodes.map((n) => {
    const copy: GraphNode = { ...n, parents: n.parents.map((p) => p + k) };
    if (n.refs !== undefined) copy.refs = n.refs.map((r) => ({ ...r }));
    return copy;
  });
  const shiftedEdges: GraphEdge[] = layout.edges.map((e) => ({
    ...e,
    from: e.from + k,
    to: e.to + k,
  }));

  const newNodes: GraphNode[] = insertable.map((s, i) => ({
    id: s.oid,
    lane: layout.laneCount + i,
    parents: [baseIndex(s) + k],
    refs: [{ name: `stash@{${s.index}}`, kind: 'stash', isHead: false }],
    summary: s.message,
    author: '',
    ts: s.ts,
    committerTs: s.ts,
  }));
  const newEdges: GraphEdge[] = insertable.map((s, i) => ({
    from: i,
    to: baseIndex(s) + k,
    lane: layout.laneCount + i,
  }));

  return {
    nodes: [...newNodes, ...shiftedNodes],
    edges: [...newEdges, ...shiftedEdges],
    laneCount: layout.laneCount + k,
    headIndex: layout.headIndex === null ? null : layout.headIndex + k,
    truncated: layout.truncated,
  };
}

/**
 * Detached-HEAD variant of the §3.5 fixture (dev-only, `?fixture=detached`):
 * HEAD is detached onto row 5 ("core work 3"), which gets the solid red HEAD
 * pill (kind `head`); `main` on row 0 loses `isHead` (its pill goes outline).
 * Same geometry as `buildMockGraph` — exercises the one pill variant the
 * locked default fixture cannot.
 */
export function buildMockGraphDetached(): GraphLayout {
  const layout = buildMockGraph();
  layout.headIndex = 5;
  const mainRef = layout.nodes[0].refs?.find((r) => r.kind === 'localBranch');
  if (mainRef !== undefined) mainRef.isHead = false;
  layout.nodes[5].refs = [{ name: 'HEAD', kind: 'head', isHead: true }];
  return layout;
}
