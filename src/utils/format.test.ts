import { describe, expect, it } from 'vitest';

import { formatBytes } from './format';

describe('formatBytes', () => {
  it('bytes below 1024 render as integers with " B"', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1)).toBe('1 B');
    expect(formatBytes(1023)).toBe('1023 B');
  });

  it('KiB boundary and one-decimal formatting', () => {
    expect(formatBytes(1024)).toBe('1.0 KiB');
    expect(formatBytes(1536)).toBe('1.5 KiB');
    expect(formatBytes(1024 * 1024 - 1)).toBe('1024.0 KiB'); // just under the MiB cut
  });

  it('MiB boundary', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MiB');
    expect(formatBytes(Math.round(2.5 * 1024 * 1024))).toBe('2.5 MiB');
  });

  it('GiB boundary and huge values stay in GiB (no TiB unit)', () => {
    expect(formatBytes(1024 ** 3)).toBe('1.0 GiB');
    expect(formatBytes(5 * 1024 ** 4)).toBe('5120.0 GiB');
  });

  it('fractional bytes below 1024 pass through as-is', () => {
    expect(formatBytes(12.7)).toBe('12.7 B');
  });

  it('negative input renders as negative bytes (documented current behavior)', () => {
    expect(formatBytes(-5)).toBe('-5 B');
  });
});
