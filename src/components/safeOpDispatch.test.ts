import { describe, expect, it, vi, type Mock } from 'vitest';

import type { IpcApi, SafeOp, SafeOpKind } from '../ipc';
import { safeOpDispatch } from './safeOpDispatch';

// ---------------------------------------------------------------------------
// P55 safety layer L7 (contract §6 dispatch table). safeOpDispatch is the ONLY
// exec glue of the highest-trust feature: a fully-resolved, user-CONFIRMED
// SafeOp routes to exactly ONE existing typed IpcApi command — no git logic, no
// re-resolution, no shell string. These tests LOCK that table: for every kind
// (and every internal branch) they assert the RIGHT method fires with the RIGHT
// args and that NO OTHER ipc method is touched (exactly one call per dispatch).
// ---------------------------------------------------------------------------

/** The exact set of IpcApi methods dispatch is allowed to reach (§6). Nothing
 *  outside this set may ever be called; each is stubbed as a spy. */
const DISPATCH_METHODS = [
  'resetBranch',
  'revertCommit',
  'checkoutBranch',
  'checkoutRemoteBranch',
  'createBranch',
  'createBranchHere',
  'deleteBranch',
  'createStash',
  'discardPaths',
  'mergeBranch',
] as const;

type DispatchMethod = (typeof DISPATCH_METHODS)[number];
type IpcStub = Record<DispatchMethod, Mock>;

const REPO = 'repo-1';

/** A stub IpcApi exposing ONLY the ~10 methods dispatch can reach, each a spy
 *  resolving to `undefined` (dispatch discards every command's resolved value). */
function makeIpc(): IpcStub {
  const ipc = {} as IpcStub;
  for (const m of DISPATCH_METHODS) ipc[m] = vi.fn().mockResolvedValue(undefined);
  return ipc;
}

/** Dispatch `op` through a fresh stub and return that stub for assertions. */
async function dispatch(op: SafeOp): Promise<IpcStub> {
  const ipc = makeIpc();
  await safeOpDispatch(ipc as unknown as IpcApi, REPO, op);
  return ipc;
}

/** Assert EXACTLY ONE ipc method fired (the expected one) and NO other. This is
 *  the "one op per confirm, nothing else mutates" guarantee. */
function expectOnly(ipc: IpcStub, method: DispatchMethod): void {
  for (const m of DISPATCH_METHODS) {
    if (m === method) expect(ipc[m]).toHaveBeenCalledTimes(1);
    else expect(ipc[m], `${m} must NOT be called`).not.toHaveBeenCalled();
  }
}

describe('safeOpDispatch — §6 dispatch table (one confirmed op → one typed command)', () => {
  it('reset → resetBranch(repoId, targetOid, mode) — mode passed through (mixed)', async () => {
    const ipc = await dispatch({
      kind: 'reset',
      targetOid: 'a1b2c3d4e5f6',
      targetShort: 'a1b2c3d',
      mode: 'mixed',
    });
    expect(ipc.resetBranch).toHaveBeenCalledWith(REPO, 'a1b2c3d4e5f6', 'mixed');
    expectOnly(ipc, 'resetBranch');
  });

  it('reset → resetBranch with a destructive (hard) mode is forwarded verbatim', async () => {
    const ipc = await dispatch({
      kind: 'reset',
      targetOid: 'deadbeefcafe',
      targetShort: 'deadbee',
      mode: 'hard',
    });
    expect(ipc.resetBranch).toHaveBeenCalledWith(REPO, 'deadbeefcafe', 'hard');
    expectOnly(ipc, 'resetBranch');
  });

  it('revert → revertCommit(repoId, oid)', async () => {
    const ipc = await dispatch({ kind: 'revert', oid: 'c0ffee1234', short: 'c0ffee1' });
    expect(ipc.revertCommit).toHaveBeenCalledWith(REPO, 'c0ffee1234');
    expectOnly(ipc, 'revertCommit');
  });

  it('switchBranch remote:false → checkoutBranch(repoId, name)', async () => {
    const ipc = await dispatch({ kind: 'switchBranch', name: 'feature/x', remote: false });
    expect(ipc.checkoutBranch).toHaveBeenCalledWith(REPO, 'feature/x');
    expectOnly(ipc, 'checkoutBranch');
  });

  it('switchBranch remote:true → checkoutRemoteBranch(repoId, name)', async () => {
    const ipc = await dispatch({ kind: 'switchBranch', name: 'origin/feature/x', remote: true });
    expect(ipc.checkoutRemoteBranch).toHaveBeenCalledWith(REPO, 'origin/feature/x');
    expectOnly(ipc, 'checkoutRemoteBranch');
  });

  it('createBranch atOid:null → createBranch(repoId, name)', async () => {
    const ipc = await dispatch({ kind: 'createBranch', name: 'feat/new', atOid: null });
    expect(ipc.createBranch).toHaveBeenCalledWith(REPO, 'feat/new');
    expectOnly(ipc, 'createBranch');
  });

  it('createBranch atOid set → createBranchHere(repoId, name, atOid)', async () => {
    const ipc = await dispatch({ kind: 'createBranch', name: 'feat/new', atOid: 'abc1234def' });
    expect(ipc.createBranchHere).toHaveBeenCalledWith(REPO, 'feat/new', 'abc1234def');
    expectOnly(ipc, 'createBranchHere');
  });

  it('deleteBranch → deleteBranch(repoId, name)', async () => {
    const ipc = await dispatch({ kind: 'deleteBranch', name: 'old/merged' });
    expect(ipc.deleteBranch).toHaveBeenCalledWith(REPO, 'old/merged');
    expectOnly(ipc, 'deleteBranch');
  });

  it("stash includeUntracked:true → createStash(repoId, message, 'allWithUntracked')", async () => {
    const ipc = await dispatch({ kind: 'stash', message: 'wip: save', includeUntracked: true });
    expect(ipc.createStash).toHaveBeenCalledWith(REPO, 'wip: save', 'allWithUntracked');
    expectOnly(ipc, 'createStash');
  });

  it("stash includeUntracked:false → createStash(repoId, message, 'all') (null message forwarded)", async () => {
    const ipc = await dispatch({ kind: 'stash', message: null, includeUntracked: false });
    expect(ipc.createStash).toHaveBeenCalledWith(REPO, null, 'all');
    expectOnly(ipc, 'createStash');
  });

  it('discard → discardPaths(repoId, paths)', async () => {
    const paths = ['src/a.ts', 'src/b.ts'];
    const ipc = await dispatch({ kind: 'discard', paths });
    expect(ipc.discardPaths).toHaveBeenCalledWith(REPO, paths);
    expectOnly(ipc, 'discardPaths');
  });

  it('merge → mergeBranch(repoId, name)', async () => {
    const ipc = await dispatch({ kind: 'merge', name: 'feature/x' });
    expect(ipc.mergeBranch).toHaveBeenCalledWith(REPO, 'feature/x');
    expectOnly(ipc, 'mergeBranch');
  });
});

describe('safeOpDispatch — exhaustiveness (no SafeOpKind falls through silently)', () => {
  // Compile-time exhaustiveness: one representative op per SafeOpKind. Adding a
  // new kind to the union without a case here is a TS error — and the runtime
  // loop below then proves that kind still routes to exactly one command
  // (a fall-through switch would fire ZERO methods and fail expectSingleCall).
  const REPRESENTATIVE: Record<SafeOpKind, SafeOp> = {
    reset: { kind: 'reset', targetOid: 'a1b2c3d', targetShort: 'a1b2c3d', mode: 'mixed' },
    revert: { kind: 'revert', oid: 'c0ffee1', short: 'c0ffee1' },
    switchBranch: { kind: 'switchBranch', name: 'main', remote: false },
    createBranch: { kind: 'createBranch', name: 'feat/x', atOid: null },
    deleteBranch: { kind: 'deleteBranch', name: 'old' },
    stash: { kind: 'stash', message: null, includeUntracked: false },
    discard: { kind: 'discard', paths: ['a.ts'] },
    merge: { kind: 'merge', name: 'feature/x' },
  };

  it('every SafeOpKind dispatches to exactly one ipc command (never zero, never many)', async () => {
    for (const kind of Object.keys(REPRESENTATIVE) as SafeOpKind[]) {
      const ipc = await dispatch(REPRESENTATIVE[kind]);
      const fired = DISPATCH_METHODS.filter((m) => ipc[m].mock.calls.length > 0);
      expect(fired, `kind '${kind}' must route to exactly one command`).toHaveLength(1);
      expect(ipc[fired[0]]).toHaveBeenCalledTimes(1);
    }
  });
});
