import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { GraphLayout, RefLabel, VerifyStatus } from '../ipc';
import { resolveTheme } from './colors';
import type { Theme } from './colors';
import {
  avatarColor,
  avatarHit,
  drawGraph,
  drawHeadEdgeMarker,
  drawHeadGuide,
  drawWipRow,
  initials,
  laneX,
  refColArea,
  relativeDate,
} from './draw';
import type { WipSummary } from './draw';
import { entityStyle, groupRefs, layoutRefLabels } from './refLabels';
import { formatAbsolute } from './dates';
import {
  chipHitAt,
  fallbackBranchRef,
  hitTestRow,
  pillHitAt,
  prBadgeHitAt,
  sameTarget,
  signalHitAt,
  targetRefOf,
} from './hitTest';
import type { TooltipState } from './hitTest';
import {
  backingStoreSize,
  clampTooltipPos,
  headGuide,
  scrollRowIntoView,
  spacerHeight,
  visibleRowCount,
  visibleRowRange,
} from './viewport';
import { runP7SelfTest } from './selfTest';
import { computeRightColumns } from './rightColumns';
import type { GraphDisplayOptions } from './rightColumns';
import type { P7SelfTestResult } from './frameStats';
import { buildEdgeIndex, edgesInRange } from './edgeIndex';
import type { IncrementalEdgeIndex } from './incrementalEdgeIndex';
import { createFrameRecorder } from './frameStats';
import type { FrameStats } from './frameStats';
import type { EffectiveMetrics } from './metrics';
import type { RevealFlash } from './reveal';
import { flashAlpha, flashRingRadius } from './revealFlash';
import { startRevealFlash } from './revealFlashRunner';

export type { WipSummary };

/** True when a `#rrggbb` background reads as dark (relative luminance < 0.5).
 *  Non-hex input defaults to dark (the app's default theme). */
function isDarkBg(hex: string): boolean {
  const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
  if (m === null) return true;
  const v = parseInt(m[1], 16);
  const r = ((v >> 16) & 0xff) / 255;
  const g = ((v >> 8) & 0xff) / 255;
  const b = (v & 0xff) / 255;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5;
}

/** Right-click target on the graph: a ref pill, or a bare commit row. */
export type GraphContextTarget =
  | { kind: 'ref'; ref: RefLabel; oid: string }
  | { kind: 'commit'; index: number; oid: string };

export interface GraphCanvasProps {
  layout: GraphLayout;
  selectedIndex: number | null;
  /** Clicking a row toggles it; empty area below the rows selects null. */
  onSelect(index: number | null): void;
  /** P1 §9: non-null when the workdir has changes — renders a frontend-
   *  composited WIP row atop the (unchanged) Rust layout, +1 row offset. */
  wip: WipSummary | null;
  /** P2b §4.4: incremented by App on every theme change — forces a
   *  `resolveTheme` re-run (colors are otherwise cached for the component's
   *  lifetime) followed by a repaint. Lane palette itself is theme-invariant. */
  themeVersion: number;
  /** P3e §5.4: false when the owning tab is display:none (zero-size). Defaults
   *  true. When it flips true the canvas remeasures + repaints from the retained
   *  last-good bitmap (the zero-size guard in resize() kept it intact). */
  active?: boolean;
  /** P5 §4.2: right-click on a ref pill or a commit row. Empty area / WIP row →
   *  not called (the native menu is suppressed regardless). clientX/clientY
   *  anchor the context menu. */
  onContextMenu?(target: GraphContextTarget, clientX: number, clientY: number): void;
  /** P11d §4.3: effective render geometry (METRICS overlaid with the user's
   *  graph knobs). Drives every dot/avatar/row/lane pixel in the draw pass. */
  metrics: EffectiveMetrics;
  /** P11d §4.3: bumped when any graph knob changes → forces a full re-measure +
   *  repaint (analogous to `themeVersion`). */
  metricsVersion: number;
  /** P50b: row indices carrying a commit-search match → an outer match ring on
   *  those dots. Empty/absent when search is closed (no ring pass). */
  matchRows?: readonly number[];
  /** P51b: persisted per-row display toggles (SHA/author/date column + date
   *  basis, ahead/behind data). Fed straight into `drawGraph` and the date-
   *  column hover hit-test; a new object identity triggers a repaint. */
  display: GraphDisplayOptions;
  /** P58c: oid → signature verdict for the LIT badge (visible rows only, cached
   *  by oid in `useCommitVerification`). Absent/missing oid ⇒ the faint P51
   *  stub. A new map identity triggers a repaint so badges light in place. */
  verifyStatus?: ReadonlyMap<string, VerifyStatus>;
  /** P58c: fired once per paint after the visible window is computed (only when
   *  first/last changed). Drives the debounced verify request for exactly the
   *  visible (overscanned) rows — the badge is virtualized. */
  onVisibleRangeChange?(first: number, last: number): void;
  /** P63: a PR badge on a branch-tip pill was clicked → open that PR in the
   *  right-pane PR panel. When absent, PR-badge clicks fall through to the
   *  normal row-select. */
  onOpenPr?(number: number): void;
  /** P65b (streamed path): the incremental edge index owned by the stream
   *  assembler. When present it REPLACES the internal `buildEdgeIndex(layout)`
   *  memo (which would be O(n) per streamed batch). Absent ⇒ one-shot path,
   *  byte-for-byte unchanged. */
  edgeIndex?: IncrementalEdgeIndex;
  /** P65b (streamed path): total row count for the scroll extent while rows are
   *  still arriving. Absent ⇒ the spacer uses `layout.nodes.length` (one-shot /
   *  grow-as-you-go). */
  totalRows?: number;
  /** P84: nonce-driven reveal flash. A NEW `nonce` (re)starts the row-pulse +
   *  dot-halo highlight on `index`; `null`/absent means no flash. Nonce-driven so
   *  re-revealing the already-selected row re-flashes. */
  revealFlash?: RevealFlash | null;
  /** P84: `prefers-reduced-motion` (read once in the container). When true the
   *  flash is a static hold, not an animated pulse (revealFlash.ts §3.1). */
  reducedMotion?: boolean;
}

/** P2c §5.2: imperative escape hatch — App needs the DOM-measured visible row
 *  count for PageUp/PageDown deltas, which App has no other way to learn
 *  without duplicating a ResizeObserver of its own. Pure view-layer index
 *  arithmetic downstream — no lane/edge math involved. */
export interface GraphCanvasHandle {
  getVisibleRowCount(): number;
}

const MOCK_MODE = import.meta.env.VITE_MOCK_IPC === '1';
const STATS_ENABLED = import.meta.env.DEV || MOCK_MODE;
/** Rows painted beyond the visible window on each side (§4.2). */
const OVERSCAN = 4;
/** Scroll activity window for inter-frame gap recording (§4.7). */
const SCROLL_ACTIVE_MS = 200;
/** Log a `[bonsai] frames` summary every this many recorded frames. */
const LOG_EVERY = 120;

/**
 * M2c scroll model (contract §4.1): fixed viewport-sized canvas (output only)
 * under a transparent overlay scroller whose spacer div provides the native
 * scrollbar. The scroller owns ALL input; scroll events only record scrollTop
 * and schedule one rAF paint. Initial/resize/data-driven paints stay
 * synchronous (rAF is throttled to zero in hidden windows).
 */
export const GraphCanvas = forwardRef<GraphCanvasHandle, GraphCanvasProps>(function GraphCanvas(
  {
    layout,
    selectedIndex,
    onSelect,
    wip,
    themeVersion,
    active = true,
    onContextMenu,
    metrics,
    metricsVersion,
    matchRows,
    display,
    verifyStatus,
    onVisibleRangeChange,
    onOpenPr,
    edgeIndex,
    totalRows,
    revealFlash,
    reducedMotion = false,
  },
  ref,
) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const themeRef = useRef<Theme | null>(null);
  // P11d §4.3: latest effective metrics, read by the per-frame paint + hit-test
  // paths (mirror of themeRef) so they never close over a stale knob set.
  const metricsRef = useRef(metrics);
  metricsRef.current = metrics;
  // P84: latest reduced-motion flag, read by the paint + flash rAF loop.
  const reducedMotionRef = useRef(reducedMotion);
  reducedMotionRef.current = reducedMotion;
  /** Row index, `null` (none), or `-1` sentinel for the synthetic WIP row. */
  const hoverRowRef = useRef<number | null>(null);
  const rafRef = useRef(0);
  // P84: active reveal flash — the target row + animation start timestamp; null
  // when no flash is running. Its own rAF handle (separate from the scroll rAF).
  const flashStateRef = useRef<{ row: number; start: number } | null>(null);
  const flashRafRef = useRef(0);
  const flashTimeoutRef = useRef(0);
  const scrollTopRef = useRef(0);
  const cssSizeRef = useRef({ w: 0, h: 0 });
  /** Cursor y relative to the scroller top; null while the pointer is outside. */
  const mouseYRef = useRef<number | null>(null);
  /** P7 §6.1: cursor x relative to the scroller left (mirror of mouseYRef); lets
   *  scroll re-runs recompute the hover target without a fresh mouse event. */
  const mouseXRef = useRef<number | null>(null);
  const lastScrollTsRef = useRef(Number.NEGATIVE_INFINITY);
  const prevFrameTsRef = useRef<number | null>(null);
  // Two recorders (P1 §4.7): paint durations and scroll inter-frame gaps are
  // different quantities — mixing them made `avg` meaningless.
  const paintRecorderRef = useRef(createFrameRecorder());
  const paintCountRef = useRef(0);
  const gapRecorderRef = useRef(createFrameRecorder());
  const gapCountRef = useRef(0);
  const firstDataPaintSkippedRef = useRef(false);

  // P7 §6: hover tooltip. State changes ONLY when the hover TARGET changes (the
  // sameTarget guard), so re-renders are rare and the per-frame canvas paint
  // path (paintNow) never depends on tooltip state. `tipRef` measures the DOM
  // node in a layout effect; `tipPos` holds the clamped {left,top} once measured
  // (initial render uses the un-clamped anchor point — see the layout effect).
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const [tipPos, setTipPos] = useState<{ left: number; top: number } | null>(null);

  useImperativeHandle(
    ref,
    () => ({
      getVisibleRowCount: () =>
        visibleRowCount(cssSizeRef.current.h, metricsRef.current.rowHeight),
    }),
    [],
  );

  // Edge culling index, built once per layout object (§4.4). P65b: on the
  // streamed path the assembler supplies `edgeIndex` (its own incremental index),
  // so we skip the internal build entirely — otherwise it would be an O(n)
  // rebuild on every streamed batch (layout identity bumps per batch).
  const memoIndex = useMemo(
    () => (edgeIndex !== undefined ? null : buildEdgeIndex(layout)),
    [layout, edgeIndex],
  );

  // P50b: search-match set, rebuilt once per matchRows prop change (not per
  // frame). null when there are no matches so the draw pass skips the ring.
  const matchSet = useMemo(
    () => (matchRows !== undefined && matchRows.length > 0 ? new Set(matchRows) : null),
    [matchRows],
  );

  // Latest props for the stable paint callback. `edgeIndex` is the streamed
  // incremental index (or undefined); `memoIndex` is the one-shot index (or null
  // when streamed) — paintNow picks whichever is present (§4.3).
  const propsRef = useRef({
    layout,
    selectedIndex,
    edgeIndex,
    memoIndex,
    wip,
    matchSet,
    display,
    verifyStatus,
    onVisibleRangeChange,
  });
  propsRef.current = {
    layout,
    selectedIndex,
    edgeIndex,
    memoIndex,
    wip,
    matchSet,
    display,
    verifyStatus,
    onVisibleRangeChange,
  };

  // P58c: last visible window reported to onVisibleRangeChange — guards
  // redundant fires (only when first/last actually changed).
  const lastRangeRef = useRef<{ first: number; last: number } | null>(null);

  const recordFrame = useCallback((kind: 'paint' | 'gap', durMs: number) => {
    const rec = kind === 'paint' ? paintRecorderRef.current : gapRecorderRef.current;
    const countRef = kind === 'paint' ? paintCountRef : gapCountRef;
    rec.record(durMs);
    if (++countRef.current >= LOG_EVERY) {
      countRef.current = 0;
      const s = rec.flushSummary();
      if (import.meta.env.DEV) {
        console.log(
          `[bonsai] frames kind=${kind} n=${s.frames} avg=${s.avgMs.toFixed(1)}ms ` +
            `max=${s.maxMs.toFixed(1)}ms >33ms=${s.over33}`,
        );
      }
    }
  }, []);

  const paintNow = useCallback(() => {
    // Direct calls supersede any pending rAF repaint.
    if (rafRef.current !== 0) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = 0;
    }
    const canvas = canvasRef.current;
    if (canvas === null) return;
    const ctx = canvas.getContext('2d');
    if (ctx === null) return;
    themeRef.current ??= resolveTheme(canvas);

    const t0 = STATS_ENABLED ? performance.now() : 0;
    const m = metricsRef.current;
    const rowHeight = m.rowHeight;
    const {
      layout: lay,
      selectedIndex: sel,
      edgeIndex: incIx,
      memoIndex: memoIx,
      wip,
      matchSet,
      display,
      verifyStatus,
      onVisibleRangeChange,
    } = propsRef.current;
    const { w, h } = cssSizeRef.current;
    const scrollTop = scrollerRef.current?.scrollTop ?? scrollTopRef.current;
    scrollTopRef.current = scrollTop;
    const n = lay.nodes.length;
    const wipOffset = wip !== null ? 1 : 0;
    const { firstRow, lastRow, layoutScrollTop } = visibleRowRange(
      scrollTop,
      wipOffset,
      rowHeight,
      h,
      n,
      OVERSCAN,
    );
    // P58c: report the (overscanned) visible window ONCE per change so the
    // verify hook fetches badges for exactly these rows. Fires after the window
    // is computed; guarded so a redundant same-range paint does not re-request.
    if (onVisibleRangeChange !== undefined && n > 0) {
      const prev = lastRangeRef.current;
      if (prev === null || prev.first !== firstRow || prev.last !== lastRow) {
        lastRangeRef.current = { first: firstRow, last: lastRow };
        onVisibleRangeChange(firstRow, lastRow);
      }
    }
    const hoverRow = hoverRowRef.current !== null && hoverRowRef.current >= 0 ? hoverRowRef.current : null;
    // P7e §13.2: reserve the native vertical-scrollbar width on the right (0 when
    // no scrollbar is present — dynamic).
    const rightInset = scrollerRef.current
      ? scrollerRef.current.offsetWidth - scrollerRef.current.clientWidth
      : 0;
    // §4.3: streamed path queries the assembler's incremental index; one-shot
    // path uses the (from,to)-sorted memo. When both are absent (empty layout
    // guard) nothing is drawn — identical to the prior one-shot behavior.
    const visibleEdges = incIx
      ? incIx.edgesInRange(firstRow, lastRow)
      : memoIx !== null
        ? edgesInRange(lay, memoIx, firstRow, lastRow)
        : [];
    // P84: compute the reveal flash (row-bg pulse + dot halo) for this frame from
    // the animation start timestamp; the rAF loop below drives repaints.
    let flash: { row: number; alpha: number; ringRadius: number } | null = null;
    const fs = flashStateRef.current;
    if (fs !== null) {
      const elapsed = performance.now() - fs.start;
      const dark = isDarkBg(themeRef.current.bg0);
      const alpha = flashAlpha(elapsed, dark, reducedMotionRef.current);
      if (alpha > 0) {
        flash = {
          row: fs.row,
          alpha,
          ringRadius: flashRingRadius(elapsed, m.avatarSelRingRadius, reducedMotionRef.current),
        };
      }
    }
    drawGraph(
      ctx,
      lay,
      visibleEdges,
      { firstRow, lastRow, scrollTop: layoutScrollTop, width: w, height: h, rightInset },
      themeRef.current,
      { hoverRow, selectedIndex: sel, matchRows: matchSet, verifyStatus: verifyStatus ?? null, flash },
      display,
      m,
    );
    // P67 §1: the dashed HEAD guideline is INDEPENDENT of the WIP row's near-top
    // gate — it must point at the checked-out commit at every scroll position.
    // Drawn before drawWipRow so the dashed WIP marker circle paints on top.
    const guide = headGuide({
      headIndex: lay.headIndex,
      layoutScrollTop,
      wipOffset,
      rowHeight,
      avatarRadius: m.avatarRadius,
      ringExtra: m.avatarBgRingExtra,
      viewportHeight: h,
    });
    if (guide !== null) {
      // A5 (§1.1a): a collapsed segment still carries an edge — the marker is
      // drawn alone so the guide never vanishes once the user scrolls past HEAD.
      // §1.3 calls the guide unconditionally; `drawHeadGuide` itself no-ops when
      // `segment === false` (single owner of that check — no duplicate here).
      drawHeadGuide(ctx, lay, guide, themeRef.current, m);
      if (guide.edge !== null) drawHeadEdgeMarker(ctx, lay, guide, h, themeRef.current, m);
    }
    if (wip !== null && scrollTop < rowHeight + 56) {
      drawWipRow(
        ctx,
        lay,
        wip,
        { firstRow: 0, lastRow: 0, scrollTop, width: w, height: h },
        themeRef.current,
        hoverRowRef.current === -1,
        m,
      );
    }
    if (STATS_ENABLED) recordFrame('paint', performance.now() - t0);
  }, [recordFrame]);

  const paintFrame = useCallback(
    (ts: number) => {
      rafRef.current = 0;
      if (STATS_ENABLED) {
        // Record inter-frame gaps while scroll activity is ongoing (§4.7).
        const scrolling = performance.now() - lastScrollTsRef.current < SCROLL_ACTIVE_MS;
        if (scrolling && prevFrameTsRef.current !== null) {
          recordFrame('gap', ts - prevFrameTsRef.current);
        }
        prevFrameTsRef.current = scrolling ? ts : null;
      }
      paintNow();
    },
    [paintNow, recordFrame],
  );

  const schedulePaint = useCallback(() => {
    if (rafRef.current === 0) rafRef.current = requestAnimationFrame(paintFrame);
  }, [paintFrame]);

  // Backing store = css size × dpr; transform set once here, not per paint.
  const resize = useCallback(() => {
    const host = hostRef.current;
    const canvas = canvasRef.current;
    if (host === null || canvas === null) return;
    const cssW = host.clientWidth;
    const cssH = host.clientHeight;
    // P3e §5.4: hidden tab (display:none) → zero client rect. Bail before
    // touching the backing store or painting: shrinking to 1×1 or repainting
    // here would blank the last-good bitmap, which must survive being hidden so
    // the graph is still there when the tab is shown again (remeasure effect).
    if (cssW === 0 || cssH === 0) return;
    cssSizeRef.current = { w: cssW, h: cssH };
    const dpr = window.devicePixelRatio || 1;
    const store = backingStoreSize(cssW, cssH, dpr);
    canvas.style.width = `${cssW}px`;
    canvas.style.height = `${cssH}px`;
    canvas.width = store.width;
    canvas.height = store.height;
    const ctx = canvas.getContext('2d');
    if (ctx !== null) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    // Canvas resize just cleared the bitmap — always repaint synchronously.
    paintNow();
  }, [paintNow]);

  // Mount: ResizeObserver on the host + DPR-change handling (re-armed
  // matchMedia listener, §4.3). resize() also performs the initial paint.
  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    resize();
    const ro = new ResizeObserver(() => resize());
    ro.observe(host);

    let mq: MediaQueryList | null = null;
    const onDprChange = (): void => {
      resize();
      arm();
    };
    const arm = (): void => {
      mq?.removeEventListener('change', onDprChange);
      mq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      mq.addEventListener('change', onDprChange);
    };
    arm();

    return () => {
      ro.disconnect();
      mq?.removeEventListener('change', onDprChange);
      if (rafRef.current !== 0) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
    };
  }, [resize]);

  // P3e §5.4: authoritative remeasure-on-show. When `active` flips true (tab
  // shown after display:none), re-run the SAME `resize()` the ResizeObserver
  // uses — it re-reads the now-nonzero host size, restores the backing-store
  // dimensions, and repaints synchronously. ResizeObserver is unreliable across
  // the display:none→shown transition, so this is the trusted path; the observer
  // stays as the steady-state handler. The initial mount run is skipped so we
  // don't double-paint over the mount effect's resize() when already active.
  const activeMountRef = useRef(false);
  useEffect(() => {
    if (!activeMountRef.current) {
      activeMountRef.current = true;
      return;
    }
    if (active) resize();
  }, [active, resize]);

  // Layout/selection changes repaint synchronously; the mount paint already
  // happened inside resize() above (single mount paint — no double paint).
  useEffect(() => {
    if (!firstDataPaintSkippedRef.current) {
      firstDataPaintSkippedRef.current = true;
      return;
    }
    paintNow();
  }, [paintNow, layout, selectedIndex, wip, matchSet, display, verifyStatus]);

  // P2b §4.4: theme changes re-resolve the cached CSS-variable colors and
  // repaint. Runs once on mount too (themeVersion starts at 0), which is
  // harmless — resize()'s initial paint already resolved the theme via the
  // `??=` fallback in paintNow, so this is a cheap re-resolve, not a second
  // distinct paint pathway.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    themeRef.current = resolveTheme(canvas);
    schedulePaint();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeVersion]);

  // P11d §4.3: a graph-knob change re-maps every row↔pixel relationship. The
  // spacer height (total scrollable extent) recomputes on render from the new
  // `metrics` prop; here we re-run the SAME `resize()` path (re-measure the host,
  // reset the HiDPI backing store, synchronous repaint) so virtualization + the
  // scroll extent line up with the new rowHeight/lane geometry. Mirrors the
  // `themeVersion` effect. The mount run is skipped (mount's resize() already
  // painted with the initial metrics — no double paint).
  const metricsMountRef = useRef(false);
  useEffect(() => {
    if (!metricsMountRef.current) {
      metricsMountRef.current = true;
      return;
    }
    resize();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [metricsVersion]);

  // P1 §6.3/§9.3: when selectedIndex changes to non-null (e.g. via ArrowUp/
  // Down in App), bring the row into view if it's outside the visible window.
  // Pure scroll adjustment — row position accounts for the WIP row offset:
  // target y = (row + wipOffset) * rowHeight.
  useEffect(() => {
    if (selectedIndex === null) return;
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const next = scrollRowIntoView(
      selectedIndex,
      wip !== null ? 1 : 0,
      metricsRef.current.rowHeight,
      scroller.scrollTop,
      scroller.clientHeight,
    );
    if (next !== null) scroller.scrollTop = next;
  }, [selectedIndex, wip]);

  // P84: nonce-driven reveal flash. A new `revealFlash.nonce` (re)starts the
  // flash on `revealFlash.index`; the runner handles both motion modes and
  // returns the cleanup. See `revealFlashRunner.ts`.
  const flashNonce = revealFlash?.nonce ?? null;
  const flashRow = revealFlash?.index ?? null;
  useEffect(() => {
    if (flashNonce === null || flashRow === null) return;
    return startRevealFlash(flashRow, reducedMotionRef.current, paintNow, {
      state: flashStateRef,
      raf: flashRafRef,
      timeout: flashTimeoutRef,
    });
  }, [flashNonce, flashRow, paintNow]);

  // P7 §6.2: clamp the tooltip inside the host. Runs synchronously after the
  // tooltip renders (at its un-clamped anchor point) but before paint, so the
  // correction is flicker-free. Default below the anchor; flip above / pull left
  // when it would overflow the host edges.
  useLayoutEffect(() => {
    if (tooltip === null) {
      setTipPos(null);
      return;
    }
    const tip = tipRef.current;
    const host = hostRef.current;
    if (tip === null || host === null) return;
    setTipPos(
      clampTooltipPos(
        tooltip.anchor,
        tip.offsetWidth,
        tip.offsetHeight,
        host.clientWidth,
        host.clientHeight,
      ),
    );
  }, [tooltip]);

  // Mock-mode dev hook: programmatic scroll sweep with frame timing (§4.7).
  useEffect(() => {
    if (!MOCK_MODE) return;
    const scrollSweep = (durationMs = 10000): Promise<FrameStats> =>
      new Promise((resolve) => {
        const scroller = scrollerRef.current;
        const rec = createFrameRecorder();
        if (scroller === null) {
          resolve(rec.flushSummary());
          return;
        }
        const maxTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
        const start = performance.now();
        let prev = start;
        const step = (ts: number): void => {
          rec.record(ts - prev);
          prev = ts;
          const t = Math.min(1, (ts - start) / durationMs);
          scroller.scrollTop = (t < 0.5 ? t * 2 : (1 - t) * 2) * maxTop;
          if (t < 1) {
            requestAnimationFrame(step);
          } else {
            const stats = rec.flushSummary();
            if (import.meta.env.DEV) console.log(`[bonsai] scroll-test ${JSON.stringify(stats)}`);
            resolve(stats);
          }
        };
        requestAnimationFrame(step);
      });

    // P7 §10 item 2: expose the pure helpers + a self-test (mock only, mirroring
    // scrollSweep). The orchestrator reads `window.__bonsai.p7SelfTest()`.
    // T3.6: the self-test body lives in selfTest.ts now (moved verbatim).
    // P67 §1.5: `headGuide` joins the bag — the only way to assert the guideline
    // geometry from a headless pane (no canvas pixel is ever produced there).
    const p7 = { initials, avatarColor, groupRefs, layoutRefLabels, refColArea, avatarHit, relativeDate, headGuide };
    const p7SelfTest = (): P7SelfTestResult => runP7SelfTest(canvasRef.current);

    window.__bonsai = { scrollSweep, p7, p7SelfTest };
    return () => {
      if (window.__bonsai?.scrollSweep === scrollSweep) delete window.__bonsai;
    };
  }, []);

  /** Hover-ref encoding: row index, `-1` for the WIP row, or `null`. */
  const hitTestAtMouseY = (yCss: number, scrollTop: number): number | null => {
    const { layout: lay, wip } = propsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTestRow(yCss, scrollTop, wipOffset, lay.nodes.length, metricsRef.current.rowHeight);
    return hit === 'wip' ? -1 : hit;
  };

  // P7 §6.1: resolve the hover tooltip target from a cursor position (scroller/
  // host CSS coords). Avatar disc → author-name tooltip; a "+n" chip in the LEFT
  // ref column → the hidden-entity list. Pure over current props/refs; returns
  // null for empty area, the WIP row, or when no ctx/theme is available.
  const computeHoverTarget = (x: number, y: number, scrollTop: number): TooltipState | null => {
    const { layout: lay, wip, display } = propsRef.current;
    const row = hitTestAtMouseY(y, scrollTop);
    if (row === null || row < 0) return null; // none, or the WIP row (-1)
    const node = lay.nodes[row];
    const ctx = canvasRef.current?.getContext('2d') ?? null;
    const theme = themeRef.current;
    if (ctx === null || theme === null) return null;
    const m = metricsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const cy = (row + wipOffset) * m.rowHeight + m.rowHeight / 2 - scrollTop;
    const cx = laneX(node.lane, m);
    if (avatarHit(x, y, cx, cy, m)) {
      const r = m.avatarRadius + m.avatarBgRingExtra;
      return {
        kind: 'avatar',
        text: node.author,
        anchor: { left: cx - r, top: cy - r, width: 2 * r, height: 2 * r },
      };
    }
    if (node.refs !== undefined && node.refs.length > 0 && x < m.refColWidth) {
      const { startX, budget } = refColArea(m);
      const entities = groupRefs(node.refs);
      const laid = layoutRefLabels(ctx, entities, node, theme, startX, budget, display);
      const chip = chipHitAt(laid, x);
      if (chip !== undefined) {
        const shown = laid.filter((l) => l.entity !== null).length;
        const lines = entities.slice(shown).map((e) => entityStyle(e, node, theme).label);
        return {
          kind: 'overflow',
          lines,
          anchor: { left: chip.x, top: cy - m.pillHeight / 2, width: chip.w, height: m.pillHeight },
        };
      }
      // §14.2: hovering a SHOWN branch pill → full branch-name tooltip.
      // Precedence: avatar (earlier) → chip (above) → shown pill.
      const hitLabel = pillHitAt(laid, x);
      if (hitLabel !== undefined && hitLabel.entity !== null && hitLabel.entity.kind === 'branch') {
        return {
          kind: 'ref',
          text: hitLabel.entity.name,
          anchor: { left: hitLabel.x, top: cy - m.pillHeight / 2, width: hitLabel.w, height: m.pillHeight },
        };
      }
      // P63: forge-signal badges — precedence AFTER the shown pill (the signal
      // rects sit to the right of the pill body, so they never overlap it).
      const sig = signalHitAt(laid, x, m.ciBadgeSize);
      if (sig !== null && sig.kind === 'pr') {
        const pr = sig.pr;
        const state = pr.badge.isDraft ? 'draft' : pr.badge.state;
        return {
          kind: 'pr',
          lines: [`PR #${pr.badge.number} (${state})`, pr.badge.title],
          anchor: { left: pr.x, top: cy - m.pillHeight / 2, width: pr.w, height: m.pillHeight },
        };
      }
      if (sig !== null && sig.kind === 'ci') {
        const ci = sig.ci;
        const half = m.ciBadgeSize / 2;
        const b = ci.badge;
        return {
          kind: 'ci',
          lines: [`Checks: ${b.passed} passed, ${b.failed} failed, ${b.pending} pending`],
          anchor: { left: ci.cx - half, top: cy - half, width: m.ciBadgeSize, height: m.ciBadgeSize },
        };
      }
    }
    // P51b: hovering the date column → FULL absolute timestamps (authored +
    // committed), one per line; the inline date stays relative. Recompute the
    // column geometry with the SAME pure helper the draw pass uses so the hit
    // box matches the drawn column exactly. (`display` is read at the top.)
    const rightInset = scrollerRef.current
      ? scrollerRef.current.offsetWidth - scrollerRef.current.clientWidth
      : 0;
    const effRight = cssSizeRef.current.w - rightInset;
    const cols = computeRightColumns(effRight, display, m);
    if (cols.date !== null && x >= cols.date.leftX && x <= cols.date.rightX) {
      return {
        kind: 'date',
        lines: [`Authored  ${formatAbsolute(node.ts)}`, `Committed ${formatAbsolute(node.committerTs)}`],
        anchor: {
          left: cols.date.leftX,
          top: cy - m.pillHeight / 2,
          width: cols.date.width,
          height: m.pillHeight,
        },
      };
    }
    return null;
  };

  // Scroll handler ONLY records scrollTop and schedules one rAF paint (§4.1).
  const handleScroll = () => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    scrollTopRef.current = scroller.scrollTop;
    lastScrollTsRef.current = performance.now();
    // Rows move under a stationary cursor while wheel-scrolling.
    if (mouseYRef.current !== null) {
      hoverRowRef.current = hitTestAtMouseY(mouseYRef.current, scroller.scrollTop);
      // P7 §6.1: keep the tooltip in sync as rows scroll under the cursor.
      if (mouseXRef.current !== null) {
        const next = computeHoverTarget(mouseXRef.current, mouseYRef.current, scroller.scrollTop);
        setTooltip((prev) => (sameTarget(prev, next) ? prev : next));
      }
    }
    schedulePaint();
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const rect = scroller.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const x = e.clientX - rect.left;
    mouseYRef.current = y;
    mouseXRef.current = x;
    const row = hitTestAtMouseY(y, scroller.scrollTop);
    if (row !== hoverRowRef.current) {
      hoverRowRef.current = row;
      schedulePaint(); // repaint only when the hovered row changed, via rAF
    }
    // P7 §6.1: recompute the hover tooltip target; setTooltip only fires on a
    // real target change (sameTarget), so this is not a per-frame React churn.
    const next = computeHoverTarget(x, y, scroller.scrollTop);
    setTooltip((prev) => (sameTarget(prev, next) ? prev : next));
  };

  const handleMouseLeave = () => {
    mouseYRef.current = null;
    mouseXRef.current = null;
    setTooltip(null); // P7 §6.2: dismiss on leave
    if (hoverRowRef.current !== null) {
      hoverRowRef.current = null;
      schedulePaint();
    }
  };

  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const rect = scroller.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const x = e.clientX - rect.left;
    const m = metricsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTestRow(y, scroller.scrollTop, wipOffset, layout.nodes.length, m.rowHeight);
    if (hit === null || hit === 'wip') {
      onSelect(null);
      return;
    }
    // P63: a PR badge in the LEFT ref band → open that PR (do NOT select the
    // row). Recompute the row's laid labels with the SAME pure helper the draw
    // pass + ref hit-tests use, so the signal rects match the pixels exactly.
    const node = layout.nodes[hit];
    if (onOpenPr !== undefined && x < m.refColWidth && node.refs !== undefined && node.refs.length > 0) {
      const ctx = canvasRef.current?.getContext('2d') ?? null;
      const theme = themeRef.current;
      if (ctx !== null && theme !== null) {
        const { startX, budget } = refColArea(m);
        const laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget, display);
        const prHit = prBadgeHitAt(laid, x);
        if (prHit !== null) {
          onOpenPr(prHit.badge.number);
          return;
        }
      }
    }
    onSelect(hit === selectedIndex ? null : hit);
  };

  // P5 §4.2 / P7 §5: right-click hit-test. Always suppress the native menu over
  // the graph; then resolve a ref label in the LEFT ref column (via the shared
  // layoutRefLabels layout — single source of truth with the draw pass) or fall
  // through to the commit row. Parity with P6 is preserved: the emitted
  // GraphContextTarget shape is unchanged; only the ref RESOLUTION moved left.
  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const rect = scroller.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const x = e.clientX - rect.left;
    const m = metricsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTestRow(y, scroller.scrollTop, wipOffset, layout.nodes.length, m.rowHeight);
    if (hit === null || hit === 'wip') return;
    const node = layout.nodes[hit];
    const ctx = canvasRef.current?.getContext('2d') ?? null;
    const theme = themeRef.current;
    if (
      ctx !== null &&
      theme !== null &&
      x < m.refColWidth &&
      node.refs !== undefined &&
      node.refs.length > 0
    ) {
      const { startX, budget } = refColArea(m);
      const laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget, display);
      const hitLabel = pillHitAt(laid, x);
      if (hitLabel !== undefined && hitLabel.entity !== null) {
        const ref = targetRefOf(hitLabel.entity);
        if (ref !== null) {
          onContextMenu?.({ kind: 'ref', ref, oid: node.id }, e.clientX, e.clientY);
          return;
        }
        // tag/head resolve to a ref whose branchMenuItems is [] → no menu opens;
        // fall through to the whole-row / commit target (matches today's behavior).
      }
    }
    // P18b: whole-row branch fallback. If no SPECIFIC pill was hit (e.g. the
    // click landed on the dot/avatar/summary, or right of the ref band), but the
    // row carries a branch/remoteBranch, open that branch's menu (the superset).
    // Runs for ANY x — not gated on the ref band. Stash/tag/head-only rows and
    // ref-less rows fall through to the commit target (the precise hit-test above
    // already covered stash/tag pills inside the band).
    if (node.refs !== undefined && node.refs.length > 0) {
      const ref = fallbackBranchRef(groupRefs(node.refs));
      if (ref !== null) {
        onContextMenu?.({ kind: 'ref', ref, oid: node.id }, e.clientX, e.clientY);
        return;
      }
    }
    // Empty band OR the "+n" chip OR a non-branch entity → commit target.
    onContextMenu?.({ kind: 'commit', index: hit, oid: node.id }, e.clientX, e.clientY);
  };

  // P11d §4.3: spacer (total scroll extent) tracks the live rowHeight knob so
  // the scrollbar range re-maps on every graph-metric change. P65b: on the
  // streamed path `totalRows` extends the extent to the full repo while rows are
  // still arriving (grow-as-you-go); absent ⇒ layout.nodes.length (unchanged).
  const spacerH = spacerHeight(
    Math.max(layout.nodes.length, totalRows ?? 0),
    wip !== null ? 1 : 0,
    metrics.rowHeight,
  );

  return (
    <div ref={hostRef} className="graph-canvas-host">
      <canvas ref={canvasRef} className="graph-canvas" data-testid="graph-canvas" />
      <div
        ref={scrollerRef}
        className="graph-scroll"
        data-testid="graph-scroller"
        onScroll={handleScroll}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        <div className="graph-spacer" style={{ height: `${spacerH}px` }} />
      </div>
      {tooltip !== null && (
        <div
          ref={tipRef}
          className="graph-tooltip"
          role="tooltip"
          style={{
            left: `${tipPos?.left ?? tooltip.anchor.left}px`,
            top: `${tipPos?.top ?? tooltip.anchor.top + tooltip.anchor.height + 4}px`,
          }}
        >
          {tooltip.kind === 'overflow' ||
          tooltip.kind === 'date' ||
          tooltip.kind === 'pr' ||
          tooltip.kind === 'ci'
            ? tooltip.lines.map((l, i) => <div key={i}>{l}</div>)
            : tooltip.text}
        </div>
      )}
    </div>
  );
});
