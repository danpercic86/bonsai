// Spec-007 paint seam: ZERO draw.ts edits (plan §Approach). The replay painter
// clamps the Viewport's row window to `[max(scrollFirstRow, cutoff), lastRow]`,
// pre-filters the edge list to `from >= cutoff` (a revealed child's edge to its
// already-revealed parent; `to > from >= c` always holds for the parent end),
// and calls the existing `drawGraph` unchanged. Unrevealed rows are therefore
// ABSENT — bare `--graph-canvas-bg` (drawGraph's pass-1 clear covers the full
// canvas). The frontier pulse paints AFTER drawGraph, on top. Owns NO topology.
import type { GraphLayout } from '../../ipc';
import { drawGraph } from '../draw';
import type { Theme } from '../colors';
import { isDarkBg } from '../colors';
import type { GraphDisplayOptions } from '../rightColumns';
import type { EffectiveMetrics } from '../metrics';
import type { EdgeIndex } from '../edgeIndex';
import { edgesInRange } from '../edgeIndex';
import { visibleRowRange } from '../viewport';
import { laneX, rowY } from '../geometry';
import { MAX_PULSE_RINGS, pulseAlpha, pulseRingRadius } from './replayPulse';
import type { ReplayPulse } from './replayPulse';

/** Rows painted beyond the visible window on each side (GraphCanvas parity). */
const OVERSCAN = 4;

export interface ReplayPaintArgs {
  ctx: CanvasRenderingContext2D;
  layout: GraphLayout;
  edgeIndex: EdgeIndex;
  theme: Theme;
  metrics: EffectiveMetrics;
  display: GraphDisplayOptions;
  /** Reveal cutoff c ∈ [0, n]; rows [c, n) are painted. */
  cutoff: number;
  scrollTop: number;
  /** Canvas CSS size. */
  width: number;
  height: number;
  /** CSS px reserved on the right for the overlay scroller's scrollbar. */
  rightInset: number;
  /** Live frontier pulses (already pruned by the caller). */
  pulses: readonly ReplayPulse[];
  /** `performance.now()` for pulse elapsed time. */
  now: number;
}

export function paintReplay(a: ReplayPaintArgs): void {
  const { ctx, layout, metrics: m, cutoff } = a;
  const n = layout.nodes.length;
  const range = visibleRowRange(a.scrollTop, 0, m.rowHeight, a.height, n, OVERSCAN);
  // Min-row clamp: nothing above the cutoff exists yet. firstRow may exceed
  // lastRow (blank canvas) — drawGraph then only runs its pass-1 clear.
  const firstRow = Math.max(range.firstRow, cutoff);
  const lastRow = range.lastRow;
  const vp = {
    firstRow,
    lastRow,
    scrollTop: range.layoutScrollTop,
    width: a.width,
    height: a.height,
    rightInset: a.rightInset,
  };
  const edges =
    firstRow <= lastRow
      ? edgesInRange(a.layout, a.edgeIndex, firstRow, lastRow).filter((e) => e.from >= cutoff)
      : [];
  drawGraph(
    ctx,
    layout,
    edges,
    vp,
    a.theme,
    {
      hoverRow: null,
      selectedIndex: null,
      matchRows: null,
      verifyStatus: null,
      flash: null,
      sway: null,
      foldRows: null,
    },
    a.display,
    m,
  );
  drawPulses(a, firstRow, lastRow);
}

/** Frontier pulses (UI §4): row tint in the row's LANE color at the pulse
 *  alpha + a halo ring expanding from the dot. Rings capped at
 *  MAX_PULSE_RINGS per paint — beyond that, tint only. */
function drawPulses(a: ReplayPaintArgs, firstRow: number, lastRow: number): void {
  if (a.pulses.length === 0 || firstRow > lastRow) return;
  const { ctx, metrics: m, theme } = a;
  const dark = isDarkBg(theme.bg0);
  const prevAlpha = ctx.globalAlpha;
  let rings = 0;
  for (const p of a.pulses) {
    if (p.row < firstRow || p.row > lastRow || p.row < a.cutoff) continue;
    const alpha = pulseAlpha(a.now - p.start, dark);
    if (alpha <= 0) continue;
    const lane = a.layout.nodes[p.row].lane;
    const color = theme.laneColors[lane % 10];
    ctx.globalAlpha = alpha;
    ctx.fillStyle = color;
    ctx.fillRect(0, p.row * m.rowHeight - a.scrollTop, a.width, m.rowHeight);
    if (rings < MAX_PULSE_RINGS) {
      rings += 1;
      ctx.beginPath();
      ctx.arc(
        laneX(lane, m),
        rowY(p.row, a.scrollTop, m),
        pulseRingRadius(a.now - p.start, m.avatarSelRingRadius),
        0,
        Math.PI * 2,
      );
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.stroke();
    }
  }
  ctx.globalAlpha = prevAlpha;
}
