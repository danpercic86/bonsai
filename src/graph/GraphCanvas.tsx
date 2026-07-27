import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { GraphLayout } from '../ipc';
import { resolveTheme } from './colors';
import type { Theme } from './colors';
import { drawGraph, rowAtPoint } from './draw';
import { buildEdgeIndex, edgesInRange } from './edgeIndex';
import { createFrameRecorder } from './frameStats';
import type { FrameStats } from './frameStats';
import { METRICS } from './metrics';

export interface GraphCanvasProps {
  layout: GraphLayout;
  selectedIndex: number | null;
  /** Clicking a row toggles it; empty area below the rows selects null. */
  onSelect(index: number | null): void;
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
export function GraphCanvas({ layout, selectedIndex, onSelect }: GraphCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const themeRef = useRef<Theme | null>(null);
  const hoverRowRef = useRef<number | null>(null);
  const rafRef = useRef(0);
  const scrollTopRef = useRef(0);
  const cssSizeRef = useRef({ w: 0, h: 0 });
  /** Cursor y relative to the scroller top; null while the pointer is outside. */
  const mouseYRef = useRef<number | null>(null);
  const lastScrollTsRef = useRef(Number.NEGATIVE_INFINITY);
  const prevFrameTsRef = useRef<number | null>(null);
  const recorderRef = useRef(createFrameRecorder());
  const frameCountRef = useRef(0);
  const firstDataPaintSkippedRef = useRef(false);

  // Edge culling index, built once per layout object (§4.4).
  const edgeIndex = useMemo(() => buildEdgeIndex(layout), [layout]);

  // Latest props for the stable paint callback.
  const propsRef = useRef({ layout, selectedIndex, edgeIndex });
  propsRef.current = { layout, selectedIndex, edgeIndex };

  const recordFrame = useCallback((durMs: number) => {
    recorderRef.current.record(durMs);
    if (++frameCountRef.current >= LOG_EVERY) {
      frameCountRef.current = 0;
      const s = recorderRef.current.flushSummary();
      console.log(
        `[bonsai] frames n=${s.frames} avg=${s.avgMs.toFixed(1)}ms ` +
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
    const { layout: lay, selectedIndex: sel, edgeIndex: ix } = propsRef.current;
    const { w, h } = cssSizeRef.current;
    const scrollTop = scrollerRef.current?.scrollTop ?? scrollTopRef.current;
    scrollTopRef.current = scrollTop;
    const n = lay.nodes.length;
    const firstRow = Math.max(0, Math.floor(scrollTop / METRICS.rowHeight) - OVERSCAN);
    const lastRow = Math.min(
      n - 1,
      Math.ceil((scrollTop + h) / METRICS.rowHeight) + OVERSCAN,
    );
    drawGraph(
      ctx,
      lay,
      edgesInRange(lay, ix, firstRow, lastRow),
      { firstRow, lastRow, scrollTop, width: w, height: h },
      themeRef.current,
      { hoverRow: hoverRowRef.current, selectedIndex: sel },
    );
    if (STATS_ENABLED) recordFrame(performance.now() - t0);
  }, [recordFrame]);

  const paintFrame = useCallback(
    (ts: number) => {
      rafRef.current = 0;
      if (STATS_ENABLED) {
        // Record inter-frame gaps while scroll activity is ongoing (§4.7).
        const scrolling = performance.now() - lastScrollTsRef.current < SCROLL_ACTIVE_MS;
        if (scrolling && prevFrameTsRef.current !== null) {
          recordFrame(ts - prevFrameTsRef.current);
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

  // Layout/selection changes repaint synchronously; the mount paint already
  // happened inside resize() above (single mount paint — no double paint).
  useEffect(() => {
    if (!firstDataPaintSkippedRef.current) {
      firstDataPaintSkippedRef.current = true;
      return;
    }
    paintNow();
  }, [paintNow, layout, selectedIndex]);

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
    window.__bonsai = { scrollSweep };
    return () => {
      if (window.__bonsai?.scrollSweep === scrollSweep) delete window.__bonsai;
    };
  }, []);

  const rowAtMouseY = (yCss: number, scrollTop: number): number | null => {
    const row = rowAtPoint(yCss, scrollTop);
    return row >= 0 && row < propsRef.current.layout.nodes.length ? row : null;
  };

  // Scroll handler ONLY records scrollTop and schedules one rAF paint (§4.1).
  const handleScroll = () => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    scrollTopRef.current = scroller.scrollTop;
    lastScrollTsRef.current = performance.now();
    // Rows move under a stationary cursor while wheel-scrolling.
    if (mouseYRef.current !== null) {
      hoverRowRef.current = rowAtMouseY(mouseYRef.current, scroller.scrollTop);
    }
    schedulePaint();
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const scroller = scrollerRef.current;
    if (scroller === null) return;
    const y = e.clientY - scroller.getBoundingClientRect().top;
    mouseYRef.current = y;
    const row = rowAtMouseY(y, scroller.scrollTop);
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
    const row = rowAtMouseY(y, scroller.scrollTop);
    if (row === null) onSelect(null);
    else onSelect(row === selectedIndex ? null : row);
  };

  const spacerHeight = layout.nodes.length * METRICS.rowHeight + 8;

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
      >
        <div className="graph-spacer" style={{ height: `${spacerHeight}px` }} />
      </div>
    </div>
  );
}
