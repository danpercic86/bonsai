//! P119 §2.6: pure classifiers from a command's own typed result to the
//! `finished.outcome` of its activity run.
//!
//! "Paused on conflicts" is detected from the result the command already
//! returned — no extra repo read, no string matching. Every classifier matches
//! its input EXHAUSTIVELY (no `_` arm), so a new result variant fails to compile
//! until someone decides its outcome. Conflicts always wins.

use crate::git::activity::GitRunOutcome;
use crate::git::branches::{CheckoutResult, CreateBranchHereResult};
use crate::git::cherrypick::CherrypickOutcome;
use crate::git::merge::MergeOutcome;
use crate::git::rebase::RebaseOutcome;
use crate::git::revert::RevertOutcome;
use crate::git::stash::ApplyStashOutcome;

/// A classifier for a command that has nothing to add beyond success/failure.
pub fn no_outcome<T>(_: &T) -> Option<GitRunOutcome> {
    None
}

pub fn merge_outcome(o: &MergeOutcome) -> Option<GitRunOutcome> {
    match o {
        MergeOutcome::UpToDate => Some(GitRunOutcome::UpToDate),
        MergeOutcome::FastForwarded { .. } => Some(GitRunOutcome::FastForwarded),
        MergeOutcome::Merged { .. } => Some(GitRunOutcome::Merged),
        MergeOutcome::Conflicts { .. } | MergeOutcome::StashPopConflicts { .. } => {
            Some(GitRunOutcome::Conflicts)
        }
    }
}

/// `Rebased` is the plain "done" case, so it carries no outcome.
pub fn rebase_outcome(o: &RebaseOutcome) -> Option<GitRunOutcome> {
    match o {
        RebaseOutcome::UpToDate => Some(GitRunOutcome::UpToDate),
        RebaseOutcome::FastForwarded { .. } => Some(GitRunOutcome::FastForwarded),
        RebaseOutcome::Rebased { .. } => None,
        RebaseOutcome::Conflicts { .. } => Some(GitRunOutcome::Conflicts),
    }
}

pub fn cherrypick_outcome(o: &CherrypickOutcome) -> Option<GitRunOutcome> {
    match o {
        CherrypickOutcome::Committed { .. } => None,
        CherrypickOutcome::Conflicts { .. } | CherrypickOutcome::StashPopConflicts { .. } => {
            Some(GitRunOutcome::Conflicts)
        }
    }
}

pub fn revert_outcome(o: &RevertOutcome) -> Option<GitRunOutcome> {
    match o {
        RevertOutcome::Committed { .. } => None,
        RevertOutcome::Conflicts { .. } | RevertOutcome::StashPopConflicts { .. } => {
            Some(GitRunOutcome::Conflicts)
        }
    }
}

pub fn stash_apply_outcome(o: &ApplyStashOutcome) -> Option<GitRunOutcome> {
    match o {
        ApplyStashOutcome::Conflicts { .. } => Some(GitRunOutcome::Conflicts),
        ApplyStashOutcome::Applied
        | ApplyStashOutcome::ReservedPaths { .. }
        | ApplyStashOutcome::AppliedSkippingReserved { .. }
        | ApplyStashOutcome::AppliedPartially { .. }
        | ApplyStashOutcome::NotApplied { .. } => None,
    }
}

/// The re-applied autostash conflicting wins over the auto fast-forward.
pub fn checkout_outcome(r: &CheckoutResult) -> Option<GitRunOutcome> {
    if r.apply.as_ref().and_then(stash_apply_outcome) == Some(GitRunOutcome::Conflicts) {
        return Some(GitRunOutcome::Conflicts);
    }
    r.fast_forwarded.then_some(GitRunOutcome::FastForwarded)
}

pub fn create_here_outcome(r: &CreateBranchHereResult) -> Option<GitRunOutcome> {
    r.apply.as_ref().and_then(stash_apply_outcome)
}

#[cfg(test)]
#[path = "activity_outcome_tests.rs"]
mod tests;
