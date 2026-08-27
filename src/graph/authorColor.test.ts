/** Spec-006: author-color formulas + memoization (UI contract §1.2). */
import { beforeEach, describe, expect, it } from 'vitest';

import {
  AUTHOR_EDGE_DARK,
  AUTHOR_EDGE_LIGHT,
  authorEdgeColor,
  cachedAuthorHue,
  resetAuthorColorCache,
} from './authorColor';
import { authorHue, avatarColor } from './geometry';

beforeEach(() => resetAuthorColorCache());

describe('authorHue', () => {
  it('is deterministic and trims like avatarColor', () => {
    expect(authorHue('Dan Percic')).toBe(authorHue('Dan Percic'));
    expect(authorHue('  Dan Percic  ')).toBe(authorHue('Dan Percic'));
    expect(authorHue('x')).toBeGreaterThanOrEqual(0);
    expect(authorHue('x')).toBeLessThan(360);
  });

  it('shares one hue identity with the avatar disc (contract §1.2)', () => {
    for (const name of ['Dan Percic', 'torvalds', '', '李', 'a'.repeat(60)]) {
      const hue = authorHue(name);
      expect(avatarColor(name).bg).toBe(`hsl(${hue}, 52%, 42%)`);
    }
  });

  it('pins the literal FNV-1a output (a silent hashString change must fail here)', () => {
    // Machine-computed: FNV-1a 32-bit over code points of the trimmed name,
    // Math.imul overflow, >>> 0, % 360. If hashString ever changes, every
    // persisted author↔hue identity silently shifts — this literal catches it.
    expect(authorHue('Grace Hopper')).toBe(283);
    expect(authorHue('Ada Lovelace')).toBe(6);
    expect(authorHue('')).toBe(61);
  });

  it('handles empty/odd names without special-casing', () => {
    expect(() => authorHue('')).not.toThrow();
    expect(authorHue('')).toBe(authorHue('   '));
  });
});

describe('authorEdgeColor', () => {
  it('applies the per-theme S/L constants (dark 60/65, light 60/33)', () => {
    const hue = authorHue('Grace Hopper');
    expect(authorEdgeColor('Grace Hopper', true)).toBe(`hsl(${hue}, 60%, 65%)`);
    expect(authorEdgeColor('Grace Hopper', false)).toBe(`hsl(${hue}, 60%, 33%)`);
    expect(AUTHOR_EDGE_DARK).toEqual({ sat: 60, light: 65 });
    expect(AUTHOR_EDGE_LIGHT).toEqual({ sat: 60, light: 33 });
  });

  it('memoizes the hue by name but never the composed string across themes', () => {
    const dark = authorEdgeColor('Ada', true);
    const light = authorEdgeColor('Ada', false);
    expect(dark).not.toBe(light); // theme flip yields a fresh string
    expect(cachedAuthorHue('Ada')).toBe(authorHue('Ada'));
    // Cached hue survives and stays consistent on repeat calls.
    expect(authorEdgeColor('Ada', true)).toBe(dark);
  });

  it('resetAuthorColorCache clears the memo (test hook)', () => {
    cachedAuthorHue('Ada');
    resetAuthorColorCache();
    expect(cachedAuthorHue('Ada')).toBe(authorHue('Ada'));
  });
});
