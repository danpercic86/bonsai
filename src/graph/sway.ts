// spec 002 §5 — settle-on-scroll sway flourish: pure, headless-testable math
// (no canvas import), mirroring `revealFlash.ts`. GraphCanvas's rAF loop feeds it
// `elapsedMs` each frame; the Bonsai Pass-4 node loop in `draw.ts` consumes the
// returned per-lane x-offset and translates the whole glyph (disc + rings +
// blossom) by it. This is the MODEL only — the rAF lifecycle lives in
// GraphCanvas (mirroring the revealFlash model/runner split).
//
// Motion spec (UI contract §5):
//  - A brief, BOUNDED settle plays AFTER scrolling stops or on selection change,
//    then the loop tears itself down. NOT a perpetual/idle animation.
//  - Node GLYPHS ONLY move; edges never move. Paint-time offset only — never
//    mutates layout, scrollTop, or hit-test coordinates.
//  - ~600–900ms damped ease-out to rest ("leaves settling after a gust").
//  - ≤1px (0.8px) peak horizontal amplitude, decaying to 0.
//  - Per-lane phase offset so adjacent lanes settle out of sync.
//  - prefers-reduced-motion → ZERO motion; the settle never arms (offset 0).

/** Total settle duration (ms) — a single damped ease-out to rest. */
export const SWAY_DURATION_MS = 800;

/** Peak horizontal amplitude (CSS px) — below the lane-ring width so lanes
 *  never appear to cross. Decays to 0 over the settle window. */
const SWAY_PEAK_PX = 0.8;

/** Oscillation cycles across the settle window (a gentle over-and-back). */
const SWAY_CYCLES = 1.5;

/** Per-lane phase seed (rad) so adjacent lanes settle out of sync. */
const SWAY_LANE_PHASE = 0.7;

/** Fraction of the window used to ramp in from 0 (avoids a start discontinuity). */
const SWAY_ATTACK = 0.08;

/** Whether a settle should be allowed to run for this motion mode. Reduced
 *  motion → never (the loop never starts). Mirrors `flashDurationMs`'s role as
 *  the single lifecycle gate the runner consults. */
export function swayEnabled(reducedMotion: boolean): boolean {
  return !reducedMotion;
}

/**
 * Per-lane paint-time x-offset at `elapsedMs`. Returns 0 under reduced motion,
 * before the settle starts, or once it has fully elapsed (caller then stops
 * requesting frames). Amplitude is a damped oscillation: a quick attack ramp, a
 * linear decay envelope to exactly 0 at the end, times a per-lane-phased sine.
 */
export function swayOffset(elapsedMs: number, lane: number, reducedMotion: boolean): number {
  if (reducedMotion) return 0;
  if (elapsedMs <= 0) return 0;
  if (elapsedMs >= SWAY_DURATION_MS) return 0;
  const t = elapsedMs / SWAY_DURATION_MS;
  const attack = Math.min(1, t / SWAY_ATTACK); // 0 → 1, smooth start
  const decay = 1 - t; // linear envelope to exactly 0 at t = 1
  const phase = lane * SWAY_LANE_PHASE;
  const osc = Math.sin(phase + t * SWAY_CYCLES * Math.PI * 2);
  return SWAY_PEAK_PX * attack * decay * osc;
}
