import type { ReflogEntry } from '../types';

/** Deterministic 40-hex oid per graph row, matching `fixtures/graph.ts`'s
 *  `oid(row)` so reflog `newOid`s land on REAL graph nodes (reveal / "Create
 *  branch here" resolve to an existing commit). */
function oid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}

const ZERO = '0'.repeat(40);
const NOW = Math.floor(Date.now() / 1000);
const HOUR = 3600;

// Named tips reused across the seeded HEAD story. c3b is the pre-rebase /
// pre-amend intermediate (also a real graph node, so reveal works).
const c1 = oid(5);
const c2 = oid(4);
const c3 = oid(3);
const c3b = oid(6);
const c4 = oid(2);

function entry(
  index: number,
  oldOid: string,
  newOid: string,
  message: string,
): ReflogEntry {
  return {
    index,
    oldOid,
    newOid,
    committerName: index % 2 === 0 ? 'Ada Lovelace' : 'Grace Hopper',
    committerEmail: index % 2 === 0 ? 'ada@example.com' : 'grace@example.com',
    // Strictly decreasing with index -> newest-first ordering.
    committerTs: NOW - index * HOUR,
    message,
  };
}

/** A believable HEAD reflog, newest-first, covering the P37/P23/P20 recovery
 *  story (force-push safety net → reset, amend, rebase, commit, pull, initial). */
export const MOCK_HEAD_REFLOG: ReflogEntry[] = [
  entry(0, c4, c3, 'reset: moving to HEAD~1'),
  entry(1, c3b, c4, 'commit (amend): tidy commit message'),
  entry(2, c3, c3b, 'rebase (finish): returning to refs/heads/main'),
  entry(3, c2, c3, 'commit: add feature'),
  entry(4, c1, c2, 'pull: Fast-forward'),
  entry(5, ZERO, c1, 'commit (initial): base'),
];

/** Per-branch reflogs keyed by local branch name; an absent key → `[]`
 *  (never-updated ref), matching the backend contract. */
export const MOCK_BRANCH_REFLOGS: Record<string, ReflogEntry[]> = {
  main: [
    entry(0, c3b, c4, 'commit (amend): tidy commit message'),
    entry(1, c2, c3b, 'commit: add feature'),
    entry(2, c1, c2, 'pull: Fast-forward'),
    entry(3, ZERO, c1, 'branch: Created from HEAD'),
  ],
};
