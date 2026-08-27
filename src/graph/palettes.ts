/** Graph lane palettes + Bonsai seasonal accents (pure data; no logic).
 *
 * Standard palettes (`LANE_COLORS_DARK` / `LANE_COLORS_LIGHT`) were moved here
 * out of `colors.ts` to keep that module logic-only under the ~500-line limit;
 * `colors.ts` re-exports them so existing imports keep working.
 *
 * Bonsai palettes + seasonal accent/backdrop values are canonical in
 * `docs/contracts/002-bonsai-graph-theme-ui.md` §1.1–1.3 / §4.1 — hex values are
 * verbatim (chosen for ≥5:1 backdrop contrast and ≥4.5:1 `adaptivePillText`).
 * Do NOT re-derive hues. */

// ── Standard lane palettes (ui-reference §5; moved from colors.ts) ──────────

/** Deterministic `lane % 10` assignment; stable while scrolling by construction.
 *  Dark palette (unchanged). */
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

/** Light palette darkens each hue to clear the 3:1 graphics bar vs `#ffffff`. */
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

// ── Bonsai lane palettes (UI contract §1.1 / §1.2) ─────────────────────────

/** Foliage on soil — bright earthy hues, all L≈0.66–0.78 so `adaptivePillText`
 *  picks near-black; each ≥5.0:1 vs the dark backdrop `#17140f`. */
export const LANE_COLORS_BONSAI_DARK: readonly string[] = [
  '#86c5b0', // 0 sage green
  '#e3c07a', // 1 warm sand
  '#c3a6e0', // 2 lilac bloom
  '#9cc873', // 3 moss leaf
  '#e39b83', // 4 clay
  '#7fccc4', // 5 jade teal
  '#ddc85f', // 6 gold ochre
  '#e6a6bf', // 7 rose bloom
  '#a3aee6', // 8 wisteria
  '#bcd17a', // 9 young lime
];

/** Bark ink on paper — deep hues, all L≈0.16–0.17 so `adaptivePillText` picks
 *  white; each ≥5.0:1 vs the light backdrop `#f4efe6`. Do NOT brighten past
 *  L≈0.18 or white pill text drops below 4.5:1. */
export const LANE_COLORS_BONSAI_LIGHT: readonly string[] = [
  '#123330', // 0 deep pine
  '#45280e', // 1 bark umber
  '#372440', // 2 plum bloom
  '#163419', // 3 forest green
  '#501e16', // 4 deep clay
  '#0b3038', // 5 deep teal
  '#332907', // 6 dark ochre
  '#4e1e30', // 7 deep rose
  '#24284e', // 8 deep indigo
  '#2c3009', // 9 dark olive
];

// ── Seasonal accent + backdrop variants (UI contract §1.3 / §4.1) ──────────

/** Blossom accent + near-flat backdrop for one season, per app light/dark mode.
 *  Seasons never alter the 10 lane hues — only these three isolated things. */
export interface SeasonPalette {
  /** Blossom/HEAD accent (§2.3) — used only by the HEAD/selected blossom. */
  blossomAccentDark: string;
  blossomAccentLight: string;
  /** Backdrop base color = `graphBackdrop` field consumed by the avatar halo. */
  backdropDark: string;
  backdropLight: string;
  /** Optional vertical-gradient endpoints (≤3% luminance spread, §4.1). When a
   *  season only tints the base, top/bottom equal the base (flat). */
  backdropDarkTop: string;
  backdropDarkBottom: string;
  backdropLightTop: string;
  backdropLightBottom: string;
}

export type GraphSeason = 'living' | 'spring' | 'autumn';

export const DEFAULT_SEASON: GraphSeason = 'living';

/** UI contract §1.3 table + §4.1 gradient endpoints (Living only). */
export const SEASON_PALETTES: Readonly<Record<GraphSeason, SeasonPalette>> = {
  living: {
    blossomAccentDark: '#e6a6bf',
    blossomAccentLight: '#7a2f4d',
    backdropDark: '#17140f',
    backdropLight: '#f4efe6',
    backdropDarkTop: '#191510',
    backdropDarkBottom: '#141109',
    backdropLightTop: '#f7f2ea',
    backdropLightBottom: '#f0eadf',
  },
  spring: {
    blossomAccentDark: '#f2b8d6',
    blossomAccentLight: '#8f3560',
    backdropDark: '#181611',
    backdropLight: '#f6f1ea',
    backdropDarkTop: '#181611',
    backdropDarkBottom: '#181611',
    backdropLightTop: '#f6f1ea',
    backdropLightBottom: '#f6f1ea',
  },
  autumn: {
    blossomAccentDark: '#e59a5b',
    blossomAccentLight: '#8a4713',
    backdropDark: '#1a1510',
    backdropLight: '#f4ece0',
    backdropDarkTop: '#1a1510',
    backdropDarkBottom: '#1a1510',
    backdropLightTop: '#f4ece0',
    backdropLightBottom: '#f4ece0',
  },
};

/** Blossom fill alpha (§2.3): softer on dark, stronger on light. */
export const BLOSSOM_ALPHA_DARK = 0.55;
export const BLOSSOM_ALPHA_LIGHT = 0.65;

// ── Edge taper widths (UI contract §3) ─────────────────────────────────────

/** Stepped stroke widths for the three bezier segments, per density. Standard
 *  theme ignores these (single-width stroke). `branch` == today's `edgeWidth`.
 *  (Task 3 may instead source these from `metrics.ts`; kept here as canonical
 *  data so `resolveTheme` can populate the Theme.) */
export interface EdgeTaperWidths {
  tip: number;
  branch: number;
  trunk: number;
}

export const EDGE_TAPER_COZY: EdgeTaperWidths = { tip: 1.5, branch: 2.0, trunk: 2.5 };
export const EDGE_TAPER_COMPACT: EdgeTaperWidths = { tip: 1.0, branch: 1.5, trunk: 2.0 };
