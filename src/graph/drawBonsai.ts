/** Bonsai-theme canvas painters (spec 002). Pure draw functions, no React.
 *
 * These are additive branches selected by `theme.bonsai` in `draw.ts`; the
 * standard paint paths are never edited. Everything here obeys the reskin-only
 * invariant (UI contract §0): NO endpoint, coordinate, ordering or topology
 * change — only `strokeStyle`/`fillStyle`/`lineWidth` and an additive blossom.
 *
 * Split out of `draw.ts` to keep that module under the ~500-line limit. */

import type { GraphEdge, GraphNode } from '../ipc';
import type { Theme } from './colors';
import type { EffectiveMetrics } from './metrics';
import { laneX, rowY } from './geometry';
import { EDGE_CLAMP_MARGIN, segmentTo } from './draw';
import type { Viewport } from './draw';

// ---------- backdrop (§4.1 paper / soil) ----------

/** Near-flat canvas backdrop, painted as the first fill (replaces the `bg0`
 *  clear in the standard theme). A single vertical gradient when the season
 *  supplies distinct top/bottom endpoints (≤3% luminance spread), else a flat
 *  fill. Repaints only the visible viewport rect — never a full-history surface. */
export function drawBonsaiBackdrop(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  theme: Theme,
): void {
  if (theme.graphBackdropTop === theme.graphBackdropBottom) {
    ctx.fillStyle = theme.graphBackdrop;
  } else {
    const g = ctx.createLinearGradient(0, 0, 0, height);
    g.addColorStop(0, theme.graphBackdropTop);
    g.addColorStop(1, theme.graphBackdropBottom);
    ctx.fillStyle = g;
  }
  ctx.fillRect(0, 0, width, height);
}

// ---------- tapered edges (§3 "older is thicker") ----------

/** Bonsai edge: the SAME three-segment bezier as `drawEdge` (endpoints and
 *  control points verbatim), but each segment stroked separately at a stepped
 *  `lineWidth` — tip (newer/top) thinnest, trunk (older/bottom) thickest. A
 *  single-row adjacent edge uses the branch width (no visible taper over one
 *  row). `lineCap = 'round'` (set by the caller) butts the stepped widths. */
export function drawBonsaiEdge(
  ctx: CanvasRenderingContext2D,
  e: GraphEdge,
  nodes: readonly GraphNode[],
  vp: Viewport,
  theme: Theme,
  m: EffectiveMetrics,
): void {
  const halfRow = m.rowHeight / 2;
  const fromLane = nodes[e.from].lane;
  const toLane = nodes[e.to].lane;
  const fx = laneX(fromLane, m);
  const fy = rowY(e.from, vp.scrollTop, m);
  const tx = laneX(toLane, m);
  const ty = rowY(e.to, vp.scrollTop, m);

  ctx.strokeStyle = theme.laneColors[e.lane % 10];

  if (e.to === e.from + 1) {
    ctx.lineWidth = theme.edgeBranchWidth;
    ctx.beginPath();
    segmentTo(ctx, fx, fy, tx, ty, halfRow);
    ctx.stroke();
    return;
  }

  const mx = laneX(e.lane, m);
  const yTop = rowY(e.from + 1, vp.scrollTop, m);
  const yBot = rowY(e.to - 1, vp.scrollTop, m);
  const clampTop = -EDGE_CLAMP_MARGIN;
  const clampBot = vp.height + EDGE_CLAMP_MARGIN;

  // top curve fromLane -> e.lane (tip: thinnest, toward the newer tip)
  if (yTop >= clampTop) {
    ctx.lineWidth = theme.edgeTipWidth;
    ctx.beginPath();
    segmentTo(ctx, fx, fy, mx, yTop, halfRow);
    ctx.stroke();
  }
  // middle straight run (branch width), y-range clamped as in `drawEdge`
  const runTop = Math.max(yTop, clampTop);
  const runBot = Math.min(yBot, clampBot);
  if (runBot > runTop) {
    ctx.lineWidth = theme.edgeBranchWidth;
    ctx.beginPath();
    ctx.moveTo(mx, runTop);
    ctx.lineTo(mx, runBot);
    ctx.stroke();
  }
  // bottom curve e.lane -> toLane (trunk: thickest, toward the older parent)
  if (yBot <= clampBot) {
    ctx.lineWidth = theme.edgeTrunkWidth;
    ctx.beginPath();
    segmentTo(ctx, mx, yBot, tx, ty, halfRow);
    ctx.stroke();
  }
}

// ---------- additive blossom (§2.3) ----------

/** HEAD → full 5-petal blossom; selected (not HEAD) → single top bud. */
export type BlossomKind = 'blossom' | 'bud';

/** Additive blossom painted BEHIND the avatar disc (§2.3) — never occludes the
 *  initials or the state rings, which remain the authoritative carriers. Filled
 *  circles at `theme.blossomAccent` / `theme.blossomAlpha`, no stroke.
 *
 *  `x`/`y` are the glyph center. Task 4 (sway) applies its ≤1px settle offset by
 *  passing an already-offset `x` here (same offset used for the disc/rings), so
 *  the whole glyph translates as a unit — no rewrite needed. Callers only invoke
 *  this in the Bonsai branch, so `blossomAlpha` is always > 0. */
export function drawBlossom(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  theme: Theme,
  m: EffectiveMetrics,
  kind: BlossomKind,
): void {
  const petalR = m.avatarRadius * 0.62;
  const ringR = m.avatarRadius * 0.95;
  const count = kind === 'blossom' ? 5 : 1;
  const prevAlpha = ctx.globalAlpha;
  ctx.globalAlpha = theme.blossomAlpha;
  ctx.fillStyle = theme.blossomAccent;
  for (let i = 0; i < count; i++) {
    const ang = -Math.PI / 2 + i * ((Math.PI * 2) / 5);
    const px = x + Math.cos(ang) * ringR;
    const py = y + Math.sin(ang) * ringR;
    ctx.beginPath();
    ctx.arc(px, py, petalR, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.globalAlpha = prevAlpha;
}
