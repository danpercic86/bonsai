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

  /** P68d FIX 2 — the P68c review's must-fix. The 12 streaming-AI constants landed
   *  in P68c with NO drift guard in either direction: this file is the repo's
   *  established mirror test and was not extended, and Rust's side is pinned by
   *  `bonsai_core::ai::tests::ai_max_concurrent_runs_matches_the_typescript_mirror`
   *  plus the `AI_*` consts in `src-tauri/src/settings.rs`. Change a number there
   *  and this test fails, which is the whole point. */
  it('pins the 12 P68 streaming-AI constants mirrored from src-tauri/src/settings.rs', () => {
    expect(R.AI_IDLE_TIMEOUT_MIN).toBe(30);
    expect(R.AI_IDLE_TIMEOUT_MAX).toBe(3600);
    expect(R.AI_HARD_CAP_MIN).toBe(60);
    expect(R.AI_HARD_CAP_MAX).toBe(86_400);
    expect(R.AI_MAX_TURNS_MIN).toBe(1);
    expect(R.AI_MAX_TURNS_MAX).toBe(20);
    expect(R.AI_BULK_MAX_BYTES_MIN).toBe(20_000);
    expect(R.AI_BULK_MAX_BYTES_MAX).toBe(4_000_000);
    expect(R.AI_MAX_BUDGET_USD_MAX).toBe(100);
    expect(R.AI_DOCK_HEIGHT_MIN).toBe(120);
    expect(R.AI_DOCK_HEIGHT_MAX).toBe(600);
    // Mirrors `bonsai_core::ai::AI_MAX_CONCURRENT_RUNS` (the authoritative copy —
    // the backend rejects an over-cap run; this one only pre-disables the UI).
    expect(R.AI_MAX_CONCURRENT_RUNS).toBe(3);
  });

  it('every P68 AI range is a positive-integer MIN strictly below its MAX', () => {
    const pairs: Array<[number, number]> = [
      [R.AI_IDLE_TIMEOUT_MIN, R.AI_IDLE_TIMEOUT_MAX],
      [R.AI_HARD_CAP_MIN, R.AI_HARD_CAP_MAX],
      [R.AI_MAX_TURNS_MIN, R.AI_MAX_TURNS_MAX],
      [R.AI_BULK_MAX_BYTES_MIN, R.AI_BULK_MAX_BYTES_MAX],
      [R.AI_DOCK_HEIGHT_MIN, R.AI_DOCK_HEIGHT_MAX],
    ];
    for (const [min, max] of pairs) {
      expect(Number.isInteger(min)).toBe(true);
      expect(Number.isInteger(max)).toBe(true);
      expect(min).toBeGreaterThan(0);
      expect(min).toBeLessThan(max);
    }
    // The budget is a MAX-only knob (0 = "no --max-budget-usd flag at all"), and
    // the concurrency cap must stay >= 2 or it re-creates the item-5 single-slot bug.
    expect(R.AI_MAX_BUDGET_USD_MAX).toBeGreaterThan(0);
    expect(R.AI_MAX_CONCURRENT_RUNS).toBeGreaterThanOrEqual(2);
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
