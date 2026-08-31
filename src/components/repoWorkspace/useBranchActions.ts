import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { describeCarry } from './carryOutcome';
import type { BranchesSnapshot } from '../../ipc';
import type { RefreshScope } from './refreshScope';
import type { BaseActionDeps, Setter } from './types';

/** Local/remote branch create, checkout, delete (P6/P11/P33). */
export function useBranchActions(
  deps: BaseActionDeps & {
    refreshAll: (scope?: RefreshScope) => Promise<void>;
    branches: BranchesSnapshot | null;
    setBranchesError: Setter<string | null>;
    setPendingCreateBranch: Setter<{ oid: string } | null>;
    setPendingRenameBranch: Setter<{ name: string } | null>;
  },
) {
  const {
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    branches,
    setBranchesError,
    setPendingCreateBranch,
    setPendingRenameBranch,
  } = deps;

  // P85 A1: every branch/ref mutation routes its post-op refresh through the
  // ECHO-ARMED `refreshAll` (the canonical coalesced round), NOT raw
  // refetchGraph/refetchBranches. Arming suppresses the handler's own
  // `.git/refs/**` watcher echo → exactly ONE refresh round per mutation, not
  // two. `refreshAll()` never throws (P81) and keeps each handler's own
  // try/catch/finally + toasts intact. P86a scopes each round to what its change
  // touched: a create/rename-at-existing-commit is `refsOnly` (no worktree scan,
  // no HEAD move); a delete or HEAD-moving op stays `full`.
  async function handleCreateBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.createBranch(repoId, name);
      // A create at an existing commit only adds a ref pill — refsOnly.
      await refreshAll('refsOnly');
    } finally {
      setMutating(false);
    }
  }

  // P33: dirty-safe switch — auto-stash → switch → auto fast-forward (no fetch)
  // → re-apply stash. Never hard-fails on a dirty tree; a conflicted re-apply is
  // a SUCCESS (stash retained at stash@{0}).
  async function handleCheckoutBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      const res = await ipc.checkoutBranch(repoId, name);
      await refreshAll();
      const notice = describeCarry(res.apply);
      if (notice) {
        pushToast(notice.level, `Switched to ${name}; ${notice.text}`);
      } else {
        let msg = `Switched to ${name}`;
        const extras: string[] = [];
        if (res.stashed) extras.push('stashed & re-applied');
        if (res.fastForwarded) {
          const upstreamLabel = branches?.local.find((b) => b.name === name)?.upstream ?? 'upstream';
          extras.push(`fast-forwarded to ${upstreamLabel}`);
        }
        if (extras.length > 0) msg += ` (${extras.join(', ')})`;
        pushToast('success', msg);
      }
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P11 §1.4: create a local branch at `oid` + check it out, carrying any
  // uncommitted work across via auto-stash. HEAD moves, so refreshAll.
  async function handleCreateBranchHere(oid: string, name: string): Promise<void> {
    setMutating(true);
    try {
      const res = await ipc.createBranchHere(repoId, name, oid);
      await refreshAll();
      const notice = describeCarry(res.apply);
      if (!res.stashed) {
        pushToast('success', `Created and checked out ${name}`);
      } else if (notice) {
        pushToast(notice.level, `Created ${name}; ${notice.text}`);
      } else {
        pushToast('success', `Created ${name} and carried your changes over`);
      }
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
      setPendingCreateBranch(null);
    }
  }

  async function handleDeleteBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.deleteBranch(repoId, name);
      // P88a row 13: a local delete moves no HEAD and touches no worktree, so match
      // handleDeleteRemoteTracking on `refsOnly` (skips status/remotes/stashes/etc).
      // The graph slice still re-walks: a tip removal is a B1 cache Miss (correct —
      // commits only reachable via the deleted branch drop out).
      await refreshAll('refsOnly');
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P60a: rename a local branch (git branch -m). Preserves upstream + reflog.
  // P85 A1: routes through the echo-armed refreshAll (whether or not HEAD moved)
  // so the ref-write watcher echo is dropped → one round. Errors toast (this is a
  // PromptDialog action, mirroring handleCreateBranchHere).
  async function handleRenameBranch(oldName: string, newName: string) {
    // P60a: renaming to the unchanged name is a no-op. The dialog intentionally
    // permits submitting the prefilled name, but the backend would reject the
    // self-collision (Exists) with a confusing "already exists" toast — so just
    // close the dialog and return without an ipc call.
    if (oldName.trim() === newName.trim()) {
      setPendingRenameBranch(null);
      return;
    }
    setMutating(true);
    try {
      const res = await ipc.renameBranch(repoId, oldName, newName);
      // A1: route through refreshAll whether or not HEAD moved. P86a: a rename of
      // HEAD moves the current-branch label (full); a non-head rename is refsOnly.
      await refreshAll(res.wasHead ? 'full' : 'refsOnly');
      pushToast(
        'success',
        `Renamed ${oldName} → ${newName}` +
          (res.upstream !== null ? ` (tracking ${res.upstream} preserved)` : ''),
      );
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
      setPendingRenameBranch(null);
    }
  }

  // Checkout arbitrary commit → detached HEAD. Non-destructive (no confirm),
  // dirty-safe (auto-stash/re-apply). HEAD moves → refreshAll full.
  async function handleCheckoutCommit(oid: string) {
    const short = oid.slice(0, 7);
    setMutating(true);
    try {
      const res = await ipc.checkoutCommit(repoId, oid);
      await refreshAll();
      const notice = describeCarry(res.apply);
      if (notice) {
        pushToast(notice.level, `Detached HEAD at ${short}; ${notice.text}`);
      } else if (res.stashed) {
        pushToast('success', `Detached HEAD at ${short} (stashed & re-applied)`);
      } else {
        pushToast('success', `Detached HEAD at ${short} — commit or create a branch to keep new work`);
      }
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P6 §4.4: GitKraken-style remote checkout — create/reuse a local tracking
  // branch and switch to it (HEAD moves, so refreshAll like handleCheckoutBranch).
  async function handleCheckoutRemote(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.checkoutRemoteBranch(repoId, name);
      await refreshAll();
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P6 §4.4: delete the LOCAL remote-tracking ref only (does not touch the
  // server). P86a: removing a remote-tracking ref only drops a pill (no local
  // HEAD move, no worktree change) — refsOnly.
  async function handleDeleteRemoteTracking(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.deleteRemoteBranch(repoId, name);
      await refreshAll('refsOnly');
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return {
    handleCreateBranch,
    handleCheckoutBranch,
    handleCheckoutCommit,
    handleCreateBranchHere,
    handleDeleteBranch,
    handleRenameBranch,
    handleCheckoutRemote,
    handleDeleteRemoteTracking,
  };
}
