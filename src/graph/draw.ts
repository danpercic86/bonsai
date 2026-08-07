/** Pure canvas draw functions for the precomputed GraphLayout — no React.
 * The ctx is already DPR-transformed; every coordinate below is CSS px.
 * Geometry and draw order are normative per contract M2-graph.md §1.3/§3.3. */

import type { GraphEdge, GraphLayout, GraphNode, RefLabel } from '../ipc';
import { STASH_BG, STASH_COLOR, TAG_BG, TAG_COLOR } from './colors';
import type { Theme } from './colors';
import { AVATAR, FONT_UI, METRICS } from './metrics';
import type { EffectiveMetrics } from './metrics';

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
  | { kind: 'head'; name: string; ref: RefLabel } // detached-HEAD label
  | { kind: 'stash'; name: string; ref: RefLabel }; // name = "stash@{n}"

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
  const stashes: RefEntity[] = [];
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
        const short = ref.name.slice(ref.name.indexOf('/') + 1); // strip remote name only, keep interior slashes: "origin/topic/x" -> "topic/x"
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
      case 'stash':
        stashes.push({ kind: 'stash', name: ref.name, ref });
        break;
    }
  }
  return [...heads, ...branches.values(), ...tags, ...stashes];
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
    case 'stash':
      return { fill: STASH_BG, text: STASH_COLOR, border: STASH_COLOR, label: e.name };
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

/** P9 §6.1: stash glyph (a drawer/tray box). Same monochrome convention as
 *  {@link drawLaptopIcon} — the CALLER sets `ctx.strokeStyle` (= `style.text`). */
export function drawStashIcon(
  ctx: CanvasRenderingContext2D,
  bx: number,
  by: number,
  S: number,
): void {
  ctx.lineWidth = 1.2;
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  // tray/box body
  ctx.beginPath();
  ctx.roundRect(bx + S * 0.1, by + S * 0.32, S * 0.8, S * 0.5, S * 0.08);
  ctx.stroke();
  // slot line across the drawer's upper third
  ctx.beginPath();
  ctx.moveTo(bx + S * 0.3, by + S * 0.48);
  ctx.lineTo(bx + S * 0.7, by + S * 0.48);
  ctx.stroke();
}

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

// ---------- LEFT ref-column layout + overflow (P7 §4) ----------

export interface LaidRefLabel {
  /** The display entity, or `null` for the "+n" overflow chip. */
  entity: RefEntity | null;
  style: PillStyle;
  /** Left edge, in canvas CSS-px (ref-column space). */
  x: number;
  /** Full pill width incl. padding + icons. */
  w: number;
  /** Glyphs to draw (all false for chip / tag / head). */
  icons: RefIcons;
}

/** Which glyphs a pill carries. A branch may show laptop and/or cloud; a stash
 *  shows exactly the stash glyph; everything else shows none. */
export interface RefIcons {
  laptop: boolean;
  cloud: boolean;
  stash: boolean;
}

/** Glyphs for an entity: laptop when it has a local ref, cloud when it has a
 *  remote (branch only); the stash glyph for stash entities. */
function iconsFor(e: RefEntity): RefIcons {
  if (e.kind === 'branch') return { laptop: e.hasLocal, cloud: e.remotes.length > 0, stash: false };
  if (e.kind === 'stash') return { laptop: false, cloud: false, stash: true };
  return { laptop: false, cloud: false, stash: false };
}

/** Combined icon-block width (icon-icon gap only between two icons). A stash
 *  pill has exactly one icon, so it needs no inter-icon gap term. */
function iconsWidth(icons: RefIcons): number {
  return (
    (icons.laptop ? METRICS.iconSize : 0) +
    (icons.cloud ? METRICS.iconSize : 0) +
    (icons.stash ? METRICS.iconSize : 0) +
    (icons.laptop && icons.cloud ? METRICS.iconGap : 0)
  );
}

/** Measures a ref-label pill (icons + truncated label), matching the width the
 *  draw pass will reproduce (P7 §4). Assumes `ctx.font` is already `pillFont`. */
function refPillWidth(
  ctx: CanvasRenderingContext2D,
  style: PillStyle,
  icons: RefIcons,
): number {
  const iconsW = iconsWidth(icons);
  const anyIcon = icons.laptop || icons.cloud || icons.stash;
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
  let hidden = entities.length - shown;
  if (hidden > 0) {
    // The chip is mandatory. P7e §13.1: RESERVE room for it so the laid set
    // (chip included) never spills past the band. Compute the chip width, then
    // while the trailing shown pill's slot leaves no room, pop it (rewinding the
    // cursor and recomputing the "+n" label for the now-larger hidden count).
    const noIcons: RefIcons = { laptop: false, cloud: false, stash: false };
    const chipStyleFor = (h: number): PillStyle => ({
      fill: theme.bg2,
      text: theme.text2,
      border: theme.border,
      label: `+${h}`,
    });
    let chipStyle = chipStyleFor(hidden);
    let chipW = refPillWidth(ctx, chipStyle, noIcons);
    while (result.length > 0 && x + chipW > startX + budget) {
      const popped = result.pop();
      if (popped === undefined) break;
      x -= popped.w + METRICS.pillGap;
      hidden++;
      chipStyle = chipStyleFor(hidden);
      chipW = refPillWidth(ctx, chipStyle, noIcons);
    }
    // If every pill got popped the chip sits alone at startX (x === startX).
    result.push({ entity: null, style: chipStyle, x, w: chipW, icons: noIcons });
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
  const anyIcon = icons.laptop || icons.cloud || icons.stash;
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
  if (icons.stash) {
    drawStashIcon(ctx, ix, by, S);
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

  // Pass 5: text row content (P7 §4.1 — three zones: LEFT ref column,
  // RIGHT summary, RIGHT relative time; author removed).
  const { startX, budget } = refColArea(m);
  const sx = summaryStartX(layout.laneCount, m);
  // P7e §13.2: keep the right-aligned relative-time (and summary) clear of the
  // vertical scrollbar by shrinking the effective right edge by `rightInset`.
  const effRight = vp.width - (vp.rightInset ?? 0);
  const dateRight = effRight - m.colGap;
  const dateLeft = dateRight - m.dateColWidth;
  const summaryMax = dateLeft - m.colGap - sx;
  const now = Math.floor(Date.now() / 1000);

  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const y = rowY(row, vp.scrollTop, m);

    // 5a (LEFT): ref column — collapsed entities capped by the fixed band with
    // a trailing "+n" chip. Layout is the shared pure helper (single source of
    // truth with the hit-test); this pass only paints the laid-out labels.
    const laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget);
    for (const l of laid) drawRefLabelAt(ctx, l, y);

    // 5b (summary).
    if (summaryMax > 0) {
      ctx.font = `${m.summaryFont} ${FONT_UI}`;
      ctx.fillStyle = theme.text1;
      ctx.textAlign = 'left';
      ctx.fillText(truncateToWidth(ctx, node.summary, summaryMax), sx, y);
    }

    // 5c: author — REMOVED (P7 §4.1).

    // 5d (date): relative date, right-aligned in the last dateColWidth px.
    ctx.font = `${m.metaFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text3;
    ctx.textAlign = 'right';
    ctx.fillText(
      truncateToWidth(ctx, relativeDate(node.ts, now), m.dateColWidth),
      dateRight,
      y,
    );
  }
  ctx.textAlign = 'left';
}
