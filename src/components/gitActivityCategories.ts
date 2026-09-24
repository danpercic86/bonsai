/**
 * P119 §4.2 — the per-category copy + glyph table for the git-activity dock.
 * Moved out of `gitActivityFormat.ts` (size). Both tables are
 * `Record<GitActivityCategory, …>`, so tsc refuses a category without an entry.
 *
 * Copy is the final P119-ui §4.3 table (keyed by the §1 wire identifiers). The
 * seven P87 rows are LOCKED and unchanged. Glyph rule (§5): one glyph per
 * family, reusing the menu glyph the user clicked; the noun carries the verb.
 */
import type { ComponentType } from 'react';

import {
  CloneIcon,
  CloudIcon,
  FetchIcon,
  InitRepoIcon,
  PullIcon,
  PushIcon,
  RefBranchIcon,
  RefDotIcon,
  StashIcon,
  SubmoduleIcon,
  WorktreeIcon,
} from './appIcons';
import {
  BisectIcon,
  CheckoutIcon,
  CherryPickIcon,
  MergeIcon,
  RebaseIcon,
  RebaseInteractiveIcon,
  ResetIcon,
  RevertIcon,
  TagIcon,
} from './menuIcons';
import type { GitActivityCategory } from '../ipc';

type IconComponent = ComponentType;

export interface CategoryMeta {
  /** Idle button + palette verb, and the announcer's phrasing ("Push", "Check out"). */
  verb: string;
  /** Layout-stable busy participle ("Pushing…"); also the running sub-label of
   *  the P119 categories in `preparing` / `finalizing` (P119-ui §4.2-4). */
  participle: string;
  /** Row + collapsed-bar noun ("Push", "Merge commit", "Checkout"). */
  noun: string;
  /** The category glyph icon (reused graph/menu icon). */
  glyph: IconComponent;
  /** Lowercase object of the blocking-hook note: `This hook blocked the {blockedNoun}.` */
  blockedNoun: string;
  /** P119-ui §4.4: the noun-slot words when `targetCount` (≥2) is set. Only the
   *  two categories that ever carry a count define it. */
  countNoun?: (n: number) => string;
  /** P119-ui §4.2-4: the `network`-phase label for a new op that talks to a remote. */
  networkLabel?: string;
}

type Extra = Pick<CategoryMeta, 'countNoun' | 'networkLabel'>;

/** A P119 entry whose verb equals its noun (every §4.3 row but checkout). */
function op(
  noun: string,
  participle: string,
  glyph: IconComponent,
  blockedNoun: string,
  extra: Extra = {},
): CategoryMeta {
  return { verb: noun, participle, noun, glyph, blockedNoun, ...extra };
}

/** Checkout: the noun is `Checkout`, the verb (announcer) is `Check out`. */
const CHECKOUT: CategoryMeta = {
  verb: 'Check out',
  participle: 'Checking out…',
  noun: 'Checkout',
  glyph: CheckoutIcon,
  blockedNoun: 'checkout',
};

const BRANCH = 'branch change';
const TAG = 'tag change';
const SUBMODULE = 'submodule change';
const WORKTREE = 'worktree change';
const REMOTE = 'remote change';

export const CATEGORY_META: Record<GitActivityCategory, CategoryMeta> = {
  // P87 (locked copy). Key order = the Rust enum's declaration order, which
  // `GIT_ACTIVITY_CATEGORIES` below relies on.
  commit: { verb: 'Commit', participle: 'Committing…', noun: 'Commit', glyph: RefDotIcon, blockedNoun: 'commit' },
  amend: { verb: 'Amend', participle: 'Amending…', noun: 'Amend', glyph: RefDotIcon, blockedNoun: 'commit' },
  mergeCommit: { verb: 'Merge', participle: 'Merging…', noun: 'Merge commit', glyph: MergeIcon, blockedNoun: 'merge' },
  push: { verb: 'Push', participle: 'Pushing…', noun: 'Push', glyph: PushIcon, blockedNoun: 'push' },
  forcePush: { verb: 'Force-push', participle: 'Force-pushing…', noun: 'Force-push', glyph: PushIcon, blockedNoun: 'push' },
  fetch: { verb: 'Fetch', participle: 'Fetching…', noun: 'Fetch', glyph: FetchIcon, blockedNoun: 'fetch' },
  pull: { verb: 'Pull', participle: 'Pulling…', noun: 'Pull', glyph: PullIcon, blockedNoun: 'pull' },
  // P119-ui §4.3
  checkoutBranch: CHECKOUT,
  checkoutCommit: CHECKOUT,
  checkoutRemote: CHECKOUT,
  createBranch: op('Create branch', 'Creating branch…', RefBranchIcon, BRANCH),
  deleteBranch: op('Delete branch', 'Deleting branch…', RefBranchIcon, BRANCH),
  deleteBranches: op('Delete branch', 'Deleting branches…', RefBranchIcon, BRANCH, {
    countNoun: (n) => `Delete ${n.toLocaleString()} branches`,
  }),
  renameBranch: op('Rename branch', 'Renaming branch…', RefBranchIcon, BRANCH),
  deleteRemoteTracking: op('Delete remote branch', 'Deleting remote branch…', RefBranchIcon, BRANCH),
  merge: op('Merge', 'Merging…', MergeIcon, 'merge'),
  abortMerge: op('Abort merge', 'Aborting merge…', MergeIcon, 'merge'),
  rebase: op('Rebase', 'Rebasing…', RebaseIcon, 'rebase'),
  interactiveRebase: op('Interactive rebase', 'Rebasing…', RebaseInteractiveIcon, 'rebase'),
  rebaseContinue: op('Continue rebase', 'Continuing rebase…', RebaseIcon, 'rebase'),
  rebaseSkip: op('Skip rebase commit', 'Skipping commit…', RebaseIcon, 'rebase'),
  rebaseAbort: op('Abort rebase', 'Aborting rebase…', RebaseIcon, 'rebase'),
  cherryPick: op('Cherry-pick', 'Cherry-picking…', CherryPickIcon, 'cherry-pick'),
  cherryPickContinue: op('Continue cherry-pick', 'Continuing cherry-pick…', CherryPickIcon, 'cherry-pick'),
  cherryPickAbort: op('Abort cherry-pick', 'Aborting cherry-pick…', CherryPickIcon, 'cherry-pick'),
  revert: op('Revert', 'Reverting…', RevertIcon, 'revert'),
  revertContinue: op('Continue revert', 'Continuing revert…', RevertIcon, 'revert'),
  revertAbort: op('Abort revert', 'Aborting revert…', RevertIcon, 'revert'),
  resetSoft: op('Soft reset', 'Resetting…', ResetIcon, 'reset'),
  resetMixed: op('Mixed reset', 'Resetting…', ResetIcon, 'reset'),
  resetHard: op('Hard reset', 'Resetting…', ResetIcon, 'reset'),
  stashCreate: op('Stash changes', 'Stashing…', StashIcon, 'stash'),
  stashApply: op('Apply stash', 'Applying stash…', StashIcon, 'stash'),
  stashPop: op('Pop stash', 'Popping stash…', StashIcon, 'stash'),
  stashDrop: op('Drop stash', 'Dropping stash…', StashIcon, 'stash'),
  createTag: op('Create tag', 'Creating tag…', TagIcon, TAG),
  deleteTag: op('Delete tag', 'Deleting tag…', TagIcon, TAG),
  pushTag: op('Push tag', 'Pushing tag…', TagIcon, TAG, { networkLabel: 'Sending objects…' }),
  deleteRemoteTag: op('Delete remote tag', 'Deleting remote tag…', TagIcon, TAG, {
    networkLabel: 'Deleting remote tag…',
  }),
  forceRefreshTag: op('Update tag', 'Updating tag…', TagIcon, TAG, { networkLabel: 'Fetching…' }),
  submoduleAdd: op('Add submodule', 'Adding submodule…', SubmoduleIcon, SUBMODULE, {
    networkLabel: 'Cloning…',
  }),
  submoduleInit: op('Initialize submodule', 'Initializing submodule…', SubmoduleIcon, SUBMODULE),
  submoduleUpdate: op('Update submodule', 'Updating submodule…', SubmoduleIcon, SUBMODULE, {
    networkLabel: 'Fetching…',
  }),
  submoduleSync: op('Sync submodule', 'Syncing submodule…', SubmoduleIcon, SUBMODULE),
  submoduleDeinit: op('Deinitialize submodule', 'Deinitializing submodule…', SubmoduleIcon, SUBMODULE),
  submoduleRemove: op('Remove submodule', 'Removing submodule…', SubmoduleIcon, SUBMODULE),
  worktreeAdd: op('Add worktree', 'Adding worktree…', WorktreeIcon, WORKTREE),
  worktreeRemove: op('Remove worktree', 'Removing worktree…', WorktreeIcon, WORKTREE),
  worktreeLock: op('Lock worktree', 'Locking worktree…', WorktreeIcon, WORKTREE),
  worktreeUnlock: op('Unlock worktree', 'Unlocking worktree…', WorktreeIcon, WORKTREE),
  discard: op('Discard changes', 'Discarding changes…', RevertIcon, 'change', {
    countNoun: (n) => `Discard changes in ${n.toLocaleString()} files`,
  }),
  bisectStart: op('Start bisect', 'Starting bisect…', BisectIcon, 'bisect'),
  bisectGood: op('Mark good', 'Marking…', BisectIcon, 'bisect'),
  bisectBad: op('Mark bad', 'Marking…', BisectIcon, 'bisect'),
  bisectSkip: op('Skip commit', 'Skipping…', BisectIcon, 'bisect'),
  bisectReset: op('End bisect', 'Ending bisect…', BisectIcon, 'bisect'),
  composeCommits: op('Apply composed commits', 'Applying commits…', RefDotIcon, 'commit'),
  cloneRepo: op('Clone', 'Cloning…', CloneIcon, 'clone', { networkLabel: 'Receiving objects…' }),
  initRepo: op('Create repository', 'Creating repository…', InitRepoIcon, 'repository creation'),
  addRemote: op('Add remote', 'Adding remote…', CloudIcon, REMOTE),
  removeRemote: op('Remove remote', 'Removing remote…', CloudIcon, REMOTE),
  renameRemote: op('Rename remote', 'Renaming remote…', CloudIcon, REMOTE),
  setRemoteUrl: op('Change remote URL', 'Changing remote URL…', CloudIcon, REMOTE),
};

/** The accessible-name joiner (P119-ui §4.2-3); `null` = direct object. */
export type TargetPreposition =
  | 'to'
  | 'from'
  | 'on'
  | 'onto'
  | 'into'
  | 'in'
  | 'at'
  | 'for'
  | null;

/** §3.7 — the preposition that joins the noun to the target in the ACCESSIBLE
 *  name only, or `null` = direct object (`Merge feature/x`). Keyed exhaustively
 *  by category so a new category cannot silently miss one. The visible row
 *  stays preposition-free (§3.2). Values = the P119-ui §4.3 `Prep` column. */
export const TARGET_PREPOSITION: Record<GitActivityCategory, TargetPreposition> = {
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
  renameBranch: 'to',
  deleteRemoteTracking: null,
  merge: null,
  abortMerge: 'on',
  rebase: 'onto',
  interactiveRebase: 'onto',
  rebaseContinue: 'on',
  rebaseSkip: 'on',
  rebaseAbort: 'on',
  cherryPick: null,
  cherryPickContinue: 'on',
  cherryPickAbort: 'on',
  revert: null,
  revertContinue: 'on',
  revertAbort: 'on',
  resetSoft: 'to',
  resetMixed: 'to',
  resetHard: 'to',
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
  bisectStart: 'at',
  bisectGood: null,
  bisectBad: null,
  bisectSkip: null,
  bisectReset: null,
  composeCommits: 'on',
  cloneRepo: 'into',
  initRepo: 'in',
  addRemote: null,
  removeRemote: null,
  renameRemote: 'to',
  setRemoteUrl: 'for',
};

/** Every category, in the Rust enum's declaration order (`GitActivityCategory::ALL`). */
export const GIT_ACTIVITY_CATEGORIES: readonly GitActivityCategory[] = Object.keys(
  CATEGORY_META,
) as GitActivityCategory[];
