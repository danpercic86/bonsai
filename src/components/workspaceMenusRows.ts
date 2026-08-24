import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import {
  BranchIcon,
  CompareIcon,
  DeleteIcon,
  RebaseIcon,
  ResetIcon,
  StashApplyIcon,
  StashPopIcon,
} from './menuIcons';
import {
  externalToolsItems,
  type ExternalToolsHandlers,
  type WorkspaceMenuDeps,
} from './workspaceMenus';
import type { SubmoduleInfo, WorktreeInfo } from '../ipc';

// P9 §6.4: build the right-click menu for a stash row. Apply/Pop need a clean,
// idle repo (gated on mutating || opActive); Drop is allowed mid-op (it only
// edits the stash reflog) → routes through the ConfirmDialog.
// F-A6-B: `oid` is the value the stash row rendered; thread it into every
// action so a stack shift between render and confirm can't hit the wrong entry.
// Optional because the graph stash pill only knows the base commit oid, not the
// stash entry oid — it omits it (guard is best-effort; sidebar rows always pass).
export function stashMenuItems(
  deps: WorkspaceMenuDeps,
  index: number,
  oid?: string,
): ContextMenuItem[] {
  const { mutating, opActive, handleApplyStash, handlePopStash, setPendingDropStash } = deps;
  const gate = mutating || opActive;
  return [
    {
      label: 'Apply',
      icon: createElement(StashApplyIcon),
      disabled: gate,
      onSelect: () => void handleApplyStash(index, oid),
    },
    {
      label: 'Pop',
      icon: createElement(StashPopIcon),
      disabled: gate,
      onSelect: () => void handlePopStash(index, oid),
    },
    {
      label: 'Drop',
      icon: createElement(DeleteIcon),
      disabled: mutating,
      onSelect: () => setPendingDropStash({ index, oid }),
    },
  ];
}

// P19 §6.4 + P73 §3.2: submodule row menu. "Initialize and check out" and
// "Update" are the same backend call (sm.update(init:true)) as a mutually-
// exclusive pair — one is live per row state, like Lock…/Unlock below. Deinit
// is a no-op once uninitialized; open-in-tab needs files on disk.
export function submoduleMenuItems(
  deps: WorkspaceMenuDeps,
  extHandlers: ExternalToolsHandlers,
  sub: SubmoduleInfo,
): ContextMenuItem[] {
  const {
    mutating,
    opActive,
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
    setPendingDeinitSubmodule,
    setPendingRemoveSubmodule,
    onOpenRepoPath,
  } = deps;
  const gate = mutating || opActive;
  const uninit = sub.status === 'uninitialized';
  return [
    {
      label: 'Initialize and check out',
      icon: createElement(BranchIcon),
      disabled: gate || !uninit,
      onSelect: () => void handleInitSubmodule(sub.name),
    },
    {
      label: 'Update',
      icon: createElement(StashApplyIcon),
      disabled: gate || uninit,
      onSelect: () => void handleUpdateSubmodule(sub.name),
    },
    {
      label: 'Sync',
      icon: createElement(RebaseIcon),
      disabled: gate,
      onSelect: () => void handleSyncSubmodule(sub.name),
    },
    // P60d: deinit clears config + empties the worktree (keeps .gitmodules).
    {
      label: 'Deinitialize…',
      icon: createElement(ResetIcon),
      disabled: gate || uninit,
      onSelect: () => setPendingDeinitSubmodule(sub.name),
    },
    {
      label: 'Remove…',
      icon: createElement(DeleteIcon),
      disabled: gate,
      tone: 'danger',
      onSelect: () => setPendingRemoveSubmodule(sub.name),
    },
    {
      label: 'Open in new tab',
      icon: createElement(CompareIcon),
      disabled: uninit,
      onSelect: () => onOpenRepoPath(sub.absPath),
    },
    // P49: launch external tools at the submodule's absolute workdir. Always
    // enabled (they touch no git state) — never gated by `gate` above.
    ...externalToolsItems(sub.absPath, extHandlers),
  ];
}

// P27 §6.4: worktree row menu. Open-in-tab needs an intact working tree;
// Lock/Unlock apply to linked worktrees only; Remove is disabled for
// main/current/locked in the UI AND refused server-side (§2.6).
export function worktreeMenuItems(
  deps: WorkspaceMenuDeps,
  extHandlers: ExternalToolsHandlers,
  wt: WorktreeInfo,
): ContextMenuItem[] {
  const {
    mutating,
    opActive,
    onOpenRepoPath,
    setWorktreeContextOpen,
    setPendingWorktreeLock,
    handleUnlockWorktree,
    setPendingWorktreeRemove,
  } = deps;
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
    // P49: launch external tools at the worktree's absolute workdir. Always
    // enabled — never gated by `gate` above.
    ...externalToolsItems(wt.absPath, extHandlers),
  ];
}

// P22 §7.2: the configured-remote management menu (sidebar rows only).
export function remoteMenuItems(deps: WorkspaceMenuDeps, name: string): ContextMenuItem[] {
  const { mutating, opActive, remotes, setPendingRenameRemote, setPendingEditUrl, setPendingRemoveRemote } =
    deps;
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
