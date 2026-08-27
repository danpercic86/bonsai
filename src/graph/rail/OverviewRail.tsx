/** Spec-005: the overview rail — one HiDPI canvas inside .graph-canvas-host
 *  (14px, full height, right: rightInset). Mounted ONLY while visible (the
 *  plan's zero-idle-cost guarantee — GraphCanvas owns the mount condition);
 *  this component owns paint, pointer, the hover-linger timer, and its own
 *  scroll subscription. `aria-hidden`: a redundant pointer affordance (UI §5).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { RefObject } from 'react';
import type { GraphLayout } from '../../ipc';
import { modelToDisplay } from '../foldModel';
import type { FoldModel } from '../foldModel';
import { drawRail } from './drawRail';
import type { RailTheme } from './drawRail';
import {
  RAIL_LINGER_MS,
  RAIL_WIDTH_PX,
  buildRailBuckets,
  coalesceTicks,
  hitTick,
  scrollTopForRailY,
  thumbRect,
} from './railMath';

/** The rail bundle RepoWorkspace assembles (see repoWorkspace/railProps.ts)
 *  and threads through WorkspaceGraphPane into GraphCanvas. */
export interface RailInput {
  /** Bumped when the graph stream reaches `done`; 0 = no done yet (the
   *  density/pip layers stay off — UI §4 "Streaming" state). */
  generation: number;
  /** graphMinimapAlwaysShow (persisted). */
  alwaysShow: boolean;
  /** A rings channel (commit search OR Ask-history) is live with matches. */
  ringsLive: boolean;
  /** MODEL rows of the live rings channel; index == the jump index below. */
  matchRows: readonly number[];
  /** MODEL row of the current search match; null on the historySearch channel. */
  currentMatchRow: number | null;
  /** Tick click → jump (index into `matchRows`). */
  onJumpToMatch(index: number): void;
}

export interface OverviewRailProps {
  rail: RailInput;
  /** DISPLAY-space layout (GraphCanvas's fold projection output). */
  layout: GraphLayout;
  foldModel: FoldModel | null;
  /** Total display rows for the scroll extent (totalRows while streaming). */
  displayRowCount: number;
  scrollerRef: RefObject<HTMLDivElement | null>;
  /** Pointer currently inside the 20px right-edge zone (GraphCanvas mousemove). */
  pointerInZone: boolean;
  /** UI §4: fade-in only on pointer-triggered reveals (latched at mount). */
  fadeIn: boolean;
  themeVersion: number;
  /** Linger expired with no keep-alive → GraphCanvas clears the hover reveal. */
  onHide(): void;
  /** Drag start/end — GraphCanvas pins the mount while a drag is live (the
   *  rail must never unmount mid-drag with pointer capture held). */
  onDraggingChange(dragging: boolean): void;
}

function readVar(style: CSSStyleDeclaration, name: string): string {
  return style.getPropertyValue(name).trim();
}

function resolveRailTheme(el: HTMLElement): RailTheme {
  const s = getComputedStyle(el);
  return {
    bg0: readVar(s, '--bg-0'),
    bg1: readVar(s, '--bg-1'),
    border: readVar(s, '--border'),
    text1: readVar(s, '--text-1'),
    text2: readVar(s, '--text-2'),
    text3: readVar(s, '--text-3'),
    accent: readVar(s, '--accent'),
    matchRing: readVar(s, '--match-ring'),
  };
}

export function OverviewRail({
  rail,
  layout,
  foldModel,
  displayRowCount,
  scrollerRef,
  pointerInZone,
  fadeIn,
  themeVersion,
  onHide,
  onDraggingChange,
}: OverviewRailProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const themeRef = useRef<RailTheme | null>(null);
  const [height, setHeight] = useState(0);
  const [dragging, setDragging] = useState(false);
  // UI §4: the mount fade applies only to pointer reveals; latched at mount so
  // a later channel change never replays the animation.
  const [fade] = useState(fadeIn);
  const rafRef = useRef(0);
  const lingerRef = useRef(0);
  const selfHoverRef = useRef(false);
  const draggingRef = useRef(false);
  const grabOffsetRef = useRef(0);
  // Drag transitions are mirrored to GraphCanvas (mount pin). Latest-callback
  // ref so the unmount cleanup below never reports through a stale closure.
  const onDraggingChangeRef = useRef(onDraggingChange);
  onDraggingChangeRef.current = onDraggingChange;
  const setDrag = useCallback((d: boolean) => {
    draggingRef.current = d;
    setDragging(d);
    onDraggingChangeRef.current(d);
  }, []);
  // Defensive: if the rail ever unmounts with a drag flag still up, release
  // the pin so GraphCanvas cannot keep it mounted forever.
  useEffect(() => () => onDraggingChangeRef.current(false), []);

  // Buckets: built once per stream `done` (generation) / fold change — NOT per
  // streamed batch (layout identity bumps per batch; the closure reads the
  // current layout at re-run time; stale-while-streaming is deliberate, §4).
  const buckets = useMemo(
    () => (rail.generation > 0 && layout.nodes.length > 0 ? buildRailBuckets(layout) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [rail.generation, foldModel],
  );

  // Match ticks: cheap O(matches) remap per keystroke / fold toggle. A match
  // hidden inside a collapsed span clamps to its pill row (tick stays visible).
  const matchDisplayRows = useMemo(
    () =>
      foldModel === null ? rail.matchRows : rail.matchRows.map((r) => modelToDisplay(foldModel, r)),
    [rail.matchRows, foldModel],
  );
  const currentDisplayRow = useMemo(
    () =>
      rail.currentMatchRow === null
        ? null
        : foldModel === null
          ? rail.currentMatchRow
          : modelToDisplay(foldModel, rail.currentMatchRow),
    [rail.currentMatchRow, foldModel],
  );
  const ticks = useMemo(
    () =>
      rail.ringsLive
        ? coalesceTicks(matchDisplayRows, currentDisplayRow, displayRowCount, height)
        : [],
    [rail.ringsLive, matchDisplayRows, currentDisplayRow, displayRowCount, height],
  );

  const paint = useCallback(() => {
    const canvas = canvasRef.current;
    const scroller = scrollerRef.current;
    if (canvas === null || scroller === null) return;
    const ctx = canvas.getContext('2d');
    if (ctx === null) return;
    themeRef.current ??= resolveRailTheme(canvas);
    // Track the live scrollbar inset so the rail hugs (never covers) it (AC7).
    const inset = `${scroller.offsetWidth - scroller.clientWidth}px`;
    if (canvas.style.right !== inset) canvas.style.right = inset;
    const h = canvas.clientHeight;
    drawRail(ctx, {
      width: RAIL_WIDTH_PX,
      height: h,
      buckets,
      ticks,
      thumb: thumbRect(scroller.scrollTop, scroller.scrollHeight, scroller.clientHeight, h),
      theme: themeRef.current,
      dragging: draggingRef.current,
    });
  }, [buckets, ticks, scrollerRef]);
  const paintRef = useRef(paint);
  paintRef.current = paint;

  const schedulePaint = useCallback(() => {
    if (rafRef.current === 0) {
      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = 0;
        paintRef.current();
      });
    }
  }, []);

  // Mount: size the HiDPI backing store (ResizeObserver on the canvas — it is
  // stretched top:0/bottom:0) and subscribe to the scroller's scroll events.
  // Unmount removes both (nothing lives past the component — zero idle cost).
  useEffect(() => {
    const canvas = canvasRef.current;
    const scroller = scrollerRef.current;
    if (canvas === null) return;
    const resize = (): void => {
      const h = canvas.clientHeight;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.round(RAIL_WIDTH_PX * dpr));
      canvas.height = Math.max(1, Math.round(h * dpr));
      const ctx = canvas.getContext('2d');
      if (ctx !== null) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      setHeight(h);
      paintRef.current();
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    const onScroll = (): void => schedulePaint();
    scroller?.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      ro.disconnect();
      scroller?.removeEventListener('scroll', onScroll);
      if (rafRef.current !== 0) cancelAnimationFrame(rafRef.current);
      window.clearTimeout(lingerRef.current);
    };
  }, [scrollerRef, schedulePaint]);

  // Data/theme changes repaint synchronously (scroll stays on the rAF path).
  useEffect(() => {
    themeRef.current = null; // themeVersion bump re-resolves the CSS vars
    paint();
  }, [paint, themeVersion, dragging]);

  // Hover linger (§4): when the pointer leaves the zone (and the rail itself)
  // and no drag is live, expire the reveal after 300ms. onHide only clears the
  // hover flag — a search-forced / always-show rail stays mounted regardless.
  const armLinger = useCallback(() => {
    window.clearTimeout(lingerRef.current);
    lingerRef.current = window.setTimeout(() => {
      if (!selfHoverRef.current && !draggingRef.current) onHide();
    }, RAIL_LINGER_MS);
  }, [onHide]);
  useEffect(() => {
    if (pointerInZone) window.clearTimeout(lingerRef.current);
    else armLinger();
  }, [pointerInZone, armLinger]);

  const endDrag = (e: React.PointerEvent<HTMLCanvasElement>): void => {
    if (!draggingRef.current) return;
    setDrag(false);
    e.currentTarget.releasePointerCapture(e.pointerId);
    if (!pointerInZone && !selfHoverRef.current) armLinger();
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>): void => {
    if (e.button !== 0) return;
    const scroller = scrollerRef.current;
    const canvas = canvasRef.current;
    if (scroller === null || canvas === null) return;
    const y = e.clientY - canvas.getBoundingClientRect().top;
    const tick = hitTick(ticks, y);
    if (tick !== null && tick.matchIndex >= 0) {
      rail.onJumpToMatch(tick.matchIndex);
      return;
    }
    const h = canvas.clientHeight;
    const thumb = thumbRect(scroller.scrollTop, scroller.scrollHeight, scroller.clientHeight, h);
    if (y >= thumb.top && y <= thumb.top + thumb.height) {
      grabOffsetRef.current = y - thumb.top; // preserve the grab offset
    } else {
      // Click outside the thumb: center the viewport there, then keep dragging.
      grabOffsetRef.current = thumb.height / 2;
      scroller.scrollTop = scrollTopForRailY(
        y,
        grabOffsetRef.current,
        scroller.scrollHeight,
        scroller.clientHeight,
        h,
      );
    }
    setDrag(true);
    canvas.setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>): void => {
    const scroller = scrollerRef.current;
    const canvas = canvasRef.current;
    if (scroller === null || canvas === null) return;
    const y = e.clientY - canvas.getBoundingClientRect().top;
    if (draggingRef.current) {
      scroller.scrollTop = scrollTopForRailY(
        y,
        grabOffsetRef.current,
        scroller.scrollHeight,
        scroller.clientHeight,
        canvas.clientHeight,
      );
      return;
    }
    // Cursor affordances (§5): pointer near a tick, grab over the thumb.
    const nearTick = hitTick(ticks, y) !== null; // ±TICK_HIT_SLOP_PX inside hitTick
    const thumb = thumbRect(
      scroller.scrollTop,
      scroller.scrollHeight,
      scroller.clientHeight,
      canvas.clientHeight,
    );
    const cursor = nearTick
      ? 'pointer'
      : y >= thumb.top && y <= thumb.top + thumb.height
        ? 'grab'
        : 'default';
    if (canvas.style.cursor !== cursor) canvas.style.cursor = cursor;
  };

  return (
    <canvas
      ref={canvasRef}
      className={`graph-rail${fade ? ' graph-rail--fade' : ''}${dragging ? ' graph-rail--dragging' : ''}`}
      data-testid="graph-rail"
      aria-hidden="true"
      onPointerEnter={() => {
        selfHoverRef.current = true;
        window.clearTimeout(lingerRef.current);
      }}
      onPointerLeave={() => {
        selfHoverRef.current = false;
        if (!pointerInZone && !draggingRef.current) armLinger();
      }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onWheel={(e) => {
        // Forward the wheel to the scroller — no dead zone over the rail.
        const scroller = scrollerRef.current;
        if (scroller !== null) scroller.scrollTop += e.deltaY;
      }}
    />
  );
}
