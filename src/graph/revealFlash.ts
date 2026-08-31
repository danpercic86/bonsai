// P84 reveal-in-graph flash curve — pure, headless-testable math (no canvas
// import), mirroring `viewport.ts` / `headGuide`. The GraphCanvas rAF loop feeds
// it `elapsedMs` each frame; `draw.ts` consumes the returned alpha + ring radius.
//
// Motion spec (P84 UI contract §3):
//  - Total 900ms. Alpha rises fast to peak by ~90ms, then ease-out fades to 0 by
//    900ms. Halo ring radius grows linearly +1 → +6px over the full 900ms.
//  - prefers-reduced-motion (§3.1): NO animation — a static overlay at a lower
//    steady alpha and a fixed ring radius, held for 1200ms then cleared in one
//    step (two paints total: on, then off).

/** Full animated flash duration (ms). */
export const FLASH_DURATION_MS = 900;
/** Reduced-motion static-hold duration (ms) before a single clearing paint. */
export const FLASH_REDUCED_MS = 1200;

/** Time (ms) the alpha takes to rise from 0 to its peak (animated path). */
const RISE_MS = 90;

/** Peak row-pulse / halo alpha, per theme (dark reads slightly stronger). */
function peakAlpha(dark: boolean): number {
  return dark ? 0.3 : 0.24;
}

/** Steady reduced-motion alpha, per theme. */
function staticAlpha(dark: boolean): number {
  return dark ? 0.18 : 0.14;
}

/** `easeOut(x) = 1 - (1 - x)^2` (quadratic ease-out), clamped to [0, 1]. */
function easeOut(x: number): number {
  const t = Math.min(1, Math.max(0, x));
  return 1 - (1 - t) * (1 - t);
}

/** How long the flash stays active for a given motion mode. */
export function flashDurationMs(reducedMotion: boolean): number {
  return reducedMotion ? FLASH_REDUCED_MS : FLASH_DURATION_MS;
}

/**
 * Composited alpha for BOTH the row-background pulse and the dot halo ring at
 * `elapsedMs`. Returns 0 once the flash has fully elapsed (caller stops drawing).
 */
export function flashAlpha(elapsedMs: number, dark: boolean, reducedMotion: boolean): number {
  if (reducedMotion) {
    return elapsedMs < FLASH_REDUCED_MS ? staticAlpha(dark) : 0;
  }
  if (elapsedMs <= 0) return 0;
  if (elapsedMs >= FLASH_DURATION_MS) return 0;
  const peak = peakAlpha(dark);
  if (elapsedMs < RISE_MS) {
    return peak * (elapsedMs / RISE_MS);
  }
  // Ease-out fade to 0 over the 90 → 900ms tail.
  const tail = (elapsedMs - RISE_MS) / (FLASH_DURATION_MS - RISE_MS);
  return peak * (1 - easeOut(tail));
}

/**
 * Dot halo ring radius at `elapsedMs`. Animated: `baseRadius + 1 → baseRadius + 6`
 * linearly over 900ms. Reduced-motion: fixed at `baseRadius + 3`.
 */
export function flashRingRadius(
  elapsedMs: number,
  baseRadius: number,
  reducedMotion: boolean,
): number {
  if (reducedMotion) return baseRadius + 3;
  const t = Math.min(1, Math.max(0, elapsedMs / FLASH_DURATION_MS));
  return baseRadius + 1 + 5 * t;
}
