/** Pure canvas draw functions for the precomputed GraphLayout — no React.
 * The ctx is already DPR-transformed; every coordinate below is CSS px.
 * Geometry and draw order are normative per contract M2-graph.md §1.3/§3.3. */

import type { GraphEdge, GraphLayout, GraphNode, RefLabel } from '../ipc';
import { TAG_BG, TAG_COLOR } from './colors';
import type { Theme } from './colors';
import { FONT_UI, METRICS } from './metrics';

export interface Viewport {
  /** Inclusive; M2b: 0. */
  firstRow: number;
  /** Inclusive; M2b: nodes.length - 1. */
  lastRow: number;
  /** CSS px; M2b: 0. */
  scrollTop: number;
  /** CSS px of the canvas. */
  width: number;
  height: number;
}

export interface Interaction {
  hoverRow: number | null;
  selectedIndex: number | null;
}

const HALF_ROW = METRICS.rowHeight / 2;
/** Long-edge middle segments are clamped to this margin around the canvas. */
const EDGE_CLAMP_MARGIN = 56;

/** x of a lane center; lanes beyond the render clamp share the last x. */
export function laneX(lane: number): number {
  return METRICS.gutter + Math.min(lane, METRICS.maxRenderLanes - 1) * METRICS.laneWidth + 8;
}

/** y of a row center after scroll translation. */
export function rowY(row: number, scrollTop: number): number {
  return row * METRICS.rowHeight + HALF_ROW - scrollTop;
}

/** Row index under a CSS-px y coordinate (may be out of range — callers check). */
export function rowAtPoint(yCss: number, scrollTop: number): number {
  return Math.floor((yCss + scrollTop) / METRICS.rowHeight);
}

/** Relative date: "now", "5m", "3h", "4d", "2mo", "1y". Pure, unit-testable. */
export function relativeDate(ts: number, now: number): string {
  const s = Math.max(0, now - ts);
  if (s < 60) return 'now';
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}d`;
  const mo = Math.floor(d / 30);
  if (mo < 12) return `${mo}mo`;
  return `${Math.max(1, Math.floor(d / 365))}y`;
}

// ---------- text measurement (cached) ----------

const MEASURE_CACHE_CAP = 4096;
const measureCache = new Map<string, number>();

function measure(ctx: CanvasRenderingContext2D, text: string): number {
  const key = `${ctx.font}\u0000${text}`;
  const cached = measureCache.get(key);
  if (cached !== undefined) return cached;
  const w = ctx.measureText(text).width;
  if (measureCache.size >= MEASURE_CACHE_CAP) measureCache.clear(); // drop-all on overflow
  measureCache.set(key, w);
  return w;
}

/** Ellipsis truncation via binary search over measureText (cached). */
export function truncateToWidth(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxPx: number,
): string {
  if (maxPx <= 0) return '';
  if (measure(ctx, text) <= maxPx) return text;
  const ellipsis = '…';
  let lo = 0;
  let hi = text.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (measure(ctx, text.slice(0, mid) + ellipsis) <= maxPx) lo = mid;
    else hi = mid - 1;
  }
  return lo === 0 ? '' : text.slice(0, lo) + ellipsis;
}

// ---------- edges (§1.3 three-segment render rule) ----------

/** One-row segment: straight vertical if same x, else cubic bézier with
 * vertical tangents — control points (x1, y1+14) and (x2, y2-14). */
function segmentTo(
  ctx: CanvasRenderingContext2D,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): void {
  ctx.moveTo(x1, y1);
  if (x1 === x2) ctx.lineTo(x2, y2);
  else ctx.bezierCurveTo(x1, y1 + HALF_ROW, x2, y2 - HALF_ROW, x2, y2);
}

function drawEdge(
  ctx: CanvasRenderingContext2D,
  e: GraphEdge,
  nodes: readonly GraphNode[],
  vp: Viewport,
  theme: Theme,
): void {
  const fromLane = nodes[e.from].lane;
  const toLane = nodes[e.to].lane;
  const fx = laneX(fromLane);
  const fy = rowY(e.from, vp.scrollTop);
  const tx = laneX(toLane);
  const ty = rowY(e.to, vp.scrollTop);

  ctx.strokeStyle = theme.laneColors[e.lane % 10];
  ctx.beginPath();
  if (e.to === e.from + 1) {
    segmentTo(ctx, fx, fy, tx, ty);
  } else {
    const mx = laneX(e.lane);
    const yTop = rowY(e.from + 1, vp.scrollTop);
    const yBot = rowY(e.to - 1, vp.scrollTop);
    const clampTop = -EDGE_CLAMP_MARGIN;
    const clampBot = vp.height + EDGE_CLAMP_MARGIN;
    // top curve fromLane -> e.lane (skip when fully above the clamp window)
    if (yTop >= clampTop) segmentTo(ctx, fx, fy, mx, yTop);
    // middle straight run, y-range clamped — never emit far-off-canvas coords
    const runTop = Math.max(yTop, clampTop);
    const runBot = Math.min(yBot, clampBot);
    if (runBot > runTop) {
      ctx.moveTo(mx, runTop);
      ctx.lineTo(mx, runBot);
    }
    // bottom curve e.lane -> toLane (skip when fully below the clamp window)
    if (yBot <= clampBot) segmentTo(ctx, mx, yBot, tx, ty);
  }
  ctx.stroke();
}

// ---------- ref pills (§3.4) ----------

interface PillStyle {
  fill: string;
  text: string;
  border: string | null;
  label: string;
}

function pillStyle(ref: RefLabel, node: GraphNode, theme: Theme): PillStyle {
  const laneColor = theme.laneColors[node.lane % 10];
  const laneAlpha = theme.laneColorsAlpha[node.lane % 10];
  switch (ref.kind) {
    case 'localBranch':
      return ref.isHead
        ? { fill: laneColor, text: theme.accentText, border: null, label: `⌂ ${ref.name}` }
        : { fill: laneAlpha, text: laneColor, border: laneColor, label: ref.name };
    case 'remoteBranch':
      return { fill: theme.bg2, text: theme.text2, border: theme.border, label: ref.name };
    case 'tag':
      return { fill: TAG_BG, text: TAG_COLOR, border: TAG_COLOR, label: `# ${ref.name}` };
    case 'head':
      return { fill: theme.danger, text: '#ffffff', border: null, label: ref.name };
  }
}

/** Draws one pill at x (left edge), row-centered at cy; returns its width. */
function drawPill(
  ctx: CanvasRenderingContext2D,
  x: number,
  cy: number,
  style: PillStyle,
): number {
  const maxTextPx = METRICS.pillMaxWidth - 2 * METRICS.pillPadX;
  const label = truncateToWidth(ctx, style.label, maxTextPx);
  const w = Math.ceil(measure(ctx, label)) + 2 * METRICS.pillPadX;
  const h = METRICS.pillHeight;
  const y = cy - h / 2;
  const r = h / 2;

  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
  ctx.fillStyle = style.fill;
  ctx.fill();
  if (style.border !== null) {
    ctx.strokeStyle = style.border;
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  ctx.fillStyle = style.text;
  ctx.fillText(label, x + METRICS.pillPadX, cy);
  return w;
}

/** Measures a pill without drawing (for the overflow budget check). */
function pillWidth(ctx: CanvasRenderingContext2D, style: PillStyle): number {
  const maxTextPx = METRICS.pillMaxWidth - 2 * METRICS.pillPadX;
  const label = truncateToWidth(ctx, style.label, maxTextPx);
  return Math.ceil(measure(ctx, label)) + 2 * METRICS.pillPadX;
}

// ---------- WIP (uncommitted changes) row (P1 §9.3) ----------

export interface WipSummary {
  fileCount: number;
}

/** Standard text-column x — same formula as drawGraph pass 5. */
function textColumnX(laneCount: number): number {
  return (
    METRICS.gutter + Math.min(laneCount, METRICS.maxRenderLanes) * METRICS.laneWidth + METRICS.textGap
  );
}

/** Draws the frontend-composited WIP row (P1 §9.1/§9.3). `vp.scrollTop` is the
 * RAW (un-offset) scroll position; the unchanged Rust layout's own scrollTop
 * (`layoutScrollTop`) is derived here as `vp.scrollTop - RH`. */
export function drawWipRow(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  wip: WipSummary,
  vp: Viewport,
  theme: Theme,
  hovered: boolean,
): void {
  const RH = METRICS.rowHeight;
  const layoutScrollTop = vp.scrollTop - RH;
  const headIndex = layout.headIndex;
  const headLane = headIndex !== null ? layout.nodes[headIndex].lane : 0;
  const x = laneX(headLane);
  const y = RH / 2 - vp.scrollTop;

  if (hovered) {
    ctx.fillStyle = theme.bg2;
    ctx.fillRect(0, -vp.scrollTop, vp.width, RH);
  }

  if (headIndex !== null) {
    const headY = headIndex * RH + RH / 2 - layoutScrollTop;
    const clampedY = Math.max(-56, Math.min(vp.height + 56, headY));
    ctx.save();
    ctx.setLineDash([3, 3]);
    ctx.lineWidth = 2;
    ctx.strokeStyle = theme.laneColors[headLane % 10];
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x, clampedY);
    ctx.stroke();
    ctx.restore();
  }

  ctx.save();
  ctx.setLineDash([3, 3]);
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(x, y, 4, 0, Math.PI * 2);
  ctx.fillStyle = theme.bg0;
  ctx.fill();
  ctx.strokeStyle = theme.warning;
  ctx.stroke();
  ctx.restore();

  ctx.textBaseline = 'middle';
  ctx.textAlign = 'left';
  const textX = textColumnX(layout.laneCount);
  ctx.font = `italic ${METRICS.summaryFont} ${FONT_UI}`;
  ctx.fillStyle = theme.text2;
  const label = 'Uncommitted changes';
  ctx.fillText(label, textX, y);
  const labelW = measure(ctx, label);

  ctx.font = `${METRICS.metaFont} ${FONT_UI}`;
  ctx.fillStyle = theme.text3;
  const count = `(${wip.fileCount} file${wip.fileCount === 1 ? '' : 's'})`;
  ctx.fillText(count, textX + labelW + 6, y);
}

// ---------- main entry ----------

export function drawGraph(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  visibleEdges: readonly GraphEdge[],
  vp: Viewport,
  theme: Theme,
  ix: Interaction,
): void {
  const { nodes } = layout;
  const n = nodes.length;
  const firstRow = Math.max(0, vp.firstRow);
  const lastRow = Math.min(n - 1, vp.lastRow);

  // Pass 1: clear.
  ctx.fillStyle = theme.bg0;
  ctx.fillRect(0, 0, vp.width, vp.height);

  // Pass 2: row backgrounds (selection wins over hover).
  const rowBg = (row: number, color: string): void => {
    ctx.fillStyle = color;
    ctx.fillRect(0, row * METRICS.rowHeight - vp.scrollTop, vp.width, METRICS.rowHeight);
  };
  if (ix.hoverRow !== null && ix.hoverRow !== ix.selectedIndex && ix.hoverRow < n) {
    rowBg(ix.hoverRow, theme.bg2);
  }
  if (ix.selectedIndex !== null && ix.selectedIndex < n) {
    rowBg(ix.selectedIndex, theme.selection);
  }

  // Pass 3: edges (under dots).
  ctx.lineWidth = METRICS.edgeWidth;
  ctx.lineCap = 'round';
  for (const e of visibleEdges) drawEdge(ctx, e, nodes, vp, theme);

  // Pass 4: dots.
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const x = laneX(node.lane);
    const y = rowY(row, vp.scrollTop);
    const color = theme.laneColors[node.lane % 10];
    const selected = ix.selectedIndex === row;

    // 2px bg ring behind the dot (edges passing under read cleanly).
    ctx.beginPath();
    ctx.arc(x, y, METRICS.dotRadius + METRICS.dotRingWidth, 0, Math.PI * 2);
    ctx.fillStyle = theme.bg0;
    ctx.fill();

    ctx.beginPath();
    ctx.arc(x, y, selected ? 5 : METRICS.dotRadius, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();

    if (layout.headIndex === row) {
      ctx.beginPath();
      ctx.arc(x, y, 6.5, 0, Math.PI * 2);
      ctx.strokeStyle = theme.text1;
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.lineWidth = METRICS.edgeWidth;
    }
    if (selected) {
      ctx.beginPath();
      ctx.arc(x, y, 7, 0, Math.PI * 2);
      ctx.strokeStyle = theme.accent;
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.lineWidth = METRICS.edgeWidth;
    }
  }

  // Pass 5: text row content.
  const graphAreaWidth =
    METRICS.gutter +
    Math.min(layout.laneCount, METRICS.maxRenderLanes) * METRICS.laneWidth +
    METRICS.textGap;
  const authorRight = vp.width - METRICS.dateColWidth - METRICS.colGap * 2;
  const authorLeft = authorRight - METRICS.authorColWidth;
  const dateRight = vp.width - METRICS.colGap;
  const pillBudget = Math.max(0, 0.4 * (authorLeft - graphAreaWidth));
  const now = Math.floor(Date.now() / 1000);

  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const y = rowY(row, vp.scrollTop);
    let x = graphAreaWidth;

    // 5a: ref pills, capped by the 40% budget with a trailing "+n" chip.
    const refs = node.refs;
    if (refs !== undefined && refs.length > 0) {
      ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
      ctx.textAlign = 'left';
      let shown = 0;
      for (const ref of refs) {
        const style = pillStyle(ref, node, theme);
        const w = pillWidth(ctx, style);
        if (shown > 0 && x + w > graphAreaWidth + pillBudget) break;
        x += drawPill(ctx, x, y, style) + METRICS.pillGap;
        shown++;
      }
      const hidden = refs.length - shown;
      if (hidden > 0) {
        x +=
          drawPill(ctx, x, y, {
            fill: theme.bg2,
            text: theme.text2,
            border: theme.border,
            label: `+${hidden}`,
          }) + METRICS.pillGap;
      }
      x += 8;
    }

    // 5b: summary.
    ctx.textAlign = 'left';
    ctx.font = `${METRICS.summaryFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text1;
    const summaryMax = authorLeft - METRICS.colGap - x;
    if (summaryMax > 0) ctx.fillText(truncateToWidth(ctx, node.summary, summaryMax), x, y);

    // 5c: author (right-aligned, fixed column).
    ctx.font = `${METRICS.metaFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text3;
    ctx.textAlign = 'right';
    ctx.fillText(truncateToWidth(ctx, node.author, METRICS.authorColWidth), authorRight, y);

    // 5d: relative date (right-aligned in the last 72px).
    ctx.fillText(
      truncateToWidth(ctx, relativeDate(node.ts, now), METRICS.dateColWidth),
      dateRight,
      y,
    );
  }
  ctx.textAlign = 'left';
}
