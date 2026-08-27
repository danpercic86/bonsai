/** Theme resolution for the canvas graph (contract M2-graph.md §3.2).
 * Colors live as CSS custom properties in styles.css; this module reads them
 * ONCE per mount/theme change — callers cache the result, never per frame.
 *
 * Lane palettes + Bonsai seasonal data live in `palettes.ts` (pure data); this
 * module is logic-only. Standard palettes are re-exported for back-compat. */

import {
  BLOSSOM_ALPHA_DARK,
  BLOSSOM_ALPHA_LIGHT,
  EDGE_TAPER_COZY,
  LANE_COLORS_BONSAI_DARK,
  LANE_COLORS_BONSAI_LIGHT,
  LANE_COLORS_DARK,
  LANE_COLORS_LIGHT,
  SEASON_PALETTES,
  type GraphSeason,
} from './palettes';

export {
  LANE_COLORS_DARK,
  LANE_COLORS_LIGHT,
  LANE_COLORS_BONSAI_DARK,
  LANE_COLORS_BONSAI_LIGHT,
} from './palettes';
export type { GraphSeason } from './palettes';

/** Bonsai vs standard graph paint. Orthogonal to app light/dark. */
export type GraphStyle = 'standard' | 'bonsai';

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
  accentText: string;
  danger: string;
  warning: string;
  /** P50b: search-match ring color (distinct from head/selection rings). */
  matchRing: string;
  /** P58c: signature-badge palette (OQ7) — green good / red warn / neutral
   *  unknown. Read once per mount/theme like the rest of the theme. */
  badgeGood: string;
  badgeWarn: string;
  badgeUnknown: string;

  // ── Bonsai styling (spec 002 UI contract). For graphStyle==='standard' these
  //    carry inert defaults (graphBackdrop === bg0, taper widths mirror the
  //    single stroke) so the standard draw paths are byte-for-byte unchanged. ──
  /** Active graph paint style; the draw layer branches on this. */
  graphStyle: GraphStyle;
  /** Convenience mirror of `graphStyle === 'bonsai'`. */
  bonsai: boolean;
  /** Backdrop base color (§4.1) — also the avatar-halo base in Bonsai. In the
   *  standard theme this equals `bg0` so the halo is unchanged. */
  graphBackdrop: string;
  /** Vertical-gradient endpoints for the canvas backdrop (§4.1). Equal to
   *  `graphBackdrop` in the standard theme. */
  graphBackdropTop: string;
  graphBackdropBottom: string;
  /** Season blossom/HEAD accent (§2.3). Falls back to `accent` in standard. */
  blossomAccent: string;
  /** Blossom fill alpha (§2.3). 0 in standard (no blossom painted). */
  blossomAlpha: number;
  /** Stepped edge taper widths (§3): tip (newer), branch (=today's edgeWidth),
   *  trunk (older). In standard all three equal the single stroke width so the
   *  standard stroke is unchanged. Bonsai defaults to the cozy step; task 3 may
   *  swap to the density-specific step from `metrics.ts`. */
  edgeTipWidth: number;
  edgeBranchWidth: number;
  edgeTrunkWidth: number;
}

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

/** One getComputedStyle pass over the element's resolved custom properties.
 *
 * `graphStyle`/`graphSeason` select the Bonsai paint layer; both default so
 * existing callers (which pass only `el`) keep the standard theme byte-for-byte.
 * The lane palette is graph-layer-owned (not CSS vars): standard selects by the
 * resolved app mode (bg0 luminance); Bonsai selects its own light/dark palette
 * by the same mode, and layers season accent + backdrop on top. */
export function resolveTheme(
  el: HTMLElement,
  graphStyle: GraphStyle = 'standard',
  graphSeason: GraphSeason = 'living',
): Theme {
  const cs = getComputedStyle(el);
  const read = (name: string): string => cs.getPropertyValue(name).trim();

  const bg0 = read('--bg-0');
  const dark = isDarkBg(bg0);
  const bonsai = graphStyle === 'bonsai';

  const standardPalette = dark ? LANE_COLORS_DARK : LANE_COLORS_LIGHT;
  const bonsaiPalette = dark ? LANE_COLORS_BONSAI_DARK : LANE_COLORS_BONSAI_LIGHT;
  const laneColors = (bonsai ? bonsaiPalette : standardPalette).slice();
  const laneColorsAlpha = laneColors.map((c) => hexToRgba(c, 0.18));

  const accent = read('--accent');
  const season = SEASON_PALETTES[graphSeason];
  const graphBackdrop = bonsai ? (dark ? season.backdropDark : season.backdropLight) : bg0;
  const graphBackdropTop = bonsai
    ? dark
      ? season.backdropDarkTop
      : season.backdropLightTop
    : bg0;
  const graphBackdropBottom = bonsai
    ? dark
      ? season.backdropDarkBottom
      : season.backdropLightBottom
    : bg0;
  const blossomAccent = bonsai
    ? dark
      ? season.blossomAccentDark
      : season.blossomAccentLight
    : accent;
  const blossomAlpha = bonsai ? (dark ? BLOSSOM_ALPHA_DARK : BLOSSOM_ALPHA_LIGHT) : 0;
  const taper = EDGE_TAPER_COZY;

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
    accent,
    accentText: read('--accent-text'),
    danger: read('--danger'),
    warning: read('--warning'),
    matchRing: read('--match-ring'),
    badgeGood: read('--badge-good'),
    badgeWarn: read('--badge-warn'),
    badgeUnknown: read('--badge-unknown'),
    graphStyle,
    bonsai,
    graphBackdrop,
    graphBackdropTop,
    graphBackdropBottom,
    blossomAccent,
    blossomAlpha,
    edgeTipWidth: bonsai ? taper.tip : taper.branch,
    edgeBranchWidth: taper.branch,
    edgeTrunkWidth: bonsai ? taper.trunk : taper.branch,
  };
}
