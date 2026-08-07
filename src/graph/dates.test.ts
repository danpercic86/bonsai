import { describe, expect, it } from 'vitest';

import { formatAbsolute, relativeDate, shortSha } from './dates';

describe('shortSha', () => {
  it('takes the first 7 hex chars by default', () => {
    expect(shortSha('0123456789abcdef0123456789abcdef01234567')).toBe('0123456');
  });
  it('honors a custom length', () => {
    expect(shortSha('0123456789abcdef', 12)).toBe('0123456789ab');
  });
  it('passes short/empty ids through unchanged', () => {
    expect(shortSha('abc')).toBe('abc');
    expect(shortSha('')).toBe('');
  });
});

describe('formatAbsolute', () => {
  it('formats a local timestamp as YYYY-MM-DD HH:mm', () => {
    // Built from a LOCAL Date so the assertion is timezone-independent:
    // formatAbsolute reads the same local fields, so the offset cancels.
    const secs = Math.floor(new Date(2026, 7, 7, 14, 32, 5).getTime() / 1000);
    expect(formatAbsolute(secs)).toBe('2026-08-07 14:32');
  });
  it('zero-pads single-digit month / day / hour / minute', () => {
    const secs = Math.floor(new Date(2026, 0, 3, 4, 9, 0).getTime() / 1000);
    expect(formatAbsolute(secs)).toBe('2026-01-03 04:09');
  });
  it('always matches the fixed YYYY-MM-DD HH:mm shape', () => {
    expect(formatAbsolute(0)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
  });
});

describe('relativeDate', () => {
  const now = 1_000_000_000;
  it('bucket boundaries: now / m / h / d / mo / y', () => {
    expect(relativeDate(now, now)).toBe('now');
    expect(relativeDate(now - 120, now)).toBe('2m');
    expect(relativeDate(now - 7200, now)).toBe('2h');
    expect(relativeDate(now - 172800, now)).toBe('2d');
    expect(relativeDate(now - 60 * 60 * 24 * 60, now)).toBe('2mo');
    expect(relativeDate(now - 60 * 60 * 24 * 400, now)).toBe('1y');
  });
  it('clamps future timestamps to "now"', () => {
    expect(relativeDate(now + 5000, now)).toBe('now');
  });
});
