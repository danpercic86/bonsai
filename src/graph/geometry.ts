/** Pure graph geometry + avatar identity helpers (T3.6 split from draw.ts).
 *  Plain data in → plain data out; zero canvas/DOM imports. Every coordinate is
 *  CSS px (the ctx is DPR-transformed by the caller). Geometry is normative per
 *  contract M2-graph.md §1.3/§3.3 and P7 §1.2/§2. */

import { AVATAR } from './metrics';
import type { EffectiveMetrics } from './metrics';

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
 *  the +8 lane inset. Feeds `summaryStartX`. */
export function graphAreaRight(laneCount: number, m: EffectiveMetrics): number {
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

// ---------- avatar identity (P7 §2) ----------

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
