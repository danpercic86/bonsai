import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  BranchIcon,
  CheckoutIcon,
  CopyIcon,
  DeleteIcon,
  EditorIcon,
  FolderOpenIcon,
  HistoryIcon,
  MergeIcon,
  RebaseIcon,
  RebaseInteractiveIcon,
  SummarizeIcon,
  TerminalIcon,
} from './menuIcons';
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
import {
  commitActionItems as commitActionItemsImpl,
  commitMenuItems as commitMenuItemsImpl,
  resetMenuItems as resetMenuItemsImpl,
} from './workspaceMenusCommit';
import { tagMenuItems as tagMenuItemsImpl } from './workspaceMenusTag';
import { buildGraphTargetItems } from './workspaceMenusGraphTarget';

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

  // P22 §7.2: the shared tag menu. Extracted to workspaceMenusTag.ts; this
  // wrapper binds the current deps.
  function tagMenuItems(name: string, oid: string | null): ContextMenuItem[] {
    return tagMenuItemsImpl(deps, name, oid);
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
  // Commit-oid menu builders (reset / commit-actions / checkout / commit-row) are
  // extracted to workspaceMenusCommit.ts; these thin wrappers bind the current
  // deps so the intra-factory callers below stay unchanged.
  function resetMenuItems(targetOid: string): ContextMenuItem[] {
    return resetMenuItemsImpl(deps, targetOid);
  }

  function commitActionItems(oid: string): ContextMenuItem[] {
    return commitActionItemsImpl(deps, oid);
  }

  function commitMenuItems(oid: string): ContextMenuItem[] {
    return commitMenuItemsImpl(deps, oid);
  }

  // P92: the graph right-click dispatcher (ref pill / "+N" ref picker / commit
  // row) is extracted to workspaceMenusGraphTarget.ts; this wrapper binds the
  // per-ref builders it composes.
  function buildContextItems(target: GraphContextTarget): ContextMenuItem[] {
    return buildGraphTargetItems(
      { deps, branchMenuItems, stashMenuItems, tagMenuItems, commitMenuItems },
      target,
    );
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
