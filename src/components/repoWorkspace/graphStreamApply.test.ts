/** Audit 2026-08-18 §3.8 — `createGraphStreamApplier`: publishes layout, edge
 *  index and total together per batch, remaps the prior selection the instant
 *  its row arrives, and contains the assembler's non-contiguous-batch throw
 *  (error surfaced ONCE, stream poisoned, later chunks dropped) instead of
 *  letting it escape into Channel.onmessage where it would silently freeze the
 *  stream in real Tauri. */
import { describe, expect, it, vi } from 'vitest';

import { createGraphStream } from '../../graph/streamAssembler';
import { createGraphStreamApplier } from './graphStreamApply';
import type { GraphChunk, StreamNode } from '../../ipc';
import type { GraphStreamSinks } from './graphStreamApply';

const oid = (i: number): string => i.toString(16).padStart(2, '0').repeat(20);

function node(i: number): StreamNode {
  return {
    id: oid(i),
    lane: 0,
    summary: `commit ${i}`,
    author: 'Ada Lovelace',
    ts: 2000 - i,
    committerTs: 2000 - i,
  };
}

const meta: GraphChunk = { kind: 'meta', total: 4, headOid: oid(0) };

function batch(startRow: number, rows: number[]): GraphChunk {
  return { kind: 'batch', startRow, laneCountSoFar: 1, nodes: rows.map(node), edges: [] };
}

function makeSinks() {
  return {
    setGraph: vi.fn<GraphStreamSinks['setGraph']>(),
    setGraphEdgeIndex: vi.fn<GraphStreamSinks['setGraphEdgeIndex']>(),
    setGraphTotal: vi.fn<GraphStreamSinks['setGraphTotal']>(),
    setSelectedIndex: vi.fn<GraphStreamSinks['setSelectedIndex']>(),
  } satisfies GraphStreamSinks;
}

describe('createGraphStreamApplier', () => {
  it('meta updates only the total; each batch publishes layout+index+total together', () => {
    const sinks = makeSinks();
    const onError = vi.fn();
    const a = createGraphStreamApplier(createGraphStream(), null, sinks, onError);
    a.handle(meta);
    expect(sinks.setGraphTotal).toHaveBeenCalledExactlyOnceWith(4);
    expect(sinks.setGraph).not.toHaveBeenCalled(); // no-flicker: meta carries no rows
    a.handle(batch(0, [0, 1]));
    expect(sinks.setGraph).toHaveBeenCalledTimes(1);
    expect(sinks.setGraphEdgeIndex).toHaveBeenCalledTimes(1);
    expect(onError).not.toHaveBeenCalled();
    expect(a.poisoned).toBe(false);
  });

  it('remaps the prior selection the instant its row arrives, exactly once', () => {
    const sinks = makeSinks();
    const a = createGraphStreamApplier(createGraphStream(), oid(2), sinks, vi.fn());
    a.handle(meta);
    a.handle(batch(0, [0, 1]));
    expect(a.remapped).toBe(false); // row 2 not streamed yet
    expect(sinks.setSelectedIndex).not.toHaveBeenCalled();
    a.handle(batch(2, [2, 3]));
    expect(a.remapped).toBe(true);
    expect(sinks.setSelectedIndex).toHaveBeenCalledExactlyOnceWith(2);
  });

  it('a non-contiguous batch surfaces ONE error, poisons, and drops later chunks (§3.8)', () => {
    const sinks = makeSinks();
    const onError = vi.fn();
    const a = createGraphStreamApplier(createGraphStream(), oid(3), sinks, onError);
    a.handle(meta);
    a.handle(batch(0, [0, 1]));
    const publishes = sinks.setGraph.mock.calls.length;
    a.handle(batch(7, [2])); // gap -> assembler throws its invariant guard
    expect(a.poisoned).toBe(true);
    expect(onError).toHaveBeenCalledTimes(1);
    expect((onError.mock.calls[0]?.[0] as Error).message).toMatch(/startRow 7 != expected 2/);
    // Every later chunk is dropped: no publish, no remap, no second error.
    a.handle(batch(2, [2, 3]));
    expect(sinks.setGraph.mock.calls.length).toBe(publishes);
    expect(sinks.setSelectedIndex).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledTimes(1);
  });
});
