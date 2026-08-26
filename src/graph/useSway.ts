// spec 002 §5 — settle-on-scroll sway lifecycle, extracted from GraphCanvas so
// the container stays lean (mirrors the revealFlash model/runner split). Owns the
// bounded, self-terminating rAF: a settle is armed after scroll stops / on
// selection change, animates to completion, then the loop tears itself down and
// the canvas returns to fully idle. Nothing is scheduled at idle, when
// graphStyle is standard, during active scroll, or under reduced motion.

import { useCallback, useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import type { GraphStyle } from './colors';
import { SWAY_DURATION_MS, swayEnabled } from './sway';

/** Quiet window (ms) after the last scroll event before a settle arms — long
 *  enough that it never contends with in-progress scroll frames. */
const SWAY_ARM_DELAY_MS = 140;

export interface UseSwayResult {
  /** Call from the scroll handler: cancels any live settle (glyphs paint at
   *  offset 0 while scrolling) and re-arms a fresh one on a scroll-stop debounce. */
  onScroll(): void;
}

/**
 * Wire the sway settle lifecycle. `graphStyleRef` / `reducedMotionRef` are the
 * live refs the paint path already keeps; `paint` is GraphCanvas's `paintNow`.
 * Arming no-ops unless Bonsai + motion allowed + the document is visible.
 */
export function useSway(opts: {
  paint: () => void;
  graphStyle: GraphStyle;
  graphStyleRef: RefObject<GraphStyle>;
  reducedMotion: boolean;
  reducedMotionRef: RefObject<boolean>;
  selectedIndex: number | null;
  /** Active settle descriptor, owned by GraphCanvas so the paint path can read
   *  it; this hook drives + clears it. */
  swayStateRef: RefObject<{ start: number } | null>;
}): UseSwayResult {
  const { paint, graphStyle, graphStyleRef, reducedMotion, reducedMotionRef, selectedIndex, swayStateRef } = opts;
  const swayRafRef = useRef(0);
  const swayArmTimerRef = useRef(0);

  // Cancel any in-flight settle + pending arm timer; repaint once at offset 0
  // only if a settle was actually running (the idle path gets no extra paint).
  const cancelSway = useCallback(() => {
    if (swayArmTimerRef.current !== 0) {
      clearTimeout(swayArmTimerRef.current);
      swayArmTimerRef.current = 0;
    }
    const wasRunning = swayStateRef.current !== null;
    if (swayRafRef.current !== 0) {
      cancelAnimationFrame(swayRafRef.current);
      swayRafRef.current = 0;
    }
    swayStateRef.current = null;
    if (wasRunning) paint();
  }, [paint, swayStateRef]);

  // Arm a bounded settle NOW. No-op unless Bonsai + motion on + document visible.
  // Mirrors startRevealFlash's animated path: a self-contained rAF repaints each
  // frame until SWAY_DURATION_MS elapses, then a final clearing paint, then the
  // loop tears itself down (raf → 0, state → null). Suspends mid-run if hidden.
  const armSway = useCallback(() => {
    if (graphStyleRef.current !== 'bonsai') return;
    if (!swayEnabled(reducedMotionRef.current)) return;
    if (typeof document !== 'undefined' && document.hidden) return;
    if (swayArmTimerRef.current !== 0) {
      clearTimeout(swayArmTimerRef.current);
      swayArmTimerRef.current = 0;
    }
    swayStateRef.current = { start: performance.now() };
    const tick = (): void => {
      const st = swayStateRef.current;
      if (st === null) return;
      if (typeof document !== 'undefined' && document.hidden) {
        swayStateRef.current = null;
        swayRafRef.current = 0;
        return;
      }
      const elapsed = performance.now() - st.start;
      paint();
      if (elapsed >= SWAY_DURATION_MS) {
        swayStateRef.current = null;
        paint(); // final clearing paint at offset 0
        swayRafRef.current = 0;
        return;
      }
      swayRafRef.current = requestAnimationFrame(tick);
    };
    if (swayRafRef.current !== 0) cancelAnimationFrame(swayRafRef.current);
    swayRafRef.current = requestAnimationFrame(tick);
  }, [paint, graphStyleRef, reducedMotionRef, swayStateRef]);

  const onScroll = useCallback(() => {
    if (swayStateRef.current !== null) {
      if (swayRafRef.current !== 0) {
        cancelAnimationFrame(swayRafRef.current);
        swayRafRef.current = 0;
      }
      swayStateRef.current = null;
    }
    if (graphStyleRef.current === 'bonsai' && swayEnabled(reducedMotionRef.current)) {
      if (swayArmTimerRef.current !== 0) clearTimeout(swayArmTimerRef.current);
      swayArmTimerRef.current = window.setTimeout(() => {
        swayArmTimerRef.current = 0;
        armSway();
      }, SWAY_ARM_DELAY_MS);
    }
  }, [armSway, graphStyleRef, reducedMotionRef, swayStateRef]);

  // Arm on selection change (skip mount so opening a repo doesn't sway).
  const selMountRef = useRef(false);
  useEffect(() => {
    if (!selMountRef.current) {
      selMountRef.current = true;
      return;
    }
    armSway();
  }, [selectedIndex, armSway]);

  // Drop a running settle when hidden; cancel everything on unmount.
  useEffect(() => {
    const onVisibility = (): void => {
      if (document.hidden) cancelSway();
    };
    document.addEventListener('visibilitychange', onVisibility);
    return () => {
      document.removeEventListener('visibilitychange', onVisibility);
      cancelSway();
    };
  }, [cancelSway]);

  // Leaving Bonsai or enabling reduced motion stops any live settle at once.
  useEffect(() => {
    if (graphStyle !== 'bonsai' || reducedMotion) cancelSway();
  }, [graphStyle, reducedMotion, cancelSway]);

  return { onScroll };
}
