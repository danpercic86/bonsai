import { describe, expect, it } from 'vitest';

import type { ComposeGroup } from '../../ipc';
import {
  planIsApplicable,
  reduceAddGroup,
  reduceDropGroup,
  reduceEditMessage,
  reduceMergeInto,
  reduceMoveFile,
  type PlanState,
} from './useCommitComposer';

// ---------------------------------------------------------------------------
// Partition-invariant harness. v1 is file-level: after ANY reducer every changed
// file must appear EXACTLY once across the groups and the unassigned bucket — no
// duplicate, no loss, nothing invented. The reducers back the composer review UI;
// this asserts they can never corrupt that partition however the user edits.
// ---------------------------------------------------------------------------

/** Every file across the groups + the unassigned bucket, in encounter order. */
function collectFiles(groups: ComposeGroup[], unassigned: string[]): string[] {
  return [...groups.flatMap((g) => g.files), ...unassigned];
}

/** Assert the PARTITION INVARIANT against a fixed universe of changed files. */
function expectPartition(groups: ComposeGroup[], unassigned: string[], universe: string[]): void {
  const all = collectFiles(groups, unassigned);
  // No duplicate: the deduped set has the same size as the flat list.
  expect(new Set(all).size).toBe(all.length);
  // Exact coverage: same multiset as the universe (no loss, nothing invented).
  expect([...all].sort()).toEqual([...universe].sort());
}

const UNIVERSE = ['README.md', 'src/a.ts', 'src/b.ts', 'src/c.ts'];

/** Two groups + one unassigned file — a valid partition of UNIVERSE. */
function seed(): PlanState {
  return {
    groups: [
      { files: ['src/a.ts', 'src/b.ts'], message: 'feat: core' },
      { files: ['src/c.ts'], message: 'test: c' },
    ],
    unassigned: ['README.md'],
  };
}

describe('seed fixture is a valid partition', () => {
  it('every universe file appears exactly once', () => {
    const s = seed();
    expectPartition(s.groups, s.unassigned, UNIVERSE);
  });
});

describe('reduceMoveFile — partition preserved', () => {
  it('moves a file between groups (removed from source, once at destination)', () => {
    const next = reduceMoveFile(seed(), 'src/a.ts', 1);
    expect(next.groups[0].files).toEqual(['src/b.ts']);
    expect(next.groups[1].files).toEqual(['src/c.ts', 'src/a.ts']);
    expectPartition(next.groups, next.unassigned, UNIVERSE);
  });

  it('moves a file from a group to the unassigned bucket', () => {
    const next = reduceMoveFile(seed(), 'src/a.ts', 'unassigned');
    expect(next.groups[0].files).toEqual(['src/b.ts']);
    expect(next.unassigned).toContain('src/a.ts');
    expectPartition(next.groups, next.unassigned, UNIVERSE);
  });

  it('moves a file from unassigned into a group', () => {
    const next = reduceMoveFile(seed(), 'README.md', 0);
    expect(next.unassigned).toEqual([]);
    expect(next.groups[0].files).toContain('README.md');
    expectPartition(next.groups, next.unassigned, UNIVERSE);
  });

  it('moving a file to the group it already occupies keeps it exactly once', () => {
    const next = reduceMoveFile(seed(), 'src/a.ts', 0);
    expect(next.groups[0].files.filter((f) => f === 'src/a.ts')).toEqual(['src/a.ts']);
    expectPartition(next.groups, next.unassigned, UNIVERSE);
  });

  it('moving an unassigned file to unassigned keeps it exactly once', () => {
    const next = reduceMoveFile(seed(), 'README.md', 'unassigned');
    expect(next.unassigned.filter((f) => f === 'README.md')).toEqual(['README.md']);
    expectPartition(next.groups, next.unassigned, UNIVERSE);
  });

  it('does not mutate its input state', () => {
    const s = seed();
    const before = JSON.stringify(s);
    reduceMoveFile(s, 'src/a.ts', 1);
    expect(JSON.stringify(s)).toBe(before);
  });
});

describe('reduceDropGroup — files fall back to unassigned', () => {
  it('drops a group and returns its files to the unassigned bucket', () => {
    const next = reduceDropGroup(seed(), 0);
    expect(next.groups).toHaveLength(1);
    expect(next.groups[0].files).toEqual(['src/c.ts']);
    expect(next.unassigned).toEqual(
      expect.arrayContaining(['README.md', 'src/a.ts', 'src/b.ts']),
    );
    expectPartition(next.groups, next.unassigned, UNIVERSE);
  });

  it('out-of-range index is a no-op (same reference)', () => {
    const s = seed();
    expect(reduceDropGroup(s, 5)).toBe(s);
    expect(reduceDropGroup(s, -1)).toBe(s);
  });
});

describe('reduceMergeInto — concatenates without dup, keeps the lower index', () => {
  it('merges into the lower index; files concatenate lo→hi; message joins with a blank line', () => {
    const s = seed();
    const groups = reduceMergeInto(s.groups, 1, 0); // merge group 1 into group 0
    expect(groups).toHaveLength(1);
    expect(groups[0].files).toEqual(['src/a.ts', 'src/b.ts', 'src/c.ts']);
    expect(groups[0].message).toBe('feat: core\n\ntest: c');
    expectPartition(groups, s.unassigned, UNIVERSE);
  });

  it('produces no duplicate files regardless of merge direction', () => {
    const s = seed();
    for (const groups of [reduceMergeInto(s.groups, 0, 1), reduceMergeInto(s.groups, 1, 0)]) {
      const all = groups.flatMap((g) => g.files);
      expect(new Set(all).size).toBe(all.length);
      expectPartition(groups, s.unassigned, UNIVERSE);
    }
  });

  it('drops an empty message when joining (keeps only the non-empty side)', () => {
    const groups: ComposeGroup[] = [
      { files: ['a'], message: '' },
      { files: ['b'], message: 'fix: b' },
    ];
    expect(reduceMergeInto(groups, 0, 1)[0].message).toBe('fix: b');
  });

  it('two empty messages merge to an empty message', () => {
    const groups: ComposeGroup[] = [
      { files: ['a'], message: '  ' },
      { files: ['b'], message: '' },
    ];
    expect(reduceMergeInto(groups, 0, 1)[0].message).toBe('');
  });

  it('gi === targetGi and out-of-range are no-ops (same reference)', () => {
    const s = seed();
    expect(reduceMergeInto(s.groups, 1, 1)).toBe(s.groups);
    expect(reduceMergeInto(s.groups, 0, 9)).toBe(s.groups);
    expect(reduceMergeInto(s.groups, -1, 0)).toBe(s.groups);
  });
});

describe('reduceEditMessage — message only, files untouched', () => {
  it('replaces the target group message and leaves the partition intact', () => {
    const s = seed();
    const groups = reduceEditMessage(s.groups, 0, 'feat: renamed');
    expect(groups[0].message).toBe('feat: renamed');
    expect(groups[0].files).toEqual(['src/a.ts', 'src/b.ts']);
    expectPartition(groups, s.unassigned, UNIVERSE);
  });

  it('out-of-range index changes nothing', () => {
    const s = seed();
    expect(reduceEditMessage(s.groups, 9, 'ignored')).toEqual(s.groups);
  });
});

describe('reduceAddGroup — appends an empty group', () => {
  it('adds an empty group without disturbing the partition', () => {
    const s = seed();
    const groups = reduceAddGroup(s.groups);
    expect(groups).toHaveLength(3);
    expect(groups[2]).toEqual({ files: [], message: '' });
    expectPartition(groups, s.unassigned, UNIVERSE);
  });
});

describe('planIsApplicable', () => {
  it('true when every group has a non-empty message and >=1 file', () => {
    expect(
      planIsApplicable([
        { files: ['a'], message: 'feat: a' },
        { files: ['b', 'c'], message: 'fix: b' },
      ]),
    ).toBe(true);
  });

  it('false for zero groups', () => {
    expect(planIsApplicable([])).toBe(false);
  });

  it('false when any group message is empty or whitespace-only', () => {
    expect(planIsApplicable([{ files: ['a'], message: '' }])).toBe(false);
    expect(planIsApplicable([{ files: ['a'], message: '   ' }])).toBe(false);
    expect(planIsApplicable([{ files: ['a'], message: '\n\t ' }])).toBe(false);
    // One bad group among good ones still fails the whole plan.
    expect(
      planIsApplicable([
        { files: ['a'], message: 'ok' },
        { files: ['b'], message: '   ' },
      ]),
    ).toBe(false);
  });

  it('false when any group has zero files', () => {
    expect(planIsApplicable([{ files: [], message: 'feat: x' }])).toBe(false);
    expect(
      planIsApplicable([
        { files: ['a'], message: 'ok' },
        { files: [], message: 'empty files' },
      ]),
    ).toBe(false);
  });
});

describe('partition invariant across a sequence of edits', () => {
  it('holds after move → drop → add → move → merge', () => {
    let state = seed();
    expectPartition(state.groups, state.unassigned, UNIVERSE);

    state = reduceMoveFile(state, 'README.md', 0); // unassigned -> group 0
    expectPartition(state.groups, state.unassigned, UNIVERSE);

    state = reduceDropGroup(state, 1); // group 1's files -> unassigned
    expectPartition(state.groups, state.unassigned, UNIVERSE);

    state = { ...state, groups: reduceAddGroup(state.groups) }; // empty group appended
    expectPartition(state.groups, state.unassigned, UNIVERSE);

    state = reduceMoveFile(state, 'src/c.ts', state.groups.length - 1); // unassigned -> new group
    expectPartition(state.groups, state.unassigned, UNIVERSE);

    state = { ...state, groups: reduceMergeInto(state.groups, state.groups.length - 1, 0) };
    expectPartition(state.groups, state.unassigned, UNIVERSE);
  });
});
