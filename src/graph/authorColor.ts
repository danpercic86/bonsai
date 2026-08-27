/** Spec-006: author-mode edge/ring colors (UI contract spec-006-ui.md §1.2;
 *  canon in ui-reference §5.2). Hue = `authorHue(name)` — the identical FNV-1a
 *  hash behind `avatarColor`, so an author's edges, lane ring, and avatar disc
 *  share one hue identity. Per-theme S/L constants are graph-layer constants
 *  (not CSS vars); ONE pair serves both graph styles.
 *
 *  Contrast canon (do not change without a design pass): dark `hsl(h,60%,65%)`
 *  clears ≥4.3:1 on both dark backdrops; light `hsl(h,60%,33%)` clears ≥3.4:1
 *  (worst: yellow vs the Bonsai paper). Raising light L past 33 breaks the
 *  WCAG 1.4.11 3:1 graphics bar. These intentionally differ from AVATAR{52,42}
 *  (disc fill is tuned for white initials, not background contrast).
 *
 *  Cache: hue memoized BY NAME only (author cardinality is small; unbounded
 *  within a mount). The HSL string is composed per call from the resolved
 *  theme — never cache name→string, it would go stale on a theme flip. */

import { authorHue } from './geometry';

/** Dark-theme saturation/lightness (contract §1.2). */
export const AUTHOR_EDGE_DARK = { sat: 60, light: 65 } as const;
/** Light-theme saturation/lightness (contract §1.2). */
export const AUTHOR_EDGE_LIGHT = { sat: 60, light: 33 } as const;

const hueCache = new Map<string, number>();

/** Memoized `authorHue` — keyed by the RAW name (trim happens in the hash). */
export function cachedAuthorHue(name: string): number {
  const hit = hueCache.get(name);
  if (hit !== undefined) return hit;
  const hue = authorHue(name);
  hueCache.set(name, hue);
  return hue;
}

/** Author-mode stroke color for edges + the avatar lane ring. `dark` is the
 *  resolved app mode (bg0 luminance), NOT the graph style. */
export function authorEdgeColor(name: string, dark: boolean): string {
  const { sat, light } = dark ? AUTHOR_EDGE_DARK : AUTHOR_EDGE_LIGHT;
  return `hsl(${cachedAuthorHue(name)}, ${sat}%, ${light}%)`;
}

/** Test hook: clear the hue memo between cases. */
export function resetAuthorColorCache(): void {
  hueCache.clear();
}
