import { describe, expect, it } from 'vitest';

import { AVATAR, FONT_MONO, FONT_UI, METRICS, effectiveMetrics } from './metrics';

const baseKnobs = {
  avatarRadius: METRICS.avatarRadius,
  rowHeight: METRICS.rowHeight,
  laneWidth: METRICS.laneWidth,
  compact: false,
};

describe('effectiveMetrics — comfortable mode', () => {
  it('with baseline knobs it reproduces METRICS exactly (pre-P51 invariant)', () => {
    expect(effectiveMetrics(baseKnobs)).toEqual({ ...METRICS });
  });

  it('user knobs pass through; everything else stays at the METRICS baseline', () => {
    const m = effectiveMetrics({ avatarRadius: 14, rowHeight: 40, laneWidth: 20, compact: false });
    expect(m.avatarRadius).toBe(14);
    expect(m.rowHeight).toBe(40);
    expect(m.laneWidth).toBe(20);
    expect(m.pillHeight).toBe(METRICS.pillHeight);
    expect(m.textGap).toBe(METRICS.textGap);
    expect(m.summaryFont).toBe(METRICS.summaryFont);
    expect(m.refColWidth).toBe(METRICS.refColWidth);
  });

  it('derives head/selection rings preserving the +2.5 / +3.5 baseline deltas', () => {
    const m = effectiveMetrics({ avatarRadius: 14, rowHeight: 32, laneWidth: 16, compact: false });
    expect(m.avatarHeadRingRadius).toBe(16.5);
    expect(m.avatarSelRingRadius).toBe(17.5);
  });

  it('extreme knob values still produce consistent rings (no clamping here)', () => {
    const m = effectiveMetrics({ avatarRadius: 0, rowHeight: 1, laneWidth: 1, compact: false });
    expect(m.avatarHeadRingRadius).toBe(2.5);
    expect(m.avatarSelRingRadius).toBe(3.5);
  });
});

describe('effectiveMetrics — compact mode', () => {
  const compact = effectiveMetrics({ avatarRadius: 14, rowHeight: 40, laneWidth: 20, compact: true });

  it('overrides row/node/pill/font geometry from the preset, ignoring the sliders', () => {
    expect(compact.rowHeight).toBe(22);
    expect(compact.avatarRadius).toBe(8);
    expect(compact.avatarBgRingExtra).toBe(1);
    expect(compact.pillHeight).toBe(15);
    expect(compact.textGap).toBe(8);
    expect(compact.avatarFont).toBe('600 10px');
    expect(compact.summaryFont).toBe('400 12px');
    expect(compact.metaFont).toBe('400 11px');
    expect(compact.shaFont).toBe('11px');
  });

  it('laneWidth ALWAYS honors its slider, even in compact mode', () => {
    expect(compact.laneWidth).toBe(20);
  });

  it('rings derive from the compact avatarRadius (8 → 10.5 / 11.5)', () => {
    expect(compact.avatarHeadRingRadius).toBe(10.5);
    expect(compact.avatarSelRingRadius).toBe(11.5);
  });

  it('non-preset fields stay at the METRICS baseline', () => {
    expect(compact.refColWidth).toBe(METRICS.refColWidth);
    expect(compact.maxRenderLanes).toBe(METRICS.maxRenderLanes);
    expect(compact.pillFont).toBe(METRICS.pillFont);
  });
});

describe('constants sanity', () => {
  it('AVATAR HSL constants are the locked P7 values', () => {
    expect(AVATAR).toEqual({ sat: 52, light: 42 });
  });

  it('font stacks end in generic families', () => {
    expect(FONT_UI.endsWith('sans-serif')).toBe(true);
    expect(FONT_MONO.endsWith('monospace')).toBe(true);
  });
});
