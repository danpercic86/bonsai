import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  BisectIcon,
  BranchIcon,
  CheckoutIcon,
  CherryPickIcon,
  CompareIcon,
  CopyIcon,
  DeleteIcon,
  EditorIcon,
  FolderOpenIcon,
  HistoryIcon,
  MergeIcon,
  RebaseIcon,
  RebaseInteractiveIcon,
  ResetIcon,
  RevertIcon,
  SummarizeIcon,
  TagIcon,
  TerminalIcon,
} from './menuIcons';
import { shortOid } from './workspaceUtils';
import { errorMessage } from '../utils/errors';
import type { PushToast } from '../ToastContext';
import type {
  AiAnalysisMode,
  AiDiffTarget,
  BranchInfo,
  BranchesSnapshot,
  ChangelogRange,
  HeadInfo,
  RemoteInfo,
  ResetMode,
  SubmoduleInfo,
  TagSyncReport,
  WorktreeInfo,
} from '../ipc';
import type { GraphContextTarget } from '../graph/GraphCanvas';
import type {
  PendingDeleteRemoteTag,
  PendingForceMoveTag,
} from './dialogs/TagSyncDialogs';
import {
  remoteMenuItems as remoteMenuItemsImpl,
  stashMenuItems as stashMenuItemsImpl,
  submoduleMenuItems as submoduleMenuItemsImpl,
  worktreeMenuItems as worktreeMenuItemsImpl,
} from './workspaceMenusRows';

/** P49: the three external-launch handlers a filesystem path is opened with.
 *  Each takes the target path so one handler set drives every entry point
 *  (row menus, tab menu, toolbar). Launches never touch git state, so items
 *  built from these are NEVER gated by `mutating`/`opActive`. */
export interface ExternalToolsHandlers {
  onOpenInTerminal(path: string): void;
  onRevealInFileManager(path: string): void;
  onOpenInEditor(path: string): void;
}

/** P49: the shared "Open externally" items for a filesystem `path`. Standalone
 *  (not a closure) so App can build the identical trio for the tab context menu
 *  without a full `createWorkspaceMenus` instance. Always enabled. */
export function externalToolsItems(
  path: string,
  h: ExternalToolsHandlers,
): ContextMenuItem[] {
  return [
    {
      label: 'Open in terminal',
      icon: createElement(TerminalIcon),
      disabled: false,
      onSelect: () => h.onOpenInTerminal(path),
    },
    {
      label: 'Reveal in file manager',
      icon: createElement(FolderOpenIcon),
      disabled: false,
      onSelect: () => h.onRevealInFileManager(path),
    },
    {
      label: 'Open in editor',
      icon: createElement(EditorIcon),
      disabled: false,
      onSelect: () => h.onOpenInEditor(path),
    },
  ];
}

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
  handleCheckoutCommit(oid: string): void;
  setPendingCreateBranch(v: { oid: string }): void;
  runSummarize(base: string, target: string): void;
  runAnalyze(target: AiDiffTarget, mode: AiAnalysisMode, title: string): void;
  // P56b: generate grouped release notes for a tag/ref range. The tag-pill entry
  // passes { kind:'sinceLastTag', target: tagName } → notes for what shipped in
  // that tag. Read-only; results/errors render in the AiOutputPanel.
  runChangelog(range: ChangelogRange, title: string): void;
  handleMergeBranch(name: string): void;
  setPendingRebase(v: { name: string; cur: string }): void;
  openRebasePlan(target: { ontoOid: string; ontoLabel: string }): void;
  handleCompareWithHead(oid: string): void;
  setPendingDeleteRemote(name: string): void;
  setPendingDeleteBranch(name: string): void;
  /** P60a: arm the rename PromptDialog for a local branch (prefilled name). */
  setPendingRenameBranch(v: { name: string }): void;
  handleApplyStash(index: number, oid?: string): void;
  handlePopStash(index: number, oid?: string): void;
  setPendingDropStash(v: { index: number; oid?: string }): void;
  handleInitSubmodule(name: string): void;
  handleUpdateSubmodule(name: string): void;
  handleSyncSubmodule(name: string): void;
  setPendingDeinitSubmodule(name: string): void;
  setPendingRemoveSubmodule(name: string): void;
  onOpenRepoPath(path: string): void;
  setWorktreeContextOpen(v: boolean): void;
  setPendingWorktreeLock(name: string): void;
  handleUnlockWorktree(name: string): void;
  setPendingWorktreeRemove(v: { name: string; absPath: string }): void;
  setPendingDeleteTag(name: string): void;
  handlePushTag(remote: string, name: string): void;
  // P77: live tag-sync report (null until the first check / when unavailable) +
  // the resolve handlers. Status-gated tag menu items read the per-name verdict
  // from this report.
  tagSync: TagSyncReport | null;
  handleForceRefreshTag(remote: string, name: string): void;
  handleFetchRemoteTag(remote: string, name: string): void;
  setPendingDeleteRemoteTag(v: PendingDeleteRemoteTag): void;
  setPendingForceMoveTag(v: PendingForceMoveTag): void;
  setPendingRenameRemote(v: { name: string }): void;
  setPendingEditUrl(v: { name: string; url: string }): void;
  setPendingRemoveRemote(name: string): void;
  setPendingCreateTag(v: { oid: string }): void;
  handleCherrypick(oid: string): void;
  handleRevert(oid: string): void;
  setPendingReset(v: { oid: string; mode: ResetMode }): void;
  onViewReflog(refName: string): void;
  // P39b: two-click bisect entry. `pendingBisectBad` = the oid already marked
  // BAD (null when none pending); `bisectActive` hides the entry mid-bisect.
  pendingBisectBad: string | null;
  bisectActive: boolean;
  handleMarkBisectBad(oid: string): void;
  handleStartBisect(bad: string, good: string): void;
  // P49: external-tool launchers (terminal / file manager / editor). Threaded
  // through so row menus can spread the shared `externalToolsItems`.
  onOpenInTerminal(path: string): void;
  onRevealInFileManager(path: string): void;
  onOpenInEditor(path: string): void;
}

export interface WorkspaceMenus {
  branchMenuItems(name: string, kind: 'localBranch' | 'remoteBranch'): ContextMenuItem[];
  stashMenuItems(index: number, oid?: string): ContextMenuItem[];
  submoduleMenuItems(sub: SubmoduleInfo): ContextMenuItem[];
  worktreeMenuItems(wt: WorktreeInfo): ContextMenuItem[];
  tagMenuItems(name: string, oid: string | null): ContextMenuItem[];
  remoteMenuItems(name: string): ContextMenuItem[];
  resetMenuItems(targetOid: string): ContextMenuItem[];
  commitMenuItems(oid: string): ContextMenuItem[];
  buildContextItems(target: GraphContextTarget): ContextMenuItem[];
  /** P49: the shared "Open externally" trio for a path, bound to this deps'
   *  handlers. Reused by the toolbar dropdown; spread into row menus below. */
  externalToolsItems(path: string): ContextMenuItem[];
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
    handleCheckoutCommit,
    setPendingCreateBranch,
    runSummarize,
    runAnalyze,
    runChangelog,
    handleMergeBranch,
    setPendingRebase,
    openRebasePlan,
    handleCompareWithHead,
    setPendingDeleteRemote,
    setPendingDeleteBranch,
    setPendingRenameBranch,
    setPendingDeleteTag,
    handlePushTag,
    tagSync,
    handleForceRefreshTag,
    handleFetchRemoteTag,
    setPendingDeleteRemoteTag,
    setPendingForceMoveTag,
    setPendingCreateTag,
    handleCherrypick,
    handleRevert,
    setPendingReset,
    onViewReflog,
    pendingBisectBad,
    bisectActive,
    handleMarkBisectBad,
    handleStartBisect,
    onOpenInTerminal,
    onRevealInFileManager,
    onOpenInEditor,
  } = deps;

  // P49: one handler bundle reused by every external-launch entry point.
  const extHandlers: ExternalToolsHandlers = {
    onOpenInTerminal,
    onRevealInFileManager,
    onOpenInEditor,
  };

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
          void (kind === 'remoteBranch'
            ? handleCheckoutRemote(name)
            : handleCheckoutBranch(name)),
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
    items.push(...commitActionItems(tip));
    items.push({
      label: 'Delete',
      icon: createElement(DeleteIcon),
      disabled: gate,
      tone: 'danger',
      onSelect: () =>
        kind === 'remoteBranch' ? setPendingDeleteRemote(name) : setPendingDeleteBranch(name),
    });
    // P20 §3.3: reset the CURRENT branch to this ref's tip (gated internally).
    items.push(...resetMenuItems(tip));
    return items;
  }

  // Row menus (stash/submodule/worktree/remote) are extracted to
  // workspaceMenusRows.ts; these thin wrappers bind the current deps/extHandlers.
  function stashMenuItems(index: number, oid?: string): ContextMenuItem[] {
    return stashMenuItemsImpl(deps, index, oid);
  }

  function submoduleMenuItems(sub: SubmoduleInfo): ContextMenuItem[] {
    return submoduleMenuItemsImpl(deps, extHandlers, sub);
  }

  function worktreeMenuItems(wt: WorktreeInfo): ContextMenuItem[] {
    return worktreeMenuItemsImpl(deps, extHandlers, wt);
  }

  // P22 §7.2: the shared tag menu — used by the graph tag pill AND the sidebar
  // tag rows. Delete (ConfirmDialog) + Copy + one "Push tag to <remote>" per
  // configured remote (§OPEN-7: 0 → no push item; 1 → single; >1 → one each).
  function tagMenuItems(name: string, oid: string | null): ContextMenuItem[] {
    const gate = mutating || opActive;
    // P77 §3: the per-name sync verdict gates the resolve items. Undefined when no
    // check has run / the remote is unavailable → the menu degrades to the
    // pre-P77 set (copy / release notes / push / delete-local).
    const entry = tagSync?.entries.find((e) => e.name === name);
    const status = entry?.status;
    const syncRemote = tagSync?.remote ?? null;
    const isRemoteOnly = status === 'remote-only';
    const oldShort = entry?.remoteOid?.slice(0, 7) ?? '';
    const newShort = entry?.localOid?.slice(0, 7) ?? '';
    // Item 7 shows only when the tag exists on the remote (in-sync/stale/remote-
    // only) — never for unpushed (nothing there) or the reserved deleted-on-remote.
    const existsOnRemote =
      status === 'in-sync' || status === 'stale' || status === 'remote-only';

    const items: ContextMenuItem[] = [];

    // Checkout is the most-primary action → FIRST. Graph tag pills pass an oid;
    // sidebar tag rows pass null (no reachable target) → no checkout (matches today).
    if (oid !== null) items.push(...checkoutMenuItems(oid));

    // 1. Update to remote target (stale) — resolve-in-place, no confirm (§3).
    if (status === 'stale' && syncRemote !== null) {
      items.push({
        label: 'Update to remote target',
        icon: createElement(TagIcon),
        disabled: gate,
        onSelect: () => void handleForceRefreshTag(syncRemote, name),
      });
    }
    // 2. Create local tag (remote-only ghost row).
    if (isRemoteOnly && syncRemote !== null) {
      items.push({
        label: 'Create local tag',
        icon: createElement(TagIcon),
        disabled: gate,
        onSelect: () => void handleFetchRemoteTag(syncRemote, name),
      });
    }
    // 3. Push tag to {remote} (existing) — one per configured remote. Skipped for
    // a remote-only row (no local tag to push).
    if (!isRemoteOnly) {
      for (const r of remotes) {
        items.push({
          label: `Push tag to ${r.name}`,
          icon: createElement(TagIcon),
          disabled: gate,
          onSelect: () => void handlePushTag(r.name, name),
        });
      }
    }
    // 4. Copy tag name (existing) — always.
    items.push({
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
    });
    // 5. Release notes since previous tag (existing) — AI-gated. Not for a ghost
    // row (no local history to summarise).
    if (!isRemoteOnly) {
      items.push({
        label: 'Release notes since previous tag',
        icon: createElement(SummarizeIcon),
        disabled: !aiEligible,
        onSelect: () =>
          runChangelog({ kind: 'sinceLastTag', target: name }, `Release notes for ${name}`),
      });
    }
    // 6. Delete tag (existing, local) — skipped for a remote-only ghost (no local
    // tag exists). Routes through the existing local confirm.
    if (!isRemoteOnly) {
      items.push({
        label: 'Delete tag',
        icon: createElement(DeleteIcon),
        disabled: gate,
        onSelect: () => setPendingDeleteTag(name),
      });
    }
    // 7. Delete tag on {remote}… (danger → confirm §4.1).
    if (existsOnRemote && syncRemote !== null) {
      items.push({
        label: `Delete tag on ${syncRemote}…`,
        icon: createElement(DeleteIcon),
        disabled: gate,
        tone: 'danger',
        onSelect: () => setPendingDeleteRemoteTag({ name, remote: syncRemote }),
      });
    }
    // 8. Force-move tag on {remote}… (stale, danger → confirm §4.2).
    if (status === 'stale' && syncRemote !== null) {
      items.push({
        label: `Force-move tag on ${syncRemote}…`,
        icon: createElement(TagIcon),
        disabled: gate,
        tone: 'danger',
        onSelect: () =>
          setPendingForceMoveTag({ name, remote: syncRemote, oldShort, newShort }),
      });
    }
    // P47 (Part A, Fork-1): a GRAPH tag pill carries its target oid (the node id),
    // so it gets the shared commit actions after the tag-specific items. Sidebar
    // tag rows pass `oid === null` and keep only the tag/sync actions above.
    if (oid !== null) items.push(...commitActionItems(oid));
    return items;
  }

  // P22 §7.2: the configured-remote management menu (sidebar rows only).
  // Extracted to workspaceMenusRows.ts; this wrapper binds the current deps.
  function remoteMenuItems(name: string): ContextMenuItem[] {
    return remoteMenuItemsImpl(deps, name);
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
  function commitActionItems(oid: string): ContextMenuItem[] {
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
  function checkoutMenuItems(oid: string): ContextMenuItem[] {
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

  function commitMenuItems(oid: string): ContextMenuItem[] {
    if (head === null || head.unborn) return [];
    const gate = mutating || opActive;
    return [
      ...checkoutMenuItems(oid),
      ...commitActionItems(oid),
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
      // P47 (Fork-1): the graph tag pill carries the node oid → pass it so the
      // shared commit actions are appended (sidebar tag rows pass null).
      if (r.kind === 'tag') return tagMenuItems(r.name, target.oid);
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
      const commitItems = commitMenuItems(entry.tip);
      // P60a: the current HEAD branch's own branch menu is empty (branchMenuItems
      // returns [] for isHead), so its graph pill / whole-row lands on this commit
      // fallback. PREPEND "Rename…" so the current branch — the most common rename
      // target — is renamable from the graph HEAD pill (exercising the wasHead
      // refresh path). Local branches only.
      // TODO(P60): sidebar HEAD-row rename parity.
      if (kind !== 'localBranch') return commitItems;
      return [
        {
          label: 'Rename…',
          icon: createElement(BranchIcon),
          disabled: mutating || opActive,
          onSelect: () => setPendingRenameBranch({ name: r.name }),
        },
        ...commitItems,
      ];
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
    externalToolsItems: (path: string) => externalToolsItems(path, extHandlers),
  };
}
