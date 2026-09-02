// The container's row → context-menu adapters: each takes the right-clicked
// row's identity plus the cursor position and arms the ONE shared ContextMenu
// state from the prebuilt item arrays in `workspaceMenus.ts`. Extracted
// verbatim from RepoWorkspace; rebuilt each render over the current state +
// menus so the produced handlers stay byte-identical to the old inline ones.
import type { Dispatch, SetStateAction } from 'react';
import type { BranchInfo, SubmoduleInfo, WorktreeInfo } from '../../ipc';
import type { ContextMenuState } from '../ContextMenu';
import type { GraphContextTarget } from '../../graph/GraphCanvas';
import type { GraphFilterController } from '../../hooks/useGraphFilter';
import type { WorkspaceMenus } from '../workspaceMenus';
import { headRowFilterMenuItems } from '../workspaceMenusFilter';
import { graphMenuState } from '../workspaceMenusRefPicker';

export interface ContextMenuOpenerDeps {
  setMenu: Dispatch<SetStateAction<ContextMenuState | null>>;
  menus: WorkspaceMenus;
  submodules: SubmoduleInfo[];
  worktrees: WorktreeInfo[];
  headBranch: BranchInfo | null;
  graphFilter: GraphFilterController;
}

export interface ContextMenuOpeners {
  handleStashContextMenu: (index: number, oid: string, clientX: number, clientY: number) => void;
  handleSubmoduleContextMenu: (name: string, clientX: number, clientY: number) => void;
  handleWorktreeContextMenu: (name: string, clientX: number, clientY: number) => void;
  handleTagContextMenu: (name: string, clientX: number, clientY: number) => void;
  handleRemoteContextMenu: (name: string, clientX: number, clientY: number) => void;
  handleGraphContextMenu: (
    target: GraphContextTarget,
    clientX: number,
    clientY: number,
  ) => void;
  handleSidebarContextMenu: (
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ) => void;
}

export function createContextMenuOpeners({
  setMenu,
  menus,
  submodules,
  worktrees,
  headBranch,
  graphFilter,
}: ContextMenuOpenerDeps): ContextMenuOpeners {
  // P9 §6.4: right-click a sidebar stash row → open the shared context menu.
  function handleStashContextMenu(index: number, oid: string, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: menus.stashMenuItems(index, oid) });
  }

  // P19 §6.4: right-click a sidebar submodule row → open the shared context
  // menu. Looks up the SubmoduleInfo by name from state.
  function handleSubmoduleContextMenu(name: string, clientX: number, clientY: number) {
    const sub = submodules.find((s) => s.name === name);
    if (sub === undefined) return;
    setMenu({ x: clientX, y: clientY, items: menus.submoduleMenuItems(sub) });
  }

  // P27 §6.4: right-click a sidebar worktree row → open the shared context
  // menu. Looks up the WorktreeInfo by name from state.
  function handleWorktreeContextMenu(name: string, clientX: number, clientY: number) {
    const wt = worktrees.find((w) => w.name === name);
    if (wt === undefined) return;
    setMenu({ x: clientX, y: clientY, items: menus.worktreeMenuItems(wt) });
  }

  function handleTagContextMenu(name: string, clientX: number, clientY: number) {
    // P47 (F3): sidebar tag rows have no cheap oid → pass null (delete/copy/push
    // only; graph tag pills pass the node oid and get the shared commit actions).
    setMenu({ x: clientX, y: clientY, items: menus.tagMenuItems(name, null) });
  }

  function handleRemoteContextMenu(name: string, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: menus.remoteMenuItems(name) });
  }

  function handleGraphContextMenu(target: GraphContextTarget, clientX: number, clientY: number) {
    const items = menus.buildContextItems(target);
    if (items.length === 0) return; // no valid actions → menu does not open
    setMenu(graphMenuState(target, items, clientX, clientY));
  }

  // P6 §4.3: right-click a sidebar branch/remote row → open the SAME shared menu
  // at the cursor. Empty items (current branch, missing entry) → no menu.
  function handleSidebarContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ) {
    let items = menus.branchMenuItems(name, kind);
    // Spec-003 §3.1: the checked-out branch gets the solo/hide group alone.
    if (items.length === 0 && kind === 'localBranch' && headBranch?.name === name)
      items = headRowFilterMenuItems(graphFilter, name);
    if (items.length === 0) return;
    setMenu({ x: clientX, y: clientY, items });
  }

  return {
    handleStashContextMenu,
    handleSubmoduleContextMenu,
    handleWorktreeContextMenu,
    handleTagContextMenu,
    handleRemoteContextMenu,
    handleGraphContextMenu,
    handleSidebarContextMenu,
  };
}
