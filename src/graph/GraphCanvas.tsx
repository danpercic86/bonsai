import { useCallback, useEffect, useRef } from 'react';
import type { GraphLayout } from '../ipc';
import { resolveTheme } from './colors';
import type { Theme } from './colors';
import { drawGraph, rowAtPoint } from './draw';
import { METRICS } from './metrics';

export interface GraphCanvasProps {
  layout: GraphLayout;
  selectedIndex: number | null;
  /** Clicking a row toggles it; empty area below the rows selects null. */
  onSelect(index: number | null): void;
}

/**
 * M2b: draws the full static layout on a canvas sized `rows * 28` CSS px
 * (the pane scrolls natively). Virtualization, ResizeObserver and DPR-change
 * handling arrive in M2c; the backing store is DPR-scaled already.
 */
export function GraphCanvas({ layout, selectedIndex, onSelect }: GraphCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const themeRef = useRef<Theme | null>(null);
  const hoverRowRef = useRef<number | null>(null);
  const rafRef = useRef(0);

  // Latest props for the stable rAF paint callback.
  const propsRef = useRef({ layout, selectedIndex });
  propsRef.current = { layout, selectedIndex };

  const paint = useCallback(() => {
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

    const { layout: lay, selectedIndex: sel } = propsRef.current;
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawGraph(
      ctx,
      lay,
      lay.edges, // M2b: all edges; M2c culls to the visible set
      {
        firstRow: 0,
        lastRow: lay.nodes.length - 1,
        scrollTop: 0,
        width: canvas.clientWidth,
        height: lay.nodes.length * METRICS.rowHeight,
      },
      themeRef.current,
      { hoverRow: hoverRowRef.current, selectedIndex: sel },
    );
  }, []);

  const schedulePaint = useCallback(() => {
    if (rafRef.current === 0) rafRef.current = requestAnimationFrame(paint);
  }, [paint]);

  // Size the canvas to the container width × rows*28, DPR-scaled backing store.
  const resize = useCallback(() => {
    const host = hostRef.current;
    const canvas = canvasRef.current;
    if (host === null || canvas === null) return;
    const cssW = host.clientWidth;
    const cssH = propsRef.current.layout.nodes.length * METRICS.rowHeight;
    const dpr = window.devicePixelRatio || 1;
    canvas.style.width = `${cssW}px`;
    canvas.style.height = `${cssH}px`;
    canvas.width = Math.max(1, Math.round(cssW * dpr));
    canvas.height = Math.max(1, Math.round(cssH * dpr));
    // Synchronous paint: setting canvas.width just cleared the bitmap, and the
    // initial mount must not depend on rAF (rAF is throttled to zero in
    // hidden/occluded windows, which would leave the canvas transparent).
    paint();
  }, [paint]);

  useEffect(() => {
    resize();
    window.addEventListener('resize', resize);
    return () => window.removeEventListener('resize', resize);
  }, [resize, layout]);

  useEffect(() => {
    paint(); // selection changes repaint synchronously too
  }, [paint, selectedIndex]);

  useEffect(() => {
    return () => {
      if (rafRef.current !== 0) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  const rowFromEvent = (e: React.MouseEvent<HTMLCanvasElement>): number | null => {
    const canvas = canvasRef.current;
    if (canvas === null) return null;
    const rect = canvas.getBoundingClientRect();
    const row = rowAtPoint(e.clientY - rect.top, 0);
    return row >= 0 && row < propsRef.current.layout.nodes.length ? row : null;
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const row = rowFromEvent(e);
    if (row !== hoverRowRef.current) {
      hoverRowRef.current = row;
      schedulePaint(); // repaint only when the hovered row changed, via rAF
    }
  };

  const handleMouseLeave = () => {
    if (hoverRowRef.current !== null) {
      hoverRowRef.current = null;
      schedulePaint();
    }
  };

  const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const row = rowFromEvent(e);
    if (row === null) onSelect(null);
    else onSelect(row === selectedIndex ? null : row);
  };

  return (
    <div ref={hostRef} className="graph-canvas-host">
      <canvas
        ref={canvasRef}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onClick={handleClick}
      />
    </div>
  );
}
