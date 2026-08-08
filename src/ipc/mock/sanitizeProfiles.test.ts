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
});
