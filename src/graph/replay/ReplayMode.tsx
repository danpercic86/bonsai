// Spec-007: the replay overlay — absolutely positioned over the graph-pane
// column (z-index 6, UI contract §3.1) while the working GraphCanvas stays
// mounted underneath with `active={false}` (frozen on its last-good bitmap).
// Exit = unmount; the working canvas's layout/selection/scroller are NEVER
// touched, so exact restore holds by construction (plan §Approach). Replays a
// SNAPSHOT of the loaded (unfolded, filter-baked) layout taken at entry.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import type { GraphLayout } from '../../ipc';
import { resolveTheme } from '../colors';
import type { GraphStyle, Theme } from '../colors';
import type { GraphSeason } from '../palettes';
import type { GraphDisplayOptions } from '../rightColumns';
import type { EffectiveMetrics } from '../metrics';
import { buildEdgeIndex } from '../edgeIndex';
import { backingStoreSize, spacerHeight } from '../viewport';
import { useCanvasResizeObserver } from '../useCanvasResizeObserver';
import { buildReplayModel } from './replayModel';
import { useReplayDriver } from './useReplayDriver';
import { paintReplay } from './replayPaint';
import { prunePulses } from './replayPulse';
import { ReplayTransport } from './ReplayTransport';

export interface ReplayModeProps {
  /** Entry snapshot — deliberately NOT the live graph prop (a watcher refresh
   *  mid-replay must not desync the model built from it). */
  layout: GraphLayout;
  metrics: EffectiveMetrics;
  display: GraphDisplayOptions;
  graphStyle: GraphStyle;
  graphSeason: GraphSeason;
  themeVersion: number;
  metricsVersion: number;
  reducedMotion: boolean;
  onExit(): void;
}

export function ReplayMode({
  layout,
  metrics,
  display,
  graphStyle,
  graphSeason,
  themeVersion,
  metricsVersion,
  reducedMotion,
  onExit,
}: ReplayModeProps) {
  const model = useMemo(() => buildReplayModel(layout, reducedMotion), [layout, reducedMotion]);
  const edgeIndex = useMemo(() => buildEdgeIndex(layout), [layout]);
  const driver = useReplayDriver(model);
  const { state } = driver;

  const rootRef = useRef<HTMLDivElement>(null);
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const playRef = useRef<HTMLButtonElement>(null);
  const sliderRef = useRef<HTMLDivElement>(null);
  const themeRef = useRef<Theme | null>(null);
  const cssSizeRef = useRef({ w: 0, h: 0 });
  const rafRef = useRef(0); // unused slot for useCanvasResizeObserver's cleanup

  // Latest paint inputs for the stable paint callback (GraphCanvas pattern).
  const paintDeps = { state, metrics, display };
  const paintDepsRef = useRef(paintDeps);
  paintDepsRef.current = paintDeps;

  const paintNow = useCallback(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d') ?? null;
    if (canvas === null || ctx === null) return;
    themeRef.current ??= resolveTheme(canvas, graphStyle, graphSeason);
    const { state: s, metrics: m, display: d } = paintDepsRef.current;
    const now = performance.now();
    // Pulses live only during playback — paused/finished paints are clean.
    const pulses = s.status === 'playing' ? prunePulses(driver.pulsesRef.current, now) : [];
    const scroller = scrollerRef.current;
    paintReplay({
      ctx,
      layout,
      edgeIndex,
      theme: themeRef.current,
      metrics: m,
      display: d,
      cutoff: s.cutoff,
      scrollTop: scroller?.scrollTop ?? 0,
      width: cssSizeRef.current.w,
      height: cssSizeRef.current.h,
      rightInset: scroller !== null ? scroller.offsetWidth - scroller.clientWidth : 0,
      pulses,
      now,
    });
  }, [layout, edgeIndex, graphStyle, graphSeason, driver.pulsesRef]);

  const resize = useCallback(() => {
    const host = hostRef.current;
    const canvas = canvasRef.current;
    if (host === null || canvas === null) return;
    const cssW = host.clientWidth;
    const cssH = host.clientHeight;
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
    paintNow();
  }, [paintNow]);
  useCanvasResizeObserver(hostRef, resize, rafRef);

  // Theme switch mid-replay re-resolves + repaints at the current cutoff (§5).
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    themeRef.current = resolveTheme(canvas, graphStyle, graphSeason);
    paintNow();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeVersion, graphStyle, graphSeason]);
  useEffect(() => {
    resize(); // metric knob changed: re-map row↔pixel + repaint
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [metricsVersion]);

  // One paint per state change — canvas and transport commit the same frame.
  // Auto-follow first: while playing with follow engaged, pin the frontier row
  // at the viewport TOP (revealed history fills downward, plan decision 6).
  const followScrollRef = useRef(false);
  useLayoutEffect(() => {
    const scroller = scrollerRef.current;
    if (scroller !== null && state.status === 'playing' && state.follow) {
      const target = Math.max(
        0,
        Math.min(state.cutoff * metrics.rowHeight, scroller.scrollHeight - scroller.clientHeight),
      );
      if (scroller.scrollTop !== target) {
        followScrollRef.current = true; // suppress the scroll handler's repaint
        scroller.scrollTop = target;
      }
    }
    paintNow();
  }, [state, metrics.rowHeight, display, paintNow]);

  // Manual scroll repaints once; the programmatic auto-follow write above
  // already painted this frame (its scroll event would double-paint).
  const onScroll = useCallback(() => {
    if (followScrollRef.current) {
      followScrollRef.current = false;
      return;
    }
    paintNow();
  }, [paintNow]);

  // Entry: autoplay when possible (spec AC1 / plan decision 4) + focus the
  // play button (scrubber under reduced motion, §3.3). Mount-only.
  const playOnEntryRef = useRef(false);
  useEffect(() => {
    if (playOnEntryRef.current) return;
    playOnEntryRef.current = true;
    if (model.canPlay) driver.togglePlay();
    (playRef.current ?? sliderRef.current)?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Overlay-owned keyboard (§3.3). stopPropagation keeps handled keys from the
  // workspace window listeners (which also gate on replayOpen as a backstop).
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>): void => {
    const handled = (): void => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (e.key === 'Escape') {
      handled();
      onExit();
    } else if ((e.key === ' ' || e.key.toLowerCase() === 'k') && !reducedMotion) {
      handled();
      driver.togglePlay();
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      // Speed radios keep their native arrow navigation (radiogroup a11y).
      if ((e.target as HTMLElement).matches('input[type="radio"]')) return;
      handled();
      driver.stepBy(e.key === 'ArrowRight' ? 1 : -1, e.shiftKey);
    } else if (e.key === 'Home') {
      handled();
      driver.toStart();
    } else if (e.key === 'End') {
      handled();
      driver.toEnd();
    } else if (e.key === 'Tab') {
      // Focus trap: cycle within the transport controls only (§3.3).
      const root = rootRef.current;
      if (root === null) return;
      const items = Array.from(
        // input:checked (not every radio): the speed radiogroup is ONE Tab
        // stop — arrows move within it (§3.3 amended).
        root.querySelectorAll<HTMLElement>('button, [role="slider"], input:checked'),
      );
      if (items.length === 0) return;
      const i = items.indexOf(document.activeElement as HTMLElement);
      handled();
      const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : i >= items.length - 1 ? 0 : i + 1;
      items[next].focus();
    }
  };

  const frontierLabel =
    state.cutoff < model.n
      ? new Date(layout.nodes[state.cutoff].committerTs * 1000).toLocaleDateString(undefined, {
          month: 'short',
          year: 'numeric',
        })
      : null;

  return (
    <div
      ref={rootRef}
      className="graph-replay-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Replay history"
      tabIndex={-1}
      onKeyDown={onKeyDown}
    >
      <div ref={hostRef} className="graph-replay-canvas-host">
        <canvas ref={canvasRef} className="graph-canvas" data-testid="graph-replay-canvas" />
        <div
          ref={scrollerRef}
          className="graph-scroll graph-replay-scroll"
          data-testid="graph-replay-scroller"
          onScroll={onScroll}
          onWheel={driver.disengageFollow}
          onPointerDown={driver.disengageFollow}
        >
          <div
            className="graph-spacer"
            style={{ height: `${spacerHeight(model.n, 0, metrics.rowHeight)}px` }}
          />
        </div>
        {layout.truncated && (
          <div className="graph-truncated-banner">
            History truncated to the most recent 100,000 commits
          </div>
        )}
      </div>
      <ReplayTransport
        status={state.status}
        reducedMotion={reducedMotion}
        revealed={model.n - state.cutoff}
        total={model.n}
        playheadMs={state.playheadMs}
        totalMs={model.totalMs}
        frontierLabel={frontierLabel}
        speed={state.speed}
        onTogglePlay={driver.togglePlay}
        onScrub={driver.scrub}
        onSetSpeed={driver.setSpeedTo}
        onExit={onExit}
        playRef={playRef}
        sliderRef={sliderRef}
      />
    </div>
  );
}
