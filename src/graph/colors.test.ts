import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  LANE_COLORS_DARK,
  LANE_COLORS_LIGHT,
  STASH_BG,
  STASH_COLOR,
  TAG_BG,
  TAG_COLOR,
  adaptivePillText,
  hexToRgba,
  resolveTheme,
} from './colors';

describe('hexToRgba', () => {
  it('converts 6-digit hex to rgba with the given alpha', () => {
    expect(hexToRgba('#ff0000', 0.5)).toBe('rgba(255, 0, 0, 0.5)');
    expect(hexToRgba('#000000', 1)).toBe('rgba(0, 0, 0, 1)');
    expect(hexToRgba('#0a141e', 0.18)).toBe('rgba(10, 20, 30, 0.18)');
  });

  it('is case-insensitive and trims surrounding whitespace', () => {
    expect(hexToRgba('#FFAA00', 1)).toBe('rgba(255, 170, 0, 1)');
    expect(hexToRgba('  #ffffff ', 0.2)).toBe('rgba(255, 255, 255, 0.2)');
  });

  it('returns non-6-digit-hex input unchanged (defensive)', () => {
    expect(hexToRgba('#fff', 0.5)).toBe('#fff'); // 3-digit
    expect(hexToRgba('#ffff0000', 0.5)).toBe('#ffff0000'); // 8-digit
    expect(hexToRgba('red', 0.5)).toBe('red');
    expect(hexToRgba('rgb(1,2,3)', 0.5)).toBe('rgb(1,2,3)');
    expect(hexToRgba('', 0.5)).toBe('');
    expect(hexToRgba('#gggggg', 0.5)).toBe('#gggggg'); // non-hex digits
  });

  it('internal whitespace is NOT tolerated', () => {
    expect(hexToRgba('# ff0000', 0.5)).toBe('# ff0000');
  });

  it('alpha value is embedded verbatim (0 and >1 pass through)', () => {
    expect(hexToRgba('#ffffff', 0)).toBe('rgba(255, 255, 255, 0)');
    expect(hexToRgba('#ffffff', 2)).toBe('rgba(255, 255, 255, 2)');
  });
});

describe('fixed pill colors', () => {
  it('TAG and STASH colors are the locked values with 18%-alpha backgrounds', () => {
    expect(TAG_COLOR).toBe('#d4a72c');
    expect(TAG_BG).toBe(hexToRgba('#d4a72c', 0.18));
    expect(STASH_COLOR).toBe('#9a7cff');
    expect(STASH_BG).toBe(hexToRgba('#9a7cff', 0.18));
  });
});

describe('resolveTheme', () => {
  afterEach(() => vi.unstubAllGlobals());

  /** Stub getComputedStyle so the node project can drive resolveTheme without
   *  a real DOM — resolveTheme only calls getPropertyValue. */
  function stubComputedStyle(values: Record<string, string>) {
    const getPropertyValue = vi.fn((name: string) => values[name] ?? '');
    vi.stubGlobal(
      'getComputedStyle',
      vi.fn(() => ({ getPropertyValue })),
    );
    return getPropertyValue;
  }

  const fakeEl = {} as HTMLElement;

  it('selects the dark lane palette on a dark --bg-0 (10 colors + 18% alpha)', () => {
    stubComputedStyle({ '--bg-0': '#16181d' });
    const theme = resolveTheme(fakeEl);
    expect(theme.laneColors).toHaveLength(10);
    expect(theme.laneColorsAlpha).toHaveLength(10);
    expect(theme.laneColors).toEqual([...LANE_COLORS_DARK]);
    expect(theme.laneColorsAlpha[0]).toBe(hexToRgba(LANE_COLORS_DARK[0], 0.18));
  });

  it('selects the darkened light lane palette on a light --bg-0', () => {
    stubComputedStyle({ '--bg-0': '#ffffff' });
    const theme = resolveTheme(fakeEl);
    expect(theme.laneColors).toEqual([...LANE_COLORS_LIGHT]);
    expect(theme.laneColorsAlpha[0]).toBe(hexToRgba(LANE_COLORS_LIGHT[0], 0.18));
  });

  it('adaptivePillText picks near-black on bright bg and white on dark bg', () => {
    // Bright light-palette lanes are still "dark" enough? verify by hue: a bright
    // dark-mode lane (yellow) -> near-black; a dark light-mode lane -> white.
    expect(adaptivePillText('#e8c341')).toBe('#16181d'); // bright yellow
    expect(adaptivePillText('#1b7d4c')).toBe('#ffffff'); // darkened green
  });

  it('maps each custom property to its Theme field and trims whitespace', () => {
    const gpv = stubComputedStyle({
      '--bg-0': ' #111111 ',
      '--accent': '#2266ff',
      '--match-ring': '#ff00ff',
      '--badge-good': '#00aa00',
    });
    const theme = resolveTheme(fakeEl);
    expect(theme.bg0).toBe('#111111'); // trimmed
    expect(theme.accent).toBe('#2266ff');
    expect(theme.matchRing).toBe('#ff00ff');
    expect(theme.badgeGood).toBe('#00aa00');
    expect(gpv).toHaveBeenCalledWith('--danger');
    expect(gpv).toHaveBeenCalledWith('--badge-unknown');
  });

  it('missing semantic properties resolve to empty strings; lanes fall back to dark', () => {
    stubComputedStyle({});
    const theme = resolveTheme(fakeEl);
    expect(theme.text1).toBe('');
    // Missing --bg-0 ('') reads as dark (non-hex luminance 0), so the dark
    // palette is used rather than empty strings.
    expect(theme.laneColors).toEqual([...LANE_COLORS_DARK]);
  });
});
