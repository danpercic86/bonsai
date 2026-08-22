//! P87 activity-recording merge-commit core (split from `merge.rs` for the
//! ~500-line limit).
//!
//! `commit_merge_with_activity` is [`super::merge`]'s `commit_merge` PLUS an
//! optional [`GitActivityRecorder`] (category `MergeCommit`): it emits phase
//! transitions + streams hook output via the shared `finalize_merge_commit`.
//! `activity == None` is the **byte-for-byte** pre-P87 path.

use std::path::Path;

use crate::error::AppError;
use crate::git::activity::GitActivityRecorder;
use crate::git::commit::CommitResult;
use crate::git::hooks::hooks_enabled;
use crate::git::merge::{finalize_merge_commit, MergeHooks};
use crate::git::stage::open_workdir_repo;

/// See the module doc. `activity == None` ≡ [`super::merge::commit_merge`].
pub fn commit_merge_with_activity(
    workdir: &Path,
    message: &str,
    sign: Option<bool>,
    skip_hooks: bool,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<CommitResult, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Merge {
        return Err(AppError::NoOperationInProgress(
            "no merge in progress".to_string(),
        ));
    }
    let index = repo.index()?;
    if index.has_conflicts() {
        let n = index.conflicts()?.count();
        return Err(AppError::UnresolvedConflicts(format!(
            "cannot commit: {n} unresolved conflict(s) remain"
        )));
    }
    let hooks = if hooks_enabled(&repo.config()?.snapshot()?, skip_hooks) {
        MergeHooks::Full
    } else {
        MergeHooks::Off
    };
    finalize_merge_commit(&mut repo, message, sign, hooks, activity)
}
