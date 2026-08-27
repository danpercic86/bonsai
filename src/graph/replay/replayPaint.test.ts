// Spec-007 plan §Testing "vitest replayPaint": the paint SEAM, not pixels.
// `drawGraph` is mocked and its arguments captured — the seam's whole contract
// is (1) the viewport min-row clamp `firstRow = max(scrollFirstRow, cutoff)`,
// (2) the edge pre-filter `from >= cutoff`, (3) the blank-canvas path when the
// cutoff sits below the scrolled window (drawGraph still called → pass-1
// clear). Pulses paint AFTER drawGraph directly on the ctx, so with drawGraph
// mocked every ctx call observed here is pulse-only (stub-ctx capture, the
// textMeasure.test precedent).
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { GraphLayout, GraphNode } from '../../ipc';
import type { Theme } from '../colors';
import type { GraphDisplayOptions } from '../rightColumns';
import { effectiveMetrics } from '../metrics';
import { buildEdgeIndex } from '../edgeIndex';
import { drawGraph } from '../draw';
import { paintReplay } from './replayPaint';
import type { ReplayPaintArgs } from './replayPaint';
import { MAX_PULSE_RINGS, PULSE_MS } from './replayPulse';

vi.mock('../draw', () => ({ drawGraph: vi.fn() }));
const drawGraphMock = vi.mocked(drawGraph);

const METRICS = effectiveMetrics({
  avatarRadius: 10,
  rowHeight: 32,
  laneWidth: 16,
  compact: false,
});

/** n rows (row 0 newest), a linear parent chain edge per row pair. */
function layoutOf(n: number): GraphLayout {
  const nodes: GraphNode[] = [];
  for (let row = 0; row < n; row += 1) {
    nodes.push({
      id: String(row).padStart(40, '0'),
      lane: row % 3,
      parents: row + 1 < n ? [row + 1] : [],
      summary: `c${row}`,
      author: 'a',
      ts: 1_700_000_000 - row,
      committerTs: 1_700_000_000 - row,
    });
  }
  const edges = nodes
    .filter((_, row) => row + 1 < n)
    .map((_, row) => ({ from: row, to: row + 1, lane: row % 3 }));
  return { nodes, edges, laneCount: 3, headIndex: 0, truncated: false };
}

/** Recording 2D-context stub — drawGraph is mocked, so every call is a pulse. */
function stubCtx() {
  const calls: { fillRects: Array<{ y: number }>; arcs: number; strokes: number } = {
    fillRects: [],
    arcs: 0,
    strokes: 0,
  };
  const ctx = {
    globalAlpha: 1,
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 0,
    fillRect: (_x: number, y: number) => calls.fillRects.push({ y }),
    beginPath: () => {},
    arc: () => {
      calls.arcs += 1;
    },
    stroke: () => {
      calls.strokes += 1;
    },
  } as unknown as CanvasRenderingContext2D;
  return { ctx, calls };
}

const THEME = {
  bg0: '#101418', // dark → dark pulse alpha branch
  laneColors: ['#a00', '#0a0', '#00a', '#aa0', '#a0a', '#0aa', '#555', '#666', '#777', '#888'],
} as unknown as Theme;

function args(over: Partial<ReplayPaintArgs>): ReplayPaintArgs {
  const layout = over.layout ?? layoutOf(100);
  return {
    ctx: stubCtx().ctx,
    layout,
    edgeIndex: buildEdgeIndex(layout),
    theme: THEME,
    metrics: METRICS,
    display: { colorMode: 'lane' } as GraphDisplayOptions, // opaque passthrough (drawGraph mocked)
    cutoff: 0,
    scrollTop: 0,
    width: 800,
    height: 320, // 10 rows visible at rowHeight 32
    rightInset: 0,
    pulses: [],
    now: 10_000,
    ...over,
  };
}

beforeEach(() => {
  drawGraphMock.mockClear();
});

type Vp = { firstRow: number; lastRow: number; scrollTop: number };
function lastCall(): { vp: Vp; edges: Array<{ from: number; to: number }> } {
  expect(drawGraphMock).toHaveBeenCalledTimes(1);
  const c = drawGraphMock.mock.calls[0];
  return { vp: c[3] as Vp, edges: c[2] as unknown as Array<{ from: number; to: number }> };
}

describe('paintReplay viewport clamp (plan: min-row clamp)', () => {
  it('cutoff above the scroll window: firstRow = cutoff, lastRow from the scroll window', () => {
    // scrollTop 0 → visible rows 0..(10+overscan-1); cutoff 5 clamps the top.
    paintReplay(args({ cutoff: 5 }));
    const { vp } = lastCall();
    expect(vp.firstRow).toBe(5);
    expect(vp.lastRow).toBe(14); // ceil((0+320)/32) + overscan 4
  });

  it('cutoff below the scrolled window leaves the scroll clamp in charge', () => {
    // scrollTop = row 50 → firstRow = 50 - overscan = 46 > cutoff 5.
    paintReplay(args({ cutoff: 5, scrollTop: 50 * 32 }));
    const { vp } = lastCall();
    expect(vp.firstRow).toBe(46);
  });

  it('blank canvas (cutoff = n while scrolled at top): drawGraph STILL called with empty edges', () => {
    paintReplay(args({ cutoff: 100 }));
    const { vp, edges } = lastCall();
    expect(vp.firstRow).toBe(100);
    expect(vp.firstRow).toBeGreaterThan(vp.lastRow); // drawGraph only clears
    expect(edges).toEqual([]);
  });
});

describe('paintReplay edge filter (plan: from >= cutoff)', () => {
  it('drops edges whose child row is above the cutoff, keeps frontier→parent edges', () => {
    paintReplay(args({ cutoff: 5 }));
    const { edges } = lastCall();
    expect(edges.length).toBeGreaterThan(0);
    for (const e of edges) {
      expect(e.from).toBeGreaterThanOrEqual(5);
      expect(e.to).toBeGreaterThan(e.from); // parent end always revealed
    }
    // The frontier row's own edge to its parent IS present (from === cutoff).
    expect(edges.some((e) => e.from === 5)).toBe(true);
  });

  it('cutoff 0 (everything revealed) filters nothing within the window', () => {
    paintReplay(args({ cutoff: 0 }));
    const { edges } = lastCall();
    // Window rows 0..13 → in-range edges all pass the from >= 0 filter.
    expect(edges.some((e) => e.from === 0)).toBe(true);
  });
});

describe('paintReplay frontier pulses (UI §4: tint + capped rings, after drawGraph)', () => {
  it('paints tint + ring for a live in-window pulse at/below the cutoff only', () => {
    const { ctx, calls } = stubCtx();
    paintReplay(
      args({
        ctx,
        cutoff: 5,
        now: 10_000,
        pulses: [
          { row: 5, start: 10_000 - PULSE_MS / 2 }, // live, visible
          { row: 3, start: 10_000 - PULSE_MS / 2 }, // above cutoff → skipped
          { row: 60, start: 10_000 - PULSE_MS / 2 }, // outside window → skipped
          { row: 6, start: 10_000 - 2 * PULSE_MS }, // expired (alpha 0) → skipped
        ],
      }),
    );
    expect(calls.fillRects).toHaveLength(1); // only row 5 tints
    expect(calls.fillRects[0].y).toBe(5 * 32);
    expect(calls.arcs).toBe(1);
    expect(calls.strokes).toBe(1);
  });

  it('caps rings at MAX_PULSE_RINGS but tints every live pulse; globalAlpha restored', () => {
    const { ctx, calls } = stubCtx();
    ctx.globalAlpha = 0.77;
    const many = Array.from({ length: MAX_PULSE_RINGS + 10 }, (_, i) => ({
      row: i,
      start: 10_000 - PULSE_MS / 2,
    }));
    // height covers all pulse rows so none is window-clipped.
    paintReplay(args({ ctx, cutoff: 0, height: (MAX_PULSE_RINGS + 20) * 32, pulses: many, now: 10_000 }));
    expect(calls.fillRects).toHaveLength(MAX_PULSE_RINGS + 10);
    expect(calls.arcs).toBe(MAX_PULSE_RINGS);
    expect(ctx.globalAlpha).toBe(0.77);
  });

  it('no pulses → zero ctx work beyond the mocked drawGraph', () => {
    const { ctx, calls } = stubCtx();
    paintReplay(args({ ctx, cutoff: 0, pulses: [] }));
    expect(calls.fillRects).toHaveLength(0);
    expect(calls.arcs).toBe(0);
  });
});
