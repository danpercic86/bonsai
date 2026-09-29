import { describe, expect, it } from 'vitest';

import { formatAge, formatIn } from './formatAge';

const NOW = 1_800_000_000_000;
const MIN = 60_000;
const HOUR = 60 * MIN;

describe('formatAge (P125-ui §8.4)', () => {
  it('spells every unit out, singular at 1', () => {
    expect(formatAge(NOW - 30_000, NOW)).toBe('just now');
    expect(formatAge(NOW - MIN, NOW)).toBe('1 minute ago');
    expect(formatAge(NOW - 25 * MIN, NOW)).toBe('25 minutes ago');
    expect(formatAge(NOW - HOUR, NOW)).toBe('1 hour ago');
    expect(formatAge(NOW - 47 * HOUR, NOW)).toBe('47 hours ago');
    expect(formatAge(NOW - 48 * HOUR, NOW)).toBe('2 days ago');
    expect(formatAge(NOW + 5_000, NOW)).toBe('just now');
  });
});

describe('formatIn (P125-ui §8.4)', () => {
  it('less than a minute · minutes · hours', () => {
    expect(formatIn(NOW + 10_000, NOW)).toBe('less than a minute');
    expect(formatIn(NOW + 5 * MIN, NOW)).toBe('5 minutes');
    expect(formatIn(NOW + HOUR, NOW)).toBe('1 hour');
    expect(formatIn(NOW + 9 * HOUR + 5 * MIN, NOW)).toBe('9 hours');
  });
});
