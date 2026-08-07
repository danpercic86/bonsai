/** LEFT ref-band subsystem for the commit graph — ref grouping/collapse, pill
 *  styling, glyph recipes, and the fixed-band layout + overflow ("+n") rule.
 *  Extracted verbatim from draw.ts (P51b §7.1) so draw.ts drops back under the
 *  file-size limit; behavior is unchanged. The layout helper is PURE (no
 *  drawing) and is the single source of truth for both the draw pass and the
 *  right-click / hover hit-tests. */

import type { GraphNode, RefLabel } from '../ipc';
import { STASH_BG, STASH_COLOR, TAG_BG, TAG_COLOR } from './colors';
import type { Theme } from './colors';
import { FONT_UI, METRICS } from './metrics';
import { measure, truncateToWidth } from './textMeasure';

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
export function drawRefLabelAt(ctx: CanvasRenderingContext2D, laid: LaidRefLabel, cy: number): void {
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
