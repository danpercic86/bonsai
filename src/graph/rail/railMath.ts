/** Spec-005: overview-rail pure math — bucket downsample, tick coalescing,
 *  thumb/track mapping, hit-testing. Zero DOM, zero React (unit-tested).
 *
 *  Deviation from the plan's draft signatures (recorded in the spec-005
 *  report): `buildRailBuckets` takes the DISPLAY-space layout directly (the
 *  spec-004 fold projection already lives inside GraphCanvas), so no
 *  `RailRowModel` parameter is needed — display mapping for match ticks goes
 *  through `foldModel.modelToDisplay` at the call site instead. */

import type { GraphLayout } from '../../ipc';

export const RAIL_BUCKETS = 1024;
export const RAIL_WIDTH_PX = 14;
export const HOVER_ZONE_PX = 20;
export const RAIL_LINGER_MS = 300;
export const THUMB_MIN_PX = 24;
export const TICK_HIT_SLOP_PX = 3;

/** RailBuckets.flags bits. */
export const RAIL_FLAG_BRANCH = 1;
export const RAIL_FLAG_TAG = 2;
export const RAIL_FLAG_HEAD = 4;

/** Fixed-resolution downsample of the whole display-row history. Typed arrays,
 *  built once per railGeneration — resampled at paint for any rail height. */
export interface RailBuckets {
  /** Display-row count at build time (>= 1 when any node exists). */
  rows: number;
  /** Commits per bucket (saturating u16). */
  density: Uint16Array;
  /** Max (lane + 1) seen per bucket — drives strip width. */
  laneMax: Uint8Array;
  /** Bit 0 branch/remote pip, bit 1 tag pip, bit 2 HEAD. */
  flags: Uint8Array;
  /** Max density over all buckets (alpha ramp normalizer; 0 when empty). */
  maxDensity: number;
}

export interface RailTick {
  /** Rail-pixel y (coalesced: unique per pixel). */
  y: number;
  /** Representative index into the caller's match array (first per pixel). */
  matchIndex: number;
  displayRow: number;
  current: boolean;
}

export interface ThumbRect {
  top: number;
  height: number;
}

/** Bucket index for display row `d` of `rows` total (clamped). */
export function bucketOf(d: number, rows: number): number {
  const b = Math.floor((d * RAIL_BUCKETS) / Math.max(rows, 1));
  return Math.min(Math.max(b, 0), RAIL_BUCKETS - 1);
}

/** O(n + refs) downsample of a DISPLAY-space layout. Fold-pill rows carry no
 *  refs of their own after projection, so they just count toward density. */
export function buildRailBuckets(layout: GraphLayout): RailBuckets {
  const n = layout.nodes.length;
  const density = new Uint16Array(RAIL_BUCKETS);
  const laneMax = new Uint8Array(RAIL_BUCKETS);
  const flags = new Uint8Array(RAIL_BUCKETS);
  let maxDensity = 0;
  for (let i = 0; i < n; i += 1) {
    const node = layout.nodes[i];
    const b = bucketOf(i, n);
    if (density[b] < 0xffff) density[b] += 1;
    if (density[b] > maxDensity) maxDensity = density[b];
    const laneDepth = Math.min(node.lane + 1, 0xff);
    if (laneDepth > laneMax[b]) laneMax[b] = laneDepth;
    const refs = node.refs;
    if (refs !== undefined) {
      for (const r of refs) {
        if (r.kind === 'localBranch' || r.kind === 'remoteBranch') flags[b] |= RAIL_FLAG_BRANCH;
        else if (r.kind === 'tag') flags[b] |= RAIL_FLAG_TAG;
      }
    }
  }
  if (layout.headIndex !== null && n > 0) {
    flags[bucketOf(layout.headIndex, n)] |= RAIL_FLAG_HEAD;
  }
  return { rows: n, density, laneMax, flags, maxDensity };
}

/** Rail-pixel y for display row `d` (proportional, clamped to the rail). */
export function railYForDisplayRow(d: number, rows: number, railHeight: number): number {
  if (rows <= 1 || railHeight <= 1) return 0;
  const y = Math.round((d / (rows - 1)) * (railHeight - 1));
  return Math.min(Math.max(y, 0), railHeight - 1);
}

/** One tick per occupied rail pixel; the FIRST match landing in a pixel is its
 *  representative (deterministic). The current match's pixel is marked/inserted
 *  last so it always survives coalescing. Result sorted by y. */
export function coalesceTicks(
  matchDisplayRows: readonly number[],
  currentMatchDisplayRow: number | null,
  displayRowCount: number,
  railHeight: number,
): RailTick[] {
  if (railHeight <= 0 || displayRowCount <= 0) return [];
  const seen = new Map<number, { matchIndex: number; displayRow: number }>();
  for (let i = 0; i < matchDisplayRows.length; i += 1) {
    const d = matchDisplayRows[i];
    const y = railYForDisplayRow(d, displayRowCount, railHeight);
    if (!seen.has(y)) seen.set(y, { matchIndex: i, displayRow: d });
  }
  const currentY =
    currentMatchDisplayRow !== null
      ? railYForDisplayRow(currentMatchDisplayRow, displayRowCount, railHeight)
      : null;
  const ticks: RailTick[] = [];
  for (const [y, t] of seen) {
    ticks.push({ y, matchIndex: t.matchIndex, displayRow: t.displayRow, current: y === currentY });
  }
  // The current row may sit in a pixel no plain match occupies (it is derived
  // separately upstream) — ensure it still gets a tick.
  if (currentY !== null && !seen.has(currentY) && currentMatchDisplayRow !== null) {
    ticks.push({ y: currentY, matchIndex: -1, displayRow: currentMatchDisplayRow, current: true });
  }
  ticks.sort((a, b) => a.y - b.y);
  return ticks;
}

/** Viewport thumb from live scroller truth: min-height clamp + track-range
 *  mapping. A non-scrollable history spans the whole rail. */
export function thumbRect(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  railHeight: number,
): ThumbRect {
  if (scrollHeight <= clientHeight || scrollHeight <= 0 || railHeight <= 0) {
    return { top: 0, height: railHeight };
  }
  const height = Math.min(
    railHeight,
    Math.max(THUMB_MIN_PX, (clientHeight / scrollHeight) * railHeight),
  );
  const maxScroll = scrollHeight - clientHeight;
  const track = railHeight - height;
  const frac = Math.min(Math.max(scrollTop / maxScroll, 0), 1);
  return { top: frac * track, height };
}

/** Inverse of `thumbRect`'s track mapping: pointer y (+ grab offset within the
 *  thumb) → clamped scrollTop. */
export function scrollTopForRailY(
  y: number,
  grabOffsetPx: number,
  scrollHeight: number,
  clientHeight: number,
  railHeight: number,
): number {
  if (scrollHeight <= clientHeight || railHeight <= 0) return 0;
  const { height } = thumbRect(0, scrollHeight, clientHeight, railHeight);
  const track = railHeight - height;
  if (track <= 0) return 0;
  const top = Math.min(Math.max(y - grabOffsetPx, 0), track);
  return (top / track) * (scrollHeight - clientHeight);
}

/** Nearest tick within ±TICK_HIT_SLOP_PX of `y`, or null. Ties → nearest. */
export function hitTick(ticks: readonly RailTick[], y: number): RailTick | null {
  let best: RailTick | null = null;
  let bestDist = TICK_HIT_SLOP_PX + 1;
  for (const t of ticks) {
    const dist = Math.abs(t.y - y);
    if (dist <= TICK_HIT_SLOP_PX && dist < bestDist) {
      best = t;
      bestDist = dist;
    }
  }
  return best;
}
