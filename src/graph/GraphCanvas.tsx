import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { GraphLayout, VerifyStatus } from '../ipc';
import { resolveTheme } from './colors';
import type { GraphStyle, Theme } from './colors';
import type { GraphSeason } from './palettes';
import { drawGraph, drawHeadEdgeMarker, drawHeadGuide, drawWipRow } from './draw';
import type { WipSummary } from './draw';
import { hitTestRow, sameTarget } from './hitTest';
import type { TooltipState } from './hitTest';
import {
  backingStoreSize,
  headGuide,
  scrollRowIntoView,
  spacerHeight,
  visibleRowCount,
  visibleRowRange,
} from './viewport';
import type { GraphDisplayOptions } from './rightColumns';
import { buildEdgeIndex, edgesInRange } from './edgeIndex';
import type { IncrementalEdgeIndex } from './incrementalEdgeIndex';
import { createFrameRecorder } from './frameStats';
import type { EffectiveMetrics } from './metrics';
import { resolveContextTarget } from './contextTarget';
import { useMockDevHooks } from './useMockDevHooks';
// Spec-004: fold display-row model — projection, pill painting, mappings.
import { projectLayout } from './foldProject';
import { collapsePillHit, drawFoldRows } from './drawFold';
import { resolveGraphClick } from './graphClick';
import { activeRowA11y, displaySelection, foldCursorFor, mapMatchRows } from './foldView';
import type { GraphFoldView } from './foldView';
import { displayToModel, pillRowOfStart } from './foldModel';
import { reportVisibleRange, resolveFlash, resolveSway } from './paintFx';
import type { RevealFlash } from './reveal';
import { startRevealFlash } from './revealFlashRunner';
import { useSway } from './useSway';
import { resolveHoverTarget } from './hoverTarget';
import { GraphTooltipOverlay } from './GraphTooltipOverlay';

export type { WipSummary };

// Right-click target on the graph — moved to contextTarget.ts (spec-004 size
// split); re-exported so existing import sites keep working.
export type { GraphContextTarget } from './contextTarget';
import type { GraphContextTarget } from './contextTarget';

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
   *  the window changed). Drives the debounced verify request for exactly the
   *  visible (overscanned) rows — the badge is virtualized. Spec-004: with fold
   *  active `first`/`last` are the min/max visible MODEL rows and `modelRows`
   *  lists exactly the visible commit rows (fold rows skipped), so a giant
   *  collapsed run never balloons the verify request. */
  onVisibleRangeChange?(first: number, last: number, modelRows?: readonly number[]): void;
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
  /** spec 002: Bonsai paint style + season → `resolveTheme` (mirrors
   *  `themeVersion`; a change re-resolves + repaints). Default standard/living. */
  graphStyle?: GraphStyle;
  graphSeason?: GraphSeason;
  /** Spec-004: fold view-model (collapsed-span mapping + expansion callbacks).
   *  Absent ⇒ fold inactive; every path is byte-identical to pre-fold. */
  fold?: GraphFoldView;
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
    graphStyle = 'standard',
    graphSeason = 'living',
    fold,
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
  // Two recorders (P1 §4.7): paint durations and scroll inter-frame gaps are
  // different quantities — mixing them made `avg` meaningless.
  const paintRecorderRef = useRef(createFrameRecorder());
  const paintCountRef = useRef(0);
  const gapRecorderRef = useRef(createFrameRecorder());
  const gapCountRef = useRef(0);
  const firstDataPaintSkippedRef = useRef(false);

  // P7 §6: hover tooltip. State changes ONLY when the hover TARGET changes (the
  // sameTarget guard), so re-renders are rare and the per-frame canvas paint
  // path (paintNow) never depends on tooltip state. Measurement/clamp + DOM
  // live in GraphTooltipOverlay (spec-004 size split).
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  useImperativeHandle(
    ref,
    () => ({
      getVisibleRowCount: () =>
        visibleRowCount(cssSizeRef.current.h, metricsRef.current.rowHeight),
    }),
    [],
  );

  // Spec-004: display-space projection. Identity (the input layout object)
  // whenever fold is inactive or nothing is collapsed. Spans only exist after
  // the stream's `done`, so this never runs per streamed batch in anger. Keyed
  // on model/expandedSpans (NOT the whole `fold` bundle) so an active-pill
  // change never re-projects 20k rows.
  const foldModel = fold !== undefined ? fold.model : null;
  const foldExpanded = fold?.expandedSpans;
  const projected = useMemo(
    () =>
      foldModel !== null && foldExpanded !== undefined
        ? projectLayout(layout, foldModel, foldExpanded)
        : null,
    [layout, foldModel, foldExpanded],
  );
  const dLayout = projected !== null ? projected.layout : layout;
  const foldRows = projected !== null && !projected.identity ? projected.foldRows : null;
  const boundaryRows =
    projected !== null && projected.boundaryRows.size > 0 ? projected.boundaryRows : null;
  const foldRowSet = useMemo(
    () => (foldRows !== null ? new Set(foldRows.keys()) : null),
    [foldRows],
  );
  // Spec-004 §3: the keyboard-active pill row (display index), derived from the
  // stable span `start` so expansion remaps never leave a dangling row.
  const activeRow =
    fold !== undefined && fold.activePillStart !== null
      ? pillRowOfStart(fold.model, fold.activePillStart)
      : null;
  // Selection in display space; a HIDDEN selection moves its ring to the pill.
  const dSel = displaySelection(foldModel, selectedIndex);

  // Edge culling index, built once per layout object (§4.4). P65b: on the
  // streamed path the assembler supplies `edgeIndex` (its own incremental index),
  // so we skip the internal build entirely — otherwise it would be an O(n)
  // rebuild on every streamed batch (layout identity bumps per batch).
  // Spec-004: a non-identity projection owns its own display-space edge array,
  // so it always builds a one-shot index (streamed or not).
  const memoIndex = useMemo(() => {
    if (projected !== null && !projected.identity) return buildEdgeIndex(projected.layout);
    return edgeIndex !== undefined ? null : buildEdgeIndex(layout);
  }, [layout, edgeIndex, projected]);
  const incIndex = projected !== null && !projected.identity ? undefined : edgeIndex;

  // P50b: search-match set, rebuilt once per matchRows prop change (not per
  // frame). null when there are no matches so the draw pass skips the ring.
  // Spec-004: model rows mapped to display rows (hidden matches dropped).
  const matchSet = useMemo(() => {
    const base = matchRows !== undefined && matchRows.length > 0 ? new Set(matchRows) : null;
    return mapMatchRows(foldModel, base);
  }, [matchRows, foldModel]);

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
  // Spec-004: `projected`/`activeRow` join the deps — expand/collapse and the
  // keyboard-active pill are pure paint-state changes.
  useEffect(() => {
    if (!firstDataPaintSkippedRef.current) {
      firstDataPaintSkippedRef.current = true;
      return;
    }
    paintNow();
  }, [paintNow, layout, selectedIndex, wip, matchSet, display, verifyStatus, projected, activeRow]);

  // P2b §4.4: theme changes re-resolve the cached CSS-variable colors and
  // repaint. Runs once on mount too (themeVersion starts at 0), which is
  // harmless — resize()'s initial paint already resolved the theme via the
  // `??=` fallback in paintNow, so this is a cheap re-resolve, not a second
  // distinct paint pathway.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    themeRef.current = resolveTheme(canvas, graphStyle, graphSeason);
    schedulePaint();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeVersion, graphStyle, graphSeason]);

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
  // Spec-004: operates on DISPLAY rows — the fold model joins the deps because
  // reveal-after-expand changes the mapping without changing `selectedIndex`.
  // A keyboard-active pill row scrolls into view the same way (never selected).
  const scrollTarget = activeRow ?? dSel.row ?? dSel.pillRow;
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
  }, [scrollTarget, wip]);

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
    setTooltip((prev) => (sameTarget(prev, next) ? prev : next));
  };

  const handleMouseLeave = () => {
    mouseYRef.current = null;
    mouseXRef.current = null;
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
      ctx: canvasRef.current?.getContext('2d') ?? null,
      m,
      display,
      effectiveWidth: cssSizeRef.current.w - (scroller.offsetWidth - scroller.clientWidth),
      prBadgesActive: onOpenPr !== undefined,
    });
    if (action.kind === 'deselect') onSelect(null);
    else if (action.kind === 'toggleSpan') fold?.onToggleSpan(action.start);
    else if (action.kind === 'openPr') onOpenPr?.(action.number);
    else if (action.kind === 'select') onSelect(action.modelRow === selectedIndex ? null : action.modelRow);
  };

  // P5 §4.2 / P7 §5: right-click hit-test. Always suppress the native menu over
  // the graph; resolution moved verbatim to contextTarget.ts (spec-004 split).
  // Fold-pill rows get no menu; commit targets carry the MODEL row index.
  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const scroller = scrollerRef.current;
    if (scroller === null || onContextMenu === undefined) return;
    const rect = scroller.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const x = e.clientX - rect.left;
    const m = metricsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTestRow(y, scroller.scrollTop, wipOffset, dLayout.nodes.length, m.rowHeight);
    if (hit === null || hit === 'wip') return;
    let index = hit;
    if (foldModel !== null) {
      const d = displayToModel(foldModel, hit);
      if (d.kind === 'fold') return; // no context menu on a pill row
      index = d.row;
    }
    const target = resolveContextTarget({
      x,
      index,
      node: dLayout.nodes[hit],
      ctx: canvasRef.current?.getContext('2d') ?? null,
      theme: themeRef.current,
      m,
      display,
    });
    onContextMenu(target, e.clientX, e.clientY);
  };

  // P11d §4.3: spacer (total scroll extent) tracks the live rowHeight knob so
  // the scrollbar range re-maps on every graph-metric change. P65b: on the
  // streamed path `totalRows` extends the extent to the full repo while rows are
  // still arriving (grow-as-you-go); absent ⇒ layout.nodes.length (unchanged).
  // Spec-004: with collapsed spans the extent is the DISPLAY row count (spans
  // only exist after `done`, so the streamed grow-as-you-go path is unaffected).
  const spacerH = spacerHeight(
    foldRows !== null ? dLayout.nodes.length : Math.max(layout.nodes.length, totalRows ?? 0),
    wip !== null ? 1 : 0,
    metrics.rowHeight,
  );
  // §4.1 amended (spec-004): aria row counts + active-descendant ids use
  // DISPLAY rows; the active pill row wins over the selection while set.
  const ariaRowCount = foldRows !== null ? dLayout.nodes.length : (totalRows ?? layout.nodes.length);
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
        role="grid"
        aria-label="Commit graph"
        aria-rowcount={ariaRowCount}
        aria-activedescendant={ariaActiveRow !== null ? `graph-row-${ariaActiveRow}` : undefined}
        onScroll={handleScroll}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {ariaActiveRow !== null && activeA11y !== null && (
          <div
            id={`graph-row-${ariaActiveRow}`}
            role="row"
            aria-rowindex={ariaActiveRow + 1}
            aria-selected={dSel.row === ariaActiveRow}
            aria-expanded={activeA11y.expanded}
            className="sr-only"
          >
            {activeA11y.label}
          </div>
        )}
        <div className="graph-spacer" style={{ height: `${spacerH}px` }} />
      </div>
      <GraphTooltipOverlay tooltip={tooltip} hostRef={hostRef} />
    </div>
  );
});
