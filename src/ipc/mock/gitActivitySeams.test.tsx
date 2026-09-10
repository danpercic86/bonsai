/** P87b FU-1 §3.10 — the six run-target harness seams, verified without a browser.
 *
 *  `?fetchAll` / `?gitNoTarget` / `?gitLongTarget` / `?gitBidiTarget` are read
 *  ONCE at module init (the file's existing `const X = query('x') !== null`
 *  idiom), so each case needs `vi.resetModules()` + a rewritten `location` — the
 *  `urlSeams.test.tsx` pattern. `?pushSlow` is checked for the property that
 *  matters: the target does not change across the long Network phase.
 *
 *  These assertions are what makes the orchestrator's harness pass (§9.9-9.13)
 *  reproducible: if a seam name is renamed or dropped, this file fails, not a
 *  silent screenshot.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { GitActivityCategory, GitActivityEvent } from '../types';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.resetModules();
  window.history.replaceState({}, '', '/');
});

/** Load a FRESH `gitActivity` module under `search`, then drive one run through
 *  it and return the `target` its `started` event carried (`undefined` = the key
 *  was dropped, matching serde's `skip_serializing_if`). */
async function targetUnder(
  search: string,
  category: GitActivityCategory,
  fixture: string | null,
): Promise<string | undefined> {
  vi.resetModules();
  window.history.replaceState({}, '', search === '' ? '/' : `/?${search}`);
  const mod = await import('./gitActivity');
  const events: GitActivityEvent[] = [];
  mod.subscribeGitActivity((e) => events.push(e));
  const done = mod.runMockActivity(category, fixture, () => Promise.resolve('ok'));
  await vi.advanceTimersByTimeAsync(10_000);
  await done;
  return events.find((e) => e.kind === 'started')?.target;
}

describe('the §3.10 target seams', () => {
  it('default push / commit carry the common-case fixtures', async () => {
    expect(await targetUnder('', 'push', 'origin/main')).toBe('origin/main');
    expect(await targetUnder('', 'commit', 'main')).toBe('main');
  });

  it('?fetchAll — fetch carries NO target, so the frontend derives `all remotes`', async () => {
    expect(await targetUnder('fetchAll', 'fetch', null)).toBeUndefined();
    // …which is also the default: the seam only gives the case a name.
    expect(await targetUnder('', 'fetch', null)).toBeUndefined();
  });

  it('?gitNoTarget — every category drops its target (the no-placeholder rule)', async () => {
    expect(await targetUnder('gitNoTarget', 'commit', 'main')).toBeUndefined();
    expect(await targetUnder('gitNoTarget', 'push', 'origin/main')).toBeUndefined();
  });

  it('?gitLongTarget — push gets the >=90-char ref, its leaf intact', async () => {
    const long = await targetUnder('gitLongTarget', 'push', 'origin/main');
    if (long === undefined) throw new Error('expected a long target');
    expect([...long].length).toBeGreaterThanOrEqual(90);
    expect(long.split('/').pop()).toBe('retry-budget-tuning');
    // Scoped to the push family — a commit keeps its own fixture.
    expect(await targetUnder('gitLongTarget', 'commit', 'main')).toBe('main');
  });

  it('?gitBidiTarget — the override is stripped, so the wire carries `origin/main`', async () => {
    const bidi = await targetUnder('gitBidiTarget', 'push', 'origin/main');
    expect(bidi).toBe('origin/main');
    // The §9.9 assertion, in code: nothing invisible survived the funnel.
    for (const ch of bidi ?? '') {
      const cp = ch.codePointAt(0) ?? 0;
      expect(cp >= 0x200b && cp <= 0x200f).toBe(false);
      expect(cp >= 0x202a && cp <= 0x202e).toBe(false);
      expect(cp >= 0x2066 && cp <= 0x2069).toBe(false);
      expect(cp).not.toBe(0xfeff);
    }
    // Scoped to the push family like ?gitLongTarget: a fetch has no single ref
    // in the real backend, so the seam must not conjure one (run-target F-3).
    expect(await targetUnder('gitBidiTarget', 'fetch', null)).toBeUndefined();
    expect(await targetUnder('gitBidiTarget', 'commit', 'main')).toBe('main');
  });

  it('?pushSlow — the target is emitted once and never revised mid-run', async () => {
    vi.resetModules();
    window.history.replaceState({}, '', '/?pushSlow');
    const mod = await import('./gitActivity');
    const events: GitActivityEvent[] = [];
    mod.subscribeGitActivity((e) => events.push(e));

    const done = mod.runMockActivity('push', 'origin/main', () => Promise.resolve('ok'));
    await vi.advanceTimersByTimeAsync(200);
    const early = events.filter((e) => e.target !== undefined);
    await vi.advanceTimersByTimeAsync(10_000);
    await done;
    const all = events.filter((e) => e.target !== undefined);

    // Exactly one event ever carried a target, and it was the `started`.
    expect(all).toHaveLength(1);
    expect(all[0]?.kind).toBe('started');
    expect(all[0]?.target).toBe('origin/main');
    // Sampled mid-Network-phase, it was already there and byte-identical.
    expect(early.map((e) => e.target)).toEqual(['origin/main']);
  });
});
