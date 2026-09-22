/** P113a — the per-host forge rate-limit back-off registry: the hint→window
 *  policy (clamped, with a default when the provider advertised nothing), the
 *  host (not repo) keying, and "the longer window wins". */
import { beforeEach, describe, expect, it } from 'vitest';

import {
  backoffMsFor,
  DEFAULT_BACKOFF_MS,
  forgeBackoffUntil,
  isForgeSuppressed,
  isRateLimit,
  MAX_BACKOFF_MS,
  noteForgeRateLimit,
  resetForgeBackoff,
} from './forgeBackoff';
import type { AppError } from '../../ipc';

beforeEach(() => resetForgeBackoff());

const NOW = 1_700_000_000_000;
function limited(retryAfterSecs?: number): AppError {
  return {
    kind: 'forgeRateLimited',
    message: 'Azure DevOps API rate limit exceeded',
    ...(retryAfterSecs === undefined ? {} : { retryAfterSecs }),
  };
}

describe('backoffMsFor', () => {
  it('uses the advertised hint', () => {
    expect(backoffMsFor(30)).toBe(30_000);
  });

  it('falls back to the default when the provider advertised nothing', () => {
    expect(backoffMsFor(undefined)).toBe(DEFAULT_BACKOFF_MS);
  });

  it('ignores a nonsensical hint rather than retrying immediately', () => {
    expect(backoffMsFor(0)).toBe(DEFAULT_BACKOFF_MS);
    expect(backoffMsFor(-5)).toBe(DEFAULT_BACKOFF_MS);
    expect(backoffMsFor(Number.NaN)).toBe(DEFAULT_BACKOFF_MS);
  });

  it('clamps: a 1 s hint gets a floor, a 24 h hint gets a ceiling', () => {
    expect(backoffMsFor(1)).toBe(5_000);
    expect(backoffMsFor(86_400)).toBe(MAX_BACKOFF_MS);
  });
});

describe('isRateLimit', () => {
  it('accepts only a forgeRateLimited AppError', () => {
    expect(isRateLimit(limited(10))).toBe(true);
    expect(isRateLimit({ kind: 'networkError', message: 'offline' })).toBe(false);
    expect(isRateLimit(new Error('boom'))).toBe(false);
    expect(isRateLimit(null)).toBe(false);
  });
});

describe('noteForgeRateLimit', () => {
  it('opens a window for the advertised interval and suppresses until it closes', () => {
    const until = noteForgeRateLimit('dev.azure.com', limited(30), NOW);
    expect(until).toBe(NOW + 30_000);
    expect(isForgeSuppressed('dev.azure.com', NOW + 29_999)).toBe(true);
    expect(isForgeSuppressed('dev.azure.com', NOW + 30_001)).toBe(false);
  });

  it('is keyed by HOST, so every repo on the account shares one budget', () => {
    noteForgeRateLimit('dev.azure.com', limited(30), NOW);
    // Five repos on one Azure DevOps org all read the same window …
    expect(isForgeSuppressed('dev.azure.com', NOW + 1)).toBe(true);
    // … and an unrelated host is unaffected.
    expect(isForgeSuppressed('github.com', NOW + 1)).toBe(false);
  });

  it('ignores errors that are not rate limits', () => {
    expect(noteForgeRateLimit('github.com', { kind: 'forgeApi', message: '404' }, NOW)).toBeNull();
    expect(isForgeSuppressed('github.com', NOW)).toBe(false);
  });

  it('a later, SHORTER hint cannot shorten an open window', () => {
    noteForgeRateLimit('github.com', limited(600), NOW);
    const until = noteForgeRateLimit('github.com', limited(10), NOW + 1_000);
    expect(until).toBe(NOW + 600_000);
  });

  it('a later, LONGER hint extends it', () => {
    noteForgeRateLimit('github.com', limited(10), NOW);
    const until = noteForgeRateLimit('github.com', limited(300), NOW + 1_000);
    expect(until).toBe(NOW + 1_000 + 300_000);
  });

  it('an expired window is forgotten on read', () => {
    noteForgeRateLimit('github.com', limited(10), NOW);
    expect(forgeBackoffUntil('github.com', NOW + 11_000)).toBeNull();
    expect(forgeBackoffUntil('github.com', NOW)).toBeNull(); // dropped, not resurrected
  });

  it('resetForgeBackoff clears one host or all of them', () => {
    noteForgeRateLimit('github.com', limited(60), NOW);
    noteForgeRateLimit('gitlab.com', limited(60), NOW);
    resetForgeBackoff('github.com');
    expect(isForgeSuppressed('github.com', NOW)).toBe(false);
    expect(isForgeSuppressed('gitlab.com', NOW)).toBe(true);
    resetForgeBackoff();
    expect(isForgeSuppressed('gitlab.com', NOW)).toBe(false);
  });
});
