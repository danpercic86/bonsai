/** Spec-005: overview-rail painter (UI contract §3 — paint order = z-order:
 *  track → density strip → ref/HEAD pips → viewport thumb → match ticks →
 *  current-match tick). Pure canvas 2D over precomputed RailBuckets/RailTick
 *  data; no DOM reads, no allocation beyond local scalars. */

import { TAG_COLOR, hexToRgba } from '../colors';
import {
  RAIL_BUCKETS,
  RAIL_FLAG_BRANCH,
  RAIL_FLAG_HEAD,
  RAIL_FLAG_TAG,
} from './railMath';
import type { RailBuckets, RailTick, ThumbRect } from './railMath';

/** Colors resolved from the app CSS variables once per mount/themeVersion
 *  (the graph Theme lacks `--bg-1`, hence a rail-local mini-theme). */
export interface RailTheme {
  bg0: string;
  bg1: string;
  border: string;
  text1: string;
  text2: string;
  text3: string;
  accent: string;
  matchRing: string;
}

export interface DrawRailOptions {
  /** CSS-pixel canvas size (transform already scaled for DPR). */
  width: number;
  height: number;
  /** null while no stream `done` has landed yet (§4 "Streaming" state — the
   *  density/pip layers are simply skipped; track/thumb/ticks still paint). */
  buckets: RailBuckets | null;
  ticks: readonly RailTick[];
  thumb: ThumbRect;
  theme: RailTheme;
  dragging: boolean;
}

/** §3.2 density alpha ramp: floor 0.22 for any non-empty bucket, linear to
 *  0.55 at the layout's max bucket density. */
export function densityAlpha(count: number, maxDensity: number): number {
  if (count <= 0) return 0;
  if (maxDensity <= 1) return 0.22;
  return 0.22 + 0.33 * ((count - 1) / (maxDensity - 1));
}

/** §3.2 strip width: 4 + 2·laneMax px, clamped to 12 (2px right margin). */
export function densityWidth(laneMax: number): number {
  return Math.min(4 + 2 * laneMax, 12);
}

function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  const radius = Math.min(r, h / 2, w / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

export function drawRail(ctx: CanvasRenderingContext2D, o: DrawRailOptions): void {
  const { width: w, height: h, buckets, ticks, thumb, theme } = o;
  ctx.clearRect(0, 0, w, h);
  // 1. Track: quiet gutter + 1px left border (§3.1).
  ctx.fillStyle = hexToRgba(theme.bg1, 0.92);
  ctx.fillRect(0, 0, w, h);
  ctx.fillStyle = theme.border;
  ctx.fillRect(0, 0, 1, h);

  if (buckets !== null && buckets.rows > 0 && h > 0) {
    // 2. Density strip: per rail pixel → nearest bucket (§3.2).
    for (let y = 0; y < h; y += 1) {
      const b = Math.min(Math.floor((y * RAIL_BUCKETS) / h), RAIL_BUCKETS - 1);
      const count = buckets.density[b];
      if (count === 0) continue;
      ctx.fillStyle = hexToRgba(theme.text3, densityAlpha(count, buckets.maxDensity));
      ctx.fillRect(0, y, densityWidth(buckets.laneMax[b]), 1);
    }
    // 3. Ref/HEAD pips (§3.3): branch left column, tag right column, HEAD full-width.
    for (let b = 0; b < RAIL_BUCKETS; b += 1) {
      const flags = buckets.flags[b];
      if (flags === 0) continue;
      const y = Math.min(Math.floor((b * h) / RAIL_BUCKETS), h - 1);
      if ((flags & RAIL_FLAG_BRANCH) !== 0) {
        ctx.fillStyle = theme.text2;
        ctx.beginPath();
        ctx.arc(4, y + 0.5, 1.5, 0, Math.PI * 2);
        ctx.fill();
      }
      if ((flags & RAIL_FLAG_TAG) !== 0) {
        ctx.fillStyle = TAG_COLOR;
        ctx.beginPath();
        ctx.arc(10, y + 0.5, 1.5, 0, Math.PI * 2);
        ctx.fill();
      }
      if ((flags & RAIL_FLAG_HEAD) !== 0) {
        ctx.fillStyle = theme.text1;
        ctx.fillRect(0, y, w, 2);
      }
    }
  }

  // 4. Viewport thumb (§3.4): rounded, 1px inset each side, accent fill+border.
  roundedRect(ctx, 1, thumb.top, w - 2, thumb.height, 4);
  ctx.fillStyle = hexToRgba(theme.accent, o.dragging ? 0.22 : 0.14);
  ctx.fill();
  ctx.strokeStyle = theme.accent;
  ctx.lineWidth = o.dragging ? 1.5 : 1;
  ctx.stroke();

  // 5. Match ticks (§3.5): full-width 2px bars, coalesced upstream.
  let current: RailTick | null = null;
  ctx.fillStyle = theme.matchRing;
  for (const t of ticks) {
    if (t.current) {
      current = t;
      continue; // painted last (§3.6)
    }
    ctx.fillRect(0, t.y - 1, w, 2);
  }
  // 6. Current-match tick: 3px bar with a 1px bg-0 halo above/below (§3.6).
  if (current !== null) {
    ctx.fillStyle = theme.bg0;
    ctx.fillRect(0, current.y - 3, w, 1);
    ctx.fillRect(0, current.y + 2, w, 1);
    ctx.fillStyle = theme.matchRing;
    ctx.fillRect(0, current.y - 2, w, 3);
  }
}
