// Spec-003 size-split: the shared branch/remote-tracking ref menu, extracted
// VERBATIM from workspaceMenus.ts (which crossed the ~500-line limit when the
// solo/hide group landed). Same builder, same order, same wiring — used
// identically by the graph pills AND the sidebar rows (P6 §4.1).
import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  BranchIcon,
  CheckoutIcon,
  CopyIcon,
  DeleteIcon,
  HistoryIcon,
  MergeIcon,
  RebaseIcon,
  RebaseInteractiveIcon,
  SummarizeIcon,
} from './menuIcons';
import { errorMessage } from '../utils/errors';
import type { BranchInfo } from '../ipc';
import type { WorkspaceMenuDeps } from './workspaceMenus';
import { commitActionItems, resetMenuItems } from './workspaceMenusCommit';

/** Resolves tip + isHead from the current `branches` snapshot by name so the two
 *  surfaces can never diverge. Returns [] (menu does not open) when: no
 *  snapshot; the entry is missing; or the entry is the current local HEAD branch. */
export function branchMenuItems(
  deps: WorkspaceMenuDeps,
  name: string,
  kind: 'localBranch' | 'remoteBranch',
): ContextMenuItem[] {
  const {
    branches,
    headBranch,
    mutating,
    opActive,
    aiEligible,
    pushToast,
    handleCheckoutRemote,
    handleCheckoutBranch,
    handleCheckoutCommit,
    runSummarize,
    runAnalyze,
    handleMergeBranch,
    setPendingRebase,
    openRebasePlan,
    setPendingDeleteRemote,
    setPendingDeleteBranch,
    setPendingRenameBranch,
    onViewReflog,
  } = deps;
  const snapshot = branches;
  if (snapshot === null) return [];
  const cur = headBranch?.name ?? null;
  const gate = mutating || opActive;
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
      // Grouped: parent default = this branch's checkout; flyout adds the
      // detached-at-tip option (UI contract §2 pill form).
      label: 'Checkout',
      icon: createElement(CheckoutIcon),
      disabled: gate,
      onSelect: () =>
        void (kind === 'remoteBranch' ? handleCheckoutRemote(name) : handleCheckoutBranch(name)),
      children: [
        {
          label: `Checkout ${name}`,
          icon: createElement(CheckoutIcon),
          disabled: gate,
          onSelect: () =>
            void (kind === 'remoteBranch'
              ? handleCheckoutRemote(name)
              : handleCheckoutBranch(name)),
        },
        {
          label: 'Checkout commit (detached)',
          icon: createElement(CheckoutIcon),
          disabled: gate,
          onSelect: () => void handleCheckoutCommit(tip),
        },
      ],
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
  // P60a: rename this local branch (git branch -m) — opens the shared
  // PromptDialog prefilled with the current name (reuses the create-branch
  // idiom). Local branches only; gated like the other mutations. (The current
  // HEAD branch returns [] above, so its own pill shows the commit fallback.)
  if (kind === 'localBranch') {
    items.push({
      label: 'Rename…',
      icon: createElement(BranchIcon),
      disabled: gate,
      onSelect: () => setPendingRenameBranch({ name }),
    });
  }
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
      onSelect: () => runAnalyze({ kind: 'branch', name }, 'review', `Review branch ${name}`),
    });
  }
  if (cur !== null) {
    items.push({
      label: `Merge ${name} into ${cur}`,
      icon: createElement(MergeIcon),
      disabled: gate,
      onSelect: () => void handleMergeBranch(name),
    });
    // Grouped rebase: click the parent = standard rebase (default); the flyout
    // exposes Standard / Interactive… (P23b §8.2 interactive rebase onto tip).
    items.push({
      label: `Rebase ${cur} onto ${name}`,
      icon: createElement(RebaseIcon),
      disabled: gate,
      onSelect: () => setPendingRebase({ name, cur }),
      children: [
        {
          label: 'Standard',
          icon: createElement(RebaseIcon),
          disabled: gate,
          onSelect: () => setPendingRebase({ name, cur }),
        },
        {
          label: 'Interactive…',
          icon: createElement(RebaseInteractiveIcon),
          disabled: gate,
          onSelect: () => void openRebasePlan({ ontoOid: tip, ontoLabel: name }),
        },
      ],
    });
  }
  // P47 (Part A): the shared oid-based commit actions (Create branch/tag here,
  // Compare with HEAD, Cherry-pick, Revert) for this branch's tip. Spread here
  // — after Merge/Rebase, before Delete — so they form one contiguous group and
  // the branch menu owns them instead of the old inline Create-branch/Compare
  // duplicates (removed above). The current-HEAD-branch pill returns [] earlier,
  // so self-cherry-pick stays excluded.
  items.push(...commitActionItems(deps, tip));
  // Spec-003 §3.1: the solo/hide group — its own separated block, below all
  // existing items and above the destructive Delete.
  items.push(
    ...deps.refFilterItems(
      kind === 'localBranch' ? `refs/heads/${name}` : `refs/remotes/${name}`,
      'branch',
    ),
  );
  items.push({
    label: 'Delete',
    icon: createElement(DeleteIcon),
    disabled: gate,
    tone: 'danger',
    onSelect: () =>
      kind === 'remoteBranch' ? setPendingDeleteRemote(name) : setPendingDeleteBranch(name),
  });
  // P20 §3.3: reset the CURRENT branch to this ref's tip (gated internally).
  items.push(...resetMenuItems(deps, tip));
  return items;
}
