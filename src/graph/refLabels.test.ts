import { describe, expect, it } from 'vitest';

import { chipTextFor, formatAheadBehind } from './refLabels';
import type { RefEntity } from './refLabels';
import type { GraphDisplayOptions } from './rightColumns';

// Pure P51c ahead/behind chip helpers — no canvas. `layoutRefLabels` itself
// needs a 2D context (measurement) and is exercised by the in-browser
// p7SelfTest; here we cover the formatting + gate logic that decides IF/WHAT a
// chip shows, which is where the branch cases live.

describe('formatAheadBehind', () => {
  it('returns null when not diverged (both counts 0 or negative)', () => {
    expect(formatAheadBehind(0, 0)).toBeNull();
    expect(formatAheadBehind(-1, -2)).toBeNull();
  });
  it('shows only the ahead arrow when behind is 0', () => {
    expect(formatAheadBehind(3, 0)).toBe('↑3');
  });
  it('shows only the behind arrow when ahead is 0', () => {
    expect(formatAheadBehind(0, 2)).toBe('↓2');
  });
  it('shows both arrows when diverged in both directions', () => {
    expect(formatAheadBehind(3, 2)).toBe('↑3 ↓2');
  });
});

function disp(over: Partial<GraphDisplayOptions> = {}): GraphDisplayOptions {
  return {
    showSha: true,
    showAuthor: false,
    showDate: true,
    dateBasis: 'author',
    showAheadBehind: true,
    branchStats: new Map(),
    ...over,
  };
}

function localBranch(name: string, hasLocal = true): RefEntity {
  return { kind: 'branch', name, hasLocal, remotes: [], isHead: false, refs: [] };
}

describe('chipTextFor', () => {
  it('is null when the toggle is off, even for a diverged branch', () => {
    const branchStats = new Map([['main', { ahead: 3, behind: 2 }]]);
    expect(chipTextFor(localBranch('main'), disp({ showAheadBehind: false, branchStats }))).toBeNull();
  });

  it('is null for a non-branch entity (tag/head/stash)', () => {
    const tag: RefEntity = { kind: 'tag', name: 'v1.0', ref: { name: 'v1.0', kind: 'tag', isHead: false } };
    expect(chipTextFor(tag, disp())).toBeNull();
  });

  it('is null for a remote-only branch (no local ref)', () => {
    const branchStats = new Map([['feat', { ahead: 1, behind: 1 }]]);
    expect(chipTextFor(localBranch('feat', false), disp({ branchStats }))).toBeNull();
  });

  it('is null for a local branch missing from branchStats (no upstream)', () => {
    expect(chipTextFor(localBranch('main'), disp())).toBeNull();
  });

  it('is null when the branch is tracked but not diverged (0/0)', () => {
    const branchStats = new Map([['main', { ahead: 0, behind: 0 }]]);
    expect(chipTextFor(localBranch('main'), disp({ branchStats }))).toBeNull();
  });

  it('is null when counts are null (defensive; map should pre-filter these)', () => {
    const branchStats = new Map([['main', { ahead: null, behind: null }]]);
    expect(chipTextFor(localBranch('main'), disp({ branchStats }))).toBeNull();
  });

  it('renders the compact chip for a diverged local branch', () => {
    const branchStats = new Map([['main', { ahead: 3, behind: 2 }]]);
    expect(chipTextFor(localBranch('main'), disp({ branchStats }))).toBe('↑3 ↓2');
  });

  it('renders one arrow when diverged in a single direction', () => {
    expect(
      chipTextFor(localBranch('a'), disp({ branchStats: new Map([['a', { ahead: 5, behind: 0 }]]) })),
    ).toBe('↑5');
    expect(
      chipTextFor(localBranch('b'), disp({ branchStats: new Map([['b', { ahead: 0, behind: 4 }]]) })),
    ).toBe('↓4');
  });
});
