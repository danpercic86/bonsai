import type { IpcApi, SafeOp } from '../ipc';

/**
 * P55c: dispatch a fully-resolved, user-CONFIRMED `SafeOp` to the EXISTING typed
 * command for its kind (contract §6, safety layer L7). This is PURE routing:
 *
 *  - no git logic runs here — Rust already resolved every ref/oid in the
 *    read-only plan step, and the user has confirmed the preview;
 *  - no re-resolution and no shell string exists anywhere in the pipeline;
 *  - the resolved value of each command is intentionally DISCARDED — the caller
 *    refreshes the workspace afterward from the returned promise.
 *
 * Some targets (`revert`, `switchBranch`, `merge`) may pause into the EXISTING
 * conflict / autostash flow surfaced by the op-state UI — we just `await` them;
 * the NL entry point deliberately adds no conflict UI of its own.
 */
export async function safeOpDispatch(ipc: IpcApi, repoId: string, op: SafeOp): Promise<void> {
  switch (op.kind) {
    case 'reset':
      await ipc.resetBranch(repoId, op.targetOid, op.mode);
      return;
    case 'revert':
      await ipc.revertCommit(repoId, op.oid);
      return;
    case 'switchBranch':
      if (op.remote) await ipc.checkoutRemoteBranch(repoId, op.name);
      else await ipc.checkoutBranch(repoId, op.name);
      return;
    case 'createBranch':
      if (op.atOid === null) await ipc.createBranch(repoId, op.name);
      else await ipc.createBranchHere(repoId, op.name, op.atOid);
      return;
    case 'deleteBranch':
      await ipc.deleteBranch(repoId, op.name);
      return;
    case 'stash':
      // The existing command takes a StashScope, not a boolean — map here
      // (contract §6: "match existing arg shape"). No 'staged'-only NL variant.
      await ipc.createStash(repoId, op.message, op.includeUntracked ? 'allWithUntracked' : 'all');
      return;
    case 'discard':
      await ipc.discardPaths(repoId, op.paths);
      return;
    case 'merge':
      await ipc.mergeBranch(repoId, op.name);
      return;
  }
}
