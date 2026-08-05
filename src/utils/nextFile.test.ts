import { describe, expect, it } from 'vitest';

import { nextFileAfter, type WorkdirChange } from './nextFile';

const ch = (path: string, section: WorkdirChange['section'] = 'unstaged'): WorkdirChange => ({
  section,
  path,
  origPath: null,
});

describe('nextFileAfter', () => {
  it('middle → next surviving entry', () => {
    const changes = [ch('a'), ch('b'), ch('c'), ch('d')];
    // staging the open file `b`; forward scan lands on `c`.
    expect(nextFileAfter(changes, 'b', ['b'])).toBe(changes[2]);
  });

  it('last entry staged → null (no wrap)', () => {
    const changes = [ch('a'), ch('b'), ch('c')];
    expect(nextFileAfter(changes, 'c', ['c'])).toBeNull();
  });

  it('stage-all (every path staged) → null', () => {
    const changes = [ch('a'), ch('b'), ch('c')];
    expect(nextFileAfter(changes, 'a', ['a', 'b', 'c'])).toBeNull();
  });

  it('openPath not found in changes → null', () => {
    const changes = [ch('a'), ch('b'), ch('c')];
    expect(nextFileAfter(changes, 'zzz', ['a'])).toBeNull();
  });

  it('next entry itself staged (multi-select) → skips to the following surviving one', () => {
    const changes = [ch('a'), ch('b'), ch('c'), ch('d')];
    // open `a`, staged {a,b}; b is skipped, c is the next survivor.
    expect(nextFileAfter(changes, 'a', ['a', 'b'])).toBe(changes[2]);
  });

  it('single-item list, that item staged → null', () => {
    const changes = [ch('only')];
    expect(nextFileAfter(changes, 'only', ['only'])).toBeNull();
  });
});
