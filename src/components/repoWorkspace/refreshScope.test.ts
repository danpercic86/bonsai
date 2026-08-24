// P86a (workstream B3) — the scope→slice matrix + pending-scope union.
import { describe, expect, it } from 'vitest';
import { type RefreshScope, slicesForScope, unionScopes } from './refreshScope';

describe('slicesForScope', () => {
  it('full runs every slice (forced tag-sync allowed)', () => {
    const s = slicesForScope('full');
    expect(s).toEqual({
      openRepo: true,
      status: true,
      graph: true,
      branches: true,
      stashes: true,
      submodules: true,
      worktrees: true,
      remotes: true,
      opState: true,
      compare: true,
      tagSync: true,
      tagSyncForcable: true,
    });
  });

  it('refsOnly = graph + branches + compare, and crucially SKIPS status + openRepo', () => {
    const s = slicesForScope('refsOnly');
    expect(s.graph).toBe(true);
    expect(s.branches).toBe(true);
    expect(s.compare).toBe(true);
    // The whole point of B3: a ref-only mutation never pays the worktree scan or reopen.
    expect(s.status).toBe(false);
    expect(s.openRepo).toBe(false);
    expect(s.remotes).toBe(false);
    expect(s.tagSync).toBe(false);
  });

  it('remoteMeta adds remotes + a NON-forced tag-sync on top of refsOnly, still no status/openRepo', () => {
    const s = slicesForScope('remoteMeta');
    expect(s.graph).toBe(true);
    expect(s.branches).toBe(true);
    expect(s.remotes).toBe(true);
    expect(s.compare).toBe(true);
    expect(s.tagSync).toBe(true);
    expect(s.tagSyncForcable).toBe(false); // never forces an ls-remote
    expect(s.status).toBe(false);
    expect(s.openRepo).toBe(false);
  });

  it('worktree = status + opState only (no graph walk, no branch refetch)', () => {
    const s = slicesForScope('worktree');
    expect(s.status).toBe(true);
    expect(s.opState).toBe(true);
    expect(s.graph).toBe(false);
    expect(s.branches).toBe(false);
    expect(s.openRepo).toBe(false);
  });

  it('stash = status + graph + stashes only (P88a; no branches/remotes/openRepo)', () => {
    const s = slicesForScope('stash');
    expect(s.status).toBe(true);
    expect(s.graph).toBe(true);
    expect(s.stashes).toBe(true);
    // Everything else is false — a stash op moves no HEAD and touches no remote/ref list.
    expect(s.openRepo).toBe(false);
    expect(s.branches).toBe(false);
    expect(s.remotes).toBe(false);
    expect(s.submodules).toBe(false);
    expect(s.worktrees).toBe(false);
    expect(s.opState).toBe(false);
    expect(s.compare).toBe(false);
    expect(s.tagSync).toBe(false);
    expect(s.tagSyncForcable).toBe(false);
  });

  it('only full reopens the repo (self-heal / header HEAD)', () => {
    for (const scope of ['refsOnly', 'remoteMeta', 'worktree', 'stash'] as const) {
      expect(slicesForScope(scope).openRepo).toBe(false);
    }
    expect(slicesForScope('full').openRepo).toBe(true);
  });
});

describe('unionScopes', () => {
  it('a single scope passes through', () => {
    for (const scope of ['full', 'refsOnly', 'remoteMeta', 'worktree', 'stash'] as const) {
      expect(unionScopes([scope])).toBe(scope);
    }
  });

  it('full dominates any set', () => {
    expect(unionScopes(['refsOnly', 'full'])).toBe('full');
    expect(unionScopes(['full', 'worktree', 'remoteMeta'])).toBe('full');
  });

  it('two distinct non-full scopes widen to full (conservative superset)', () => {
    expect(unionScopes(['refsOnly', 'worktree'])).toBe('full');
    expect(unionScopes(['remoteMeta', 'refsOnly'])).toBe('full');
  });

  it('repeated identical scopes collapse to that scope', () => {
    expect(unionScopes(['refsOnly', 'refsOnly', 'refsOnly'])).toBe('refsOnly');
  });

  it('an empty set defaults to full', () => {
    expect(unionScopes(new Set<RefreshScope>())).toBe('full');
  });
});
