/** P92 §5 — harness fixtures for the multi-ref commit UI.
 *
 *  Own module (not appended to `graph.ts` / `branches.ts`) because it is a
 *  static data table: `MULTI_REF_ROW_REFS` decorates ONE graph row with 14 refs
 *  so the "+N" overflow chip has 12 hidden entities to picker over, and
 *  `MULTI_REF_*` supply the matching `branches` snapshot entries WITHOUT which
 *  `branchMenuItems` returns `[]` and every picker row would render disabled
 *  (see the note at `branches.ts` §1.4).
 *
 *  The other two cases the contract asks for already exist in `graph.ts`:
 *  row 0 carries 4 collapsing refs (`main`+`origin/main` → ONE row) = the small
 *  overflow case, and row 1 (`feat`) is the single-ref control that must keep
 *  today's flat menu.
 */

import type { BranchInfo, RefLabel, RemoteBranchInfo } from '../types';

/** §5.3: proves ellipsis + `title` in the pill AND in the picker row. */
export const LONG_BRANCH = 'feature/very-long-topic-branch-name-that-definitely-overflows-the-pill';

/** Local branch names on the 14-ref fixture row, in wire order (locals first). */
const LOCALS = [
  // SECOND, deliberately: one 180px-wide pill on its own consumes the whole ref
  // band, so putting it first would hide EVERY other ref and make the chip read
  // `+14` instead of the `+12` §5 specifies. With a short pill first, the band
  // shows a mix and the long one still appears (in the picker), exercising both
  // the shown and the hidden case.
  'release/2026-01',
  LONG_BRANCH,
  'release/2026-02',
  'release/2026-03',
  'topic/alpha',
  'topic/beta',
  'topic/gamma',
  'topic/delta',
  'topic/epsilon',
  'topic/zeta',
  'topic/eta',
  'topic/theta',
];

/** Deterministic 40-hex tip per fixture branch (distinct from graph.ts oids). */
function tipFor(i: number): string {
  return `c0ffee${i.toString(16).padStart(2, '0')}`.padEnd(40, '0');
}

/** The row's wire refs: 12 locals + 1 remote-only + 1 tag = 14 entities. The
 *  band shows the first one or two, so the chip reads "+12". */
export const MULTI_REF_ROW_REFS: RefLabel[] = [
  ...LOCALS.map((name): RefLabel => ({ name, kind: 'localBranch', isHead: false })),
  { name: 'origin/topic/remote-only', kind: 'remoteBranch', isHead: false },
  { name: 'v0.8', kind: 'tag', isHead: false },
];

export const MULTI_REF_LOCALS: BranchInfo[] = LOCALS.map((name, i) => ({
  name,
  isHead: false,
  upstream: null,
  ahead: null,
  behind: null,
  tip: tipFor(i),
}));

export const MULTI_REF_REMOTES: RemoteBranchInfo[] = [
  { name: 'origin/topic/remote-only', tip: tipFor(LOCALS.length) },
];

export const MULTI_REF_TAGS = ['v0.8'];
