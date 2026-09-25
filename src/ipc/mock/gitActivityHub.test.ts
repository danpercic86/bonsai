/** P123 V6 — the mock hub mirrors `activity_hub.rs` R4-R6: routing by repo,
 *  broadcast for an unscoped run, idempotent unsubscribe, `gitActivityActive`. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  emitGitActivity,
  gitActivityActive,
  gitActivitySubscriberCount,
  subscribeGitActivity,
  unsubscribeGitActivity,
} from './gitActivityHub';
import type { GitActivityEvent, GitActivityListener } from '../types';

const EV: GitActivityEvent = { id: 'git-hub-1', seq: 0, kind: 'started', elapsedMs: 0 };
const live: string[] = [];

function sub(repoId: string | null) {
  const cb = vi.fn<GitActivityListener>();
  live.push(subscribeGitActivity(repoId, cb));
  return cb;
}

afterEach(() => {
  for (const id of live.splice(0)) unsubscribeGitActivity(id);
});

describe('mock gitActivityHub (P123)', () => {
  it('routes a scoped emit to its repo (+ wildcard) and an unscoped one to all', () => {
    const a1 = sub('a');
    const a2 = sub('a');
    const b = sub('b');
    const w = sub(null);
    emitGitActivity('a', EV);
    expect([a1, a2, b, w].map((f) => f.mock.calls.length)).toEqual([1, 1, 0, 1]);
    emitGitActivity(null, EV);
    expect([a1, a2, b, w].map((f) => f.mock.calls.length)).toEqual([2, 2, 1, 2]);
  });

  it('mints distinct ids; unsubscribe is exact and idempotent', () => {
    const base = gitActivitySubscriberCount();
    const cb = vi.fn<GitActivityListener>();
    const id1 = subscribeGitActivity('a', cb);
    const id2 = subscribeGitActivity('a', cb);
    expect(id1).not.toBe(id2);
    expect(id1).toMatch(/^gas-mock-\d+$/);
    unsubscribeGitActivity(id1);
    unsubscribeGitActivity(id1);
    unsubscribeGitActivity('gas-mock-unknown');
    expect(gitActivitySubscriberCount() - base).toBe(1);
    emitGitActivity('a', EV);
    expect(cb).toHaveBeenCalledTimes(1);
    unsubscribeGitActivity(id2);
    expect(gitActivitySubscriberCount()).toBe(base);
  });

  it('gitActivityActive is per route; null = any sub', () => {
    expect(gitActivityActive('b')).toBe(false);
    sub('a');
    expect(gitActivityActive('a')).toBe(true);
    expect(gitActivityActive('b')).toBe(false);
    expect(gitActivityActive(null)).toBe(true);
  });
});
