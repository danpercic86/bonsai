/** Theme resolution for the canvas graph (contract M2-graph.md §3.2).
 * Colors live as CSS custom properties in styles.css; this module reads them
 * ONCE per mount/theme change — callers cache the result, never per frame. */

export interface Theme {
  /** 10 entries, --lane-0..--lane-9 (ui-reference §5). */
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

/** One getComputedStyle pass over the element's resolved custom properties. */
export function resolveTheme(el: HTMLElement): Theme {
  const cs = getComputedStyle(el);
  const read = (name: string): string => cs.getPropertyValue(name).trim();

  const laneColors: string[] = [];
  for (let i = 0; i < 10; i++) laneColors.push(read(`--lane-${i}`));
  const laneColorsAlpha = laneColors.map((c) => hexToRgba(c, 0.18));

  return {
    laneColors,
    laneColorsAlpha,
    bg0: read('--bg-0'),
    bg2: read('--bg-2'),
    border: read('--border'),
    text1: read('--text-1'),
    text2: read('--text-2'),
    text3: read('--text-3'),
    selection: read('--selection'),
    accent: read('--accent'),
    accentText: read('--accent-text'),
    danger: read('--danger'),
    warning: read('--warning'),
  };
}
