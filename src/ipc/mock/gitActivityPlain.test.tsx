/** P119 §5.1 — the mock's plain-run script and dispatch: a new category is never
 *  dressed up as a commit, `targetCount`/`outcome` mirror the backend's wire
 *  rules, and a failure ends with the error-message line (not for hook
 *  rejections). `.tsx` so it runs under jsdom (it rewrites `location`). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppError, GitActivityCategory, GitActivityEvent, GitRunOutcome } from '../types';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.resetModules();
  window.history.replaceState({}, '', '/');
});

interface RunOpts {
  search?: string;
  count?: number;
  classify?: (r: string) => GitRunOutcome | null;
  fail?: AppError;
}

/** Drive ONE run through a fresh module and return every event it emitted. */
async function eventsOf(category: GitActivityCategory, opts: RunOpts = {}): Promise<GitActivityEvent[]> {
  vi.resetModules();
  const search = opts.search ?? '';
  window.history.replaceState({}, '', search === '' ? '/' : `/?${search}`);
  const mod = await import('./gitActivity');
  const events: GitActivityEvent[] = [];
  mod.subscribeGitActivity((e) => events.push(e));
  const fail = opts.fail;
  const done = mod
    .runMockActivity(
      category,
      'topic',
      () => (fail !== undefined ? Promise.reject(fail) : Promise.resolve('ok')),
      { count: opts.count, classify: opts.classify },
    )
    .catch(() => undefined);
  await vi.advanceTimersByTimeAsync(10_000);
  await done;
  return events;
}

describe('runPlain', () => {
  it('a new category runs the plain script — no commit hooks', async () => {
    const events = await eventsOf('deleteBranch');
    expect(events.map((e) => e.kind)).toEqual(['started', 'finished']);
    expect(events[0]).toMatchObject({ category: 'deleteBranch', target: 'topic' });
    expect(events[1]).toEqual(expect.objectContaining({ code: 0, success: true }));
    expect(events[1]).not.toHaveProperty('outcome');
  });

  it('network rows emit phase(network) right after started', async () => {
    const events = await eventsOf('pushTag');
    expect(events.map((e) => e.kind)).toEqual(['started', 'phase', 'finished']);
    expect(events[1].phase).toEqual({ kind: 'network' });
  });

  it('?fetchSlow ramps determinate progress on a clone', async () => {
    const events = await eventsOf('cloneRepo', { search: 'fetchSlow' });
    expect(events.filter((e) => e.kind === 'progress').length).toBeGreaterThan(1);
  });

  it('count >= 2 carries targetCount and drops the target', async () => {
    const [started] = await eventsOf('deleteBranches', { count: 3 });
    expect(started.targetCount).toBe(3);
    expect(started).not.toHaveProperty('target');
    const [single] = await eventsOf('deleteBranches', { count: 1 });
    expect(single).not.toHaveProperty('targetCount');
    expect(single.target).toBe('topic');
  });

  it('classify sets outcome on a successful finished', async () => {
    const events = await eventsOf('merge', { classify: () => 'fastForwarded' });
    expect(events.at(-1)).toMatchObject({ kind: 'finished', success: true, outcome: 'fastForwarded' });
  });

  it('a failure ends with the error message line, then a failed finished', async () => {
    const events = await eventsOf('deleteBranch', {
      fail: { kind: 'branchNotFound', message: "branch 'topic' not found" },
    });
    expect(events.slice(-2)).toEqual([
      expect.objectContaining({ kind: 'stderrLine', line: "branch 'topic' not found" }),
      expect.objectContaining({ kind: 'finished', success: false }),
    ]);
    expect(events.at(-1)).not.toHaveProperty('outcome');
  });

  it('a P87 script gains the failure line too, but a hook rejection does not repeat it', async () => {
    const fetchFail = await eventsOf('fetch', { fail: { kind: 'networkError', message: 'offline' } });
    expect(fetchFail.at(-2)).toMatchObject({ kind: 'stderrLine', line: 'offline' });
    const hook = await eventsOf('commit', { fail: { kind: 'hookRejected', message: 'lint failed' } });
    const lines = hook.filter((e) => e.kind === 'stderrLine').map((e) => e.line);
    expect(lines).toEqual(['lint failed']);
  });
});
