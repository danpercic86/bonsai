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
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0 },
    {
      name: 'feature/sidebar',
      isHead: false,
      upstream: 'origin/feature/sidebar',
      ahead: 2,
      behind: 1,
    },
    { name: 'fix/watcher-debounce', isHead: false, upstream: null, ahead: null, behind: null },
    { name: 'experiment-unmerged', isHead: false, upstream: null, ahead: null, behind: null },
  ],
  remote: [{ name: 'origin/main' }, { name: 'origin/feature/sidebar' }],
  tags: ['v0.1.0', 'v0.2.0'],
  head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
};
