import { describe, expect, it } from 'vitest';

import { branchTipShas, collectCiShas, isFresh, rebuildCiCache } from './useForgeSignals';
import type { CiBadge } from '../../graph/forgeBadges';
import type { CheckRollup, CommitStatus, GraphLayout, GraphNode, RefLabel } from '../../ipc';

function node(id: string, refs?: RefLabel[]): GraphNode {
  return { id, lane: 0, parents: [], refs, summary: '', author: '', ts: 0, committerTs: 0 };
}
function layout(nodes: GraphNode[]): GraphLayout {
  return { nodes, edges: [], laneCount: 1, headIndex: null, truncated: false };
}
const local = (name: string): RefLabel => ({ name, kind: 'localBranch', isHead: false });
const remote = (name: string): RefLabel => ({ name, kind: 'remoteBranch', isHead: false });
const tag = (name: string): RefLabel => ({ name, kind: 'tag', isHead: false });
const stash = (name: string): RefLabel => ({ name, kind: 'stash', isHead: false });

describe('branchTipShas', () => {
  it('returns only the ids of nodes carrying a local or remote branch ref', () => {
    const l = layout([
      node('s-local', [local('feat')]),
      node('s-remote', [remote('origin/x')]),
      node('s-tag', [tag('v1')]),
      node('s-bare'),
      node('s-stash', [stash('stash@{0}')]),
    ]);
    expect(branchTipShas(l)).toEqual(['s-local', 's-remote']);
  });

  it('counts a local+remote branch node once, and preserves node order', () => {
    const l = layout([
      node('s0', [local('main'), remote('origin/main')]),
      node('s1', [local('dev')]),
    ]);
    expect(branchTipShas(l)).toEqual(['s0', 's1']);
  });

  it('is empty for a layout with no branch tips', () => {
    expect(branchTipShas(layout([node('a', [tag('v1')]), node('b')]))).toEqual([]);
  });
});

describe('isFresh', () => {
  const ttl = 60_000;
  it('is fresh just under the TTL and stale at/after it', () => {
    expect(isFresh(1000, 1000 + 59_999, ttl)).toBe(true);
    expect(isFresh(1000, 1000 + 60_000, ttl)).toBe(false);
    expect(isFresh(1000, 1000 + 60_001, ttl)).toBe(false);
  });
});

describe('collectCiShas', () => {
  const now = 100_000;
  const ttl = 60_000;
  const badge: CiBadge = { rollup: 'success', passed: 0, failed: 0, pending: 0, total: 0 };
  const cached = new Map([
    ['a', { badge, tsMs: now - 1_000 }], // fresh
    ['b', { badge, tsMs: now - 70_000 }], // stale
  ]);

  it('skips cached-fresh shas, keeps stale + uncached, and dedups tips ∪ PR heads', () => {
    // a=fresh→skip, b=stale→keep, c=uncached (in both lists→once), d=uncached.
    expect(collectCiShas(['a', 'b', 'c'], ['c', 'd'], cached, now, ttl, false, 10)).toEqual([
      'b',
      'c',
      'd',
    ]);
  });

  it('force=true includes even cached-fresh shas (still deduped)', () => {
    expect(collectCiShas(['a', 'b', 'c'], ['c', 'd'], cached, now, ttl, true, 10)).toEqual([
      'a',
      'b',
      'c',
      'd',
    ]);
  });

  it('caps the result at `max`', () => {
    expect(collectCiShas(['a', 'b', 'c', 'd'], [], new Map(), now, ttl, true, 2)).toEqual([
      'a',
      'b',
    ]);
  });

  it('returns nothing when everything is cached-fresh and not forced', () => {
    const allFresh = new Map([
      ['a', { badge, tsMs: now }],
      ['b', { badge, tsMs: now }],
    ]);
    expect(collectCiShas(['a', 'b'], [], allFresh, now, ttl, false, 10)).toEqual([]);
  });
});

describe('rebuildCiCache (replace-not-merge — bounds the cache)', () => {
  const ci = (rollup: CheckRollup): CiBadge => ({
    rollup,
    passed: 0,
    failed: 0,
    pending: 0,
    total: 0,
  });
  const status = (sha: string, state: CheckRollup): CommitStatus => ({
    sha,
    state,
    total: 1,
    passed: state === 'success' ? 1 : 0,
    failed: state === 'failure' ? 1 : 0,
    pending: 0,
    contexts: [],
  });

  it('carries over cached-fresh in-set shas NOT refetched, and adds fetched ones', () => {
    const prev = new Map([['keep', { badge: ci('success'), tsMs: 5 }]]);
    const out = rebuildCiCache(
      prev,
      new Set(['keep', 'new']),
      new Set(['new']),
      [status('new', 'pending')],
      99,
    );
    expect(out.get('keep')?.tsMs).toBe(5); // untouched fresh entry
    expect(out.get('new')?.tsMs).toBe(99); // freshly fetched
    expect(out.get('new')?.badge.rollup).toBe('pending');
    expect(out.size).toBe(2);
  });

  it('drops shas no longer in the current set (branch deleted / PR closed)', () => {
    const prev = new Map([
      ['gone', { badge: ci('success'), tsMs: 5 }],
      ['stay', { badge: ci('success'), tsMs: 5 }],
    ]);
    const out = rebuildCiCache(prev, new Set(['stay']), new Set(), [], 99);
    expect(out.has('gone')).toBe(false);
    expect(out.has('stay')).toBe(true);
  });

  it('drops a requested-but-OMITTED (404) sha even if in-set and previously cached', () => {
    const prev = new Map([['t', { badge: ci('failure'), tsMs: 5 }]]);
    // t was requested this cycle but the batch omitted it (force-pushed/gone tip).
    const out = rebuildCiCache(prev, new Set(['t']), new Set(['t']), [], 99);
    expect(out.has('t')).toBe(false);
  });

  it('overwrites an old value + timestamp with the freshly fetched status', () => {
    const prev = new Map([['t', { badge: ci('pending'), tsMs: 5 }]]);
    const out = rebuildCiCache(prev, new Set(['t']), new Set(['t']), [status('t', 'failure')], 99);
    expect(out.get('t')?.badge.rollup).toBe('failure');
    expect(out.get('t')?.tsMs).toBe(99);
  });
});
