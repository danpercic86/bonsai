import { describe, expect, it } from 'vitest';
import type { GraphLayout, GraphNode } from '../../ipc';
import { buildFoldModel, modelToDisplay } from '../foldModel';
import {
  RAIL_BUCKETS,
  RAIL_FLAG_BRANCH,
  RAIL_FLAG_HEAD,
  RAIL_FLAG_TAG,
  THUMB_MIN_PX,
  TICK_HIT_SLOP_PX,
  bucketOf,
  buildRailBuckets,
  coalesceTicks,
  hitTick,
  railYForDisplayRow,
  scrollTopForRailY,
  thumbRect,
} from './railMath';

function node(i: number, lane: number, refs?: GraphNode['refs']): GraphNode {
  return {
    id: `oid${i}`,
    lane,
    parents: [],
    summary: `c${i}`,
    author: 'a',
    ts: 0,
    committerTs: 0,
    ...(refs !== undefined ? { refs } : {}),
  };
}

function layoutOf(nodes: GraphNode[], headIndex: number | null = null): GraphLayout {
  return { nodes, edges: [], laneCount: 4, headIndex, truncated: false };
}

describe('buildRailBuckets', () => {
  it('accumulates density, laneMax and ref/HEAD flags', () => {
    const nodes = [
      node(0, 0, [{ name: 'main', kind: 'localBranch', isHead: true }]),
      node(1, 2, [{ name: 'v1', kind: 'tag', isHead: false }]),
      node(2, 1),
    ];
    const b = buildRailBuckets(layoutOf(nodes, 0));
    expect(b.rows).toBe(3);
    const b0 = bucketOf(0, 3);
    const b1 = bucketOf(1, 3);
    expect(b.flags[b0] & RAIL_FLAG_BRANCH).toBeTruthy();
    expect(b.flags[b0] & RAIL_FLAG_HEAD).toBeTruthy();
    expect(b.flags[b1] & RAIL_FLAG_TAG).toBeTruthy();
    expect(b.laneMax[b1]).toBe(3); // lane 2 -> depth 3
    const total = b.density.reduce((s, v) => s + v, 0);
    expect(total).toBe(3);
    expect(b.maxDensity).toBeGreaterThan(0);
  });

  it('short history (< RAIL_BUCKETS rows) spreads without NaN or overlap loss', () => {
    const nodes = Array.from({ length: 40 }, (_, i) => node(i, 0));
    const b = buildRailBuckets(layoutOf(nodes));
    const total = b.density.reduce((s, v) => s + v, 0);
    expect(total).toBe(40);
    // Every commit lands in a distinct bucket when rows << buckets.
    const occupied = b.density.reduce((s, v) => s + (v > 0 ? 1 : 0), 0);
    expect(occupied).toBe(40);
    expect(b.maxDensity).toBe(1);
  });

  it('large history saturates buckets deterministically', () => {
    const nodes = Array.from({ length: RAIL_BUCKETS * 3 }, (_, i) => node(i, i % 4));
    const a = buildRailBuckets(layoutOf(nodes));
    const c = buildRailBuckets(layoutOf(nodes));
    expect(Array.from(a.density)).toEqual(Array.from(c.density));
    expect(a.density[0]).toBe(3);
    expect(a.maxDensity).toBe(3);
  });

  it('empty layout yields zeroed buckets', () => {
    const b = buildRailBuckets(layoutOf([]));
    expect(b.rows).toBe(0);
    expect(b.maxDensity).toBe(0);
  });
});

describe('fold-clamped display mapping feeding the rail', () => {
  it('rows inside a collapsed span land on the pill row bucket/tick', () => {
    // 30 model rows; rows 5..14 collapsed (start 5, count 10).
    const m = buildFoldModel(30, [{ start: 5, count: 10, lane: 0 }], new Set());
    const rows = m.displayRowCount; // 30 - 9 = 21
    const pillDisplay = modelToDisplay(m, 5);
    for (const hidden of [5, 9, 14]) {
      expect(modelToDisplay(m, hidden)).toBe(pillDisplay);
    }
    const yPill = railYForDisplayRow(pillDisplay, rows, 200);
    const yHidden = railYForDisplayRow(modelToDisplay(m, 9), rows, 200);
    expect(yHidden).toBe(yPill);
    // Round-trip: a tick built from the clamped display row is at the pill's y.
    const ticks = coalesceTicks([modelToDisplay(m, 9)], null, rows, 200);
    expect(ticks).toHaveLength(1);
    expect(ticks[0].y).toBe(yPill);
  });
});

describe('coalesceTicks', () => {
  it('coalesces 10k matches to <= railHeight ticks, first index per pixel', () => {
    const rows = 20000;
    const matches = Array.from({ length: 10000 }, (_, i) => i * 2);
    const ticks = coalesceTicks(matches, null, rows, 300);
    expect(ticks.length).toBeLessThanOrEqual(300);
    // Deterministic: same input, same output.
    expect(coalesceTicks(matches, null, rows, 300)).toEqual(ticks);
    // Representative = FIRST match in that pixel.
    expect(ticks[0].matchIndex).toBe(0);
    for (let i = 1; i < ticks.length; i += 1) {
      expect(ticks[i].matchIndex).toBeGreaterThan(ticks[i - 1].matchIndex);
    }
  });

  it('marks the current pixel and inserts one when no plain match shares it', () => {
    const ticks = coalesceTicks([0, 100], 50, 200, 200);
    const current = ticks.filter((t) => t.current);
    expect(current).toHaveLength(1);
    expect(current[0].matchIndex).toBe(-1); // synthesized current-only tick
    const marked = coalesceTicks([0, 50, 100], 50, 200, 200);
    const cur = marked.filter((t) => t.current);
    expect(cur).toHaveLength(1);
    expect(cur[0].matchIndex).toBe(1); // the real match at row 50
  });

  it('returns [] on empty rail or empty history', () => {
    expect(coalesceTicks([1], null, 0, 100)).toEqual([]);
    expect(coalesceTicks([1], null, 100, 0)).toEqual([]);
  });
});

describe('thumbRect / scrollTopForRailY', () => {
  it('clamps to the min height and round-trips with the inverse mapping', () => {
    const railH = 400;
    const scrollH = 100000;
    const clientH = 800;
    const t = thumbRect(0, scrollH, clientH, railH);
    expect(t.height).toBe(THUMB_MIN_PX); // 800/100000*400 = 3.2 -> clamp
    for (const scrollTop of [0, 12345, 99200]) {
      const r = thumbRect(scrollTop, scrollH, clientH, railH);
      const back = scrollTopForRailY(r.top + 5, 5, scrollH, clientH, railH);
      expect(back).toBeCloseTo(Math.min(scrollTop, scrollH - clientH), 3);
    }
  });

  it('short history: thumb spans the whole rail; mapping is inert', () => {
    const t = thumbRect(0, 500, 600, 300);
    expect(t).toEqual({ top: 0, height: 300 });
    expect(scrollTopForRailY(150, 0, 500, 600, 300)).toBe(0);
  });

  it('clamps drag at both ends', () => {
    const railH = 300;
    expect(scrollTopForRailY(-50, 0, 10000, 500, railH)).toBe(0);
    expect(scrollTopForRailY(9999, 0, 10000, 500, railH)).toBe(9500);
  });
});

describe('perf (plan §Testing — 20k budget)', () => {
  it('buildRailBuckets on a 20k-row layout completes within a few ms', () => {
    const nodes = Array.from({ length: 20000 }, (_, i) =>
      node(
        i,
        i % 6,
        i % 500 === 0 ? [{ name: `b${i}`, kind: 'localBranch' as const, isHead: false }] : undefined,
      ),
    );
    const layout = layoutOf(nodes, 0);
    buildRailBuckets(layout); // warm-up (JIT)
    const t0 = performance.now();
    const b = buildRailBuckets(layout);
    const ms = performance.now() - t0;
    expect(b.rows).toBe(20000);
    // Plan budget "a few ms at 100k"; 50 ms is a generous CI-safe ceiling.
    expect(ms).toBeLessThan(50);
  });
});

describe('hitTick', () => {
  it('honors the ±slop boundary and prefers the nearest tick', () => {
    const ticks = coalesceTicks([0, 100], null, 101, 101); // y=0 and y=100
    expect(hitTick(ticks, TICK_HIT_SLOP_PX)?.y).toBe(0);
    expect(hitTick(ticks, TICK_HIT_SLOP_PX + 1)).toBeNull();
    expect(hitTick(ticks, 100 - TICK_HIT_SLOP_PX)?.y).toBe(100);
    const pair = coalesceTicks([50, 53], null, 101, 101);
    expect(hitTick(pair, 51)?.y).toBe(50);
    expect(hitTick(pair, 52)?.y).toBe(53);
  });
});
