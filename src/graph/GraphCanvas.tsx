import {
  forwardRef,
  useCallback,
  useEffect,
  useId,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';
import { resolveTheme } from './colors';
import type { Theme } from './colors';
import { drawGraph, drawHeadEdgeMarker, drawHeadGuide, drawWipRow } from './draw';
import type { WipSummary } from './draw';
import { groupRefs } from './refLabels';
import { hitTestRow, sameTarget } from './hitTest';
import type { TooltipState } from './hitTest';
import {
  backingStoreSize,
  headGuide,
  spacerHeight,
  visibleRowCount,
  visibleRowRange,
} from './viewport';
import { edgesInRange } from './edgeIndex';
import { GraphKeyboardHint } from './GraphKeyboardHint';
import { useGraphRenderCount } from './graphObs';
import { chipHiddenEntitiesAt, resolveContextTarget, rowMenuAnchor } from './contextTarget';
import type { GraphContextTarget } from './contextTarget';
import { useMockDevHooks } from './useMockDevHooks';
// Spec-004: fold display-row model — projection, pill painting, mappings.
import { collapsePillHit, drawFoldRows } from './drawFold';
import { resolveGraphClick } from './graphClick';
import { activeRowA11y, foldCursorFor } from './foldView';
import { displayToModel } from './foldModel';
import { reportVisibleRange, resolveFlash, resolveSway } from './paintFx';
import { startRevealFlash } from './revealFlashRunner';
import { useSway } from './useSway';
import { resolveHoverTarget } from './hoverTarget';
import { GraphTooltipOverlay } from './GraphTooltipOverlay';
import { useCanvasResizeObserver } from './useCanvasResizeObserver';
// Spec-005: overview rail — mounted ONLY while visible (zero idle cost).
import { OverviewRail } from './rail/OverviewRail';
import { useRailReveal } from './rail/useRailReveal';
import { useFrameRecorder } from './useFrameRecorder';
import { useGraphDisplayModel } from './useGraphDisplayModel';
import {
  useRemeasureOnMetrics,
  useRemeasureOnShow,
  useScrollSelectionIntoView,
  useThemeRepaint,
} from './useGraphCanvasEffects';
import type { GraphCanvasHandle, GraphCanvasProps } from './graphCanvasProps';

export type { WipSummary };

// Right-click target on the graph — moved to contextTarget.ts (spec-004 size
// split); re-exported so existing import sites keep working.
export type { GraphContextTarget };

export type { GraphCanvasHandle, GraphCanvasProps } from './graphCanvasProps';

const MOCK_MODE = import.meta.env.VITE_MOCK_IPC === '1';
const STATS_ENABLED = import.meta.env.DEV || MOCK_MODE;
/** Rows painted beyond the visible window on each side (§4.2). */
const OVERSCAN = 4;
/** Scroll activity window for inter-frame gap recording (§4.7). */
const SCROLL_ACTIVE_MS = 200;

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
    graphStyle = 'standard',
    graphSeason = 'living',
    fold,
    rail,
  },
  ref,
) {
  useGraphRenderCount({ layout, selectedIndex, themeVersion, metricsVersion, active, totalRows });
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
  // spec 002: latest Bonsai style/season for the paint path's theme re-resolve.
  const graphStyleRef = useRef(graphStyle);
  graphStyleRef.current = graphStyle;
  const graphSeasonRef = useRef(graphSeason);
  graphSeasonRef.current = graphSeason;
  /** Row index, `null` (none), or `-1` sentinel for the synthetic WIP row. */
  const hoverRowRef = useRef<number | null>(null);
  /** Spec-004 §1: fold-pill row under a held mouse button (pressed tint). */
  const pressedRowRef = useRef<number | null>(null);
  const rafRef = useRef(0);
  // P84: active reveal flash — the target row + animation start timestamp; null
  // when no flash is running. Its own rAF handle (separate from the scroll rAF).
  const flashStateRef = useRef<{ row: number; start: number } | null>(null);
  const flashRafRef = useRef(0);
  const flashTimeoutRef = useRef(0);
  // spec 002 §5: active settle descriptor (arm timestamp) read by the paint path;
  // the rAF lifecycle that drives + clears it lives in `useSway`.
  const swayStateRef = useRef<{ start: number } | null>(null);
  const scrollTopRef = useRef(0);
  const cssSizeRef = useRef({ w: 0, h: 0 });
  /** Cursor y relative to the scroller top; null while the pointer is outside. */
  const mouseYRef = useRef<number | null>(null);
  /** P7 §6.1: cursor x relative to the scroller left (mirror of mouseYRef); lets
   *  scroll re-runs recompute the hover target without a fresh mouse event. */
  const mouseXRef = useRef<number | null>(null);
  const lastScrollTsRef = useRef(Number.NEGATIVE_INFINITY);
  const prevFrameTsRef = useRef<number | null>(null);
  const firstDataPaintSkippedRef = useRef(false);
  /** P95 §1.4: per-instance id — two graph panes can be mounted across tabs. */
  const hintId = useId();

  // P7 §6: hover tooltip. State changes ONLY when the hover TARGET changes (the
  // sameTarget guard), so re-renders are rare and the per-frame canvas paint
  // path (paintNow) never depends on tooltip state. Measurement/clamp + DOM
  // live in GraphTooltipOverlay (spec-004 size split).
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  // Spec-005: rail hover-zone / reveal-latch / drag-pin state (useRailReveal).
  const railReveal = useRailReveal();

  useImperativeHandle(
    ref,
    () => ({
      getVisibleRowCount: () =>
        visibleRowCount(cssSizeRef.current.h, metricsRef.current.rowHeight),
      // P95 §2.2: `preventScroll` is required — without it the browser scrolls
      // the scroller to its own idea of the focus target and fights
      // `scrollRowIntoView`.
      focusScroller: () => scrollerRef.current?.focus({ preventScroll: true }),
    }),
    [],
  );

  // Spec-004 display-space derivation (projection, edge-index choice, display
  // selection, match set) — moved verbatim to useGraphDisplayModel.ts. Returned
  // under the original identifier names, so every use below is unchanged.
  const {
    activeRow,
    boundaryRows,
    dLayout,
    dSel,
    foldModel,
    foldRows,
    foldRowSet,
    incIndex,
    matchSet,
    memoIndex,
    projected,
  } = useGraphDisplayModel({ edgeIndex, fold, layout, matchRows, selectedIndex });

  // Latest props for the stable paint callback. `edgeIndex` is the streamed
  // incremental index (or undefined); `memoIndex` is the one-shot index (or null
  // when streamed) — paintNow picks whichever is present (§4.3).
  const currentProps = {
    layout: dLayout,
    selectedIndex: dSel.row,
    edgeIndex: incIndex,
    memoIndex,
    wip,
    matchSet,
    display,
    verifyStatus,
    onVisibleRangeChange,
    foldModel,
    foldRows,
    boundaryRows,
    foldRowSet,
    activeRow,
    selPillRow: dSel.pillRow,
  };
  const propsRef = useRef(currentProps);
  propsRef.current = currentProps;

  // P58c: last visible window reported to onVisibleRangeChange — guards
  // redundant fires (only when the window actually changed). `count` is the
  // spec-004 modelRows length (-1 when fold is inactive).
  const lastRangeRef = useRef<{ first: number; last: number; count: number } | null>(null);

  // Frame-timing recorders + the periodic summary log — moved verbatim to
  // useFrameRecorder.ts. `recordFrame` keeps its stable ([]-dep) identity.
  const { recordFrame } = useFrameRecorder();

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
    themeRef.current ??= resolveTheme(canvas, graphStyleRef.current, graphSeasonRef.current);

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
      foldModel,
      foldRows,
      boundaryRows,
      foldRowSet,
      activeRow,
      selPillRow,
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
      const next = reportVisibleRange(
        onVisibleRangeChange, lastRangeRef.current, firstRow, lastRow, n, foldModel, foldRows,
      );
      if (next !== null) lastRangeRef.current = next;
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
    // P84 reveal flash + spec-002 sway, resolved per frame (paintFx.ts).
    const flash = resolveFlash(
      flashStateRef.current, themeRef.current, m.avatarSelRingRadius, reducedMotionRef.current, foldModel,
    );
    const sway = resolveSway(swayStateRef.current, graphStyleRef.current, reducedMotionRef.current);
    drawGraph(
      ctx,
      lay,
      visibleEdges,
      { firstRow, lastRow, scrollTop: layoutScrollTop, width: w, height: h, rightInset },
      themeRef.current,
      { hoverRow, selectedIndex: sel, matchRows: matchSet, verifyStatus: verifyStatus ?? null, flash, sway, foldRows: foldRowSet },
      display,
      m,
    );
    // Spec-004: fold pills + boundary collapse pills paint over the row grid
    // (fold rows themselves were skipped by drawGraph's avatar/text passes).
    if (foldRows !== null || boundaryRows !== null) {
      drawFoldRows(
        ctx,
        foldRows ?? new Map(),
        boundaryRows ?? new Map(),
        { firstRow, lastRow, scrollTop: layoutScrollTop, width: w },
        themeRef.current,
        m,
        {
          hoverRow,
          pressedRow: pressedRowRef.current,
          activeRow,
          selectionRow: selPillRow,
        },
      );
    }
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

  // spec 002 §5: settle-on-scroll sway lifecycle (bounded, self-terminating).
  // The hook owns its own rAF + arm-debounce timer; `swayStateRef` is read by the
  // paint path below and `onScroll` is called from `handleScroll`.
  const { onScroll: onSwayScroll } = useSway({
    paint: paintNow,
    graphStyle,
    graphStyleRef,
    reducedMotion,
    reducedMotionRef,
    selectedIndex,
    swayStateRef,
  });

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

  // Mount: ResizeObserver + DPR handling — moved verbatim to its own hook
  // (spec-005 size offset). resize() also performs the initial paint.
  useCanvasResizeObserver(hostRef, resize, rafRef);

  // P3e §5.4: authoritative remeasure-on-show when `active` flips true (and the
  // rail-state reset when it flips false) — moved verbatim to
  // useGraphCanvasEffects.ts.
  useRemeasureOnShow(active, resize, railReveal);

  // Layout/selection changes repaint synchronously; the mount paint already
  // happened inside resize() above (single mount paint — no double paint).
  // Spec-004: `projected`/`activeRow` join the deps — expand/collapse and the
  // keyboard-active pill are pure paint-state changes.
  useEffect(() => {
    if (!firstDataPaintSkippedRef.current) {
      firstDataPaintSkippedRef.current = true;
      return;
    }
    paintNow();
  }, [paintNow, layout, selectedIndex, wip, matchSet, display, verifyStatus, projected, activeRow]);

  // P2b §4.4 / spec 002: theme + style/season changes re-resolve the cached
  // CSS-variable colors and repaint — moved verbatim to useGraphCanvasEffects.ts.
  useThemeRepaint(themeVersion, graphStyle, graphSeason, canvasRef, themeRef, schedulePaint);

  // P11d §4.3: a graph-knob change re-runs the SAME resize() path (re-measure +
  // HiDPI reset + synchronous repaint) — moved verbatim to useGraphCanvasEffects.ts.
  useRemeasureOnMetrics(metricsVersion, resize);

  // P1 §6.3/§9.3: bring the selected / keyboard-active DISPLAY row into view —
  // moved verbatim to useGraphCanvasEffects.ts.
  const scrollTarget = activeRow ?? dSel.row ?? dSel.pillRow;
  useScrollSelectionIntoView(scrollTarget, wip, scrollerRef, metricsRef);

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

  // Mock-mode dev hooks (`window.__bonsai`) — moved to useMockDevHooks.ts.
  useMockDevHooks(canvasRef, scrollerRef);

  const ctx2d = (): CanvasRenderingContext2D | null => canvasRef.current?.getContext('2d') ?? null;

  /** Hover-ref encoding: row index, `-1` for the WIP row, or `null`. */
  const hitTestAtMouseY = (yCss: number, scrollTop: number): number | null => {
    const { layout: lay, wip } = propsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTestRow(yCss, scrollTop, wipOffset, lay.nodes.length, metricsRef.current.rowHeight);
    return hit === 'wip' ? -1 : hit;
  };

  // P7 §6.1: resolve the hover tooltip target from a cursor position (scroller/
  // host CSS coords). Pure resolution lives in `hoverTarget.ts`; this wrapper
  // just gathers the container's live props/refs/measurements for it.
  const computeHoverTarget = (x: number, y: number, scrollTop: number): TooltipState | null => {
    // Spec-004: fold-pill rows carry no hover tooltip (the pill is the label).
    const row = hitTestAtMouseY(y, scrollTop);
    if (row !== null && row >= 0 && propsRef.current.foldRowSet?.has(row) === true) return null;
    return resolveHoverTarget({
      x,
      y,
      scrollTop,
      row,
      layout: propsRef.current.layout,
      wip: propsRef.current.wip,
      display: propsRef.current.display,
      m: metricsRef.current,
      ctx: canvasRef.current?.getContext('2d') ?? null,
      theme: themeRef.current,
      rightInset: scrollerRef.current
        ? scrollerRef.current.offsetWidth - scrollerRef.current.clientWidth
        : 0,
      cssWidth: cssSizeRef.current.w,
    });
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
    // spec 002 §5: sway plays AFTER motion stops, never during — the hook cancels
    // any live settle (offset 0 while scrolling) and re-arms on a scroll-stop
    // debounce. Inert under standard/reduced-motion, so the idle path stays quiet.
    onSwayScroll();
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
    // Spec-005: right-edge hover zone (scrollbar included); transitions only.
    if (rail !== undefined) railReveal.onZoneCheck(x, scroller);
    const row = hitTestAtMouseY(y, scroller.scrollTop);
    if (row !== hoverRowRef.current) {
      hoverRowRef.current = row;
      schedulePaint(); // repaint only when the hovered row changed, via rAF
    }
    // Spec-004 §1/§2: pointer cursor over fold-pill rows / the collapse pill.
    if (propsRef.current.foldRowSet !== null || propsRef.current.boundaryRows !== null) {
      const ctx = canvasRef.current?.getContext('2d') ?? null;
      scroller.style.cursor = foldCursorFor(
        row,
        x,
        propsRef.current.foldRowSet,
        propsRef.current.boundaryRows,
        (span, px) => ctx !== null && collapsePillHit(ctx, span, metricsRef.current, px),
      );
    } else if (scroller.style.cursor !== '') {
      scroller.style.cursor = '';
    }
    // P7 §6.1: recompute the hover tooltip target; setTooltip only fires on a
    // real target change (sameTarget), so this is not a per-frame React churn.
    const next = computeHoverTarget(x, y, scroller.scrollTop);
    // P92 §1.4: the "+n" chip is clickable, so it — and only it — shows a pointer
    // cursor. Driven by the already-computed hover target: no extra hit pass, no
    // canvas repaint, no React state.
    scroller.style.cursor = next?.kind === 'overflow' ? 'pointer' : '';
    setTooltip((prev) => (sameTarget(prev, next) ? prev : next));
  };

  const handleMouseLeave = () => {
    if (scrollerRef.current !== null) scrollerRef.current.style.cursor = '';
    mouseYRef.current = null;
    mouseXRef.current = null;
    railReveal.onLeave(); // spec-005: leaving the scroller exits the hover zone
    setTooltip(null); // P7 §6.2: dismiss on leave
    // Spec-004: drop the pointer cursor + a held pill press on leave.
    if (scrollerRef.current !== null) scrollerRef.current.style.cursor = '';
    if (pressedRowRef.current !== null) {
      pressedRowRef.current = null;
      schedulePaint();
    }
    if (hoverRowRef.current !== null) {
      hoverRowRef.current = null;
      schedulePaint();
    }
  };

  // Spec-004 §1: pressed-frame tint on a fold-pill row.
  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null || propsRef.current.foldRowSet === null) return;
    const row = hitTestAtMouseY(e.clientY - scroller.getBoundingClientRect().top, scroller.scrollTop);
    if (row !== null && row >= 0 && propsRef.current.foldRowSet.has(row)) {
      pressedRowRef.current = row;
      schedulePaint();
    }
  };
  const handleMouseUp = () => {
    if (pressedRowRef.current !== null) {
      pressedRowRef.current = null;
      schedulePaint();
    }
  };

  // Click resolution lives in graphClick.ts (spec-004 split): empty area →
  // deselect; fold pill / boundary collapse pill → toggle; PR badge → open PR;
  // else select the MODEL row (toggle-off when it is already selected).
  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const rect = scroller.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const m = metricsRef.current;
    const action = resolveGraphClick({
      hit: hitTestRow(
        e.clientY - rect.top,
        scroller.scrollTop,
        wip !== null ? 1 : 0,
        dLayout.nodes.length,
        m.rowHeight,
      ),
      x,
      layout: dLayout,
      foldModel,
      foldRows,
      boundaryRows,
      ctx: ctx2d(),
      m,
      display,
      effectiveWidth: cssSizeRef.current.w - (scroller.offsetWidth - scroller.clientWidth),
      prBadgesActive: onOpenPr !== undefined,
    });
    if (action.kind === 'deselect') onSelect(null);
    else if (action.kind === 'toggleSpan') fold?.onToggleSpan(action.start);
    else if (action.kind === 'openPr') onOpenPr?.(action.number);
    else if (action.kind === 'select') {
      // P92 §1.1: left-click the "+n" chip → the same ref picker the right-click
      // opens (hover = read, click = act), BEFORE the row-select toggle.
      const node = layout.nodes[action.modelRow];
      if (onContextMenu !== undefined && node !== undefined) {
        const chipEntities = chipHiddenEntitiesAt({
          node,
          x,
          m,
          ctx: ctx2d(),
          theme: themeRef.current,
          display,
        });
        if (chipEntities !== null) {
          onContextMenu({ kind: 'refPicker', entities: chipEntities, oid: node.id }, e.clientX, e.clientY);
          return;
        }
      }
      onSelect(action.modelRow === selectedIndex ? null : action.modelRow);
    }
  };

  // P5 §4.2 / P7 §5 / P92 §1.1: right-click hit-test. Always suppress the native
  // menu over the graph; the pill / "+N" chip / whole-row RESOLUTION is pure and
  // lives in contextTarget.ts (same layoutRefLabels layout as the draw pass), so
  // this handler only gathers live measurements. Fold-pill rows get no menu;
  // commit targets carry the MODEL row index.
  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const scroller = scrollerRef.current;
    if (scroller === null || onContextMenu === undefined) return;
    const rect = scroller.getBoundingClientRect();
    const m = metricsRef.current;
    const hit = hitTestRow(
      e.clientY - rect.top,
      scroller.scrollTop,
      wip !== null ? 1 : 0,
      dLayout.nodes.length,
      m.rowHeight,
    );
    if (hit === null || hit === 'wip') return;
    let index = hit;
    if (foldModel !== null) {
      const d = displayToModel(foldModel, hit);
      if (d.kind === 'fold') return; // no context menu on a pill row
      index = d.row;
    }
    const target = resolveContextTarget({
      node: dLayout.nodes[hit],
      x: e.clientX - rect.left,
      m,
      ctx: ctx2d(),
      theme: themeRef.current,
      display,
      row: index,
    });
    onContextMenu(target, e.clientX, e.clientY);
  };

  /** P92 §1.5: Menu key / Shift+F10 on the focused graph scroller opens the
   *  SELECTED row's menu, anchored at that row — the keyboard route to every ref
   *  on a multi-ref commit (the canvas-drawn "+N" chip is not a tab stop).
   *  Spec-004: the MENU carries the MODEL row; the ANCHOR uses the row's DISPLAY
   *  position (its fold pill's row when the selection sits inside a collapsed
   *  span), so it lands on the pixels the user sees. */
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'ContextMenu' && !(e.key === 'F10' && e.shiftKey)) return;
    const scroller = scrollerRef.current;
    const index = selectedIndex;
    if (scroller === null || onContextMenu === undefined || index === null) return;
    if (index < 0 || index >= layout.nodes.length) return;
    const anchorRow = dSel.row ?? dSel.pillRow;
    if (anchorRow === null) return;
    e.preventDefault();
    const node = layout.nodes[index];
    const at = rowMenuAnchor(
      scroller.getBoundingClientRect(),
      scroller.scrollTop,
      anchorRow,
      wip !== null ? 1 : 0,
      metricsRef.current,
    );
    onContextMenu(
      { kind: 'commit', index, oid: node.id, entities: groupRefs(node.refs) },
      at.x,
      at.y,
    );
  };

  // P11d §4.3: spacer (total scroll extent) tracks the live rowHeight knob so
  // the scrollbar range re-maps on every graph-metric change. P65b: on the
  // streamed path `totalRows` extends the extent to the full repo while rows are
  // still arriving (grow-as-you-go); absent ⇒ layout.nodes.length (unchanged).
  // Spec-004: with collapsed spans the extent is the DISPLAY row count (spans
  // only exist after `done`, so the streamed grow-as-you-go path is unaffected).
  const displayRowCount =
    foldRows !== null ? dLayout.nodes.length : Math.max(layout.nodes.length, totalRows ?? 0);
  const spacerH = spacerHeight(displayRowCount, wip !== null ? 1 : 0, metrics.rowHeight);
  // §4.1 amended (spec-004 + P95 reconciliation): active-descendant ids use
  // DISPLAY rows; the active pill row wins over the selection while set.
  // `aria-rowcount` / `role="row"` / `aria-rowindex` are deliberately ABSENT:
  // they are only meaningful under a grid/table role, and P95 settled the
  // scroller as `role="group"`. `group` does support `aria-activedescendant`,
  // so the IDREF below stays valid and the row ordinal reaches the user via
  // GraphSelectionAnnouncer's "Row {n+1} of {N}" clause instead.
  const ariaActiveRow = activeRow ?? dSel.row ?? dSel.pillRow;
  // WCAG 4.1.2 (ui-designer ruling): the active-descendant id must resolve to a
  // real element — ONE sr-only row, updated per active row, carrying the §3
  // accessible name (fold-off: the plain row text, same emission as pre-fold).
  const activeA11y =
    ariaActiveRow !== null
      ? activeRowA11y(ariaActiveRow, dLayout.nodes, foldRows, boundaryRows)
      : null;

  return (
    <div ref={hostRef} className="graph-canvas-host">
      <canvas ref={canvasRef} className="graph-canvas" data-testid="graph-canvas" />
      <div
        ref={scrollerRef}
        className="graph-scroll"
        data-testid="graph-scroller"
        tabIndex={0}
        role="group"
        aria-label="Commit graph"
        aria-activedescendant={ariaActiveRow !== null ? `graph-row-${ariaActiveRow}` : undefined}
        aria-describedby={hintId}
        onScroll={handleScroll}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        onContextMenu={handleContextMenu}
      >
        {ariaActiveRow !== null && activeA11y !== null && (
          <div
            id={`graph-row-${ariaActiveRow}`}
            aria-selected={dSel.row === ariaActiveRow}
            aria-expanded={activeA11y.expanded}
            className="sr-only"
          >
            {activeA11y.label}
          </div>
        )}
        <div className="graph-spacer" style={{ height: `${spacerH}px` }} />
      </div>
      {/* Spec-005: mounted ONLY while visible — hidden ⇒ zero DOM/listeners. */}
      {rail !== undefined && displayRowCount > 0 &&
        (rail.ringsLive || rail.alwaysShow || railReveal.revealed) && (
          <OverviewRail
            rail={rail} layout={dLayout} foldModel={foldModel}
            displayRowCount={displayRowCount} scrollerRef={scrollerRef}
            pointerInZone={railReveal.pointerInZone}
            fadeIn={!rail.ringsLive && !rail.alwaysShow}
            themeVersion={themeVersion} onHide={railReveal.hide}
            onDraggingChange={railReveal.onDraggingChange}
          />
        )}
      <GraphKeyboardHint id={hintId} />
      <GraphTooltipOverlay tooltip={tooltip} hostRef={hostRef} />
    </div>
  );
});
