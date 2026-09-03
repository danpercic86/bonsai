/** `GraphCanvas`'s version-bump / selection-driven side effects, moved VERBATIM
 *  out of `GraphCanvas.tsx` (file-size ratchet — no behavior change). Each hook
 *  is called from the container at the EXACT position its `useEffect` occupied
 *  before, so the relative order in which these effects run is unchanged.
 *
 *  Mirrors `useCanvasResizeObserver.ts`, which was split off the same mount
 *  block earlier. Nothing here is on the per-frame paint path. */
import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import { resolveTheme } from './colors';
import type { GraphStyle, Theme } from './colors';
import type { WipSummary } from './draw';
import type { EffectiveMetrics } from './metrics';
import type { GraphSeason } from './palettes';
import { scrollRowIntoView } from './viewport';

// P3e §5.4: authoritative remeasure-on-show. When `active` flips true (tab
// shown after display:none), re-run the SAME `resize()` the ResizeObserver
// uses — it re-reads the now-nonzero host size, restores the backing-store
// dimensions, and repaints synchronously. ResizeObserver is unreliable across
// the display:none→shown transition, so this is the trusted path; the observer
// stays as the steady-state handler. The initial mount run is skipped so we
// don't double-paint over the mount effect's resize() when already active.
export function useRemeasureOnShow(
  active: boolean,
  resize: () => void,
  /** Spec-005 rail reveal state — reset when the pane goes hidden. */
  railReveal: { reset(): void },
): void {
  const activeMountRef = useRef(false);
  useEffect(() => {
    if (!activeMountRef.current) {
      activeMountRef.current = true;
      return;
    }
    if (active) resize();
    else railReveal.reset(); // spec-005: drop the rail's hover state when hidden
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, resize]);
}

// P2b §4.4: theme changes re-resolve the cached CSS-variable colors and
// repaint. Runs once on mount too (themeVersion starts at 0), which is
// harmless — resize()'s initial paint already resolved the theme via the
// `??=` fallback in paintNow, so this is a cheap re-resolve, not a second
// distinct paint pathway.
export function useThemeRepaint(
  themeVersion: number,
  graphStyle: GraphStyle,
  graphSeason: GraphSeason,
  canvasRef: RefObject<HTMLCanvasElement | null>,
  themeRef: RefObject<Theme | null>,
  schedulePaint: () => void,
): void {
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    themeRef.current = resolveTheme(canvas, graphStyle, graphSeason);
    schedulePaint();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeVersion, graphStyle, graphSeason]);
}

// P11d §4.3: a graph-knob change re-maps every row↔pixel relationship. The
// spacer height (total scrollable extent) recomputes on render from the new
// `metrics` prop; here we re-run the SAME `resize()` path (re-measure the host,
// reset the HiDPI backing store, synchronous repaint) so virtualization + the
// scroll extent line up with the new rowHeight/lane geometry. Mirrors the
// `themeVersion` effect. The mount run is skipped (mount's resize() already
// painted with the initial metrics — no double paint).
export function useRemeasureOnMetrics(metricsVersion: number, resize: () => void): void {
  const metricsMountRef = useRef(false);
  useEffect(() => {
    if (!metricsMountRef.current) {
      metricsMountRef.current = true;
      return;
    }
    resize();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [metricsVersion]);
}

// P1 §6.3/§9.3: when selectedIndex changes to non-null (e.g. via ArrowUp/
// Down in App), bring the row into view if it's outside the visible window.
// Pure scroll adjustment — row position accounts for the WIP row offset:
// target y = (row + wipOffset) * rowHeight.
// Spec-004: operates on DISPLAY rows — the fold model joins the deps because
// reveal-after-expand changes the mapping without changing `selectedIndex`.
// A keyboard-active pill row scrolls into view the same way (never selected).
export function useScrollSelectionIntoView(
  scrollTarget: number | null,
  wip: WipSummary | null,
  scrollerRef: RefObject<HTMLDivElement | null>,
  metricsRef: RefObject<EffectiveMetrics>,
): void {
  useEffect(() => {
    if (scrollTarget === null) return;
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const next = scrollRowIntoView(
      scrollTarget,
      wip !== null ? 1 : 0,
      metricsRef.current.rowHeight,
      scroller.scrollTop,
      scroller.clientHeight,
    );
    if (next !== null) scroller.scrollTop = next;
    // `scrollerRef` / `metricsRef` are stable ref objects the container owns;
    // the rule can no longer see that from here (they arrive as parameters), so
    // it is silenced rather than those deps being added.
    // The `wip` dep is deliberately the BOOLEAN the body actually consumes (the
    // one-row offset), not the object: a WIP *count* change (staging or editing
    // a file) must not re-run the scroll adjustment and jump the view.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrollTarget, wip !== null]);
}
