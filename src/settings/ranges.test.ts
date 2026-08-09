import { describe, expect, it } from 'vitest';

import * as R from './ranges';

describe('settings ranges', () => {
  it('pins the exact values mirrored from src-tauri/src/settings.rs', () => {
    expect(R.AUTO_FETCH_INTERVAL_MIN).toBe(1);
    expect(R.AUTO_FETCH_INTERVAL_MAX).toBe(120);
    expect(R.HEALTH_REFRESH_INTERVAL_MIN).toBe(1);
    expect(R.HEALTH_REFRESH_INTERVAL_MAX).toBe(240);
    expect(R.AVATAR_RADIUS_MIN).toBe(6);
    expect(R.AVATAR_RADIUS_MAX).toBe(16);
    expect(R.ROW_HEIGHT_MIN).toBe(24);
    expect(R.ROW_HEIGHT_MAX).toBe(48);
    expect(R.LANE_WIDTH_MIN).toBe(10);
    expect(R.LANE_WIDTH_MAX).toBe(28);
  });

  it('every MIN is a positive integer strictly below its MAX', () => {
    const pairs: Array<[number, number]> = [
      [R.AUTO_FETCH_INTERVAL_MIN, R.AUTO_FETCH_INTERVAL_MAX],
      [R.HEALTH_REFRESH_INTERVAL_MIN, R.HEALTH_REFRESH_INTERVAL_MAX],
      [R.AVATAR_RADIUS_MIN, R.AVATAR_RADIUS_MAX],
      [R.ROW_HEIGHT_MIN, R.ROW_HEIGHT_MAX],
      [R.LANE_WIDTH_MIN, R.LANE_WIDTH_MAX],
    ];
    for (const [min, max] of pairs) {
      expect(Number.isInteger(min)).toBe(true);
      expect(Number.isInteger(max)).toBe(true);
      expect(min).toBeGreaterThan(0);
      expect(min).toBeLessThan(max);
    }
  });

  it('METRICS defaults sit inside the slider ranges (knob sanity)', async () => {
    const { METRICS } = await import('../graph/metrics');
    expect(METRICS.avatarRadius).toBeGreaterThanOrEqual(R.AVATAR_RADIUS_MIN);
    expect(METRICS.avatarRadius).toBeLessThanOrEqual(R.AVATAR_RADIUS_MAX);
    expect(METRICS.rowHeight).toBeGreaterThanOrEqual(R.ROW_HEIGHT_MIN);
    expect(METRICS.rowHeight).toBeLessThanOrEqual(R.ROW_HEIGHT_MAX);
    expect(METRICS.laneWidth).toBeGreaterThanOrEqual(R.LANE_WIDTH_MIN);
    expect(METRICS.laneWidth).toBeLessThanOrEqual(R.LANE_WIDTH_MAX);
  });
});
