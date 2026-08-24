// @vitest-environment jsdom
/** Contract §3/§10 (checkout-commit-*.md): the mock `checkoutCommit` handler
 *  flips the repo to a DETACHED HEAD at the target oid (so the graph HEAD pill
 *  moves on the next refreshAll), honours the already-detached-at-oid no-op
 *  guard, and — under the `?checkout=detachconflict` URL seam — returns a
 *  conflicted re-apply outcome with the stash RETAINED. */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { mockIpc } from '../../mock';
import { MOCK_REPO_PATH, repos } from '../repoState';

const TARGET_OID = '9f1c2d3e4b5a60718293a4b5c6d7e8f901234567';

async function openDefault(): Promise<string> {
  const { repoId } = await mockIpc.openRepo(MOCK_REPO_PATH);
  return repoId;
}

function setSeam(value: string | null): void {
  const search = value === null ? '' : `?checkout=${value}`;
  window.history.replaceState({}, '', `/${search}`);
}

describe('mock checkoutCommit → detached HEAD', () => {
  beforeEach(() => {
    repos.clear();
    setSeam(null);
  });
  afterEach(() => setSeam(null));

  it('flips the repo to detached at the oid; listBranches reports it, isHead cleared', async () => {
    const repoId = await openDefault();
    const before = await mockIpc.listBranches(repoId);
    expect(before.head.detached).toBe(false);
    expect(before.local.some((b) => b.isHead)).toBe(true);

    await mockIpc.checkoutCommit(repoId, TARGET_OID);

    const after = await mockIpc.listBranches(repoId);
    expect(after.head.detached).toBe(true);
    expect(after.head.oid).toBe(TARGET_OID);
    expect(after.head.branchName).toBeNull();
    // No local branch is HEAD any longer.
    expect(after.local.some((b) => b.isHead)).toBe(false);
  });

  it('dirty default tree → stashed & re-applied (apply.kind=applied)', async () => {
    const repoId = await openDefault();
    const res = await mockIpc.checkoutCommit(repoId, TARGET_OID);
    // The default fixture repo is dirty → auto-stash + clean re-apply.
    expect(res.stashed).toBe(true);
    expect(res.fastForwarded).toBe(false);
    expect(res.apply).toEqual({ kind: 'applied' });
  });

  it('no-op guard: already detached at the exact oid → clean result, still detached', async () => {
    const repoId = await openDefault();
    await mockIpc.checkoutCommit(repoId, TARGET_OID);
    const res = await mockIpc.checkoutCommit(repoId, TARGET_OID);
    expect(res).toEqual({ stashed: false, fastForwarded: false, apply: null });
    const after = await mockIpc.listBranches(repoId);
    expect(after.head.detached).toBe(true);
    expect(after.head.oid).toBe(TARGET_OID);
  });

  it('?checkout=detachconflict seam → conflicted re-apply, stash retained (conflict entry added)', async () => {
    setSeam('detachconflict');
    const repoId = await openDefault();
    const res = await mockIpc.checkoutCommit(repoId, TARGET_OID);
    expect(res.stashed).toBe(true);
    expect(res.apply).toEqual({ kind: 'conflicts', paths: ['src/app.ts'] });
    // Stash retained ⇒ the worktree carries a synthetic conflict entry.
    const status = await mockIpc.getStatus(repoId);
    expect(status.conflicted.some((f) => f.path === 'src/app.ts')).toBe(true);
    // Still detached at the target.
    const after = await mockIpc.listBranches(repoId);
    expect(after.head.detached).toBe(true);
    expect(after.head.oid).toBe(TARGET_OID);
  });
});
