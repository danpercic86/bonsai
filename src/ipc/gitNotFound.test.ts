/** P70 §7.4 / UI §10.3: the `gitNotFound` predicate, the latch, and the routing
 *  rule for a user-pressed remote op (latch + ONE keyed toast; everything else
 *  keeps its existing unkeyed toast). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { isGitNotFound } from './errors';
import {
  clearGitNotFoundLatch,
  GIT_NOT_FOUND_TOAST_KEY,
  gitNotFoundLatched,
  gitNotFoundToastText,
  noteGitNotFound,
  reportRemoteOpError,
  resetGitNotFoundLatchForTests,
  subscribeGitNotFound,
} from './gitNotFound';
import type { AppError } from './types';

const GIT_NOT_FOUND: AppError = { kind: 'gitNotFound', message: 'Git is not available. …' };
const AUTH_FAILED: AppError = { kind: 'authFailed', message: 'authentication failed for …' };

beforeEach(() => resetGitNotFoundLatchForTests());
afterEach(() => {
  resetGitNotFoundLatchForTests();
  vi.restoreAllMocks();
});

describe('isGitNotFound', () => {
  it('narrows only the gitNotFound kind, and survives junk input', () => {
    expect(isGitNotFound(GIT_NOT_FOUND)).toBe(true);
    // authFailed must NOT be swept up: an SSH auth failure with git missing is
    // still a genuine auth failure (backend §3.1).
    expect(isGitNotFound(AUTH_FAILED)).toBe(false);
    expect(isGitNotFound(null)).toBe(false);
    expect(isGitNotFound(undefined)).toBe(false);
    expect(isGitNotFound('gitNotFound')).toBe(false);
    expect(isGitNotFound(new Error('boom'))).toBe(false);
    expect(isGitNotFound({ kind: 'gitNotFound' })).toBe(false); // no message
  });
});

describe('the latch', () => {
  it('is idempotent: repeated notes fire the subscriber exactly once', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeGitNotFound(listener);
    expect(gitNotFoundLatched()).toBe(false);

    noteGitNotFound();
    noteGitNotFound();
    noteGitNotFound();
    expect(gitNotFoundLatched()).toBe(true);
    expect(listener).toHaveBeenCalledTimes(1);

    unsubscribe();
    clearGitNotFoundLatch();
    expect(gitNotFoundLatched()).toBe(false);
    expect(listener).toHaveBeenCalledTimes(1); // unsubscribed
  });
});

describe('reportRemoteOpError', () => {
  it('gitNotFound ⇒ latch + one KEYED toast in plain language', () => {
    const push = vi.fn();
    reportRemoteOpError('Fetch', GIT_NOT_FOUND, push);
    expect(gitNotFoundLatched()).toBe(true);
    expect(push).toHaveBeenCalledTimes(1);
    expect(push).toHaveBeenCalledWith(
      'error',
      "Fetch failed — Bonsai can't run Git to read your saved sign-in.",
      GIT_NOT_FOUND_TOAST_KEY,
    );
    // The user-facing text never says "authentication" or "credential helper".
    const text = gitNotFoundToastText('Push');
    expect(text).not.toMatch(/authentication|credential helper/i);
  });

  it('any other error keeps its existing unkeyed toast and never latches', () => {
    const push = vi.fn();
    reportRemoteOpError('Pull', AUTH_FAILED, push);
    expect(gitNotFoundLatched()).toBe(false);
    expect(push).toHaveBeenCalledWith('error', AUTH_FAILED.message);
    expect(push.mock.calls[0]).toHaveLength(2); // no key argument
  });

  it('every op label produces its own text, so a replacement is visible', () => {
    const texts = (['Fetch', 'Pull', 'Push', 'Fetch all', 'Clone'] as const).map(
      gitNotFoundToastText,
    );
    expect(new Set(texts).size).toBe(texts.length);
  });
});
