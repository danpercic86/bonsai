/** Theme resolution for the canvas graph (contract M2-graph.md §3.2).
 * Colors live as CSS custom properties in styles.css; this module reads them
 * ONCE per mount/theme change — callers cache the result, never per frame. */

export interface Theme {
  /** 10 entries, graph-layer lane palette for the resolved theme (ui-reference §5). */
  laneColors: string[];
  /** laneColors at 18% alpha, precomputed once per theme (pill backgrounds). */
  laneColorsAlpha: string[];
  bg0: string;
  bg2: string;
  border: string;
  text1: string;
  text2: string;
  text3: string;
  selection: string;
  accent: string;
  danger: string;
  warning: string;
  /** P50b: search-match ring color (distinct from head/selection rings). */
  matchRing: string;
  /** P58c: signature-badge palette (OQ7) — green good / red warn / neutral
   *  unknown. Read once per mount/theme like the rest of the theme. */
  badgeGood: string;
  badgeWarn: string;
  badgeUnknown: string;
}

/** Lane palettes (ui-reference §5). Deterministic `lane % 10` assignment; stable
 *  while scrolling by construction. Theme-specific: the light palette darkens each
 *  hue to clear the 3:1 graphics bar vs `#ffffff` (the dark palette is unchanged).
 *  These live as graph-layer constants (NOT CSS `--lane-N` vars) so the draw layer
 *  owns the palette and selects by resolved theme. Hue order 0..9 preserved. */
export const LANE_COLORS_DARK: readonly string[] = [
  '#4f8cff', // 0 blue
  '#f2994a', // 1 orange
  '#9b6dff', // 2 purple
  '#43b97f', // 3 green
  '#e5534b', // 4 red
  '#3ec6c0', // 5 teal
  '#e8c341', // 6 yellow
  '#f26d9c', // 7 pink
  '#7a86ff', // 8 indigo
  '#8fbf4d', // 9 lime
];
export const LANE_COLORS_LIGHT: readonly string[] = [
  '#2f6fe4', // 0 blue
  '#b0530f', // 1 orange
  '#7b46d6', // 2 purple
  '#1b7d4c', // 3 green
  '#c62f33', // 4 red
  '#0c7d78', // 5 teal
  '#8a6f08', // 6 yellow
  '#c8437a', // 7 pink
  '#5560e0', // 8 indigo
  '#517c20', // 9 lime
];

/** Detached-HEAD pill background (ui-reference §6). Fixed dark red in both themes
 *  (white text = 6.54:1); replaces `--danger`, which gave only 3.70:1 in dark. */
export const DETACHED_HEAD_BG = '#b3261e';

/** Relative luminance of a `#rrggbb` color (WCAG-style, simple sRGB). Non-hex
 *  input returns 0 (treated as dark). */
function relLuminance(hex: string): number {
  const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
  if (m === null) return 0;
  const v = parseInt(m[1], 16);
  const r = ((v >> 16) & 0xff) / 255;
  const g = ((v >> 8) & 0xff) / 255;
  const b = (v & 0xff) / 255;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** True when a `#rrggbb` background reads as dark (luminance < 0.5). Non-hex
 *  input defaults to dark (the app's default theme). */
export function isDarkBg(hex: string): boolean {
  return relLuminance(hex) < 0.5;
}

/** Luminance-adaptive pill text (ui-reference §6): near-black `#16181d` on bright
 *  backgrounds, white `#ffffff` on dark ones — whichever maximizes contrast. */
export function adaptivePillText(bg: string): string {
  return isDarkBg(bg) ? '#ffffff' : '#16181d';
}

/** Tag pill color is fixed across themes (ui-reference §6). */
export const TAG_COLOR = '#d4a72c';

/** `#rrggbb` -> `rgba(r, g, b, alpha)`. Returns the input unchanged if it is
 * not a 6-digit hex color (defensive; theme values are ours). */
export function hexToRgba(hex: string, alpha: number): string {
  const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
  if (m === null) return hex;
  const v = parseInt(m[1], 16);
  const r = (v >> 16) & 0xff;
  const g = (v >> 8) & 0xff;
  const b = v & 0xff;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Tag pill background, precomputed at module load. */
export const TAG_BG = hexToRgba(TAG_COLOR, 0.18);

/** Stash pill color is fixed across themes — a muted violet (P9 §6.1). */
export const STASH_COLOR = '#9a7cff';

/** Stash pill background, precomputed at module load. */
export const STASH_BG = hexToRgba(STASH_COLOR, 0.18);

/** One getComputedStyle pass over the element's resolved custom properties. */
export function resolveTheme(el: HTMLElement): Theme {
  const cs = getComputedStyle(el);
  const read = (name: string): string => cs.getPropertyValue(name).trim();

  // Lane palette is graph-layer-owned (not CSS vars); select by resolved theme.
  const bg0 = read('--bg-0');
  const palette = isDarkBg(bg0) ? LANE_COLORS_DARK : LANE_COLORS_LIGHT;
  const laneColors = palette.slice();
  const laneColorsAlpha = laneColors.map((c) => hexToRgba(c, 0.18));

  return {
    laneColors,
    laneColorsAlpha,
    bg0,
    bg2: read('--bg-2'),
    border: read('--border'),
    text1: read('--text-1'),
    text2: read('--text-2'),
    text3: read('--text-3'),
    selection: read('--selection'),
    accent: read('--accent'),
    danger: read('--danger'),
    warning: read('--warning'),
    matchRing: read('--match-ring'),
    badgeGood: read('--badge-good'),
    badgeWarn: read('--badge-warn'),
    badgeUnknown: read('--badge-unknown'),
  };
}
