import { describe, expect, it } from 'vitest';

import { buildPathTree, flattenTreeLeaves } from '../../utils/pathTree';
import { filterByName, filterItems, filterTree } from './listFilter';

describe('filterByName (flat names)', () => {
  it('matches a case-insensitive substring', () => {
    expect(filterByName(['v1.0', 'v0.9', 'main'], 'V1')).toEqual(['v1.0']);
  });

  it('blank / whitespace query is the identity (same array ref)', () => {
    const names = ['a', 'b'];
    expect(filterByName(names, '')).toBe(names);
    expect(filterByName(names, '   ')).toBe(names);
  });

  it('no match yields an empty list', () => {
    expect(filterByName(['alpha', 'beta'], 'zzz')).toEqual([]);
  });
});

describe('filterItems (flat object rows)', () => {
  const rows = [{ name: 'feature/sidebar' }, { name: 'main' }, { name: 'fix/watcher' }];

  it('filters objects by their full display name', () => {
    expect(filterItems(rows, 'MAIN', (r) => r.name)).toEqual([{ name: 'main' }]);
  });

  it('matches a mid-path segment, keeping the full-name leaves', () => {
    expect(filterItems(rows, 'feature', (r) => r.name)).toEqual([{ name: 'feature/sidebar' }]);
  });

  it('no match yields an empty list', () => {
    expect(filterItems(rows, 'nope', (r) => r.name)).toEqual([]);
  });
});

describe('filterTree (tree mode ancestor-keep)', () => {
  const tree = buildPathTree(
    ['feature/sidebar', 'feature/graph', 'fix/watcher', 'main'],
    (s) => s,
  );

  it('keeps ancestor dirs so a matching leaf stays reachable', () => {
    const filtered = filterTree(tree, 'sidebar', (s) => s);
    // Only the "feature" dir survives, holding just the matching leaf.
    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.kind).toBe('dir');
    expect(flattenTreeLeaves(filtered)).toEqual(['feature/sidebar']);
  });

  it('matching an ancestor segment keeps every descendant leaf', () => {
    const filtered = filterTree(tree, 'feature', (s) => s);
    expect(flattenTreeLeaves(filtered).sort()).toEqual(['feature/graph', 'feature/sidebar']);
  });

  it('no match yields an empty tree', () => {
    expect(filterTree(tree, 'zzz', (s) => s)).toEqual([]);
  });

  it('blank query returns the tree unchanged', () => {
    expect(filterTree(tree, '  ', (s) => s)).toEqual(tree);
  });
});
