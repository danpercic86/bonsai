import type { BranchesSnapshot } from '../types';

/** Canonical HEAD oid of the mock fixture repo (shared with mock.ts). */
export const MOCK_OID = '9fceb02d0ae598e95dc970b74767f19372d61af8';

/**
 * M5 contract §5: initial branch snapshot for the stateful mock.
 * - `fix/watcher-debounce` is the designated dirty-checkout branch
 *   (checkoutBranch throws checkoutConflict on it).
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
    // §1.4: graph-fixture pill names (feat / exp / gh-pages) get local entries so
    // their right-click menus resolve (branchMenuItems returns [] for absent names).
    { name: 'feat', isHead: false, upstream: null, ahead: null, behind: null, tip: 'd'.repeat(40) },
    { name: 'exp', isHead: false, upstream: null, ahead: null, behind: null, tip: 'e'.repeat(40) },
    {
      name: 'gh-pages',
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: 'f'.repeat(40),
    },
  ],
  remote: [
    { name: 'origin/main', tip: MOCK_OID },
    { name: 'origin/feature/sidebar', tip: 'a'.repeat(40) },
    // §1.4: a remote with NO matching local so the harness exercises the
    // create-and-switch checkout path.
    { name: 'origin/release', tip: '1'.repeat(40) },
  ],
  tags: ['v0.1.0', 'v0.2.0'],
  head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
};
