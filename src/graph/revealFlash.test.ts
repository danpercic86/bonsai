import { describe, expect, it } from 'vitest';
import {
  FLASH_DURATION_MS,
  FLASH_REDUCED_MS,
  flashAlpha,
  flashDurationMs,
  flashRingRadius,
} from './revealFlash';

describe('flashAlpha (animated)', () => {
  it('is 0 at/before start and at/after the full duration', () => {
    expect(flashAlpha(0, true, false)).toBe(0);
    expect(flashAlpha(-10, true, false)).toBe(0);
    expect(flashAlpha(FLASH_DURATION_MS, true, false)).toBe(0);
    expect(flashAlpha(FLASH_DURATION_MS + 100, true, false)).toBe(0);
  });

  it('rises to the theme peak at ~90ms then fades monotonically', () => {
    const peakDark = flashAlpha(90, true, false);
    expect(peakDark).toBeCloseTo(0.3, 5);
    // Rising phase: half-way up by 45ms.
    expect(flashAlpha(45, true, false)).toBeCloseTo(0.15, 5);
    // Fade is monotonically decreasing after the peak.
    const a1 = flashAlpha(200, true, false);
    const a2 = flashAlpha(500, true, false);
    const a3 = flashAlpha(800, true, false);
    expect(a1).toBeGreaterThan(a2);
    expect(a2).toBeGreaterThan(a3);
    expect(a3).toBeGreaterThan(0);
  });

  it('uses a lower peak in light theme', () => {
    expect(flashAlpha(90, false, false)).toBeCloseTo(0.24, 5);
  });
});

describe('flashAlpha (reduced motion)', () => {
  it('holds a steady lower alpha then clears at 1200ms', () => {
    expect(flashAlpha(0, true, true)).toBeCloseTo(0.18, 5);
    expect(flashAlpha(600, true, true)).toBeCloseTo(0.18, 5);
    expect(flashAlpha(1199, true, true)).toBeCloseTo(0.18, 5);
    expect(flashAlpha(FLASH_REDUCED_MS, true, true)).toBe(0);
    expect(flashAlpha(600, false, true)).toBeCloseTo(0.14, 5);
  });
});

describe('flashRingRadius', () => {
  it('grows +1 → +6 linearly over the animated duration', () => {
    expect(flashRingRadius(0, 10, false)).toBeCloseTo(11, 5);
    expect(flashRingRadius(FLASH_DURATION_MS, 10, false)).toBeCloseTo(16, 5);
    expect(flashRingRadius(FLASH_DURATION_MS / 2, 10, false)).toBeCloseTo(13.5, 5);
  });

  it('is fixed at +3 under reduced motion', () => {
    expect(flashRingRadius(0, 10, true)).toBe(13);
    expect(flashRingRadius(600, 10, true)).toBe(13);
  });
});

describe('flashDurationMs', () => {
  it('reflects the active mode', () => {
    expect(flashDurationMs(false)).toBe(FLASH_DURATION_MS);
    expect(flashDurationMs(true)).toBe(FLASH_REDUCED_MS);
  });
});
