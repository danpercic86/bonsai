// P82 (AC c) — determinism + table-order of the display-fallback helpers in
// identityProfileColor.ts. Pure functions, no DOM.
import { describe, expect, it } from 'vitest';

import {
  ASSIGNABLE_COLORS,
  autoDistinctColors,
  nextFreeHue,
  PROFILE_COLORS,
  resolveProfileColor,
} from './identityProfileColor';
import type { IdentityProfile, ProfileColor } from '../ipc';

function profile(id: string, color?: ProfileColor): IdentityProfile {
  return {
    id,
    label: id,
    userName: 'A',
    userEmail: 'a@example.com',
    signingKey: null,
    ...(color === undefined ? {} : { color }),
  };
}

describe('PROFILE_COLORS / ASSIGNABLE_COLORS table order', () => {
  it('lists all 9 variants in ui-reference §12.8 order', () => {
    expect(PROFILE_COLORS).toEqual([
      'neutral',
      'slate',
      'blue',
      'teal',
      'green',
      'amber',
      'orange',
      'purple',
      'pink',
    ]);
  });

  it('ASSIGNABLE_COLORS is the palette minus neutral, order preserved', () => {
    expect(ASSIGNABLE_COLORS).toEqual([
      'slate',
      'blue',
      'teal',
      'green',
      'amber',
      'orange',
      'purple',
      'pink',
    ]);
  });
});

describe('resolveProfileColor', () => {
  it('reads a missing color as neutral', () => {
    expect(resolveProfileColor(profile('a'))).toBe('neutral');
  });
  it('honours an explicit color (including deliberate neutral)', () => {
    expect(resolveProfileColor(profile('a', 'blue'))).toBe('blue');
    expect(resolveProfileColor(profile('a', 'neutral'))).toBe('neutral');
  });
});

describe('nextFreeHue determinism + table order', () => {
  it('returns the first assignable hue for an empty list', () => {
    expect(nextFreeHue([])).toBe('slate');
  });

  it('skips used hues in table order', () => {
    expect(nextFreeHue([profile('a', 'slate')])).toBe('blue');
    expect(nextFreeHue([profile('a', 'slate'), profile('b', 'blue')])).toBe('teal');
  });

  it('ignores neutral (and color-less) profiles when picking', () => {
    expect(nextFreeHue([profile('a', 'neutral'), profile('b')])).toBe('slate');
  });

  it('when all 8 are taken, returns the least-used, ties broken by table order', () => {
    const all = ASSIGNABLE_COLORS.map((c, i) => profile(`p${i}`, c));
    // every hue used once ⇒ first in table order (slate)
    expect(nextFreeHue(all)).toBe('slate');
    // give slate a second use ⇒ next least-used is blue
    expect(nextFreeHue([...all, profile('extra', 'slate')])).toBe('blue');
  });

  it('is a pure function (same input ⇒ same output)', () => {
    const ps = [profile('a', 'slate'), profile('b', 'teal')];
    expect(nextFreeHue(ps)).toBe(nextFreeHue(ps));
  });
});

describe('autoDistinctColors', () => {
  it('assigns ASSIGNABLE_COLORS[i % 8] to color-less profiles by index', () => {
    const ps = Array.from({ length: 10 }, (_, i) => profile(`p${i}`));
    expect(autoDistinctColors(ps)).toEqual([
      'slate',
      'blue',
      'teal',
      'green',
      'amber',
      'orange',
      'purple',
      'pink',
      'slate', // wraps at index 8
      'blue',
    ]);
  });

  it('honours explicit colors (including neutral) and only falls back for undefined', () => {
    const ps = [profile('a', 'pink'), profile('b'), profile('c', 'neutral')];
    // index 1 is color-less ⇒ ASSIGNABLE_COLORS[1] = 'blue'
    expect(autoDistinctColors(ps)).toEqual(['pink', 'blue', 'neutral']);
  });
});
