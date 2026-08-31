// P65b/c: mock `streamGraph` — chunks the SAME layout `getGraph` serves (via
// `resolveLayout`) into meta -> batch* -> done, mirroring the Rust
// `stream_graph` wire protocol byte-for-byte (StreamNode drops `parents`; each
// StreamEdge carries its child's parent `ord`). `getGraph` stays (searchCommits
// still reuses `resolveLayout`). A per-repo generation models backend supersede:
// a newer streamGraph for the same repo makes the older loop STOP before `done`.
import type { GraphChunk, IpcApi, StreamEdge, StreamNode } from '../../types';
import { resolveLayout } from './layout';
import { delay, requireRepo } from '../repoState';

/** Mirror of the Rust batch consts (crates/bonsai-core `graph.rs`): a small
 *  first flush for instant paint, then large steady-state batches. */
const STREAM_FIRST_BATCH = 512;
const STREAM_BATCH = 4096;
/** Progressive-paint pause between batches so the harness shows the graph fill
 *  in and the assembler's multi-batch path is exercised. */
const BATCH_DELAY_MS = 30;

/** Per-repo stream generation. A newer `streamGraph` (or repo switch) bumps this
 *  so any older loop STOPS before emitting `done` (models cancellation, §6). */
const streamGen = new Map<string, number>();

export const graphStreamHandlers = {
  async streamGraph(repoId: string, onChunk: (c: GraphChunk) => void): Promise<void> {
    const layout = resolveLayout(requireRepo(repoId));
    const myGen = (streamGen.get(repoId) ?? 0) + 1;
    streamGen.set(repoId, myGen);

    const total = layout.nodes.length;
    onChunk({
      kind: 'meta',
      total,
      headOid: layout.headIndex !== null ? layout.nodes[layout.headIndex].id : null,
    });

    let start = 0;
    let limit = STREAM_FIRST_BATCH;
    let maxLane = 0;
    while (start < total) {
      const end = Math.min(total, start + limit);
      const nodes: StreamNode[] = [];
      for (let r = start; r < end; r++) {
        const n = layout.nodes[r];
        if (n.lane > maxLane) maxLane = n.lane;
        const sn: StreamNode = {
          id: n.id,
          lane: n.lane,
          summary: n.summary,
          author: n.author,
          ts: n.ts,
          committerTs: n.committerTs,
        };
        if (n.refs !== undefined && n.refs.length > 0) sn.refs = n.refs;
        nodes.push(sn);
      }
      // Edges FINALIZED in this batch = those whose parent `to` lands in [start,
      // end). The child `from` (< to) was delivered earlier or in this batch.
      const edges: StreamEdge[] = [];
      for (const e of layout.edges) {
        if (e.to >= start && e.to < end) {
          if (e.lane > maxLane) maxLane = e.lane;
          const ord = layout.nodes[e.from].parents.indexOf(e.to);
          edges.push({ from: e.from, to: e.to, lane: e.lane, ord: ord < 0 ? 0 : ord });
        }
      }
      // Monotonic running-max width, capped at the layout's final lane count.
      const laneCountSoFar = Math.min(layout.laneCount, maxLane + 1);
      onChunk({ kind: 'batch', startRow: start, laneCountSoFar, nodes, edges });

      start = end;
      limit = STREAM_BATCH;
      if (start < total) {
        await delay(BATCH_DELAY_MS);
        // Superseded by a newer stream for this repo (or a repo switch) → stop
        // before `done`, exactly like the backend walk halting when its channel
        // send fails.
        if (streamGen.get(repoId) !== myGen) return;
      }
    }

    onChunk({
      kind: 'done',
      totalRows: total,
      laneCount: layout.laneCount,
      headIndex: layout.headIndex,
      truncated: layout.truncated,
    });
  },
} satisfies Partial<IpcApi>;
