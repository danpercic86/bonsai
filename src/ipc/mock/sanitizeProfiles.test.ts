import { describe, expect, it } from 'vitest';
import { sanitizeProfiles } from './persistence';
import type { IdentityProfile } from '../types';

const valid: IdentityProfile = {
  id: 'p1',
  label: 'Work',
  userName: 'Alice',
  userEmail: 'alice@example.com',
  signingKey: null,
};

describe('sanitizeProfiles (audit fix: per-element validation)', () => {
  it('returns null for non-array input', () => {
    expect(sanitizeProfiles(undefined)).toBeNull();
    expect(sanitizeProfiles(null)).toBeNull();
    expect(sanitizeProfiles('nope')).toBeNull();
    expect(sanitizeProfiles({ 0: valid })).toBeNull();
  });

  it('keeps a legitimately empty list empty (no seed resurrection)', () => {
    expect(sanitizeProfiles([])).toEqual([]);
  });

  it('passes through fully valid profiles', () => {
    const withKey = { ...valid, id: 'p2', signingKey: 'ABCDEF' };
    expect(sanitizeProfiles([valid, withKey])).toEqual([valid, withKey]);
  });

  it('drops malformed elements but keeps survivors', () => {
    const out = sanitizeProfiles([
      valid,
      null,
      42,
      'str',
      { id: 'x' }, // missing fields
      { ...valid, userEmail: 7 }, // wrong type
      { ...valid, signingKey: 5 }, // signingKey must be string|null
    ]);
    expect(out).toEqual([valid]);
  });

  it('returns null when a NON-empty array yields no survivors (all corrupt)', () => {
    expect(sanitizeProfiles([null, 1, { id: 3 }])).toBeNull();
  });

  // --- P82 color normalization (AC d) ---------------------------------------

  it('keeps a profile with a valid color unchanged', () => {
    const colored = { ...valid, color: 'blue' as const };
    expect(sanitizeProfiles([colored])).toEqual([colored]);
  });

  it('preserves a missing color as undefined (legacy ⇒ neutral on read)', () => {
    const [out] = sanitizeProfiles([valid])!;
    expect('color' in out ? out.color : undefined).toBeUndefined();
  });

  it('coerces an invalid color to neutral rather than dropping the profile', () => {
    const bad = { ...valid, color: 'chartreuse' } as unknown as IdentityProfile;
    const out = sanitizeProfiles([bad]);
    expect(out).toEqual([{ ...valid, color: 'neutral' }]);
  });

  it('accepts every palette color (all 9 variants) unchanged', () => {
    const palette = [
      'neutral',
      'slate',
      'blue',
      'teal',
      'green',
      'amber',
      'orange',
      'purple',
      'pink',
    ] as const;
    const input = palette.map((color, i) => ({ ...valid, id: `p${i}`, color }));
    expect(sanitizeProfiles(input)).toEqual(input);
  });
});
