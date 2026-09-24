/**
 * P119 §4.2 — the per-category copy + glyph table for the git-activity dock.
 * Moved out of `gitActivityFormat.ts` (size). Both tables are
 * `Record<GitActivityCategory, …>`, so tsc refuses a category without an entry.
 *
 * P119-1 lands PLACEHOLDER copy for the 56 new categories (noun = verb = the
 * operation name); P119-4 replaces it with the final P119-ui table (which also
 * owns any extra fields such as a count noun).
 */
import type { ComponentType } from 'react';

import {
  CloudIcon,
  FetchIcon,
  PullIcon,
  PushIcon,
  RefDotIcon,
  StashIcon,
  UndoIcon,
  WorktreeIcon,
} from './appIcons';
import {
  BisectIcon,
  BranchIcon,
  CheckoutIcon,
  CherryPickIcon,
  DeleteIcon,
  FolderOpenIcon,
  MergeIcon,
  RebaseIcon,
  RebaseInteractiveIcon,
  ResetIcon,
  RevertIcon,
  StashApplyIcon,
  StashPopIcon,
  TagIcon,
} from './menuIcons';
import type { GitActivityCategory } from '../ipc';

type IconComponent = ComponentType;

export interface CategoryMeta {
  /** Idle button + palette verb ("Push"). */
  verb: string;
  /** Layout-stable busy button participle ("Pushing…"). */
  participle: string;
  /** Terminal row noun ("Push", "Merge commit"). */
  noun: string;
  /** The category glyph icon (reused graph/menu icon). */
  glyph: IconComponent;
}

/** Placeholder entry: noun = verb (P119-1; final copy lands in P119-4). */
function meta(verb: string, participle: string, glyph: IconComponent): CategoryMeta {
  return { verb, participle, noun: verb, glyph };
}

export const CATEGORY_META: Record<GitActivityCategory, CategoryMeta> = {
  // P87 (locked copy). Key order = the Rust enum's declaration order, which
  // `GIT_ACTIVITY_CATEGORIES` below relies on.
  commit: { verb: 'Commit', participle: 'Committing…', noun: 'Commit', glyph: RefDotIcon },
  amend: { verb: 'Amend', participle: 'Amending…', noun: 'Amend', glyph: RefDotIcon },
  mergeCommit: { verb: 'Merge', participle: 'Merging…', noun: 'Merge commit', glyph: MergeIcon },
  push: { verb: 'Push', participle: 'Pushing…', noun: 'Push', glyph: PushIcon },
  forcePush: { verb: 'Force-push', participle: 'Force-pushing…', noun: 'Force-push', glyph: PushIcon },
  fetch: { verb: 'Fetch', participle: 'Fetching…', noun: 'Fetch', glyph: FetchIcon },
  pull: { verb: 'Pull', participle: 'Pulling…', noun: 'Pull', glyph: PullIcon },
  // P119 — placeholders
  checkoutBranch: meta('Checkout', 'Checking out…', CheckoutIcon),
  checkoutCommit: meta('Checkout commit', 'Checking out…', CheckoutIcon),
  checkoutRemote: meta('Checkout remote branch', 'Checking out…', CheckoutIcon),
  createBranch: meta('Create branch', 'Creating branch…', BranchIcon),
  deleteBranch: meta('Delete branch', 'Deleting branch…', DeleteIcon),
  deleteBranches: meta('Delete branches', 'Deleting branches…', DeleteIcon),
  renameBranch: meta('Rename branch', 'Renaming branch…', BranchIcon),
  deleteRemoteTracking: meta('Delete remote-tracking branch', 'Deleting…', DeleteIcon),
  merge: meta('Merge', 'Merging…', MergeIcon),
  abortMerge: meta('Abort merge', 'Aborting merge…', MergeIcon),
  rebase: meta('Rebase', 'Rebasing…', RebaseIcon),
  interactiveRebase: meta('Interactive rebase', 'Rebasing…', RebaseInteractiveIcon),
  rebaseContinue: meta('Continue rebase', 'Rebasing…', RebaseIcon),
  rebaseSkip: meta('Skip rebase step', 'Rebasing…', RebaseIcon),
  rebaseAbort: meta('Abort rebase', 'Aborting rebase…', RebaseIcon),
  cherryPick: meta('Cherry-pick', 'Cherry-picking…', CherryPickIcon),
  cherryPickContinue: meta('Continue cherry-pick', 'Cherry-picking…', CherryPickIcon),
  cherryPickAbort: meta('Abort cherry-pick', 'Aborting cherry-pick…', CherryPickIcon),
  revert: meta('Revert', 'Reverting…', RevertIcon),
  revertContinue: meta('Continue revert', 'Reverting…', RevertIcon),
  revertAbort: meta('Abort revert', 'Aborting revert…', RevertIcon),
  resetSoft: meta('Soft reset', 'Resetting…', ResetIcon),
  resetMixed: meta('Mixed reset', 'Resetting…', ResetIcon),
  resetHard: meta('Hard reset', 'Resetting…', ResetIcon),
  stashCreate: meta('Stash', 'Stashing…', StashIcon),
  stashApply: meta('Apply stash', 'Applying stash…', StashApplyIcon),
  stashPop: meta('Pop stash', 'Popping stash…', StashPopIcon),
  stashDrop: meta('Drop stash', 'Dropping stash…', DeleteIcon),
  createTag: meta('Create tag', 'Creating tag…', TagIcon),
  deleteTag: meta('Delete tag', 'Deleting tag…', TagIcon),
  pushTag: meta('Push tag', 'Pushing tag…', TagIcon),
  deleteRemoteTag: meta('Delete remote tag', 'Deleting remote tag…', TagIcon),
  forceRefreshTag: meta('Update tag', 'Updating tag…', TagIcon),
  submoduleAdd: meta('Add submodule', 'Adding submodule…', FolderOpenIcon),
  submoduleInit: meta('Initialize submodule', 'Initializing submodule…', FolderOpenIcon),
  submoduleUpdate: meta('Update submodule', 'Updating submodule…', FolderOpenIcon),
  submoduleSync: meta('Sync submodule', 'Syncing submodule…', FolderOpenIcon),
  submoduleDeinit: meta('Deinitialize submodule', 'Deinitializing submodule…', FolderOpenIcon),
  submoduleRemove: meta('Remove submodule', 'Removing submodule…', DeleteIcon),
  worktreeAdd: meta('Add worktree', 'Adding worktree…', WorktreeIcon),
  worktreeRemove: meta('Remove worktree', 'Removing worktree…', WorktreeIcon),
  worktreeLock: meta('Lock worktree', 'Locking worktree…', WorktreeIcon),
  worktreeUnlock: meta('Unlock worktree', 'Unlocking worktree…', WorktreeIcon),
  discard: meta('Discard changes', 'Discarding…', UndoIcon),
  bisectStart: meta('Start bisect', 'Starting bisect…', BisectIcon),
  bisectGood: meta('Mark good', 'Marking…', BisectIcon),
  bisectBad: meta('Mark bad', 'Marking…', BisectIcon),
  bisectSkip: meta('Skip commit', 'Skipping…', BisectIcon),
  bisectReset: meta('End bisect', 'Ending bisect…', BisectIcon),
  composeCommits: meta('Compose commits', 'Composing…', RefDotIcon),
  cloneRepo: meta('Clone', 'Cloning…', CloudIcon),
  initRepo: meta('Initialize repository', 'Initializing…', FolderOpenIcon),
  addRemote: meta('Add remote', 'Adding remote…', CloudIcon),
  removeRemote: meta('Remove remote', 'Removing remote…', CloudIcon),
  renameRemote: meta('Rename remote', 'Renaming remote…', CloudIcon),
  setRemoteUrl: meta('Change remote URL', 'Updating remote…', CloudIcon),
};

/** §3.7 — the preposition that joins the noun to the target in the ACCESSIBLE
 *  name only (`to` / `from` / `on`, or `null` = noun and target side by side).
 *  Keyed exhaustively by category so a new category cannot silently miss one.
 *  The visible row stays preposition-free (§3.2). P119 placeholders: `on` for
 *  the HEAD-branch rows, `null` for the rest. */
export const TARGET_PREPOSITION: Record<GitActivityCategory, 'to' | 'from' | 'on' | null> = {
  push: 'to',
  forcePush: 'to',
  fetch: 'from',
  pull: 'from',
  commit: 'on',
  amend: 'on',
  mergeCommit: 'on',
  checkoutBranch: null,
  checkoutCommit: null,
  checkoutRemote: null,
  createBranch: null,
  deleteBranch: null,
  deleteBranches: null,
  renameBranch: null,
  deleteRemoteTracking: null,
  merge: null,
  abortMerge: 'on',
  rebase: null,
  interactiveRebase: null,
  rebaseContinue: 'on',
  rebaseSkip: 'on',
  rebaseAbort: 'on',
  cherryPick: null,
  cherryPickContinue: 'on',
  cherryPickAbort: 'on',
  revert: null,
  revertContinue: 'on',
  revertAbort: 'on',
  resetSoft: null,
  resetMixed: null,
  resetHard: null,
  stashCreate: 'on',
  stashApply: null,
  stashPop: null,
  stashDrop: null,
  createTag: null,
  deleteTag: null,
  pushTag: null,
  deleteRemoteTag: null,
  forceRefreshTag: null,
  submoduleAdd: null,
  submoduleInit: null,
  submoduleUpdate: null,
  submoduleSync: null,
  submoduleDeinit: null,
  submoduleRemove: null,
  worktreeAdd: null,
  worktreeRemove: null,
  worktreeLock: null,
  worktreeUnlock: null,
  discard: null,
  bisectStart: null,
  bisectGood: null,
  bisectBad: null,
  bisectSkip: null,
  bisectReset: null,
  composeCommits: 'on',
  cloneRepo: null,
  initRepo: null,
  addRemote: null,
  removeRemote: null,
  renameRemote: null,
  setRemoteUrl: null,
};

/** Every category, in the Rust enum's declaration order (`GitActivityCategory::ALL`). */
export const GIT_ACTIVITY_CATEGORIES: readonly GitActivityCategory[] = Object.keys(
  CATEGORY_META,
) as GitActivityCategory[];
