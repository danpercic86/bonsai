/** Pure canvas draw functions for the precomputed GraphLayout — no React.
 * The ctx is already DPR-transformed; every coordinate below is CSS px.
 * Geometry and draw order are normative per contract M2-graph.md §1.3/§3.3. */

import type { GraphEdge, GraphLayout, GraphNode, RefLabel } from '../ipc';
import { TAG_BG, TAG_COLOR } from './colors';
import type { Theme } from './colors';
import { AVATAR, FONT_UI, METRICS } from './metrics';

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

/** x of a lane center; lanes beyond the render clamp share the last x.
 *  P7 §1.2: gains the fixed LEFT ref-band offset (`refColWidth`); the +8 lane
 *  inset is preserved. The global right-shift flows through edges automatically
 *  (they use `laneX`/`rowY`). */
export function laneX(lane: number): number {
  return (
    METRICS.refColWidth +
    METRICS.gutter +
    Math.min(lane, METRICS.maxRenderLanes - 1) * METRICS.laneWidth +
    8
  );
}

/** P7 §1.2: right edge of the graph area (clamped lane band), independent of
 *  the +8 lane inset. Internal — feeds `summaryStartX`. */
function graphAreaRight(laneCount: number): number {
  return (
    METRICS.refColWidth +
    METRICS.gutter +
    Math.min(laneCount, METRICS.maxRenderLanes) * METRICS.laneWidth
  );
}

/** P7 §1.2: summary column origin (replaces the old `textColumnX`; no pills
 *  live here now — refs moved to the LEFT band). */
export function summaryStartX(laneCount: number): number {
  return graphAreaRight(laneCount) + METRICS.textGap;
}

/** P7 §1.2: fixed LEFT ref-column layout window (analog of the old `pillArea`,
 *  but NOT a function of viewport width or laneCount — the band is fixed). */
export function refColArea(): { startX: number; budget: number } {
  return {
    startX: METRICS.refColPadLeft,
    budget: Math.max(0, METRICS.refColWidth - METRICS.refColPadLeft - METRICS.refColPadRight),
  };
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
export function avatarHit(px: number, py: number, cx: number, cy: number): boolean {
  const r = METRICS.avatarRadius + METRICS.avatarBgRingExtra;
  const dx = px - cx;
  const dy = py - cy;
  return dx * dx + dy * dy <= r * r;
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

// ---------- shared pill style (P7 §3.3: consumed by entityStyle / LaidRefLabel) ----------

export interface PillStyle {
  fill: string;
  text: string;
  border: string | null;
  label: string;
}

// ---------- ref grouping / collapse transform (P7 §3) ----------

/** P7 §3.1: per-node display entity. Collapses a local branch and its
 *  same-commit remote into ONE `branch` (laptop + cloud, short name once);
 *  diverged refs land on different nodes and stay separate. */
export type RefEntity =
  | {
      kind: 'branch';
      /** SHORT name shown once ("main"); the group key. */
      name: string;
      hasLocal: boolean; // laptop glyph when true
      remotes: string[]; // full remote shorthands, e.g. ["origin/main"]; cloud glyph when non-empty
      isHead: boolean; // attached HEAD local branch
      refs: RefLabel[]; // underlying wire refs (right-click targeting)
    }
  | { kind: 'tag'; name: string; ref: RefLabel }
  | { kind: 'head'; name: string; ref: RefLabel }; // detached-HEAD label

/** P7 §3.2: group a node's wire refs into display entities. Insertion order is
 *  preserved; input is already sorted local-head-first / remotes / tags. Output
 *  order: detached `head`, then branch entities (local-first), then tags. */
export function groupRefs(refs: readonly RefLabel[] | undefined): RefEntity[] {
  const branches = new Map<
    string,
    { kind: 'branch'; name: string; hasLocal: boolean; remotes: string[]; isHead: boolean; refs: RefLabel[] }
  >();
  const tags: RefEntity[] = [];
  const heads: RefEntity[] = [];
  for (const ref of refs ?? []) {
    switch (ref.kind) {
      case 'localBranch': {
        const key = ref.name;
        const e =
          branches.get(key) ??
          { kind: 'branch' as const, name: key, hasLocal: false, remotes: [], isHead: false, refs: [] };
        branches.set(key, e);
        e.hasLocal = true;
        e.isHead = e.isHead || ref.isHead;
        e.refs.push(ref);
        break;
      }
      case 'remoteBranch': {
        const short = ref.name.slice(ref.name.lastIndexOf('/') + 1); // "origin/main" -> "main"
        const e =
          branches.get(short) ??
          { kind: 'branch' as const, name: short, hasLocal: false, remotes: [], isHead: false, refs: [] };
        branches.set(short, e);
        e.remotes.push(ref.name);
        e.refs.push(ref);
        break;
      }
      case 'tag':
        tags.push({ kind: 'tag', name: ref.name, ref });
        break;
      case 'head':
        heads.push({ kind: 'head', name: ref.name, ref });
        break;
    }
  }
  return [...heads, ...branches.values(), ...tags];
}

/** P7 §3.3: resolve an entity's pill visuals (reuses {@link PillStyle}). Icons
 *  are computed separately (see {@link layoutRefLabels}); the old "⌂ " HEAD
 *  prefix is dropped (the laptop icon + solid fill convey local + head). */
export function entityStyle(e: RefEntity, node: GraphNode, theme: Theme): PillStyle {
  const laneColor = theme.laneColors[node.lane % 10];
  const laneAlpha = theme.laneColorsAlpha[node.lane % 10];
  switch (e.kind) {
    case 'branch':
      if (e.isHead) {
        return { fill: laneColor, text: theme.accentText, border: null, label: e.name };
      }
      if (e.hasLocal) {
        return { fill: laneAlpha, text: laneColor, border: laneColor, label: e.name };
      }
      return { fill: theme.bg2, text: theme.text2, border: theme.border, label: e.name };
    case 'tag':
      return { fill: TAG_BG, text: TAG_COLOR, border: TAG_COLOR, label: `# ${e.name}` };
    case 'head':
      return { fill: theme.danger, text: '#ffffff', border: null, label: e.name };
  }
}

// ---------- ref-label icon recipes (P7 §3.4) ----------

/** P7 §3.4: laptop glyph (local). Monochrome — the CALLER sets `ctx.strokeStyle`
 *  (= `style.text`); this recipe sets width/join/cap and draws with the current
 *  stroke. Box is `S × S` at `(bx, by)`. */
export function drawLaptopIcon(
  ctx: CanvasRenderingContext2D,
  bx: number,
  by: number,
  S: number,
): void {
  ctx.lineWidth = 1.2;
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  // screen (rounded rect)
  ctx.beginPath();
  ctx.roundRect(bx + S * 0.15, by + S * 0.1, S * 0.7, S * 0.52, S * 0.08);
  ctx.stroke();
  // base + sloped sides
  ctx.beginPath();
  ctx.moveTo(bx + S * 0.15, by + S * 0.62);
  ctx.lineTo(bx + S * 0.05, by + S * 0.82);
  ctx.lineTo(bx + S * 0.95, by + S * 0.82);
  ctx.lineTo(bx + S * 0.85, by + S * 0.62);
  ctx.stroke();
}

/** P7 §3.4: cloud glyph (remote). Same monochrome convention as
 *  {@link drawLaptopIcon}. */
export function drawCloudIcon(
  ctx: CanvasRenderingContext2D,
  bx: number,
  by: number,
  S: number,
): void {
  ctx.lineWidth = 1.2;
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  const b = by + S * 0.74; // baseline
  ctx.beginPath();
  ctx.arc(bx + S * 0.34, b - S * 0.16, S * 0.2, Math.PI * 0.5, Math.PI * 1.5); // left lobe
  ctx.arc(bx + S * 0.52, b - S * 0.3, S * 0.22, Math.PI * 1.05, Math.PI * 1.95); // top lobe
  ctx.arc(bx + S * 0.68, b - S * 0.14, S * 0.18, Math.PI * 1.5, Math.PI * 0.5); // right lobe
  ctx.lineTo(bx + S * 0.3, b);
  ctx.closePath();
  ctx.stroke();
}

// ---------- LEFT ref-column layout + overflow (P7 §4) ----------

export interface LaidRefLabel {
  /** The display entity, or `null` for the "+n" overflow chip. */
  entity: RefEntity | null;
  style: PillStyle;
  /** Left edge, in canvas CSS-px (ref-column space). */
  x: number;
  /** Full pill width incl. padding + icons. */
  w: number;
  /** Glyphs to draw (both false for chip / tag / head). */
  icons: { laptop: boolean; cloud: boolean };
}

/** Glyphs for an entity: laptop when it has a local ref, cloud when it has a
 *  remote. Only `branch` entities carry icons. */
function iconsFor(e: RefEntity): { laptop: boolean; cloud: boolean } {
  if (e.kind === 'branch') return { laptop: e.hasLocal, cloud: e.remotes.length > 0 };
  return { laptop: false, cloud: false };
}

/** Combined icon-block width (icon-icon gap only between two icons). */
function iconsWidth(icons: { laptop: boolean; cloud: boolean }): number {
  return (
    (icons.laptop ? METRICS.iconSize : 0) +
    (icons.cloud ? METRICS.iconSize : 0) +
    (icons.laptop && icons.cloud ? METRICS.iconGap : 0)
  );
}

/** Measures a ref-label pill (icons + truncated label), matching the width the
 *  draw pass will reproduce (P7 §4). Assumes `ctx.font` is already `pillFont`. */
function refPillWidth(
  ctx: CanvasRenderingContext2D,
  style: PillStyle,
  icons: { laptop: boolean; cloud: boolean },
): number {
  const iconsW = iconsWidth(icons);
  const anyIcon = icons.laptop || icons.cloud;
  const labelMaxPx = METRICS.pillMaxWidth - 2 * METRICS.pillPadX - iconsW - (anyIcon ? METRICS.iconGap : 0);
  const labelText = truncateToWidth(ctx, style.label, labelMaxPx);
  return (
    2 * METRICS.pillPadX + iconsW + (anyIcon ? METRICS.iconGap : 0) + Math.ceil(measure(ctx, labelText))
  );
}

/** P7 §4: lay entities L→R in the fixed band `[startX, startX+budget]`; break
 *  before an entity that would exceed the budget (except the first); append a
 *  "+n" chip counting HIDDEN ENTITIES. Mirrors the old `layoutRowPills` overflow
 *  rule exactly. PURE (no drawing); sets `ctx.font` internally. Single source of
 *  truth for both the draw pass and hit-testing. */
export function layoutRefLabels(
  ctx: CanvasRenderingContext2D,
  entities: readonly RefEntity[],
  node: GraphNode,
  theme: Theme,
  startX: number,
  budget: number,
): LaidRefLabel[] {
  const result: LaidRefLabel[] = [];
  if (entities.length === 0) return result;
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  let x = startX;
  let shown = 0;
  for (const e of entities) {
    const style = entityStyle(e, node, theme);
    const icons = iconsFor(e);
    const w = refPillWidth(ctx, style, icons);
    if (shown > 0 && x + w > startX + budget) break;
    result.push({ entity: e, style, x, w, icons });
    x += w + METRICS.pillGap;
    shown++;
  }
  const hidden = entities.length - shown;
  if (hidden > 0) {
    const chipStyle: PillStyle = {
      fill: theme.bg2,
      text: theme.text2,
      border: theme.border,
      label: `+${hidden}`,
    };
    const noIcons = { laptop: false, cloud: false };
    result.push({ entity: null, style: chipStyle, x, w: refPillWidth(ctx, chipStyle, noIcons), icons: noIcons });
  }
  return result;
}

/** P7 §4.1: draws one laid-out ref label at row-center `cy`. Reuses the pill
 *  rounded-rect body (fill + optional border), then draws the laptop/cloud
 *  glyphs starting at `x + pillPadX`, then the label. The label is re-truncated
 *  to the SAME `labelMaxPx` {@link layoutRefLabels} measured with, so the drawn
 *  and laid-out widths stay pixel-identical. Nothing draws past the ref band
 *  (guaranteed by the layout budget). */
function drawRefLabelAt(ctx: CanvasRenderingContext2D, laid: LaidRefLabel, cy: number): void {
  const { style, x, w, icons } = laid;
  const h = METRICS.pillHeight;
  const y = cy - h / 2;
  const r = h / 2;

  // pill body (reuses the old drawPillAt rounded-rect + border).
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
  ctx.fillStyle = style.fill;
  ctx.fill();
  if (style.border !== null) {
    ctx.strokeStyle = style.border;
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  // icons (monochrome, colored by style.text), from x + pillPadX.
  const S = METRICS.iconSize;
  const by = cy - S / 2;
  const anyIcon = icons.laptop || icons.cloud;
  let ix = x + METRICS.pillPadX;
  ctx.strokeStyle = style.text;
  if (icons.laptop) {
    drawLaptopIcon(ctx, ix, by, S);
    ix += S;
  }
  if (icons.cloud) {
    if (icons.laptop) ix += METRICS.iconGap;
    drawCloudIcon(ctx, ix, by, S);
    ix += S;
  }
  if (anyIcon) ix += METRICS.iconGap;

  // label — same font + labelMaxPx as layoutRefLabels for pixel-identical width.
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  const labelMaxPx =
    METRICS.pillMaxWidth - 2 * METRICS.pillPadX - iconsWidth(icons) - (anyIcon ? METRICS.iconGap : 0);
  ctx.fillStyle = style.text;
  ctx.textAlign = 'left';
  ctx.fillText(truncateToWidth(ctx, style.label, labelMaxPx), ix, cy);
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
  // P7 §7: WIP label moves to the summary zone; the LEFT ref band stays empty.
  const textX = summaryStartX(layout.laneCount);
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

  // Pass 4: author-initials avatars (P7 §2.1 — replaces the plain lane dot).
  // Inner→outer: bg ring → avatar disc → lane ring → initials → HEAD ring →
  // selection ring. Drawn per visible row only (virtualized).
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const x = laneX(node.lane);
    const y = rowY(row, vp.scrollTop);
    const laneColor = theme.laneColors[node.lane % 10];
    const ac = avatarColor(node.author);
    const selected = ix.selectedIndex === row;

    // bg ring — bg0 halo so edges passing under the avatar read cleanly.
    ctx.beginPath();
    ctx.arc(x, y, METRICS.avatarRadius + METRICS.avatarBgRingExtra, 0, Math.PI * 2);
    ctx.fillStyle = theme.bg0;
    ctx.fill();

    // avatar disc — theme-invariant hashed name color.
    ctx.beginPath();
    ctx.arc(x, y, METRICS.avatarRadius, 0, Math.PI * 2);
    ctx.fillStyle = ac.bg;
    ctx.fill();

    // lane ring — ties the avatar to its lane color.
    ctx.beginPath();
    ctx.arc(x, y, METRICS.avatarRadius, 0, Math.PI * 2);
    ctx.strokeStyle = laneColor;
    ctx.lineWidth = METRICS.avatarRingWidth;
    ctx.stroke();

    // initials (centered baseline).
    ctx.font = `${METRICS.avatarFont} ${FONT_UI}`;
    ctx.fillStyle = ac.text;
    ctx.fillText(initials(node.author), x, y);

    if (layout.headIndex === row) {
      ctx.beginPath();
      ctx.arc(x, y, METRICS.avatarHeadRingRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.text1;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    if (selected) {
      ctx.beginPath();
      ctx.arc(x, y, METRICS.avatarSelRingRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.accent;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
  }
  // Restore the text-pass expectations (textAlign) and edge lineWidth so the
  // next paint's edges are unaffected (matches the old pass-4 cleanup).
  ctx.textAlign = 'left';
  ctx.lineWidth = METRICS.edgeWidth;

  // Pass 5: text row content (P7 §4.1 — three zones: LEFT ref column,
  // RIGHT summary, RIGHT relative time; author removed).
  const { startX, budget } = refColArea();
  const sx = summaryStartX(layout.laneCount);
  const dateLeft = vp.width - METRICS.dateColWidth - METRICS.colGap;
  const dateRight = vp.width - METRICS.colGap;
  const summaryMax = dateLeft - METRICS.colGap - sx;
  const now = Math.floor(Date.now() / 1000);

  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const y = rowY(row, vp.scrollTop);

    // 5a (LEFT): ref column — collapsed entities capped by the fixed band with
    // a trailing "+n" chip. Layout is the shared pure helper (single source of
    // truth with the hit-test); this pass only paints the laid-out labels.
    const laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget);
    for (const l of laid) drawRefLabelAt(ctx, l, y);

    // 5b (summary).
    if (summaryMax > 0) {
      ctx.font = `${METRICS.summaryFont} ${FONT_UI}`;
      ctx.fillStyle = theme.text1;
      ctx.textAlign = 'left';
      ctx.fillText(truncateToWidth(ctx, node.summary, summaryMax), sx, y);
    }

    // 5c: author — REMOVED (P7 §4.1).

    // 5d (date): relative date, right-aligned in the last dateColWidth px.
    ctx.font = `${METRICS.metaFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text3;
    ctx.textAlign = 'right';
    ctx.fillText(
      truncateToWidth(ctx, relativeDate(node.ts, now), METRICS.dateColWidth),
      dateRight,
      y,
    );
  }
  ctx.textAlign = 'left';
}
