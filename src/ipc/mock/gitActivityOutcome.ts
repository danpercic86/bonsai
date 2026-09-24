/**
 * P119 §5.1 — the mock's mirrors of the §2.6 outcome classifiers
 * (`crates/bonsai-core/src/git/activity_outcome.rs`) over the TS result types.
 *
 * Each is `(result) => GitRunOutcome | null` and CONFLICTS WINS. Every switch is
 * exhaustive with no `default`, so a new result variant fails to compile here
 * exactly as it does in Rust.
 */
import type {
  ApplyStashOutcome,
  CheckoutResult,
  CherrypickOutcome,
  CreateBranchHereResult,
  GitActivityCategory,
  GitRunOutcome,
  MergeOutcome,
  RebaseOutcome,
  RevertOutcome,
} from '../types';

/** `merge_outcome`. */
export function mockMergeOutcome(r: MergeOutcome): GitRunOutcome | null {
  switch (r.kind) {
    case 'upToDate':
      return 'upToDate';
    case 'fastForwarded':
      return 'fastForwarded';
    case 'merged':
      return 'merged';
    case 'conflicts':
    case 'stashPopConflicts':
      return 'conflicts';
  }
}

/** `rebase_outcome` — a completed replay (`rebased`) adds nothing. */
export function mockRebaseOutcome(r: RebaseOutcome): GitRunOutcome | null {
  switch (r.kind) {
    case 'upToDate':
      return 'upToDate';
    case 'fastForwarded':
      return 'fastForwarded';
    case 'rebased':
      return null;
    case 'conflicts':
      return 'conflicts';
  }
}

/** `cherrypick_outcome` / `revert_outcome` (identical shapes). */
export function mockPickOutcome(r: CherrypickOutcome | RevertOutcome): GitRunOutcome | null {
  switch (r.kind) {
    case 'committed':
      return null;
    case 'conflicts':
    case 'stashPopConflicts':
      return 'conflicts';
  }
}

/** `stash_apply_outcome` — only a conflicted apply is more than "done". */
export function mockStashApplyOutcome(r: ApplyStashOutcome): GitRunOutcome | null {
  return r.kind === 'conflicts' ? 'conflicts' : null;
}

/** `checkout_outcome` — the stash re-apply's conflicts win over the auto-FF. */
export function mockCheckoutOutcome(r: CheckoutResult): GitRunOutcome | null {
  if (r.apply?.kind === 'conflicts') return 'conflicts';
  return r.fastForwarded ? 'fastForwarded' : null;
}

/** `create_here_outcome`. */
export function mockCreateHereOutcome(r: CreateBranchHereResult): GitRunOutcome | null {
  return r.apply?.kind === 'conflicts' ? 'conflicts' : null;
}

/** The categories whose command has a conflict-capable classifier (§1 `Out`
 *  column) — the only ones `?gitConflicts` may force to `outcome: 'conflicts'`
 *  (forcing it on, say, `deleteTag` would show a state the app cannot produce). */
export const MOCK_CONFLICT_CAPABLE: ReadonlySet<GitActivityCategory> = new Set<GitActivityCategory>(
  [
    'checkoutBranch',
    'checkoutCommit',
    'createBranch',
    'merge',
    'rebase',
    'interactiveRebase',
    'rebaseContinue',
    'rebaseSkip',
    'cherryPick',
    'cherryPickContinue',
    'revert',
    'revertContinue',
    'stashApply',
    'stashPop',
  ],
);
