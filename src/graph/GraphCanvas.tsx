import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef } from 'react';
import type { GraphLayout, GraphNode, RefLabel } from '../ipc';
import { resolveTheme } from './colors';
import type { Theme } from './colors';
import {
  avatarColor,
  avatarHit,
  drawGraph,
  drawWipRow,
  groupRefs,
  initials,
  layoutRefLabels,
  layoutRowPills,
  pillArea,
  refColArea,
  relativeDate,
} from './draw';
import type { WipSummary } from './draw';
import type { P7SelfTestResult } from './frameStats';
import { buildEdgeIndex, edgesInRange } from './edgeIndex';
import { createFrameRecorder } from './frameStats';
import type { FrameStats } from './frameStats';
import { METRICS } from './metrics';

export type { WipSummary };

/** Right-click target on the graph: a ref pill, or a bare commit row. */
export type GraphContextTarget =
  | { kind: 'ref'; ref: RefLabel }
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
}

/** P2c §5.2: imperative escape hatch — App needs the DOM-measured visible row
 *  count for PageUp/PageDown deltas, which App has no other way to learn
 *  without duplicating a ResizeObserver of its own. Pure view-layer index
 *  arithmetic downstream — no lane/edge math involved. */
export interface GraphCanvasHandle {
  getVisibleRowCount(): number;
}

/** Row hit-test result: a layout row index, the synthetic WIP row, or none. */
type HitRow = number | 'wip' | null;

/** raw = floor((y + scrollTop) / RH); raw < wipOffset -> 'wip' (only possible
 * when wipOffset === 1); else row = raw - wipOffset (P1 §9.3). */
function hitTest(yCss: number, scrollTop: number, wipOffset: number, nodesLen: number): HitRow {
  const raw = Math.floor((yCss + scrollTop) / METRICS.rowHeight);
  if (raw < 0) return null;
  if (raw < wipOffset) return 'wip';
  const row = raw - wipOffset;
  return row >= 0 && row < nodesLen ? row : null;
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
  { layout, selectedIndex, onSelect, wip, themeVersion, active = true, onContextMenu },
  ref,
) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const themeRef = useRef<Theme | null>(null);
  /** Row index, `null` (none), or `-1` sentinel for the synthetic WIP row. */
  const hoverRowRef = useRef<number | null>(null);
  const rafRef = useRef(0);
  const scrollTopRef = useRef(0);
  const cssSizeRef = useRef({ w: 0, h: 0 });
  /** Cursor y relative to the scroller top; null while the pointer is outside. */
  const mouseYRef = useRef<number | null>(null);
  const lastScrollTsRef = useRef(Number.NEGATIVE_INFINITY);
  const prevFrameTsRef = useRef<number | null>(null);
  // Two recorders (P1 §4.7): paint durations and scroll inter-frame gaps are
  // different quantities — mixing them made `avg` meaningless.
  const paintRecorderRef = useRef(createFrameRecorder());
  const paintCountRef = useRef(0);
  const gapRecorderRef = useRef(createFrameRecorder());
  const gapCountRef = useRef(0);
  const firstDataPaintSkippedRef = useRef(false);

  useImperativeHandle(
    ref,
    () => ({
      getVisibleRowCount: () => Math.max(1, Math.floor(cssSizeRef.current.h / METRICS.rowHeight)),
    }),
    [],
  );

  // Edge culling index, built once per layout object (§4.4).
  const edgeIndex = useMemo(() => buildEdgeIndex(layout), [layout]);

  // Latest props for the stable paint callback.
  const propsRef = useRef({ layout, selectedIndex, edgeIndex, wip });
  propsRef.current = { layout, selectedIndex, edgeIndex, wip };

  const recordFrame = useCallback((kind: 'paint' | 'gap', durMs: number) => {
    const rec = kind === 'paint' ? paintRecorderRef.current : gapRecorderRef.current;
    const countRef = kind === 'paint' ? paintCountRef : gapCountRef;
    rec.record(durMs);
    if (++countRef.current >= LOG_EVERY) {
      countRef.current = 0;
      const s = rec.flushSummary();
      console.log(
        `[bonsai] frames kind=${kind} n=${s.frames} avg=${s.avgMs.toFixed(1)}ms ` +
          `max=${s.maxMs.toFixed(1)}ms >33ms=${s.over33}`,
      );
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
    const { layout: lay, selectedIndex: sel, edgeIndex: ix, wip } = propsRef.current;
    const { w, h } = cssSizeRef.current;
    const scrollTop = scrollerRef.current?.scrollTop ?? scrollTopRef.current;
    scrollTopRef.current = scrollTop;
    const n = lay.nodes.length;
    const wipOffset = wip !== null ? 1 : 0;
    const layoutScrollTop = scrollTop - wipOffset * METRICS.rowHeight;
    const firstRow = Math.max(0, Math.floor(layoutScrollTop / METRICS.rowHeight) - OVERSCAN);
    const lastRow = Math.min(
      n - 1,
      Math.ceil((layoutScrollTop + h) / METRICS.rowHeight) + OVERSCAN,
    );
    const hoverRow = hoverRowRef.current !== null && hoverRowRef.current >= 0 ? hoverRowRef.current : null;
    drawGraph(
      ctx,
      lay,
      edgesInRange(lay, ix, firstRow, lastRow),
      { firstRow, lastRow, scrollTop: layoutScrollTop, width: w, height: h },
      themeRef.current,
      { hoverRow, selectedIndex: sel },
    );
    if (wip !== null && scrollTop < METRICS.rowHeight + 56) {
      drawWipRow(
        ctx,
        lay,
        wip,
        { firstRow: 0, lastRow: 0, scrollTop, width: w, height: h },
        themeRef.current,
        hoverRowRef.current === -1,
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
    canvas.style.width = `${cssW}px`;
    canvas.style.height = `${cssH}px`;
    canvas.width = Math.max(1, Math.round(cssW * dpr));
    canvas.height = Math.max(1, Math.round(cssH * dpr));
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
  }, [paintNow, layout, selectedIndex, wip]);

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

  // P1 §6.3/§9.3: when selectedIndex changes to non-null (e.g. via ArrowUp/
  // Down in App), bring the row into view if it's outside the visible window.
  // Pure scroll adjustment — row position accounts for the WIP row offset:
  // target y = (row + wipOffset) * rowHeight.
  useEffect(() => {
    if (selectedIndex === null) return;
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const wipOffset = wip !== null ? 1 : 0;
    const rowTop = (selectedIndex + wipOffset) * METRICS.rowHeight;
    const rowBottom = rowTop + METRICS.rowHeight;
    const viewTop = scroller.scrollTop;
    const viewBottom = viewTop + scroller.clientHeight;
    if (rowTop < viewTop) {
      scroller.scrollTop = Math.max(0, rowTop - METRICS.rowHeight);
    } else if (rowBottom > viewBottom) {
      scroller.scrollTop = rowBottom - scroller.clientHeight + METRICS.rowHeight;
    }
  }, [selectedIndex, wip]);

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
            console.log(`[bonsai] scroll-test ${JSON.stringify(stats)}`);
            resolve(stats);
          }
        };
        requestAnimationFrame(step);
      });

    // P7 §10 item 2: expose the pure helpers + a self-test (mock only, mirroring
    // scrollSweep). The orchestrator reads `window.__bonsai.p7SelfTest()`.
    const p7 = { initials, avatarColor, groupRefs, layoutRefLabels, refColArea, avatarHit, relativeDate };

    const p7SelfTest = (): P7SelfTestResult => {
      let pass = 0;
      const failures: string[] = [];
      const check = (name: string, cond: boolean): void => {
        if (cond) pass++;
        else failures.push(name);
      };

      // initials
      check('initials "Dan Percic"→"DP"', initials('Dan Percic') === 'DP');
      check('initials "torvalds"→"TO"', initials('torvalds') === 'TO');
      check('initials "x"→"X"', initials('x') === 'X');
      check('initials ""→"?"', initials('') === '?');
      check('initials "  a  b "→"AB"', initials('  a  b ') === 'AB');

      // avatarColor
      const c1 = avatarColor('Dan Percic');
      const c2 = avatarColor('Dan Percic');
      check('avatarColor deterministic', c1.bg === c2.bg);
      check('avatarColor bg format', /^hsl\(\d{1,3}, 52%, 42%\)$/.test(c1.bg));
      check('avatarColor text white', c1.text === '#ffffff');
      const distinct = new Set(
        ['Alice', 'Bob', 'Carol', 'Dan Percic', 'torvalds', 'Grace Hopper'].map(
          (n) => avatarColor(n).bg,
        ),
      );
      check('avatarColor varies across names', distinct.size >= 2);

      // groupRefs — same-commit collapse
      const sameCommit: RefLabel[] = [
        { name: 'main', kind: 'localBranch', isHead: true },
        { name: 'origin/main', kind: 'remoteBranch', isHead: false },
        { name: 'v1.0', kind: 'tag', isHead: false },
      ];
      const g = groupRefs(sameCommit);
      check('groupRefs same-commit length 2', g.length === 2);
      const b0 = g[0];
      check(
        'groupRefs same-commit branch main',
        b0 !== undefined &&
          b0.kind === 'branch' &&
          b0.name === 'main' &&
          b0.hasLocal === true &&
          b0.remotes.length === 1 &&
          b0.remotes[0] === 'origin/main' &&
          b0.isHead === true,
      );
      const t1 = g[1];
      check('groupRefs same-commit tag v1.0', t1 !== undefined && t1.kind === 'tag' && t1.name === 'v1.0');

      // groupRefs — diverged (each ref on its own node)
      const localFeat = groupRefs([{ name: 'feat', kind: 'localBranch', isHead: false }]);
      const lf = localFeat[0];
      check(
        'groupRefs diverged local feat',
        lf !== undefined &&
          lf.kind === 'branch' &&
          lf.name === 'feat' &&
          lf.hasLocal === true &&
          lf.remotes.length === 0,
      );
      const remoteFeat = groupRefs([{ name: 'origin/feat', kind: 'remoteBranch', isHead: false }]);
      const rf = remoteFeat[0];
      check(
        'groupRefs diverged remote feat',
        rf !== undefined &&
          rf.kind === 'branch' &&
          rf.name === 'feat' &&
          rf.hasLocal === false &&
          rf.remotes.length === 1 &&
          rf.remotes[0] === 'origin/feat',
      );

      // refColArea
      const area = refColArea();
      check('refColArea startX', area.startX === METRICS.refColPadLeft);
      check(
        'refColArea budget',
        area.budget === METRICS.refColWidth - METRICS.refColPadLeft - METRICS.refColPadRight,
      );

      // avatarHit
      check('avatarHit center', avatarHit(10, 10, 10, 10));
      check('avatarHit outside', !avatarHit(100, 100, 10, 10));

      // relativeDate regression guard
      const now = 1_000_000_000;
      check('relativeDate now', relativeDate(now, now) === 'now');
      check('relativeDate 2m', relativeDate(now - 120, now) === '2m');
      check('relativeDate 2h', relativeDate(now - 7200, now) === '2h');
      check('relativeDate 2d', relativeDate(now - 172800, now) === '2d');

      // layoutRefLabels overflow — needs a ctx + theme.
      const canvas = canvasRef.current;
      const ctx = canvas?.getContext('2d') ?? null;
      const theme = canvas !== null ? resolveTheme(canvas) : null;
      if (ctx !== null && theme !== null) {
        const manyRefs: RefLabel[] = [
          { name: 'main', kind: 'localBranch', isHead: true },
          { name: 'develop', kind: 'localBranch', isHead: false },
          { name: 'feature-long-name', kind: 'localBranch', isHead: false },
          { name: 'release', kind: 'localBranch', isHead: false },
          { name: 'hotfix', kind: 'localBranch', isHead: false },
        ];
        const node: GraphNode = {
          id: '0'.repeat(40),
          lane: 0,
          parents: [],
          refs: manyRefs,
          summary: '',
          author: '',
          ts: 0,
        };
        const entities = groupRefs(manyRefs);
        const { startX } = refColArea();
        const laid = layoutRefLabels(ctx, entities, node, theme, startX, 50);
        check('layoutRefLabels first entity laid', laid.length >= 1 && laid[0].entity !== null);
        const last = laid[laid.length - 1];
        check('layoutRefLabels trailing overflow chip', last !== undefined && last.entity === null);
        const shownCount = laid.filter((l) => l.entity !== null).length;
        const hiddenCount = entities.length - shownCount;
        check('layoutRefLabels overflow count', hiddenCount > 0);
        check(
          'layoutRefLabels chip label',
          last !== undefined && last.entity === null && last.style.label === `+${hiddenCount}`,
        );
      } else {
        failures.push('layoutRefLabels: no canvas ctx/theme available');
      }

      const result: P7SelfTestResult = { pass, fail: failures.length, failures };
      console.log(`[bonsai] p7SelfTest ${JSON.stringify(result)}`);
      return result;
    };

    window.__bonsai = { scrollSweep, p7, p7SelfTest };
    return () => {
      if (window.__bonsai?.scrollSweep === scrollSweep) delete window.__bonsai;
    };
  }, []);

  /** Hover-ref encoding: row index, `-1` for the WIP row, or `null`. */
  const hitTestAtMouseY = (yCss: number, scrollTop: number): number | null => {
    const { layout: lay, wip } = propsRef.current;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTest(yCss, scrollTop, wipOffset, lay.nodes.length);
    return hit === 'wip' ? -1 : hit;
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
    }
    schedulePaint();
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const y = e.clientY - scroller.getBoundingClientRect().top;
    mouseYRef.current = y;
    const row = hitTestAtMouseY(y, scroller.scrollTop);
    if (row !== hoverRowRef.current) {
      hoverRowRef.current = row;
      schedulePaint(); // repaint only when the hovered row changed, via rAF
    }
  };

  const handleMouseLeave = () => {
    mouseYRef.current = null;
    if (hoverRowRef.current !== null) {
      hoverRowRef.current = null;
      schedulePaint();
    }
  };

  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const y = e.clientY - scroller.getBoundingClientRect().top;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTest(y, scroller.scrollTop, wipOffset, layout.nodes.length);
    if (hit === null || hit === 'wip') onSelect(null);
    else onSelect(hit === selectedIndex ? null : hit);
  };

  // P5 §4.2: right-click hit-test. Always suppress the native menu over the
  // graph; then resolve a ref pill (via the shared pill layout) or a commit row.
  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const rect = scroller.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const x = e.clientX - rect.left;
    const wipOffset = wip !== null ? 1 : 0;
    const hit = hitTest(y, scroller.scrollTop, wipOffset, layout.nodes.length);
    if (hit === null || hit === 'wip') return;
    const node = layout.nodes[hit];
    const ctx = canvasRef.current?.getContext('2d') ?? null;
    const theme = themeRef.current;
    if (ctx !== null && theme !== null && node.refs !== undefined && node.refs.length > 0) {
      const { startX, budget } = pillArea(cssSizeRef.current.w, layout.laneCount);
      const pills = layoutRowPills(ctx, node, theme, startX, budget);
      const hitPill = pills.find((p) => p.ref !== null && x >= p.x && x <= p.x + p.w);
      if (hitPill !== undefined && hitPill.ref !== null) {
        onContextMenu?.({ kind: 'ref', ref: hitPill.ref }, e.clientX, e.clientY);
        return;
      }
    }
    onContextMenu?.({ kind: 'commit', index: hit, oid: node.id }, e.clientX, e.clientY);
  };

  const spacerHeight = (layout.nodes.length + (wip !== null ? 1 : 0)) * METRICS.rowHeight + 8;

  return (
    <div ref={hostRef} className="graph-canvas-host">
      <canvas ref={canvasRef} className="graph-canvas" />
      <div
        ref={scrollerRef}
        className="graph-scroll"
        onScroll={handleScroll}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        <div className="graph-spacer" style={{ height: `${spacerHeight}px` }} />
      </div>
    </div>
  );
});
