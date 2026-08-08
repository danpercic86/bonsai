/** Pure canvas draw functions for the precomputed GraphLayout — no React.
 * The ctx is already DPR-transformed; every coordinate below is CSS px.
 * Geometry and draw order are normative per contract M2-graph.md §1.3/§3.3.
 *
 * P51b split this file: the LEFT ref-band subsystem moved to `refLabels.ts`,
 * the per-row RIGHT text pass to `drawRowText.ts`, the right-column model to
 * `rightColumns.ts`, cached measurement to `textMeasure.ts`, and the date/oid
 * formatters to `dates.ts`. This module keeps geometry, edges, avatars, the
 * stash/WIP rows, and the `drawGraph` orchestration. */

import type { GraphEdge, GraphLayout, GraphNode, VerifyStatus } from '../ipc';
import { STASH_COLOR } from './colors';
import type { Theme } from './colors';
import { AVATAR, FONT_UI } from './metrics';
import type { EffectiveMetrics } from './metrics';
import { drawRowText } from './drawRowText';
import { drawRefLabelAt, drawStashIcon, groupRefs, layoutRefLabels } from './refLabels';
import { computeRightColumns } from './rightColumns';
import type { GraphDisplayOptions } from './rightColumns';
import { measure } from './textMeasure';

// P51b: relativeDate lives in dates.ts now; re-export it so the many existing
// `import { relativeDate } from '../graph/draw'` call sites keep working.
export { relativeDate } from './dates';

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
  /** P7e §13.2: CSS px reserved on the RIGHT for the vertical scrollbar (the
   *  `.graph-scroll` overlay's native scrollbar paints over the full-width
   *  canvas's right edge). Absent/`undefined` → treated as 0, i.e. identical to
   *  pre-P7e behavior. */
  rightInset?: number;
}

export interface Interaction {
  hoverRow: number | null;
  selectedIndex: number | null;
  /** P50b: rows carrying a commit-search match → an outer `--match-ring` ring.
   *  `null` when search is closed / has no visible matches (no ring pass). */
  matchRows: Set<number> | null;
  /** P58c: oid → signature verdict for the LIT badge (visible rows only,
   *  cached by oid). `null` / a missing oid ⇒ the faint P51 stub. */
  verifyStatus: ReadonlyMap<string, VerifyStatus> | null;
}

/** Long-edge middle segments are clamped to this margin around the canvas. */
const EDGE_CLAMP_MARGIN = 56;

/** x of a lane center; lanes beyond the render clamp share the last x.
 *  P7 §1.2: gains the fixed LEFT ref-band offset (`refColWidth`); the +8 lane
 *  inset is preserved. The global right-shift flows through edges automatically
 *  (they use `laneX`/`rowY`). */
export function laneX(lane: number, m: EffectiveMetrics): number {
  return (
    m.refColWidth +
    m.gutter +
    Math.min(lane, m.maxRenderLanes - 1) * m.laneWidth +
    8
  );
}

/** P7 §1.2: right edge of the graph area (clamped lane band), independent of
 *  the +8 lane inset. Internal — feeds `summaryStartX`. */
function graphAreaRight(laneCount: number, m: EffectiveMetrics): number {
  return (
    m.refColWidth +
    m.gutter +
    Math.min(laneCount, m.maxRenderLanes) * m.laneWidth
  );
}

/** P7 §1.2: summary column origin (replaces the old `textColumnX`; no pills
 *  live here now — refs moved to the LEFT band). */
export function summaryStartX(laneCount: number, m: EffectiveMetrics): number {
  return graphAreaRight(laneCount, m) + m.textGap;
}

/** P7 §1.2: fixed LEFT ref-column layout window (analog of the old `pillArea`,
 *  but NOT a function of viewport width or laneCount — the band is fixed). */
export function refColArea(m: EffectiveMetrics): { startX: number; budget: number } {
  return {
    startX: m.refColPadLeft,
    budget: Math.max(0, m.refColWidth - m.refColPadLeft - m.refColPadRight),
  };
}

/** y of a row center after scroll translation. */
export function rowY(row: number, scrollTop: number, m: EffectiveMetrics): number {
  return row * m.rowHeight + m.rowHeight / 2 - scrollTop;
}

/** Row index under a CSS-px y coordinate (may be out of range — callers check). */
export function rowAtPoint(yCss: number, scrollTop: number, m: EffectiveMetrics): number {
  return Math.floor((yCss + scrollTop) / m.rowHeight);
}

// ---------- avatar (P7 §2, replaces the pass-4 dot) ----------

/** P7 §2.2: 1–2 uppercased chars from an author display name. Surrogate-safe
 *  (Array.from splits by code point). Examples: "Dan Percic"→"DP",
 *  "torvalds"→"TO", "x"→"X", ""→"?", "  Grace  Hopper "→"GH". */
export function initials(name: string): string {
  const tokens = name
    .trim()
    .split(/\s+/)
    .filter((t) => t.length > 0);
  if (tokens.length === 0) return '?';
  if (tokens.length === 1) {
    const chars = Array.from(tokens[0]);
    return (chars[0] + (chars[1] ?? '')).toUpperCase();
  }
  return (Array.from(tokens[0])[0] + Array.from(tokens[1])[0]).toUpperCase();
}

/** P7 §2.3: avatar colors. `bg` is a theme-invariant hashed HSL; `text` is
 *  fixed white (legible ≥3:1 on both canvases at S=52%/L=42%). */
export interface AvatarColor {
  bg: string;
  text: string;
}

/** FNV-1a 32-bit over code points; `Math.imul` keeps the 32-bit overflow. */
function hashString(s: string): number {
  let h = 0x811c9dc5;
  for (const cp of Array.from(s)) h = Math.imul(h ^ (cp.codePointAt(0) ?? 0), 0x01000193);
  return h >>> 0;
}

/** P7 §2.3: deterministic name→color. Same name ⇒ same hue, always. */
export function avatarColor(name: string): AvatarColor {
  const hue = hashString(name.trim()) % 360;
  return { bg: `hsl(${hue}, ${AVATAR.sat}%, ${AVATAR.light}%)`, text: '#ffffff' };
}

/** P7 §2.4: avatar hit-test (shared by the tooltip hover). Uses the bg-ring
 *  radius so the whole visible disc is hoverable. */
export function avatarHit(
  px: number,
  py: number,
  cx: number,
  cy: number,
  m: EffectiveMetrics,
): boolean {
  const r = m.avatarRadius + m.avatarBgRingExtra;
  const dx = px - cx;
  const dy = py - cy;
  return dx * dx + dy * dy <= r * r;
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
  halfRow: number,
): void {
  ctx.moveTo(x1, y1);
  if (x1 === x2) ctx.lineTo(x2, y2);
  else ctx.bezierCurveTo(x1, y1 + halfRow, x2, y2 - halfRow, x2, y2);
}

function drawEdge(
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
  ctx.beginPath();
  if (e.to === e.from + 1) {
    segmentTo(ctx, fx, fy, tx, ty, halfRow);
  } else {
    const mx = laneX(e.lane, m);
    const yTop = rowY(e.from + 1, vp.scrollTop, m);
    const yBot = rowY(e.to - 1, vp.scrollTop, m);
    const clampTop = -EDGE_CLAMP_MARGIN;
    const clampBot = vp.height + EDGE_CLAMP_MARGIN;
    // top curve fromLane -> e.lane (skip when fully above the clamp window)
    if (yTop >= clampTop) segmentTo(ctx, fx, fy, mx, yTop, halfRow);
    // middle straight run, y-range clamped — never emit far-off-canvas coords
    const runTop = Math.max(yTop, clampTop);
    const runBot = Math.min(yBot, clampBot);
    if (runBot > runTop) {
      ctx.moveTo(mx, runTop);
      ctx.lineTo(mx, runBot);
    }
    // bottom curve e.lane -> toLane (skip when fully below the clamp window)
    if (yBot <= clampBot) segmentTo(ctx, mx, yBot, tx, ty, halfRow);
  }
  ctx.stroke();
}

// ---------- stash node (P10 §2.1) ----------

/** P10 §2.1: a stash node — violet disc + centered white stash glyph + violet
 *  lane ring. Draws in place of the author avatar; the caller has ALREADY drawn
 *  the bg-ring halo and will draw HEAD/selection rings afterwards. The ring
 *  matches the disc color (STASH_COLOR) rather than the row's lane color, so the
 *  stash reads as "not a real branch commit" (a deliberate choice). */
export function drawStashNode(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  m: EffectiveMetrics,
): void {
  // disc
  ctx.beginPath();
  ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
  ctx.fillStyle = STASH_COLOR;
  ctx.fill();

  // lane ring (violet, matches disc)
  ctx.beginPath();
  ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
  ctx.strokeStyle = STASH_COLOR;
  ctx.lineWidth = m.avatarRingWidth;
  ctx.stroke();

  // glyph (white, legible on the violet disc)
  const S = m.avatarRadius * 1.4;
  ctx.strokeStyle = '#ffffff';
  drawStashIcon(ctx, x - S / 2, y - S / 2, S);
}

// ---------- WIP (uncommitted changes) row (P1 §9.3) ----------

export interface WipSummary {
  fileCount: number;
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
  m: EffectiveMetrics,
): void {
  const RH = m.rowHeight;
  const layoutScrollTop = vp.scrollTop - RH;
  const headIndex = layout.headIndex;
  const headLane = headIndex !== null ? layout.nodes[headIndex].lane : 0;
  const x = laneX(headLane, m);
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
  // P7 §7: WIP label moves to the summary zone; the LEFT ref band stays empty.
  const textX = summaryStartX(layout.laneCount, m);
  ctx.font = `italic ${m.summaryFont} ${FONT_UI}`;
  ctx.fillStyle = theme.text2;
  const label = 'Uncommitted changes';
  ctx.fillText(label, textX, y);
  const labelW = measure(ctx, label);

  ctx.font = `${m.metaFont} ${FONT_UI}`;
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
  display: GraphDisplayOptions,
  m: EffectiveMetrics,
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
    ctx.fillRect(0, row * m.rowHeight - vp.scrollTop, vp.width, m.rowHeight);
  };
  if (ix.hoverRow !== null && ix.hoverRow !== ix.selectedIndex && ix.hoverRow < n) {
    rowBg(ix.hoverRow, theme.bg2);
  }
  if (ix.selectedIndex !== null && ix.selectedIndex < n) {
    rowBg(ix.selectedIndex, theme.selection);
  }

  // Pass 3: edges (under dots).
  ctx.lineWidth = m.edgeWidth;
  ctx.lineCap = 'round';
  for (const e of visibleEdges) drawEdge(ctx, e, nodes, vp, theme, m);

  // Pass 4: author-initials avatars (P7 §2.1 — replaces the plain lane dot).
  // Inner→outer: bg ring → avatar disc → lane ring → initials → HEAD ring →
  // selection ring. Drawn per visible row only (virtualized).
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const x = laneX(node.lane, m);
    const y = rowY(row, vp.scrollTop, m);
    const laneColor = theme.laneColors[node.lane % 10];
    const ac = avatarColor(node.author);
    const selected = ix.selectedIndex === row;
    // P10 §2.1: a stash node draws a violet disc + glyph instead of the avatar.
    const isStash = node.refs?.some((r) => r.kind === 'stash') ?? false;

    // bg ring — bg0 halo so edges passing under the avatar read cleanly.
    ctx.beginPath();
    ctx.arc(x, y, m.avatarRadius + m.avatarBgRingExtra, 0, Math.PI * 2);
    ctx.fillStyle = theme.bg0;
    ctx.fill();

    if (isStash) {
      drawStashNode(ctx, x, y, m);
    } else {
      // avatar disc — theme-invariant hashed name color.
      ctx.beginPath();
      ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
      ctx.fillStyle = ac.bg;
      ctx.fill();

      // lane ring — ties the avatar to its lane color.
      ctx.beginPath();
      ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
      ctx.strokeStyle = laneColor;
      ctx.lineWidth = m.avatarRingWidth;
      ctx.stroke();

      // initials (centered baseline).
      ctx.font = `${m.avatarFont} ${FONT_UI}`;
      ctx.fillStyle = ac.text;
      ctx.fillText(initials(node.author), x, y);
    }

    if (layout.headIndex === row) {
      ctx.beginPath();
      ctx.arc(x, y, m.avatarHeadRingRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.text1;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    if (selected) {
      ctx.beginPath();
      ctx.arc(x, y, m.avatarSelRingRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.accent;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    // P50b: search-match ring — an outer ring in --match-ring so matches stay
    // spottable while scrolling (distinct radius + color from head/selection).
    if (ix.matchRows !== null && ix.matchRows.has(row)) {
      ctx.beginPath();
      ctx.arc(x, y, m.avatarSelRingRadius + 1.5, 0, Math.PI * 2);
      ctx.strokeStyle = theme.matchRing;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
  }
  // Restore the text-pass expectations (textAlign) and edge lineWidth so the
  // next paint's edges are unaffected (matches the old pass-4 cleanup).
  ctx.textAlign = 'left';
  ctx.lineWidth = m.edgeWidth;

  // Pass 5: text row content — LEFT ref column (refLabels) + the RIGHT summary
  // and optional author/SHA/date columns (drawRowText). The right columns are
  // packed ONCE here (computeRightColumns) and shared with the hover hit-test.
  const { startX, budget } = refColArea(m);
  const sx = summaryStartX(layout.laneCount, m);
  // P7e §13.2: keep the right columns clear of the vertical scrollbar by
  // shrinking the effective right edge by `rightInset`.
  const effRight = vp.width - (vp.rightInset ?? 0);
  const cols = computeRightColumns(effRight, display, m);
  const now = Math.floor(Date.now() / 1000);

  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const y = rowY(row, vp.scrollTop, m);

    // 5a (LEFT): ref column — collapsed entities capped by the fixed band with
    // a trailing "+n" chip. Layout is the shared pure helper (single source of
    // truth with the hit-test); this pass only paints the laid-out labels.
    const laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget, display);
    for (const l of laid) drawRefLabelAt(ctx, l, y, theme);

    // 5b–5e (RIGHT): summary (flex) + optional author / SHA(+badge) / date
    // columns, packed by `cols`. Toggling a column off reclaims its width.
    // P58c: the SHA-slot badge lights from this row's cached verdict (undefined
    // ⇒ the faint stub, so off-screen/unverified rows stay faint).
    const status = ix.verifyStatus?.get(node.id);
    drawRowText(ctx, node, y, sx, cols, display, theme, m, now, status);
  }
  ctx.textAlign = 'left';
}
