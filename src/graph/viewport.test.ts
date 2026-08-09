import { describe, expect, it } from 'vitest';

import {
  backingStoreSize,
  clampTooltipPos,
  scrollRowIntoView,
  spacerHeight,
  visibleRowCount,
  visibleRowRange,
} from './viewport';
import type { Rect } from './viewport';

const RH = 32; // METRICS.rowHeight baseline

describe('visibleRowCount', () => {
  it('floors to whole rows', () => {
    expect(visibleRowCount(320, RH)).toBe(10);
    expect(visibleRowCount(335, RH)).toBe(10);
    expect(visibleRowCount(351.9, RH)).toBe(10);
    expect(visibleRowCount(352, RH)).toBe(11);
  });

  it('zero-height canvas → still 1 (PageUp/Down must move)', () => {
    expect(visibleRowCount(0, RH)).toBe(1);
  });

  it('height smaller than one row → 1', () => {
    expect(visibleRowCount(31, RH)).toBe(1);
  });
});

describe('visibleRowRange', () => {
  it('top of the list: firstRow clamps at 0 despite overscan', () => {
    const r = visibleRowRange(0, 0, RH, 320, 100, 4);
    expect(r.firstRow).toBe(0);
    expect(r.layoutScrollTop).toBe(0);
    // ceil(320/32) + 4 = 10 + 4
    expect(r.lastRow).toBe(14);
  });

  it('bottom of a 20k-row list: lastRow clamps at n-1', () => {
    const n = 20_000;
    const scrollTop = n * RH - 320; // fully scrolled
    const r = visibleRowRange(scrollTop, 0, RH, 320, n, 4);
    expect(r.lastRow).toBe(n - 1);
    expect(r.firstRow).toBe(Math.floor(scrollTop / RH) - 4);
    expect(r.firstRow).toBeGreaterThanOrEqual(0);
    expect(r.lastRow - r.firstRow).toBeLessThan(20); // stays a small window
  });

  it('mid-list window is overscanned on both sides', () => {
    const r = visibleRowRange(100 * RH, 0, RH, 320, 20_000, 4);
    expect(r.firstRow).toBe(96);
    expect(r.lastRow).toBe(114); // ceil((3200+320)/32)+4 = 110+4
  });

  it('fractional scrollTop floors/ceils correctly', () => {
    const r = visibleRowRange(100 * RH + 0.5, 0, RH, 320, 20_000, 4);
    expect(r.firstRow).toBe(96); // floor(100.015..) - 4
    expect(r.lastRow).toBe(115); // ceil(110.015..) + 4
    expect(r.layoutScrollTop).toBeCloseTo(3200.5);
  });

  it('WIP offset shifts the layout scrollTop by one row', () => {
    const withWip = visibleRowRange(10 * RH, 1, RH, 320, 100, 4);
    const without = visibleRowRange(9 * RH, 0, RH, 320, 100, 4);
    expect(withWip.layoutScrollTop).toBe(9 * RH);
    expect(withWip.firstRow).toBe(without.firstRow);
    expect(withWip.lastRow).toBe(without.lastRow);
  });

  it('WIP row at scrollTop 0 → layoutScrollTop is negative, firstRow still 0', () => {
    const r = visibleRowRange(0, 1, RH, 320, 100, 4);
    expect(r.layoutScrollTop).toBe(-RH);
    expect(r.firstRow).toBe(0);
  });

  it('empty graph → lastRow -1 (an empty paint loop)', () => {
    const r = visibleRowRange(0, 0, RH, 320, 0, 4);
    expect(r.firstRow).toBe(0);
    expect(r.lastRow).toBe(-1);
  });

  it('zero-height viewport still yields a valid (overscan-only) window', () => {
    const r = visibleRowRange(10 * RH, 0, RH, 0, 100, 4);
    expect(r.firstRow).toBe(6);
    expect(r.lastRow).toBe(14);
  });
});

describe('scrollRowIntoView', () => {
  // viewport: rows 10..19 fully visible (viewTop 320, height 320).
  const viewTop = 10 * RH;
  const viewH = 320;

  it('fully visible row → null (no adjustment)', () => {
    expect(scrollRowIntoView(10, 0, RH, viewTop, viewH)).toBeNull();
    expect(scrollRowIntoView(19, 0, RH, viewTop, viewH)).toBeNull();
    expect(scrollRowIntoView(15, 0, RH, viewTop, viewH)).toBeNull();
  });

  it('row above → scrolls to one row of breathing room above it', () => {
    expect(scrollRowIntoView(9, 0, RH, viewTop, viewH)).toBe(8 * RH);
    expect(scrollRowIntoView(0, 0, RH, viewTop, viewH)).toBe(0); // clamped at 0
  });

  it('row 1 above the top clamps at 0 (rowTop - rowHeight = 0)', () => {
    expect(scrollRowIntoView(1, 0, RH, viewTop, viewH)).toBe(0);
  });

  it('row below → bottom-aligns with one row of breathing room', () => {
    // row 20: rowBottom = 21*RH; new top = 21*RH - 320 + RH = 22*RH - 320
    expect(scrollRowIntoView(20, 0, RH, viewTop, viewH)).toBe(22 * RH - viewH);
  });

  it('WIP offset shifts the target row down by one', () => {
    // row 9 with wipOffset 1 occupies the slot of row 10 → visible → null.
    expect(scrollRowIntoView(9, 1, RH, viewTop, viewH)).toBeNull();
    // row 19 with wipOffset 1 → slot 20 → below → adjust.
    expect(scrollRowIntoView(19, 1, RH, viewTop, viewH)).toBe(22 * RH - viewH);
  });

  it('20k-row jump lands exactly', () => {
    const row = 19_999;
    const next = scrollRowIntoView(row, 0, RH, 0, viewH);
    expect(next).toBe((row + 2) * RH - viewH);
  });
});

describe('clampTooltipPos', () => {
  const anchor: Rect = { left: 100, top: 50, width: 40, height: 18 };

  it('default: below the anchor at its left edge', () => {
    expect(clampTooltipPos(anchor, 80, 20, 800, 600)).toEqual({ left: 100, top: 72 });
  });

  it('right overflow pulls left (host - width - 4)', () => {
    expect(clampTooltipPos(anchor, 720, 20, 800, 600)).toEqual({ left: 76, top: 72 });
  });

  it('left clamp never goes under 4', () => {
    const a: Rect = { left: 0, top: 50, width: 10, height: 18 };
    expect(clampTooltipPos(a, 900, 20, 800, 600).left).toBe(4);
  });

  it('bottom overflow flips above the anchor', () => {
    const a: Rect = { left: 100, top: 580, width: 40, height: 18 };
    expect(clampTooltipPos(a, 80, 30, 800, 600)).toEqual({ left: 100, top: 580 - 30 - 4 });
  });

  it('exact fit at the right edge does not shift', () => {
    // left + tw === hostW → NOT > → unchanged (boundary is exclusive).
    expect(clampTooltipPos(anchor, 700, 20, 800, 600).left).toBe(100);
  });

  it('exact fit at the bottom edge does not flip', () => {
    // top (72) + th === hostH → NOT > → stays below.
    expect(clampTooltipPos(anchor, 80, 528, 800, 600).top).toBe(72);
  });
});

describe('backingStoreSize', () => {
  it('DPR 1: exact CSS size', () => {
    expect(backingStoreSize(800, 600, 1)).toEqual({ width: 800, height: 600 });
  });

  it('DPR 2: doubled', () => {
    expect(backingStoreSize(800, 600, 2)).toEqual({ width: 1600, height: 1200 });
  });

  it('DPR 1.5: rounds to nearest device pixel', () => {
    expect(backingStoreSize(801, 601, 1.5)).toEqual({ width: 1202, height: 902 });
    expect(backingStoreSize(333, 333, 1.5)).toEqual({ width: 500, height: 500 }); // 499.5 rounds up
  });

  it('zero / sub-pixel CSS size never yields a 0 store', () => {
    expect(backingStoreSize(0, 0, 2)).toEqual({ width: 1, height: 1 });
    expect(backingStoreSize(0.2, 0.2, 1)).toEqual({ width: 1, height: 1 });
  });
});

describe('spacerHeight', () => {
  it('rows × rowHeight + 8 breathing room', () => {
    expect(spacerHeight(100, 0, RH)).toBe(100 * RH + 8);
  });

  it('WIP row adds one row', () => {
    expect(spacerHeight(100, 1, RH)).toBe(101 * RH + 8);
  });

  it('empty graph → just the breathing room', () => {
    expect(spacerHeight(0, 0, RH)).toBe(8);
  });

  it('20k rows at a custom rowHeight knob', () => {
    expect(spacerHeight(20_000, 0, 22)).toBe(440_008);
  });
});
