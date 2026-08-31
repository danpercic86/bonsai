import { describe, expect, it } from 'vitest';

import { chipTextFor, formatAheadBehind, layoutRefLabels } from './refLabels';
import type { LaidRefLabel, RefEntity } from './refLabels';
import type { GraphNode } from '../ipc';
import type { Theme } from './colors';
import { METRICS } from './metrics';
import type { GraphDisplayOptions } from './rightColumns';

// Pure P51c ahead/behind chip helpers — no canvas. `layoutRefLabels` itself
// needs a 2D context (measurement) and is exercised by the in-browser
// p7SelfTest; here we cover the formatting + gate logic that decides IF/WHAT a
// chip shows, which is where the branch cases live.

describe('formatAheadBehind', () => {
  it('returns null when not diverged (both counts 0 or negative)', () => {
    expect(formatAheadBehind(0, 0)).toBeNull();
    expect(formatAheadBehind(-1, -2)).toBeNull();
  });
  it('shows only the ahead arrow when behind is 0', () => {
    expect(formatAheadBehind(3, 0)).toBe('↑3');
  });
  it('shows only the behind arrow when ahead is 0', () => {
    expect(formatAheadBehind(0, 2)).toBe('↓2');
  });
  it('shows both arrows when diverged in both directions', () => {
    expect(formatAheadBehind(3, 2)).toBe('↑3 ↓2');
  });
});

function disp(over: Partial<GraphDisplayOptions> = {}): GraphDisplayOptions {
  return {
    showSha: true,
    showAuthor: false,
    showDate: true,
    dateBasis: 'author',
    showAheadBehind: true,
    branchStats: new Map(),
    showSignatureBadge: false,
    showPrBadge: false,
    showCiStatus: false,
    prByBranch: new Map(),
    ciBySha: new Map(),
    ...over,
  };
}

function localBranch(name: string, hasLocal = true): RefEntity {
  return { kind: 'branch', name, hasLocal, remotes: [], isHead: false, refs: [] };
}

describe('chipTextFor', () => {
  it('is null when the toggle is off, even for a diverged branch', () => {
    const branchStats = new Map([['main', { ahead: 3, behind: 2 }]]);
    expect(chipTextFor(localBranch('main'), disp({ showAheadBehind: false, branchStats }))).toBeNull();
  });

  it('is null for a non-branch entity (tag/head/stash)', () => {
    const tag: RefEntity = { kind: 'tag', name: 'v1.0', ref: { name: 'v1.0', kind: 'tag', isHead: false } };
    expect(chipTextFor(tag, disp())).toBeNull();
  });

  it('is null for a remote-only branch (no local ref)', () => {
    const branchStats = new Map([['feat', { ahead: 1, behind: 1 }]]);
    expect(chipTextFor(localBranch('feat', false), disp({ branchStats }))).toBeNull();
  });

  it('is null for a local branch missing from branchStats (no upstream)', () => {
    expect(chipTextFor(localBranch('main'), disp())).toBeNull();
  });

  it('is null when the branch is tracked but not diverged (0/0)', () => {
    const branchStats = new Map([['main', { ahead: 0, behind: 0 }]]);
    expect(chipTextFor(localBranch('main'), disp({ branchStats }))).toBeNull();
  });

  it('is null when counts are null (defensive; map should pre-filter these)', () => {
    const branchStats = new Map([['main', { ahead: null, behind: null }]]);
    expect(chipTextFor(localBranch('main'), disp({ branchStats }))).toBeNull();
  });

  it('renders the compact chip for a diverged local branch', () => {
    const branchStats = new Map([['main', { ahead: 3, behind: 2 }]]);
    expect(chipTextFor(localBranch('main'), disp({ branchStats }))).toBe('↑3 ↓2');
  });

  it('renders one arrow when diverged in a single direction', () => {
    expect(
      chipTextFor(localBranch('a'), disp({ branchStats: new Map([['a', { ahead: 5, behind: 0 }]]) })),
    ).toBe('↑5');
    expect(
      chipTextFor(localBranch('b'), disp({ branchStats: new Map([['b', { ahead: 0, behind: 4 }]]) })),
    ).toBe('↓4');
  });
});

// ---- layoutRefLabels: chip space-reservation + "+n" overflow (P51c) ----------
// The subtle P51c layout logic (reserving the ahead/behind chip's width during
// forward layout, and the pop-rewind that subtracts that same chipAdvance when
// the "+n" chip needs room) has no unit coverage otherwise — it is only hit by
// the in-browser self-test. `layoutRefLabels` is PURE layout: it measures text
// but never draws. A deterministic fake 2D context (fixed CHAR_W per char,
// font-independent) makes every pill/chip width exact and keeps the module-level
// measure cache self-consistent — so this runs headless in the Node vitest env
// with NO canvas launch.
const CHAR_W = 10;
function makeCtx(): CanvasRenderingContext2D {
  return {
    font: '',
    measureText: (t: string) => ({ width: t.length * CHAR_W }) as TextMetrics,
  } as unknown as CanvasRenderingContext2D;
}
// Only the fields entityStyle / the overflow-chip style / chipFor read.
const THEME = {
  laneColors: Array<string>(10).fill('#22aa55'),
  laneColorsAlpha: Array<string>(10).fill('rgba(34,170,85,0.2)'),
  bg2: '#1b1b1b',
  text2: '#e6e6e6',
  border: '#444444',
} as unknown as Theme;
const NODE = { lane: 0 } as unknown as GraphNode;

function lay(
  entities: RefEntity[],
  display: GraphDisplayOptions,
  budget: number,
  startX = 0,
): LaidRefLabel[] {
  return layoutRefLabels(makeCtx(), entities, NODE, THEME, startX, budget, display);
}
/** Branch entities that were actually laid (excludes the trailing "+n" chip). */
function shownPills(r: LaidRefLabel[]): LaidRefLabel[] {
  return r.filter((l) => l.entity !== null);
}
/** The trailing "+n" overflow chip, or undefined when everything fit. */
function overflowChip(r: LaidRefLabel[]): LaidRefLabel | undefined {
  const last = r[r.length - 1];
  return last !== undefined && last.entity === null ? last : undefined;
}
/** The n in a "+n" chip's label (0 when there is no overflow chip). */
function hiddenCount(r: LaidRefLabel[]): number {
  const o = overflowChip(r);
  return o === undefined ? 0 : Number(o.style.label.slice(1));
}
function branchName(l: LaidRefLabel): string | null {
  return l.entity !== null && l.entity.kind === 'branch' ? l.entity.name : null;
}
/** Position-only projection for comparing two layouts (chip carried too). */
function pos(l: LaidRefLabel): { x: number; w: number; chip: unknown } {
  return { x: l.x, w: l.w, chip: l.chip };
}

const DIVERGED = new Map([['main', { ahead: 3, behind: 2 }]]);

describe('layoutRefLabels — chip reservation + "+n" overflow (P51c)', () => {
  // main carries a "↑3 ↓2" chip; a..e are plain local branches (absent from
  // branchStats → no chip). With CHAR_W 10 + comfortable metrics, main is wide
  // enough (pill + reserved chip) that only a few pills fit at budget 260.
  const withChipFirst: RefEntity[] = [
    localBranch('main'),
    localBranch('a'),
    localBranch('b'),
    localBranch('c'),
    localBranch('d'),
    localBranch('e'),
  ];

  it('(a) the "+n" hidden count is EXACT — every entity is shown or counted', () => {
    const r = lay(withChipFirst, disp({ branchStats: DIVERGED }), 260);
    // Overflow actually happened, and the first (chip-bearing) pill is shown.
    expect(overflowChip(r)).toBeDefined();
    expect(r[0].chip).not.toBeNull();
    // Conservation: shown branch pills + the "+n" count === total entities.
    expect(shownPills(r).length + hiddenCount(r)).toBe(withChipFirst.length);

    // Tie the "+n" number to the actual popped set: an unbounded layout fits
    // every entity with NO overflow chip; the hidden ones are exactly those
    // present unbounded but absent here — and that count === the "+n" number.
    const all = lay(withChipFirst, disp({ branchStats: DIVERGED }), 100000);
    expect(overflowChip(all)).toBeUndefined();
    expect(shownPills(all).length).toBe(withChipFirst.length);
    const shownNames = new Set(shownPills(r).map(branchName));
    const popped = shownPills(all)
      .map(branchName)
      .filter((n) => !shownNames.has(n));
    expect(popped.length).toBe(hiddenCount(r));
  });

  it('(b) a chip-bearing pill never overlaps its chip — the next pill sits clear', () => {
    const r = lay(withChipFirst, disp({ branchStats: DIVERGED }), 260);
    const main = r[0];
    const next = r[1];
    expect(main.chip).not.toBeNull();
    expect(next.entity).not.toBeNull(); // a real pill, not the "+n" chip
    // The chip is painted in [x+w+chipGap, x+w+chipGap+chip.width]; the next
    // pill must begin at or beyond that, i.e. exactly one pillGap clear of it.
    const chipEnd = main.x + main.w + METRICS.chipGap + main.chip!.width;
    expect(next.x).toBeGreaterThanOrEqual(chipEnd);
    expect(next.x).toBe(chipEnd + METRICS.pillGap);
  });

  it('(c) a chip-bearing pill popped into "+n" leaves the count AND cursor correct', () => {
    // main (with chip) is placed in the forward pass, then popped when the "+n"
    // chip needs room. Order [a, b, main, c, d] + budget 230 forces exactly that.
    const entities: RefEntity[] = [
      localBranch('a'),
      localBranch('b'),
      localBranch('main'),
      localBranch('c'),
      localBranch('d'),
    ];
    const r = lay(entities, disp({ branchStats: DIVERGED }), 230);
    const o = overflowChip(r);
    expect(o).toBeDefined();
    // main (the chip-bearing pill) was popped out of the visible set…
    expect(shownPills(r).map(branchName)).not.toContain('main');
    // …and it is accounted for in the "+n" count (conservation still holds).
    expect(shownPills(r).length + hiddenCount(r)).toBe(entities.length);

    // Cursor correctness: the "+n" chip sits FLUSH after the last surviving
    // pill (one pillGap clear), i.e. the pop rewound the popped pill's FULL
    // advance including its reserved chipAdvance. A rewind that forgot to
    // subtract chipAdvance would leave the "+n" chip floating to the right.
    const last = shownPills(r)[shownPills(r).length - 1];
    const advLast = last.chip === null ? 0 : METRICS.chipGap + last.chip.width;
    expect(o!.x).toBe(last.x + last.w + advLast + METRICS.pillGap);
    // And the "+n" chip stays inside the band.
    expect(o!.x + o!.w).toBeLessThanOrEqual(230);
  });

  it('(d) toggling ahead/behind OFF (or non-diverged) reserves zero extra width', () => {
    // All three fit in a wide band so positions compare directly.
    const entities: RefEntity[] = [localBranch('a'), localBranch('main'), localBranch('b')];
    const BIG = 100000;
    const on = lay(entities, disp({ branchStats: DIVERGED }), BIG);
    const off = lay(entities, disp({ showAheadBehind: false, branchStats: DIVERGED }), BIG);
    const noStats = lay(entities, disp({ branchStats: new Map() }), BIG); // toggle on, but no divergence

    // Toggle-off is byte-identical to the no-chip (non-diverged) layout: no chip
    // anywhere and the exact same pill geometry — the chip reserved nothing.
    expect(off.map(pos)).toEqual(noStats.map(pos));
    for (const l of off) expect(l.chip).toBeNull();

    // With the chip ON, main reserves gap+chip; every pill AFTER it shifts right
    // by exactly that chipAdvance, while main itself and everything before it
    // stay put. This is what OFF reclaims.
    const i = on.findIndex((l) => branchName(l) === 'main');
    expect(on[i].chip).not.toBeNull();
    const chipAdvance = METRICS.chipGap + on[i].chip!.width;
    expect(on[i].x).toBe(off[i].x); // the chip pill itself does not move
    expect(on[i + 1].x - off[i + 1].x).toBe(chipAdvance); // the pill after it shifts by chipAdvance
    expect(on[i - 1].x).toBe(off[i - 1].x); // the pill before it is unaffected
  });

  it('conserves all entities (shown + hidden === total) at every budget', () => {
    // Budget-independent invariant: no entity is ever lost or double-counted,
    // regardless of where the pack/overflow boundary lands.
    for (let b = 10; b <= 500; b += 5) {
      const r = lay(withChipFirst, disp({ branchStats: DIVERGED }), b);
      expect(shownPills(r).length + hiddenCount(r)).toBe(withChipFirst.length);
    }
  });
});

// PR-badge-placement: forge signals no longer live in the LEFT ref band — the
// CI dot + PR pill moved to the dedicated FORGE column. The band layout is now
// signal-agnostic (its geometry is unaffected by the forge toggles/maps). The
// forge-cell geometry + selection are covered in forgeBadges.test.ts; the
// column reservation in rightColumns.test.ts; the hit-test in hitTest.test.ts.
describe('layoutRefLabels — signal-agnostic band (PR-badge-placement)', () => {
  it('forge toggles/maps do not affect band pill geometry', () => {
    const entities = [localBranch('feat'), localBranch('other')];
    const withMaps = disp({
      showPrBadge: true,
      showCiStatus: true,
      prByBranch: new Map([['feat', { number: 7, title: 't', state: 'open', isDraft: false, url: 'u' }]]),
      ciBySha: new Map([['tip', { rollup: 'success', passed: 1, failed: 0, pending: 0, total: 1 }]]),
    });
    const on = layoutRefLabels(makeCtx(), entities, { lane: 0, id: 'tip' } as unknown as GraphNode, THEME, 0, 100000, withMaps);
    const off = layoutRefLabels(makeCtx(), entities, { lane: 0, id: 'tip' } as unknown as GraphNode, THEME, 0, 100000, disp());
    expect(on.map((l) => ({ x: l.x, w: l.w }))).toEqual(off.map((l) => ({ x: l.x, w: l.w })));
    expect(on.every((l) => !('signals' in l))).toBe(true);
  });
});
