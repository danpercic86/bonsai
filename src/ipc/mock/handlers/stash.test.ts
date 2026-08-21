// @vitest-environment jsdom
/** F-A6-B — the mock stash handlers HONOR the wrong-target guard: when the
 *  caller passes an `expectedOid` that no longer matches the entry at that stack
 *  index, they reject with the SAME message the Rust core returns, BEFORE
 *  mutating the stack. Undefined `expectedOid` skips the check (legacy callers). */
import { beforeEach, describe, expect, it } from 'vitest';

import { mockIpc } from '../../mock';
import { MOCK_REPO_PATH, repos } from '../repoState';

const GUARD_MSG = 'stash list changed; refresh and retry';

async function openDefault(): Promise<string> {
  const { repoId } = await mockIpc.openRepo(MOCK_REPO_PATH);
  return repoId;
}

describe('mock stash wrong-target guard (F-A6-B)', () => {
  beforeEach(() => repos.clear());

  it('the default repo seeds a stash stack with oids', async () => {
    const repoId = await openDefault();
    const stashes = await mockIpc.listStashes(repoId);
    expect(stashes.length).toBeGreaterThan(0);
    expect(stashes[0].oid).toBeTruthy();
  });

  it('applyStash: matching oid applies; wrong oid rejects with the guard message', async () => {
    const repoId = await openDefault();
    const [top] = await mockIpc.listStashes(repoId);
    // Matching oid → behaves as before (no throw).
    await expect(mockIpc.applyStash(repoId, top.index, false, top.oid)).resolves.toBeTruthy();
    // Wrong oid → guard rejection before touching anything.
    await expect(mockIpc.applyStash(repoId, top.index, false, 'deadbeef')).rejects.toThrow(GUARD_MSG);
  });

  it('popStash: wrong oid rejects and leaves the stack unchanged', async () => {
    const repoId = await openDefault();
    const before = await mockIpc.listStashes(repoId);
    await expect(mockIpc.popStash(repoId, before[0].index, false, 'deadbeef')).rejects.toThrow(
      GUARD_MSG,
    );
    const after = await mockIpc.listStashes(repoId);
    expect(after.length).toBe(before.length);
  });

  it('dropStash: wrong oid rejects and drops nothing; matching oid drops', async () => {
    const repoId = await openDefault();
    const before = await mockIpc.listStashes(repoId);
    await expect(mockIpc.dropStash(repoId, before[0].index, 'deadbeef')).rejects.toThrow(GUARD_MSG);
    expect((await mockIpc.listStashes(repoId)).length).toBe(before.length);

    await expect(mockIpc.dropStash(repoId, before[0].index, before[0].oid)).resolves.toBeUndefined();
    expect((await mockIpc.listStashes(repoId)).length).toBe(before.length - 1);
  });

  it('undefined expectedOid skips the guard (legacy callers)', async () => {
    const repoId = await openDefault();
    const [top] = await mockIpc.listStashes(repoId);
    await expect(mockIpc.applyStash(repoId, top.index, false)).resolves.toBeTruthy();
  });
});
