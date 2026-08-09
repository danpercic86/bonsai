import { describe, expect, it } from 'vitest';

import {
  avatarColor,
  avatarHit,
  graphAreaRight,
  initials,
  laneX,
  refColArea,
  rowAtPoint,
  rowY,
  summaryStartX,
} from './geometry';
import { METRICS } from './metrics';
import type { EffectiveMetrics } from './metrics';

const M: EffectiveMetrics = METRICS;

describe('laneX', () => {
  it('lane 0 sits at refColWidth + gutter + 8', () => {
    expect(laneX(0, M)).toBe(M.refColWidth + M.gutter + 8); // 180 + 12 + 8
  });

  it('advances one laneWidth per lane', () => {
    expect(laneX(1, M) - laneX(0, M)).toBe(M.laneWidth);
    expect(laneX(5, M)).toBe(laneX(0, M) + 5 * M.laneWidth);
  });

  it('clamps at maxRenderLanes - 1 — deep lanes share the last x', () => {
    const last = laneX(M.maxRenderLanes - 1, M);
    expect(laneX(M.maxRenderLanes, M)).toBe(last);
    expect(laneX(1000, M)).toBe(last);
  });
});

describe('graphAreaRight / summaryStartX', () => {
  it('right edge scales with laneCount up to the clamp', () => {
    expect(graphAreaRight(1, M)).toBe(M.refColWidth + M.gutter + M.laneWidth);
    expect(graphAreaRight(3, M)).toBe(M.refColWidth + M.gutter + 3 * M.laneWidth);
    expect(graphAreaRight(M.maxRenderLanes + 50, M)).toBe(
      M.refColWidth + M.gutter + M.maxRenderLanes * M.laneWidth,
    );
  });

  it('summary column starts textGap after the graph area', () => {
    expect(summaryStartX(2, M)).toBe(graphAreaRight(2, M) + M.textGap);
  });
});

describe('refColArea', () => {
  it('fixed band: startX = padLeft, budget = width - pads', () => {
    expect(refColArea(M)).toEqual({
      startX: M.refColPadLeft,
      budget: M.refColWidth - M.refColPadLeft - M.refColPadRight,
    });
  });

  it('budget never goes negative when the band is narrower than its pads', () => {
    // refColWidth is a literal `180` in the METRICS baseline; widen through
    // unknown to model a hypothetical narrower band.
    const tiny = { ...M, refColWidth: 4 } as unknown as EffectiveMetrics;
    expect(refColArea(tiny).budget).toBe(0);
  });
});

describe('rowY / rowAtPoint', () => {
  it('rowY is the row center shifted by scrollTop', () => {
    expect(rowY(0, 0, M)).toBe(M.rowHeight / 2);
    expect(rowY(10, 0, M)).toBe(10 * M.rowHeight + M.rowHeight / 2);
    expect(rowY(10, 3 * M.rowHeight, M)).toBe(7 * M.rowHeight + M.rowHeight / 2);
  });

  it('rowAtPoint inverts rowY over the whole row extent', () => {
    for (const row of [0, 1, 7, 19_999]) {
      const scrollTop = 5 * M.rowHeight + 0.25;
      const y = rowY(row, scrollTop, M);
      expect(rowAtPoint(y, scrollTop, M)).toBe(row);
      // top edge belongs to the row; the exact bottom edge to the next.
      expect(rowAtPoint(row * M.rowHeight - scrollTop, scrollTop, M)).toBe(row);
      expect(rowAtPoint((row + 1) * M.rowHeight - scrollTop, scrollTop, M)).toBe(row + 1);
    }
  });

  it('rowAtPoint may return out-of-range values (callers check)', () => {
    expect(rowAtPoint(-1, 0, M)).toBe(-1);
  });
});

describe('initials', () => {
  it('contract examples (P7 §2.2)', () => {
    expect(initials('Dan Percic')).toBe('DP');
    expect(initials('torvalds')).toBe('TO');
    expect(initials('x')).toBe('X');
    expect(initials('')).toBe('?');
    expect(initials('  Grace  Hopper ')).toBe('GH');
  });

  it('whitespace-only → "?"', () => {
    expect(initials('   ')).toBe('?');
  });

  it('takes the first two words only', () => {
    expect(initials('a b c d')).toBe('AB');
  });

  it('surrogate-safe: astral code points are not split', () => {
    expect(initials('𝔘nicode')).toBe('𝔘N');
    expect(initials('👩 👨')).toBe('👩👨'.toUpperCase());
  });
});

describe('avatarColor', () => {
  it('deterministic per (trimmed) name', () => {
    expect(avatarColor('Dan Percic')).toEqual(avatarColor('  Dan Percic  '));
  });

  it('hsl format with fixed S/L and white text', () => {
    const c = avatarColor('Alice');
    expect(c.bg).toMatch(/^hsl\(\d{1,3}, 52%, 42%\)$/);
    expect(c.text).toBe('#ffffff');
  });

  it('hue stays in [0, 360)', () => {
    for (const n of ['', 'a', 'Zebra Quux', '日本語', '𝕏']) {
      const m = /^hsl\((\d{1,3}),/.exec(avatarColor(n).bg);
      expect(m).not.toBeNull();
      expect(Number(m?.[1])).toBeLessThan(360);
      expect(Number(m?.[1])).toBeGreaterThanOrEqual(0);
    }
  });

  it('different names generally differ', () => {
    const hues = new Set(
      ['Alice', 'Bob', 'Carol', 'Dave', 'Erin', 'Frank'].map((n) => avatarColor(n).bg),
    );
    expect(hues.size).toBeGreaterThanOrEqual(2);
  });
});

describe('avatarHit', () => {
  const r = M.avatarRadius + M.avatarBgRingExtra; // hit radius incl. bg ring

  it('center hits; exact radius hits (<=); just outside misses', () => {
    expect(avatarHit(50, 50, 50, 50, M)).toBe(true);
    expect(avatarHit(50 + r, 50, 50, 50, M)).toBe(true);
    expect(avatarHit(50, 50 + r, 50, 50, M)).toBe(true);
    expect(avatarHit(50 + r + 0.01, 50, 50, 50, M)).toBe(false);
  });

  it('diagonal uses euclidean distance, not a bounding box', () => {
    const d = r / Math.SQRT2;
    expect(avatarHit(50 + d - 0.01, 50 + d - 0.01, 50, 50, M)).toBe(true);
    expect(avatarHit(50 + r - 0.01, 50 + r - 0.01, 50, 50, M)).toBe(false);
  });

  it('negative coordinates are handled (plain math)', () => {
    expect(avatarHit(-10, -10, -10, -10, M)).toBe(true);
    expect(avatarHit(0, 0, -10, -10, M)).toBe(false);
  });
});
