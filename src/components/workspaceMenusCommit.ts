import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  BisectIcon,
  BranchIcon,
  CheckoutIcon,
  CherryPickIcon,
  CompareIcon,
  RebaseInteractiveIcon,
  ResetIcon,
  RevertIcon,
  SummarizeIcon,
  TagIcon,
} from './menuIcons';
import { shortOid } from './workspaceUtils';
import type { WorkspaceMenuDeps } from './workspaceMenus';

// P5 §5.2 / P6 §4.2: the commit-row menu — "Create branch here" + "Compare
// with HEAD" (both read-only entry points; unavailable when HEAD is unborn,
// §1.3). Factored out (P18b) so the whole-row ref fallback can reuse it.
// P20 §3.3: the three "Reset <branch> to here" items, gated on an attached
// born HEAD, an idle repo, and a target that is not already the current tip.
// Hard is suffixed "…" (opens the extra-warning ConfirmDialog). Returns [] when
// reset is not offered (so callers can spread unconditionally).
export function resetMenuItems(deps: WorkspaceMenuDeps, targetOid: string): ContextMenuItem[] {
  const { head, headBranch, mutating, opActive, setPendingReset } = deps;
  if (head === null || head.unborn || head.detached) return [];
  if (targetOid === head.oid) return [];
  const gate = mutating || opActive;
  const b = headBranch?.name ?? 'HEAD';
  // Grouped reset: click the parent = mixed reset (default); the flyout exposes
  // Soft / Mixed / Hard…. Hard is suffixed "…" (extra-warning ConfirmDialog) and
  // flagged danger. Returned as a single-element array so callers keep spreading.
  return [
    {
      label: `Reset ${b} to here`,
      icon: createElement(ResetIcon),
      disabled: gate,
      onSelect: () => setPendingReset({ oid: targetOid, mode: 'mixed' }),
      children: [
        {
          label: 'Soft',
          icon: createElement(ResetIcon),
          disabled: gate,
          onSelect: () => setPendingReset({ oid: targetOid, mode: 'soft' }),
        },
        {
          label: 'Mixed',
          icon: createElement(ResetIcon),
          disabled: gate,
          onSelect: () => setPendingReset({ oid: targetOid, mode: 'mixed' }),
        },
        {
          label: 'Hard…',
          icon: createElement(ResetIcon),
          disabled: gate,
          tone: 'danger',
          onSelect: () => setPendingReset({ oid: targetOid, mode: 'hard' }),
        },
      ],
    },
  ];
}

// P47 (Part A): the shared oid-based commit-action set. Extracted from
// commitMenuItems so branch pills and graph tag pills reach the same actions
// as commit rows (no arbitrary split) — spread by callers exactly as
// resetMenuItems is. Order: Create branch here, Create tag here, Compare with
// HEAD, then (attached HEAD only) Cherry-pick, Revert — matching the commit-row
// order preserved below. Returns [] when there is no usable HEAD so callers can
// spread unconditionally.
export function commitActionItems(deps: WorkspaceMenuDeps, oid: string): ContextMenuItem[] {
  const {
    head,
    mutating,
    opActive,
    aiEligible,
    setPendingCreateBranch,
    setPendingCreateTag,
    handleCompareWithHead,
    runAnalyze,
    handleCherrypick,
    handleRevert,
  } = deps;
  if (head === null || head.unborn) return [];
  const gate = mutating || opActive;
  const items: ContextMenuItem[] = [
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
  ];
  // P53b §5.2: "Explain this commit" — reuses the existing ai_analyze_diff
  // explain path ({kind:'commit'}); the backend now grounds the explanation on
  // the full commit MESSAGE (D2), not just the diff. Read-only, so it sits in
  // this read-only group AFTER "Compare with HEAD" and BEFORE Cherry-pick/
  // Revert, and is offered on detached HEAD too. Disabled unless AI is eligible
  // (installed && enabled && consented). runAnalyze's req-id guards staleness.
  items.push({
    label: 'Explain this commit',
    icon: createElement(SummarizeIcon),
    disabled: !aiEligible,
    onSelect: () =>
      runAnalyze({ kind: 'commit', oid }, 'explain', `Explain commit ${oid.slice(0, 7)}`),
  });
  // P20 §5.2/§6: cherry-pick / revert onto the current branch. Gated on an
  // attached born HEAD (excluded on detached HEAD, which the backend rejects
  // — mirrors resetMenuItems) and an idle repo. On Conflicts the existing
  // OpBanner/conflict flow takes over.
  if (!head.detached) {
    items.push(
      {
        label: 'Cherry-pick onto current…',
        icon: createElement(CherryPickIcon),
        disabled: gate,
        onSelect: () => void handleCherrypick(oid),
      },
      {
        label: 'Revert commit',
        icon: createElement(RevertIcon),
        disabled: gate,
        onSelect: () => void handleRevert(oid),
      },
    );
  }
  return items;
}

// Checkout entries for a commit oid — the most-primary action, so prepended
// FIRST in commit/tag menus (UI contract §2). Keyed off the LOCAL branches
// tipping this oid, excluding the current HEAD branch:
//   0 → single top-level `Checkout commit (detached)`
//   1 → grouped: parent = that branch's checkout + flyout (branch, detached)
//  ≥2 → INERT parent `Checkout` (no onSelect → opens flyout only); flyout =
//       one `Checkout <name>` per branch (snapshot order) then detached LAST.
export function checkoutMenuItems(deps: WorkspaceMenuDeps, oid: string): ContextMenuItem[] {
  const { head, mutating, opActive, branches, handleCheckoutCommit, handleCheckoutBranch } = deps;
  if (head === null || head.unborn) return [];
  const gate = mutating || opActive;
  const localTips = branches?.local.filter((b) => b.tip === oid && !b.isHead) ?? [];

  const detached: ContextMenuItem = {
    label: 'Checkout commit (detached)',
    icon: createElement(CheckoutIcon),
    disabled: gate,
    onSelect: () => void handleCheckoutCommit(oid),
  };

  if (localTips.length === 0) {
    // Pure no-op (already detached at this oid, no other branch) → omit.
    if (head.detached && head.oid === oid) return [];
    return [detached];
  }

  const children: ContextMenuItem[] = localTips.map((b) => ({
    label: `Checkout ${b.name}`,
    icon: createElement(CheckoutIcon),
    disabled: gate,
    onSelect: () => void handleCheckoutBranch(b.name),
  }));
  children.push(detached);

  if (localTips.length === 1) {
    const only = localTips[0];
    return [
      {
        label: `Checkout ${only.name}`,
        icon: createElement(CheckoutIcon),
        disabled: gate,
        onSelect: () => void handleCheckoutBranch(only.name),
        children,
      },
    ];
  }

  // ≥2 branches → INERT parent: no onSelect (opens flyout only). Its
  // `disabled` is left unset so the flyout stays openable while a write is in
  // flight; the child actions carry `disabled: gate`.
  return [
    {
      label: 'Checkout',
      icon: createElement(CheckoutIcon),
      children,
    },
  ];
}

export function commitMenuItems(deps: WorkspaceMenuDeps, oid: string): ContextMenuItem[] {
  const {
    head,
    mutating,
    opActive,
    openRebasePlan,
    pendingBisectBad,
    bisectActive,
    handleMarkBisectBad,
    handleStartBisect,
  } = deps;
  if (head === null || head.unborn) return [];
  const gate = mutating || opActive;
  return [
    ...checkoutMenuItems(deps, oid),
    ...commitActionItems(deps, oid),
    // Interactive-rebase-from-here + bisect stay commit-row-only (not part of
    // the shared commit-action set). Gated on an attached born HEAD, matching
    // the cherry-pick/revert gate in commitActionItems.
    ...(head.detached
      ? []
      : [
          // P23b §8.2: interactive rebase replaying THIS commit..HEAD onto the
          // selected commit (it becomes the `onto` base). Gated like cherry-pick.
          {
            label: 'Interactive rebase from here…',
            icon: createElement(RebaseInteractiveIcon),
            disabled: gate,
            onSelect: () => void openRebasePlan({ ontoOid: oid, ontoLabel: shortOid(oid) }),
          },
          // P39b §8: two-click git-bisect start. First mark a BAD commit
          // (records the oid + hints), then on an OLDER commit "Mark GOOD &
          // start bisect" begins the search. Hidden while a bisect is already
          // running (the OpBanner drives it then). The good item is enabled
          // only once a bad is pending; picking a non-ancestor good surfaces
          // the backend error.
          ...(bisectActive
            ? []
            : [
                {
                  label: 'Start bisect: mark this BAD',
                  icon: createElement(BisectIcon),
                  disabled: gate,
                  onSelect: () => handleMarkBisectBad(oid),
                },
                {
                  label: 'Mark GOOD & start bisect',
                  icon: createElement(BisectIcon),
                  disabled: gate || pendingBisectBad === null || pendingBisectBad === oid,
                  onSelect: () => {
                    if (pendingBisectBad !== null) handleStartBisect(pendingBisectBad, oid);
                  },
                },
              ]),
        ]),
    ...resetMenuItems(deps, oid),
  ];
}
