import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  EditorIcon,
  FolderOpenIcon,
  TerminalIcon,
} from './menuIcons';
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
  commitMenuItems as commitMenuItemsImpl,
  resetMenuItems as resetMenuItemsImpl,
} from './workspaceMenusCommit';
import { tagMenuItems as tagMenuItemsImpl } from './workspaceMenusTag';
import { branchMenuItems as branchMenuItemsImpl } from './workspaceMenusBranch';
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
  /** Spec-003 §3.1: the solo/hide item group for a FULL ref name — built by
   *  RepoWorkspace from useGraphFilter (workspaceMenusFilter.ts). */
  refFilterItems(fullRef: string, noun: 'branch' | 'tag'): ContextMenuItem[];
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
  // used identically by the graph pills AND the sidebar rows. Extracted to
  // workspaceMenusBranch.ts (spec-003 size split); this wrapper binds the deps.
  function branchMenuItems(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
  ): ContextMenuItem[] {
    return branchMenuItemsImpl(deps, name, kind);
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
