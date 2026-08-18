/** P65b/audit §3.8 — the per-chunk application step of `refetchGraph`, extracted
 *  from RepoWorkspace so it stays pure enough to unit-test: folds each chunk
 *  into the assembler, publishes layout/edge-index/total together (so the three
 *  never disagree in one render), remaps the prior selection the instant its
 *  row arrives, and contains assembler throws via `guardChunks` (see
 *  chunkGuard.ts for why an escaped throw silently freezes the stream). */

import { guardChunks } from '../../graph/chunkGuard';
import type { GraphStream } from '../../graph/streamAssembler';
import type { IncrementalEdgeIndex } from '../../graph/incrementalEdgeIndex';
import type { GraphChunk, GraphLayout } from '../../ipc';

/** P65b: bump the layout object identity per applied stream batch (the growing
 *  arrays inside are shared) so GraphCanvas's `[..., layout]` repaint effect
 *  fires once per batch. A shallow spread is enough — a fresh outer object each
 *  call is the repaint trigger. */
function wrapStreamLayout(layout: GraphLayout): GraphLayout {
  return { ...layout };
}

/** The state setters the applier publishes into (RepoWorkspace's useState). */
export interface GraphStreamSinks {
  setGraph(layout: GraphLayout): void;
  setGraphEdgeIndex(index: IncrementalEdgeIndex): void;
  setGraphTotal(total: number | null): void;
  setSelectedIndex(index: number | null): void;
}

export interface GraphStreamApplier {
  /** True once a chunk threw (audit §3.8); all later chunks are dropped. */
  readonly poisoned: boolean;
  /** True once the prior selection was reselected mid-stream. */
  readonly remapped: boolean;
  handle(chunk: GraphChunk): void;
}

export function createGraphStreamApplier(
  stream: GraphStream,
  prevSelectedId: string | null,
  sinks: GraphStreamSinks,
  onError: (e: unknown) => void,
): GraphStreamApplier {
  let remapped = false;
  const guarded = guardChunks<GraphChunk>((chunk) => {
    stream.apply(chunk);
    // No-flicker: keep showing the PREVIOUS layout until the first paintable
    // chunk of the NEW stream lands. `meta` only stashes total/headOid — it
    // carries no rows, so update just the scroll extent and wait.
    if (chunk.kind === 'meta') {
      sinks.setGraphTotal(stream.total);
      return;
    }
    // Identity bump -> GraphCanvas repaints; edge index + total set together
    // with the layout so the three never disagree in one render.
    sinks.setGraph(wrapStreamLayout(stream.layout));
    sinks.setGraphEdgeIndex(stream.edgeIndex);
    sinks.setGraphTotal(stream.total);
    // Progressive selection remap: reselect the prior commit the instant its
    // row arrives (don't wait for the full stream).
    if (prevSelectedId !== null && !remapped && stream.oidToRow.has(prevSelectedId)) {
      sinks.setSelectedIndex(stream.oidToRow.get(prevSelectedId) ?? null);
      remapped = true;
    }
  }, onError);
  return {
    get poisoned() {
      return guarded.poisoned;
    },
    get remapped() {
      return remapped;
    },
    handle: (chunk) => guarded.handle(chunk),
  };
}
