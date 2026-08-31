import type { BranchesSnapshot } from '../types';
import {
  MULTI_REF_LOCALS,
  MULTI_REF_REMOTES,
  MULTI_REF_TAGS,
} from './multiRefRow';

/** Canonical HEAD oid of the mock fixture repo (shared with mock.ts). */
export const MOCK_OID = '9fceb02d0ae598e95dc970b74767f19372d61af8';

/**
 * M5 contract §5: initial branch snapshot for the stateful mock.
 * - `fix/watcher-debounce` is the designated dirty-checkout branch: per P33
 *   auto-stash, checking it out over a dirty tree carries the work across and
 *   yields a conflicted re-apply (apply: Conflicts, stash retained) — it no
 *   longer throws checkoutConflict.
 * - `experiment-unmerged` is the designated unmerged branch
 *   (deleteBranch throws unmergedBranch on it).
 */
export const INITIAL_BRANCHES: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0, tip: MOCK_OID },
    {
      name: 'feature/sidebar',
      isHead: false,
      upstream: 'origin/feature/sidebar',
      ahead: 2,
      behind: 1,
      tip: 'a'.repeat(40),
    },
    {
      name: 'fix/watcher-debounce',
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: 'b'.repeat(40),
    },
    {
      name: 'experiment-unmerged',
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: 'c'.repeat(40),
    },
    // P25 §6.3: stale-cleanup demo locals. `feature/merged-a` and
    // `feature/merged-b` are fully merged into `main` (classified `merged`);
    // `feature/gone` has a configured upstream whose remote-tracking ref was
    // deleted (classified `goneUpstream`). `experiment-unmerged` above is NEITHER
    // → excluded from the stale report. Consumed by STALE_SEED in mock.ts.
    {
      name: 'feature/merged-a',
      isHead: false,
      upstream: 'origin/feature/merged-a',
      ahead: 0,
      behind: 3,
      tip: '7'.repeat(40),
    },
    {
      name: 'feature/merged-b',
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: '8'.repeat(40),
    },
    {
      name: 'feature/gone',
      isHead: false,
      upstream: 'origin/feature/gone',
      ahead: 4,
      behind: 1,
      tip: '9'.repeat(40),
    },
    // §1.4: graph-fixture pill names (feat / exp / gh-pages / dev) get local
    // entries so their right-click menus resolve (branchMenuItems returns [] for
    // absent names). Appended in graph-pill order (matches existing fixture style).
    // P7 §9: `feat` is DIVERGED from its remote (local row 1 vs origin/feat row 4)
    // → nonzero ahead/behind so the two render as separate laptop/cloud labels.
    { name: 'feat', isHead: false, upstream: 'origin/feat', ahead: 1, behind: 1, tip: 'd'.repeat(40) },
    { name: 'exp', isHead: false, upstream: null, ahead: null, behind: null, tip: 'e'.repeat(40) },
    // P7 §9: `dev` collapses with `origin/dev` on graph row 0 (laptop+cloud label).
    { name: 'dev', isHead: false, upstream: 'origin/dev', ahead: 0, behind: 0, tip: '2'.repeat(40) },
    {
      name: 'gh-pages',
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: 'f'.repeat(40),
    },
    // P92 §5: the 12-hidden fixture row's locals — present so every picker row
    // resolves a real menu instead of rendering disabled.
    ...MULTI_REF_LOCALS,
  ],
  remote: [
    { name: 'origin/main', tip: MOCK_OID },
    { name: 'origin/feature/sidebar', tip: 'a'.repeat(40) },
    // §1.4: a remote with NO matching local so the harness exercises the
    // create-and-switch checkout path. (P7 §9: renders as remote-only `release`.)
    { name: 'origin/release', tip: '1'.repeat(40) },
    // P7 §9: remotes for the graph-row collapse (origin/dev) and the diverged
    // pair (origin/feat) so their LEFT-column right-click menus resolve.
    { name: 'origin/dev', tip: '4'.repeat(40) },
    { name: 'origin/feat', tip: '3'.repeat(40) },
    ...MULTI_REF_REMOTES,
  ],
  // P7 §9: v0.9 / v1.0 are referenced on graph row 0; add them here too.
  // P50d: ≥6 tags so the Tags section clears the inline-filter row threshold
  // (FILTER_MIN_ROWS) in the harness; shared prefixes exercise live filtering.
  tags: ['v0.1.0', 'v0.2.0', 'v0.3.0', ...MULTI_REF_TAGS, 'v0.9', 'v1.0', 'v1.1.0', 'v2.0.0-rc.1'],
  head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
};
