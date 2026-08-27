// spec-002 §5 — tests for the settle-on-scroll sway MODEL (pure math). The rAF
// runner lifecycle in `useSway.ts` is a USER CHECKPOINT (headless harness pauses
// rAF), so this suite pins only the offset function + the enable gate.
import { describe, expect, it } from 'vitest';

import { SWAY_DURATION_MS, swayEnabled, swayOffset } from './sway';

const PEAK = 0.8;

describe('swayEnabled', () => {
  it('gates purely on reduced motion', () => {
    expect(swayEnabled(false)).toBe(true);
    expect(swayEnabled(true)).toBe(false);
  });
});

describe('swayOffset', () => {
  it('is exactly 0 at the end of the settle window', () => {
    expect(swayOffset(SWAY_DURATION_MS, 0, false)).toBe(0);
    expect(swayOffset(SWAY_DURATION_MS, 3, false)).toBe(0);
  });

  it('is exactly 0 past the window and at/before the start', () => {
    expect(swayOffset(SWAY_DURATION_MS + 1, 0, false)).toBe(0);
    expect(swayOffset(10_000, 2, false)).toBe(0);
    expect(swayOffset(0, 0, false)).toBe(0);
    expect(swayOffset(-50, 1, false)).toBe(0);
  });

  it('never exceeds the peak amplitude across the whole window (all lanes)', () => {
    for (let lane = 0; lane < 10; lane++) {
      for (let e = 0; e <= SWAY_DURATION_MS; e += 5) {
        expect(Math.abs(swayOffset(e, lane, false))).toBeLessThanOrEqual(PEAK);
      }
    }
  });

  it('returns 0 under reduced motion even mid-window', () => {
    expect(swayOffset(400, 0, true)).toBe(0);
    expect(swayOffset(200, 3, true)).toBe(0);
    expect(swayOffset(SWAY_DURATION_MS / 2, 5, true)).toBe(0);
  });

  it('is non-zero somewhere in the interior (the settle actually moves)', () => {
    const samples = [];
    for (let e = 50; e < SWAY_DURATION_MS; e += 25) samples.push(swayOffset(e, 0, false));
    expect(samples.some((v) => Math.abs(v) > 0.05)).toBe(true);
  });

  it('gives adjacent lanes a different phase (they settle out of sync)', () => {
    // Sample mid-window where the decay envelope has not yet crushed the signal.
    for (const e of [200, 300, 400]) {
      expect(swayOffset(e, 0, false)).not.toBe(swayOffset(e, 1, false));
    }
  });
});
