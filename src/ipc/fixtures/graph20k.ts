import type { GraphEdge, GraphLayout, GraphNode, RefLabel } from '../types';

/**
 * 20 000-row procedural harness fixture (contract M2-graph.md §5.4, behind
 * `?fixture=20k`). This intentionally does NOT reproduce the Rust layout
 * algorithm — it emits a pattern whose lanes are known by construction and
 * geometrically valid per §1.3:
 *
 * - Lane 0: main spine — every row not otherwise assigned; each main row's
 *   parent is the next main row.
 * - Lane 1: per 50-row block b — row b*50+5 is a merge commit on lane 0 with
 *   parents [next main row, b*50+8]; rows b*50+8/+16/+24 are lane-1 feature
 *   nodes chained down, the last with a bottom-curve edge into main near
 *   row b*50+30.
 * - Lane 2: a long-lived branch — a node every 40 rows starting row 20,
 *   chained by lane-2 edges, forked from main near the bottom; `long-0` pill
 *   on its first node.
 *
 * Pills: `⌂ main` + `origin/main` on row 0; `# v{i}` every 200 rows;
 * `feat-{b}` on every 10th block's feature-head row.
 */

const ROWS = 20_000;
const BLOCK = 50;
const MERGE_OFF = 5;
const FEATURE_OFFS = [8, 16, 24] as const;
const FORK_OFF = 30;
const LANE2_START = 20;
const LANE2_STEP = 40;

function isLane2(row: number): boolean {
  return row >= LANE2_START && (row - LANE2_START) % LANE2_STEP === 0;
}

function isLane1(row: number): boolean {
  const off = row % BLOCK;
  return (FEATURE_OFFS as readonly number[]).includes(off);
}

/** Everything that is not a lane-1 feature or lane-2 node is on the main
 * spine (merge rows included). */
function isMain(row: number): boolean {
  return !isLane1(row) && !isLane2(row);
}

/** First main-spine row at or after `row` (clamped to the last row). */
function nextMainRow(row: number): number {
  let r = row;
  while (r < ROWS && !isMain(r)) r++;
  return Math.min(r, ROWS - 1);
}

/** Deterministic 40-hex oid per row. */
function oid(row: number): string {
  return row.toString(16).padStart(8, '0').repeat(5);
}

export function generateLayout20k(): GraphLayout {
  const base = Math.floor(Date.now() / 1000) - 60; // newest commit ~1min ago
  const nodes: GraphNode[] = new Array<GraphNode>(ROWS);
  const edges: GraphEdge[] = [];

  const lastLane2Row = LANE2_START + Math.floor((ROWS - 1 - LANE2_START) / LANE2_STEP) * LANE2_STEP;

  for (let row = 0; row < ROWS; row++) {
    let lane: number;
    let parents: number[];
    let refs: RefLabel[] | undefined;

    if (isLane2(row)) {
      lane = 2;
      if (row === lastLane2Row) {
        // Fork from main near the bottom: single bottom-curve edge to the
        // adjacent row (row+1 is never lane-1/lane-2 when row is lane-2).
        const p = row + 1;
        parents = [p];
        edges.push({ from: row, to: p, lane: 2 });
      } else {
        const p = row + LANE2_STEP;
        parents = [p];
        edges.push({ from: row, to: p, lane: 2 });
      }
      if (row === LANE2_START) {
        refs = [{ name: 'long-0', kind: 'localBranch', isHead: false }];
      }
    } else if (isLane1(row)) {
      lane = 1;
      const off = row % BLOCK;
      const blockStart = row - off;
      if (off === FEATURE_OFFS[FEATURE_OFFS.length - 1]) {
        // Feature tail: bottom-curve edge into the main spine near +30.
        const p = nextMainRow(blockStart + FORK_OFF);
        parents = [p];
        edges.push({ from: row, to: p, lane: 1 });
      } else {
        const p = row + 8; // next feature node in the chain
        parents = [p];
        edges.push({ from: row, to: p, lane: 1 });
      }
      const block = Math.floor(row / BLOCK);
      if (off === FEATURE_OFFS[0] && block % 10 === 0) {
        refs = [{ name: `feat-${block}`, kind: 'localBranch', isHead: false }];
      }
    } else {
      lane = 0;
      const mainNext = row + 1 < ROWS ? nextMainRow(row + 1) : ROWS;
      if (mainNext >= ROWS) {
        parents = []; // root
      } else if (row % BLOCK === MERGE_OFF) {
        // Merge commit: [next main, feature head of this block].
        const featureHead = row - MERGE_OFF + FEATURE_OFFS[0];
        parents = [mainNext, featureHead];
        edges.push({ from: row, to: mainNext, lane: 0 });
        edges.push({ from: row, to: featureHead, lane: 1 });
      } else {
        parents = [mainNext];
        edges.push({ from: row, to: mainNext, lane: 0 });
      }
      if (row === 0) {
        refs = [
          { name: 'main', kind: 'localBranch', isHead: true },
          { name: 'origin/main', kind: 'remoteBranch', isHead: false },
        ];
      } else if (row % 200 === 0) {
        const tag: RefLabel = { name: `v${row / 200}`, kind: 'tag', isHead: false };
        refs = refs === undefined ? [tag] : [...refs, tag];
      }
    }

    const ts = base - row * 60;
    const node: GraphNode = {
      id: oid(row),
      lane,
      parents,
      summary: `commit #${row}`,
      author: row % 2 === 0 ? 'Ada Lovelace' : 'Grace Hopper',
      ts,
      // P51: committer time == author time in the procedural fixture (the small
      // graph.ts fixture carries the rebase/amend skew rows for the toggle demo).
      committerTs: ts,
    };
    if (refs !== undefined) node.refs = refs;
    nodes[row] = node;
  }

  edges.sort((a, b) => a.from - b.from || a.to - b.to);

  return { nodes, edges, laneCount: 3, headIndex: 0, truncated: false };
}
