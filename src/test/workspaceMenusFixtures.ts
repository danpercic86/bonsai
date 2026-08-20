/** T3.1 shared fixtures for the workspaceMenus tests (lives in src/test/ so it
 *  is excluded from coverage and never picked up as a test file itself). */
import { vi } from 'vitest';

import type { ContextMenuItem } from '../components/ContextMenu';
import type { WorkspaceMenuDeps } from '../components/workspaceMenus';
import type {
  BranchesSnapshot,
  BranchInfo,
  HeadInfo,
  SubmoduleInfo,
  WorktreeInfo,
} from '../ipc';

export const OID_HEAD = 'a'.repeat(40);
export const OID_FEATURE = 'b'.repeat(40);
export const OID_REMOTE = 'c'.repeat(40);
export const OID_OTHER = 'd'.repeat(40);

export function makeHead(over: Partial<HeadInfo> = {}): HeadInfo {
  return { branchName: 'main', oid: OID_HEAD, detached: false, unborn: false, ...over };
}

export function mainBranch(over: Partial<BranchInfo> = {}): BranchInfo {
  return {
    name: 'main',
    isHead: true,
    upstream: 'origin/main',
    ahead: 0,
    behind: 0,
    tip: OID_HEAD,
    ...over,
  };
}

export function featureBranch(over: Partial<BranchInfo> = {}): BranchInfo {
  return {
    name: 'feature',
    isHead: false,
    upstream: null,
    ahead: null,
    behind: null,
    tip: OID_FEATURE,
    ...over,
  };
}

export function makeSnapshot(over: Partial<BranchesSnapshot> = {}): BranchesSnapshot {
  return {
    local: [featureBranch(), mainBranch()],
    remote: [{ name: 'origin/feature', tip: OID_REMOTE }],
    tags: ['v1.0'],
    head: makeHead(),
    ...over,
  };
}

/** Every handler is a fresh vi.fn(); scalars default to an idle attached repo. */
export function makeDeps(over: Partial<WorkspaceMenuDeps> = {}): WorkspaceMenuDeps {
  return {
    branches: makeSnapshot(),
    headBranch: mainBranch(),
    head: makeHead(),
    mutating: false,
    opActive: false,
    aiEligible: false,
    remotes: [{ name: 'origin', url: 'https://example.com/r.git' }],
    pushToast: vi.fn(),
    handleCheckoutRemote: vi.fn(),
    handleCheckoutBranch: vi.fn(),
    setPendingCreateBranch: vi.fn(),
    runSummarize: vi.fn(),
    runAnalyze: vi.fn(),
    runChangelog: vi.fn(),
    handleMergeBranch: vi.fn(),
    setPendingRebase: vi.fn(),
    openRebasePlan: vi.fn(),
    handleCompareWithHead: vi.fn(),
    setPendingDeleteRemote: vi.fn(),
    setPendingDeleteBranch: vi.fn(),
    setPendingRenameBranch: vi.fn(),
    handleApplyStash: vi.fn(),
    handlePopStash: vi.fn(),
    setPendingDropStash: vi.fn(),
    handleInitSubmodule: vi.fn(),
    handleUpdateSubmodule: vi.fn(),
    handleSyncSubmodule: vi.fn(),
    setPendingDeinitSubmodule: vi.fn(),
    setPendingRemoveSubmodule: vi.fn(),
    onOpenRepoPath: vi.fn(),
    setWorktreeContextOpen: vi.fn(),
    setPendingWorktreeLock: vi.fn(),
    handleUnlockWorktree: vi.fn(),
    setPendingWorktreeRemove: vi.fn(),
    setPendingDeleteTag: vi.fn(),
    handlePushTag: vi.fn(),
    tagSync: null,
    handleForceRefreshTag: vi.fn(),
    handleFetchRemoteTag: vi.fn(),
    setPendingDeleteRemoteTag: vi.fn(),
    setPendingForceMoveTag: vi.fn(),
    setPendingRenameRemote: vi.fn(),
    setPendingEditUrl: vi.fn(),
    setPendingRemoveRemote: vi.fn(),
    setPendingCreateTag: vi.fn(),
    handleCherrypick: vi.fn(),
    handleRevert: vi.fn(),
    setPendingReset: vi.fn(),
    onViewReflog: vi.fn(),
    pendingBisectBad: null,
    bisectActive: false,
    handleMarkBisectBad: vi.fn(),
    handleStartBisect: vi.fn(),
    onOpenInTerminal: vi.fn(),
    onRevealInFileManager: vi.fn(),
    onOpenInEditor: vi.fn(),
    ...over,
  };
}

export function makeSubmodule(over: Partial<SubmoduleInfo> = {}): SubmoduleInfo {
  return {
    name: 'libs/dep',
    path: 'libs/dep',
    absPath: '/repo/libs/dep',
    url: 'https://example.com/dep.git',
    headOid: OID_OTHER,
    indexOid: OID_OTHER,
    wtOid: OID_OTHER,
    status: 'upToDate',
    ...over,
  };
}

export function makeWorktree(over: Partial<WorktreeInfo> = {}): WorktreeInfo {
  return {
    name: 'wt1',
    absPath: '/repo/.worktrees/wt1',
    relPath: null,
    branch: 'feature',
    headOid: OID_FEATURE,
    locked: false,
    lockReason: null,
    isMain: false,
    isCurrent: false,
    prunable: false,
    valid: true,
    ...over,
  };
}

export const labelsOf = (items: ContextMenuItem[]): string[] => items.map((i) => i.label);

export function itemByLabel(items: ContextMenuItem[], label: string): ContextMenuItem {
  const it = items.find((i) => i.label === label);
  if (it === undefined) throw new Error(`menu item not found: ${label}`);
  return it;
}
