import { describe, expect, it } from 'vitest';

import {
  HEAD_GUIDE_PAD,
  backingStoreSize,
  clampTooltipPos,
  headGuide,
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

describe('headGuide', () => {
  // P67 §1: METRICS baseline — avatarRadius 10 + avatarBgRingExtra 2 ⇒ halo 12.
  // The `y1` expectations below are hand-computed LITERALS that bake that 12 in,
  // deliberately, so they cannot drift with the implementation's own formula.
  const H = 320; // viewport height
  const base = {
    headIndex: 5 as number | null,
    layoutScrollTop: -RH, // raw scrollTop 0 with a WIP row
    wipOffset: 1,
    rowHeight: RH,
    avatarRadius: 10,
    ringExtra: 2,
    viewportHeight: H,
  };
  /** The unclamped HEAD row centre in viewport px, mirroring the algorithm. */
  const center = (headIndex: number, layoutScrollTop: number, rowHeight = RH): number =>
    headIndex * rowHeight + rowHeight / 2 - layoutScrollTop;

  it('unknown HEAD (streamed graph before HEAD arrives) → null', () => {
    expect(headGuide({ ...base, headIndex: null })).toBeNull();
  });

  it('WIP row at raw scrollTop 0 → anchored on the WIP dot, stopped at the halo', () => {
    const g = headGuide(base);
    expect(g).not.toBeNull();
    if (g === null) return;
    expect(g.y0).toBe(RH / 2); // identical to the pre-P67 `RH/2 - vp.scrollTop`
    // Hand-computed, NOT re-derived from the implementation's own centre formula:
    // row 5 centre in content px = 5*32 + 16 = 176; the WIP row shifts the view up
    // by one row (layoutScrollTop -32) ⇒ centre on screen 176 + 32 = 208;
    // minus the halo (10 + 2) ⇒ 196.
    expect(g.y1).toBe(196);
    expect(g.edge).toBeNull();
    expect(g.segment).toBe(true);
    expect(g.dashOffset).toBe(0); // unclamped anchor ⇒ no phase compensation
  });

  it('REGRESSION (the reported bug): still returns a segment far past the WIP row', () => {
    // Raw scrollTop 5000 ⇒ layoutScrollTop 4968. The old gate
    // (`scrollTop < rowHeight + 56` = 88px) drew nothing at all here.
    const g = headGuide({ ...base, headIndex: 200, layoutScrollTop: 4968 });
    expect(g).not.toBeNull();
    if (g === null) return;
    expect(Math.abs(g.y1 - g.y0)).toBeGreaterThanOrEqual(1);
    expect(g.segment).toBe(true);
  });

  it("HEAD just above the top edge → edge 'top'", () => {
    // Clean tree, HEAD centre at -10 (inside the halo's reach of the -PAD anchor
    // yet not collapsed): the marker points up.
    const g = headGuide({ ...base, wipOffset: 0, headIndex: 0, layoutScrollTop: 26 });
    expect(g).not.toBeNull();
    if (g === null) return;
    expect(center(0, 26)).toBe(-10);
    expect(g.edge).toBe('top');
    // A5: for THESE arguments the segment is genuinely drawable, not collapsed —
    // anchor clamps to -8 and the target sits one halo BELOW the centre at +2,
    // a 10px run. (§1.1a's "now also segment === false" assumed a tighter band.)
    expect(g.y0).toBe(-HEAD_GUIDE_PAD);
    expect(g.y1).toBe(2);
    expect(g.segment).toBe(true);
  });

  it("HEAD below the bottom edge → edge 'bottom', target clamped to the edge", () => {
    const g = headGuide({ ...base, headIndex: 200, layoutScrollTop: 4968 });
    expect(g).not.toBeNull();
    if (g === null) return;
    expect(center(200, 4968)).toBeGreaterThan(H);
    expect(g.edge).toBe('bottom');
    expect(g.y1).toBe(H + HEAD_GUIDE_PAD);
    expect(g.segment).toBe(true);
  });

  it('PERF guard: absurd scroll distances keep BOTH ends inside the padded viewport', () => {
    const g = headGuide({ ...base, headIndex: 100_000, layoutScrollTop: 1e6 });
    expect(g).not.toBeNull();
    if (g === null) return;
    for (const y of [g.y0, g.y1]) {
      expect(y).toBeGreaterThanOrEqual(-HEAD_GUIDE_PAD);
      expect(y).toBeLessThanOrEqual(H + HEAD_GUIDE_PAD);
    }
    // Bounded stroke length ⇒ bounded dash-segment count per frame.
    expect(Math.abs(g.y1 - g.y0)).toBeLessThanOrEqual(H + 2 * HEAD_GUIDE_PAD);
    expect(g.segment).toBe(true);
  });

  it('CRAWL guard (A6.2): the dash grid stays pinned to the CONTENT, not the viewport', () => {
    // Periodicity + non-negativity alone are satisfied by EITHER sign of the
    // clamp compensation, so they cannot catch a phase inversion. Pin the phase:
    // canvas strokes from y0 and `lineDashOffset = off` shifts the pattern, so
    // the on-screen dash grid sits at y ≡ y0 - off (mod 6). Content-anchoring
    // demands that grid coincide with the anchor's own grid, y ≡ anchor.
    const deep = { ...base, headIndex: 2000, layoutScrollTop: 50_000 };
    const mod6 = (v: number): number => ((v % 6) + 6) % 6;
    // The anchor is the WIP dot centre in viewport px. Derived from the INPUT
    // contract of `visibleRowRange` (layoutScrollTop = rawScrollTop - rowHeight
    // when a WIP row exists), not from `headGuide`'s body:
    //   anchor = rowHeight/2 - rawScrollTop = 16 - (layoutScrollTop + 32).
    const wipDotCentre = (layoutScrollTop: number): number => RH / 2 - (layoutScrollTop + RH);
    expect(wipDotCentre(50_000)).toBe(-50_016); // hand check of the helper

    const a = headGuide(deep);
    const b = headGuide({ ...deep, layoutScrollTop: 50_006 });
    expect(a).not.toBeNull();
    expect(b).not.toBeNull();
    if (a === null || b === null) return;
    expect(a.dashOffset).toBe(b.dashOffset); // 6px of scroll ⇒ same phase
    // Known answer, hand-computed: y0 = -8, anchor = -50016 ⇒ 50008 mod 6 = 4.
    expect(a.dashOffset).toBe(4);

    // Swept across CONSECUTIVE 1px scroll positions: the grid must track the
    // content (i.e. move with the anchor), which the inverted sign does not.
    for (let s = 50_000; s < 50_012; s++) {
      const g = headGuide({ ...deep, layoutScrollTop: s });
      expect(g).not.toBeNull();
      if (g === null) continue;
      expect(g.segment).toBe(true);
      expect(g.dashOffset).toBeGreaterThanOrEqual(0); // plain `%` would go negative
      expect(g.dashOffset).toBeLessThan(6);
      expect(mod6(g.y0 - g.dashOffset)).toBe(mod6(wipDotCentre(s)));
    }
  });

  it('stops one halo short of the HEAD centre — on the near side, both signs', () => {
    const below = headGuide({ ...base, headIndex: 5, layoutScrollTop: -RH });
    expect(below?.y1).toBe(196); // 5*32 + 16 + 32 (WIP shift) - 12 (halo) = 196
    const above = headGuide({ ...base, wipOffset: 0, headIndex: 0, layoutScrollTop: 26 });
    expect(above?.y1).toBe(2); // centre 16 - 26 = -10; sign flips ⇒ -10 + 12 = 2
  });

  it('clean tree (no WIP row) anchors at -PAD and still draws', () => {
    const g = headGuide({ ...base, wipOffset: 0, headIndex: 5, layoutScrollTop: 0 });
    expect(g).not.toBeNull();
    if (g === null) return;
    expect(g.y0).toBe(-HEAD_GUIDE_PAD);
    expect(g.y1).toBe(164); // 5*32 + 16 = 176 (no WIP shift) - 12 (halo) = 164
    expect(g.segment).toBe(true);
  });

  it("collapsed segment (HEAD's halo covers the anchor) → null", () => {
    // Clean tree, HEAD centre 8px below the -PAD anchor ⇒ target clamps onto y0.
    expect(headGuide({ ...base, wipOffset: 0, headIndex: 0, layoutScrollTop: 16 })).toBeNull();
    // WIP row exactly one halo above the HEAD centre (rowHeight 12 ⇒ gap 12).
    expect(
      headGuide({ ...base, headIndex: 0, rowHeight: 12, layoutScrollTop: -12 }),
    ).toBeNull();
  });

  it('echoes headIndex so the draw layer needs no non-null assertion', () => {
    expect(headGuide({ ...base, headIndex: 5 })?.headIndex).toBe(5);
    expect(headGuide({ ...base, headIndex: 200, layoutScrollTop: 4968 })?.headIndex).toBe(200);
  });

  it("A5: scrolled BELOW HEAD with a WIP row → marker-only (segment false, edge 'top')", () => {
    // The WIP row always sits above HEAD, so once HEAD scrolls off the TOP both
    // anchor and target clamp to -PAD. The segment collapses, but the up-marker
    // MUST still be drawn — otherwise `edge: 'top'` is unreachable with
    // uncommitted changes and the guide vanishes exactly when it is needed.
    for (const args of [
      { ...base, headIndex: 0, layoutScrollTop: 1e6 },
      { ...base, headIndex: 3, layoutScrollTop: 500 },
    ]) {
      const g = headGuide(args);
      expect(g).not.toBeNull();
      if (g === null) continue;
      expect(g.segment).toBe(false);
      expect(g.edge).toBe('top');
      expect(g.y0).toBe(-HEAD_GUIDE_PAD);
      expect(g.y1).toBe(-HEAD_GUIDE_PAD);
      expect(g.dashOffset).toBeGreaterThanOrEqual(0);
      expect(g.dashOffset).toBeLessThan(6);
    }
  });

  it('A5: WIP row with HEAD on screen and its halo over the anchor → null', () => {
    // Nothing to point at: the segment collapsed AND HEAD's centre is visible.
    const g = headGuide({ ...base, headIndex: 0, rowHeight: 12, layoutScrollTop: -12 });
    expect(g).toBeNull();
  });

  it("A6.3: dir === 0 (HEAD's centre exactly ON the anchor) → marker-only, not null", () => {
    // Clean tree ⇒ anchor = -PAD = -8. Row 0's centre is 16 - layoutScrollTop, so
    // layoutScrollTop 24 puts it at exactly -8 ⇒ headCenter - anchor === 0.
    // The deleted `if (dir === 0) return null` ran BEFORE `edge` and suppressed a
    // marker that the user should see (HEAD is one row above the top edge).
    const g = headGuide({ ...base, wipOffset: 0, headIndex: 0, layoutScrollTop: 24 });
    expect(center(0, 24)).toBe(-8); // premise: centre == the -PAD anchor
    expect(g).not.toBeNull();
    if (g === null) return;
    expect(g.edge).toBe('top');
    expect(g.segment).toBe(false); // dir 0 ⇒ target === anchor ⇒ collapsed
    expect(g.y0).toBe(-HEAD_GUIDE_PAD);
    expect(g.y1).toBe(-HEAD_GUIDE_PAD);
    expect(g.dashOffset).toBe(0); // anchor is unclamped here ⇒ no compensation
  });
});
