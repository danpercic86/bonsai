import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  BranchIcon,
  CheckoutIcon,
  CompareIcon,
  CopyIcon,
  DeleteIcon,
  HistoryIcon,
  MergeIcon,
  RebaseIcon,
  StashApplyIcon,
  StashPopIcon,
  SummarizeIcon,
  TagIcon,
} from './menuIcons';
import { shortOid } from './workspaceUtils';
import { errorMessage } from '../utils/errors';
import type { PushToast } from '../ToastContext';
import type {
  AiAnalysisMode,
  AiDiffTarget,
  BranchInfo,
  BranchesSnapshot,
  HeadInfo,
  RemoteInfo,
  ResetMode,
  SubmoduleInfo,
  WorktreeInfo,
} from '../ipc';
import type { GraphContextTarget } from '../graph/GraphCanvas';

/** P3e §menu-extraction: the current state + handlers a menu build needs. Every
 *  field mirrors the value/callback the inline builders closed over before the
 *  extraction; the produced item arrays are byte-identical (same order/labels/
 *  wiring). RepoWorkspace rebuilds this on each render so values are current. */
export interface WorkspaceMenuDeps {
  branches: BranchesSnapshot | null;
  headBranch: BranchInfo | null;
  head: HeadInfo | null;
  mutating: boolean;
  opActive: boolean;
  aiEligible: boolean;
  remotes: RemoteInfo[];
  pushToast: PushToast;
  handleCheckoutRemote(name: string): void;
  handleCheckoutBranch(name: string): void;
  setPendingCreateBranch(v: { oid: string }): void;
  runSummarize(base: string, target: string): void;
  runAnalyze(target: AiDiffTarget, mode: AiAnalysisMode, title: string): void;
  handleMergeBranch(name: string): void;
  handleRebaseBranch(name: string): void;
  openRebasePlan(target: { ontoOid: string; ontoLabel: string }): void;
  handleCompareWithHead(oid: string): void;
  setPendingDeleteRemote(name: string): void;
  setPendingDeleteBranch(name: string): void;
  handleApplyStash(index: number): void;
  handlePopStash(index: number): void;
  setPendingDropStash(index: number): void;
  handleInitSubmodule(name: string): void;
  handleUpdateSubmodule(name: string): void;
  handleSyncSubmodule(name: string): void;
  onOpenRepoPath(path: string): void;
  setWorktreeContextOpen(v: boolean): void;
  setPendingWorktreeLock(name: string): void;
  handleUnlockWorktree(name: string): void;
  setPendingWorktreeRemove(v: { name: string; absPath: string }): void;
  setPendingDeleteTag(name: string): void;
  handlePushTag(remote: string, name: string): void;
  setPendingRenameRemote(v: { name: string }): void;
  setPendingEditUrl(v: { name: string; url: string }): void;
  setPendingRemoveRemote(name: string): void;
  setPendingCreateTag(v: { oid: string }): void;
  handleCherrypick(oid: string): void;
  handleRevert(oid: string): void;
  setPendingReset(v: { oid: string; mode: ResetMode }): void;
  onViewReflog(refName: string): void;
}

export interface WorkspaceMenus {
  branchMenuItems(name: string, kind: 'localBranch' | 'remoteBranch'): ContextMenuItem[];
  stashMenuItems(index: number): ContextMenuItem[];
  submoduleMenuItems(sub: SubmoduleInfo): ContextMenuItem[];
  worktreeMenuItems(wt: WorktreeInfo): ContextMenuItem[];
  tagMenuItems(name: string): ContextMenuItem[];
  remoteMenuItems(name: string): ContextMenuItem[];
  resetMenuItems(targetOid: string): ContextMenuItem[];
  commitMenuItems(oid: string): ContextMenuItem[];
  buildContextItems(target: GraphContextTarget): ContextMenuItem[];
}

/** Factory over the current deps returning every context-menu item-array
 *  builder. The builders reference each other via the closure exactly as the
 *  inline versions did (branch→reset, commit→reset, buildContextItems→
 *  stash/tag/branch/commit). Pure: constructs arrays, performs no side effects
 *  until an item's onSelect fires the passed-in handler. */
export function createWorkspaceMenus(deps: WorkspaceMenuDeps): WorkspaceMenus {
  const {
    branches,
    headBranch,
    head,
    mutating,
    opActive,
    aiEligible,
    remotes,
    pushToast,
    handleCheckoutRemote,
    handleCheckoutBranch,
    setPendingCreateBranch,
    runSummarize,
    runAnalyze,
    handleMergeBranch,
    handleRebaseBranch,
    openRebasePlan,
    handleCompareWithHead,
    setPendingDeleteRemote,
    setPendingDeleteBranch,
    handleApplyStash,
    handlePopStash,
    setPendingDropStash,
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
    onOpenRepoPath,
    setWorktreeContextOpen,
    setPendingWorktreeLock,
    handleUnlockWorktree,
    setPendingWorktreeRemove,
    setPendingDeleteTag,
    handlePushTag,
    setPendingRenameRemote,
    setPendingEditUrl,
    setPendingRemoveRemote,
    setPendingCreateTag,
    handleCherrypick,
    handleRevert,
    setPendingReset,
    onViewReflog,
  } = deps;

  // P6 §4.1: the single shared builder for a branch/remote-tracking ref menu,
  // used identically by the graph pills AND the sidebar rows. Resolves tip +
  // isHead from the current `branches` snapshot by name so the two surfaces can
  // never diverge. Returns [] (menu does not open) when: no snapshot; the entry
  // is missing; or the entry is the current local HEAD branch.
  function branchMenuItems(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
  ): ContextMenuItem[] {
    const snapshot = branches;
    if (snapshot === null) return [];
    const cur = headBranch?.name ?? null;
    const gate = mutating || opActive;
    const headUnborn = head === null || head.unborn;
    const entry =
      kind === 'localBranch'
        ? snapshot.local.find((b) => b.name === name)
        : snapshot.remote.find((r) => r.name === name);
    if (entry === undefined) return [];
    const isHead = kind === 'localBranch' ? (entry as BranchInfo).isHead : false;
    if (isHead) return [];
    const tip = entry.tip;
    const items: ContextMenuItem[] = [
      {
        label: 'Checkout',
        icon: createElement(CheckoutIcon),
        disabled: gate,
        onSelect: () =>
          void (kind === 'remoteBranch'
            ? handleCheckoutRemote(name)
            : handleCheckoutBranch(name)),
      },
      {
        label: 'Create branch here',
        icon: createElement(BranchIcon),
        disabled: gate,
        onSelect: () => setPendingCreateBranch({ oid: tip }),
      },
      {
        label: 'Copy branch name',
        icon: createElement(CopyIcon),
        disabled: false,
        onSelect: () => {
          const p =
            navigator.clipboard?.writeText(name) ??
            Promise.reject(new Error('Clipboard unavailable'));
          void p
            .then(() => pushToast('success', 'Copied branch name'))
            .catch((e) => pushToast('error', `Copy failed: ${errorMessage(e)}`));
        },
      },
    ];
    // P38 §7.3: view this branch's reflog (local branches only — remote-tracking
    // reflogs are out of v1 scope). Read-only, so never gated.
    if (kind === 'localBranch') {
      items.push({
        label: 'View reflog',
        icon: createElement(HistoryIcon),
        disabled: false,
        onSelect: () => onViewReflog(name),
      });
    }
    // P15c: "Summarize branch…" (local branches only, AI-eligible only). Base
    // selection is a frontend policy (§7.5): the repo's primary branch (main,
    // else master, else the current HEAD branch) UNLESS the target IS that
    // primary, in which case the base is the target's upstream. When no usable
    // base can be resolved (primary missing, or target == primary with no
    // upstream), the item is omitted.
    if (kind === 'localBranch' && aiEligible) {
      const localEntry = entry as BranchInfo;
      const primary = snapshot.local.some((b) => b.name === 'main')
        ? 'main'
        : snapshot.local.some((b) => b.name === 'master')
          ? 'master'
          : (headBranch?.name ?? null);
      const summaryBase = name === primary ? localEntry.upstream : primary;
      if (summaryBase !== null && summaryBase !== name) {
        items.push({
          label: 'Summarize branch…',
          icon: createElement(SummarizeIcon),
          disabled: false,
          onSelect: () => runSummarize(summaryBase, name),
        });
      }
      // P25b: "Review branch…" (local branches only, AI-eligible only). Reviews
      // the branch's diff vs its auto-resolved base (backend resolves
      // upstream→origin/HEAD→main→master), so no base is passed. Guarded by
      // runAnalyze's req-id, hence disabled:false.
      items.push({
        label: 'Review branch…',
        icon: createElement(SummarizeIcon),
        disabled: false,
        onSelect: () =>
          runAnalyze({ kind: 'branch', name }, 'review', `Review branch ${name}`),
      });
    }
    if (cur !== null) {
      items.push({
        label: `Merge ${name} into ${cur}`,
        icon: createElement(MergeIcon),
        disabled: gate,
        onSelect: () => void handleMergeBranch(name),
      });
      items.push({
        label: `Rebase ${cur} onto ${name}`,
        icon: createElement(RebaseIcon),
        disabled: gate,
        onSelect: () => void handleRebaseBranch(name),
      });
      // P23b §8.2: interactive rebase of the current branch onto this ref's tip.
      items.push({
        label: `Rebase ${cur} onto ${name} (interactive)…`,
        icon: createElement(RebaseIcon),
        disabled: gate,
        onSelect: () => void openRebasePlan({ ontoOid: tip, ontoLabel: name }),
      });
    }
    if (!headUnborn) {
      items.push({
        label: 'Compare with HEAD',
        icon: createElement(CompareIcon),
        disabled: false,
        onSelect: () => handleCompareWithHead(tip),
      });
    }
    items.push({
      label: 'Delete',
      icon: createElement(DeleteIcon),
      disabled: gate,
      onSelect: () =>
        kind === 'remoteBranch' ? setPendingDeleteRemote(name) : setPendingDeleteBranch(name),
    });
    // P20 §3.3: reset the CURRENT branch to this ref's tip (gated internally).
    items.push(...resetMenuItems(tip));
    return items;
  }

  // P9 §6.4: build the right-click menu for a stash row. Apply/Pop need a clean,
  // idle repo (gated on mutating || opActive); Drop is allowed mid-op (it only
  // edits the stash reflog) → routes through the ConfirmDialog.
  function stashMenuItems(index: number): ContextMenuItem[] {
    const gate = mutating || opActive;
    return [
      {
        label: 'Apply',
        icon: createElement(StashApplyIcon),
        disabled: gate,
        onSelect: () => void handleApplyStash(index),
      },
      {
        label: 'Pop',
        icon: createElement(StashPopIcon),
        disabled: gate,
        onSelect: () => void handlePopStash(index),
      },
      {
        label: 'Drop',
        icon: createElement(DeleteIcon),
        disabled: mutating,
        onSelect: () => setPendingDropStash(index),
      },
    ];
  }

  // P19 §6.4: submodule row menu. "Update" on an uninitialized row
  // init-then-updates (backend §OPEN-4), so it is always enabled; "Init" is a
  // no-op once initialized → disabled unless uninitialized. "Open in new tab"
  // needs a checked-out worktree → disabled while uninitialized.
  function submoduleMenuItems(sub: SubmoduleInfo): ContextMenuItem[] {
    const gate = mutating || opActive;
    return [
      {
        label: 'Init',
        icon: createElement(BranchIcon),
        disabled: gate || sub.status !== 'uninitialized',
        onSelect: () => void handleInitSubmodule(sub.name),
      },
      {
        label: 'Update',
        icon: createElement(StashApplyIcon),
        disabled: gate,
        onSelect: () => void handleUpdateSubmodule(sub.name),
      },
      {
        label: 'Sync',
        icon: createElement(RebaseIcon),
        disabled: gate,
        onSelect: () => void handleSyncSubmodule(sub.name),
      },
      {
        label: 'Open in new tab',
        icon: createElement(CompareIcon),
        disabled: sub.status === 'uninitialized',
        onSelect: () => onOpenRepoPath(sub.absPath),
      },
    ];
  }

  // P27 §6.4: worktree row menu. Open-in-tab needs an intact working tree;
  // Lock/Unlock apply to linked worktrees only; Remove is disabled for
  // main/current/locked in the UI AND refused server-side (§2.6).
  function worktreeMenuItems(wt: WorktreeInfo): ContextMenuItem[] {
    const gate = mutating || opActive;
    return [
      {
        label: 'Open in new tab',
        icon: createElement(CompareIcon),
        disabled: wt.isCurrent || !wt.valid,
        onSelect: () => onOpenRepoPath(wt.absPath),
      },
      {
        label: 'AI context…',
        // Read-only matrix — always openable; per-row activation is gated
        // inside the dialog (D6) and by the preview safety gate.
        disabled: false,
        onSelect: () => setWorktreeContextOpen(true),
      },
      {
        label: 'Lock…',
        disabled: gate || wt.isMain || wt.locked,
        onSelect: () => setPendingWorktreeLock(wt.name),
      },
      {
        label: 'Unlock',
        disabled: gate || !wt.locked,
        onSelect: () => void handleUnlockWorktree(wt.name),
      },
      {
        label: 'Remove…',
        icon: createElement(DeleteIcon),
        disabled: gate || wt.isMain || wt.isCurrent || wt.locked,
        onSelect: () => setPendingWorktreeRemove({ name: wt.name, absPath: wt.absPath }),
      },
    ];
  }

  // P22 §7.2: the shared tag menu — used by the graph tag pill AND the sidebar
  // tag rows. Delete (ConfirmDialog) + Copy + one "Push tag to <remote>" per
  // configured remote (§OPEN-7: 0 → no push item; 1 → single; >1 → one each).
  function tagMenuItems(name: string): ContextMenuItem[] {
    const gate = mutating || opActive;
    const items: ContextMenuItem[] = [
      {
        label: 'Delete tag',
        icon: createElement(DeleteIcon),
        disabled: gate,
        onSelect: () => setPendingDeleteTag(name),
      },
      {
        label: 'Copy tag name',
        icon: createElement(CopyIcon),
        disabled: false,
        onSelect: () => {
          const p =
            navigator.clipboard?.writeText(name) ??
            Promise.reject(new Error('Clipboard unavailable'));
          void p
            .then(() => pushToast('success', 'Copied tag name'))
            .catch((e) => pushToast('error', `Copy failed: ${errorMessage(e)}`));
        },
      },
    ];
    for (const r of remotes) {
      items.push({
        label: `Push tag to ${r.name}`,
        icon: createElement(TagIcon),
        disabled: gate,
        onSelect: () => void handlePushTag(r.name, name),
      });
    }
    return items;
  }

  // P22 §7.2: the configured-remote management menu (sidebar rows only).
  function remoteMenuItems(name: string): ContextMenuItem[] {
    const gate = mutating || opActive;
    const url = remotes.find((r) => r.name === name)?.url ?? '';
    return [
      {
        label: 'Rename…',
        icon: createElement(BranchIcon),
        disabled: gate,
        onSelect: () => setPendingRenameRemote({ name }),
      },
      {
        label: 'Edit URL…',
        icon: createElement(CompareIcon),
        disabled: gate,
        onSelect: () => setPendingEditUrl({ name, url }),
      },
      {
        label: 'Remove…',
        icon: createElement(DeleteIcon),
        disabled: gate,
        onSelect: () => setPendingRemoveRemote(name),
      },
    ];
  }

  // P5 §5.2 / P6 §4.2: the commit-row menu — "Create branch here" + "Compare
  // with HEAD" (both read-only entry points; unavailable when HEAD is unborn,
  // §1.3). Factored out (P18b) so the whole-row ref fallback can reuse it.
  // P20 §3.3: the three "Reset <branch> to here" items, gated on an attached
  // born HEAD, an idle repo, and a target that is not already the current tip.
  // Hard is suffixed "…" (opens the extra-warning ConfirmDialog). Returns [] when
  // reset is not offered (so callers can spread unconditionally).
  function resetMenuItems(targetOid: string): ContextMenuItem[] {
    if (head === null || head.unborn || head.detached) return [];
    if (targetOid === head.oid) return [];
    const gate = mutating || opActive;
    const b = headBranch?.name ?? 'HEAD';
    const make = (mode: ResetMode, label: string): ContextMenuItem => ({
      label,
      icon: createElement(RebaseIcon),
      disabled: gate,
      onSelect: () => setPendingReset({ oid: targetOid, mode }),
    });
    return [
      make('soft', `Reset ${b} to here (soft)`),
      make('mixed', `Reset ${b} to here (mixed)`),
      make('hard', `Reset ${b} to here (hard)…`),
    ];
  }

  function commitMenuItems(oid: string): ContextMenuItem[] {
    if (head === null || head.unborn) return [];
    const gate = mutating || opActive;
    return [
      {
        label: 'Create branch here',
        icon: createElement(BranchIcon),
        disabled: gate,
        onSelect: () => setPendingCreateBranch({ oid }),
      },
      {
        label: 'Create tag here',
        icon: createElement(TagIcon),
        disabled: gate,
        onSelect: () => setPendingCreateTag({ oid }),
      },
      {
        label: 'Compare with HEAD',
        icon: createElement(CompareIcon),
        disabled: false,
        onSelect: () => handleCompareWithHead(oid),
      },
      // P20 §5.2/§6: cherry-pick / revert onto the current branch. Gated on an
      // attached born HEAD (excluded on detached HEAD, which the backend rejects
      // — mirrors resetMenuItems) and an idle repo. On Conflicts the existing
      // OpBanner/conflict flow takes over.
      ...(head.detached
        ? []
        : [
            {
              label: 'Cherry-pick onto current',
              icon: createElement(RebaseIcon),
              disabled: gate,
              onSelect: () => void handleCherrypick(oid),
            },
            {
              label: 'Revert commit',
              icon: createElement(RebaseIcon),
              disabled: gate,
              onSelect: () => void handleRevert(oid),
            },
            // P23b §8.2: interactive rebase replaying THIS commit..HEAD onto the
            // selected commit (it becomes the `onto` base). Gated like cherry-pick.
            {
              label: 'Interactive rebase from here…',
              icon: createElement(RebaseIcon),
              disabled: gate,
              onSelect: () => void openRebasePlan({ ontoOid: oid, ontoLabel: shortOid(oid) }),
            },
          ]),
      ...resetMenuItems(oid),
    ];
  }

  // P5 §5.2 / P6 §4.2: build the right-click menu items for a graph target. Ref
  // pills delegate to the shared branchMenuItems builder; commit rows offer
  // "Compare with HEAD" (read-only; unavailable when HEAD is unborn).
  function buildContextItems(target: GraphContextTarget): ContextMenuItem[] {
    if (target.kind === 'ref') {
      const r = target.ref;
      // P10 §5: a stash pill → Apply/Pop/Drop menu (parse the index from the name).
      if (r.kind === 'stash') {
        const m = /^stash@\{(\d+)\}$/.exec(r.name);
        if (m === null) return []; // malformed name → no menu (defensive)
        return stashMenuItems(Number(m[1]));
      }
      if (r.kind === 'head') return [];
      // P22 §7.2: the graph tag pill opens the same menu as the sidebar tag row.
      if (r.kind === 'tag') return tagMenuItems(r.name);
      const kind = r.kind === 'remoteBranch' ? 'remoteBranch' : 'localBranch';
      const items = branchMenuItems(r.name, kind);
      if (items.length > 0) return items;
      // P18b: whole-row right-click resolved to a branch whose branch menu is
      // empty — the current HEAD branch. Fall back to the commit menu (resolving
      // the row's oid from the branch tip) so the row still opens a useful menu.
      const snapshot = branches;
      if (snapshot === null) return [];
      const entry =
        kind === 'localBranch'
          ? snapshot.local.find((b) => b.name === r.name)
          : snapshot.remote.find((b) => b.name === r.name);
      if (entry === undefined) return [];
      return commitMenuItems(entry.tip);
    }
    // Commit row → Create branch here + Compare with HEAD.
    return commitMenuItems(target.oid);
  }

  return {
    branchMenuItems,
    stashMenuItems,
    submoduleMenuItems,
    worktreeMenuItems,
    tagMenuItems,
    remoteMenuItems,
    resetMenuItems,
    commitMenuItems,
    buildContextItems,
  };
}
