// Spec-007 §4: frontier-pulse motion math — revealFlash-derived, but 600ms
// total (shorter than revealFlash's 900ms: pulses overlap at 2×/4× and must
// not smear) and the ring grows +1 → +5px. Pure, headless-testable; painted by
// replayPaint.ts AFTER drawGraph. Alphas are code constants (revealFlash
// precedent, UI contract preamble) — never CSS tokens.

/** Total pulse duration (ms). */
export const PULSE_MS = 600;
/** Alpha rise time to peak (ms). */
const RISE_MS = 90;
/** Peak tint/ring alpha per theme (UI contract §4). */
export const PULSE_PEAK_DARK = 0.3;
export const PULSE_PEAK_LIGHT = 0.24;
/** Ring budget per paint: beyond this, tint only (UI contract §4 cap). */
export const MAX_PULSE_RINGS = 24;

/** One revealed-row pulse: the row + its wall-clock start timestamp. */
export interface ReplayPulse {
  row: number;
  start: number;
}

function easeOut(x: number): number {
  const t = Math.min(1, Math.max(0, x));
  return 1 - (1 - t) * (1 - t);
}

/** Composited alpha for the row tint + halo ring at `elapsedMs`; 0 when done. */
export function pulseAlpha(elapsedMs: number, dark: boolean): number {
  if (elapsedMs <= 0 || elapsedMs >= PULSE_MS) return 0;
  const peak = dark ? PULSE_PEAK_DARK : PULSE_PEAK_LIGHT;
  if (elapsedMs < RISE_MS) return peak * (elapsedMs / RISE_MS);
  return peak * (1 - easeOut((elapsedMs - RISE_MS) / (PULSE_MS - RISE_MS)));
}

/** Halo ring radius: `base + 1 → base + 5` linearly over the pulse. */
export function pulseRingRadius(elapsedMs: number, baseRadius: number): number {
  const t = Math.min(1, Math.max(0, elapsedMs / PULSE_MS));
  return baseRadius + 1 + 4 * t;
}

/** Drop pulses that have fully elapsed at `now`. Returns the same array when
 *  nothing expired (cheap steady-state check for the paint path). */
export function prunePulses(pulses: readonly ReplayPulse[], now: number): readonly ReplayPulse[] {
  if (pulses.length === 0) return pulses;
  if (now - pulses[0].start < PULSE_MS) return pulses; // oldest first ⇒ all live
  return pulses.filter((p) => now - p.start < PULSE_MS);
}
