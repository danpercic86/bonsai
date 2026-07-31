//! Revert a single commit on the current branch (P20 contract §6).
//! Structurally identical to `cherrypick.rs` with `repo.revert` + REVERT_HEAD.
//! Clean reverts auto-commit (the revert is authored as YOU — both author and
//! committer are the current signature, like `git revert`); conflicts pause into
//! `RepoOpState::Revert` and flow through the EXISTING conflict.rs / opstate.rs /
//! OpBanner framework — no parallel conflict code.
//!
//! v1 LIMITATION (contract §11 OPEN #9): Bonsai only ever *starts* a SINGLE
//! revert. git2 has no sequencer support, so a CLI-started multi-commit
//! `git revert A..B` sequence is NOT advanced by `revert_continue` — we commit
//! the one in-progress revert and `cleanup_state`.
//!
//! Pure git2, no Tauri types, no network.

use std::path::Path;

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

/// Wire: tagged "kind", camelCase (identical recipe to `CherrypickOutcome`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RevertOutcome {
    /// Clean revert, auto-committed. `oid` = the new commit.
    Committed { oid: String },
    /// Index/worktree hold conflict markers; REVERT_HEAD written; repo paused
    /// in state Revert. `paths` = sorted conflicted paths.
    Conflicts { paths: Vec<String> },
}

/// Byte-exact `git revert --no-edit` message (contract §6): first line of the
/// reverted commit's message becomes `<subject>`, its full 40-hex id the oid.
fn revert_message(reverted: &git2::Commit) -> String {
    let subject = reverted
        .message()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");
    format!("Revert \"{subject}\"\n\nThis reverts commit {}.\n", reverted.id())
}

/// Shared finalize for the clean path AND `revert_continue`: read the reverted
/// oid from REVERT_HEAD, commit the current (resolved) index with the `git
/// revert --no-edit` message, authoring BOTH author and committer as the
/// supplied current signature. Parallels `cherrypick::finalize_cherrypick`.
fn finalize_revert(
    repo: &git2::Repository,
    sig: &git2::Signature,
) -> Result<RevertOutcome, AppError> {
    let head_path = repo.path().join("REVERT_HEAD");
    let raw = std::fs::read_to_string(&head_path)
        .map_err(|_| AppError::Git("REVERT_HEAD missing".to_string()))?;
    let reverted_oid = git2::Oid::from_str(raw.trim())
        .map_err(|_| AppError::Git("REVERT_HEAD is not a valid oid".to_string()))?;
    let reverted = repo.find_commit(reverted_oid)?;

    let message = revert_message(&reverted);

    let head_commit = repo.head()?.peel_to_commit()?;
    let tree = repo.find_tree(repo.index()?.write_tree()?)?;

    // Empty guard (contract §6 / OPEN #6): a revert with no net change is git's
    // default refusal — clean up the sequencer state first.
    if tree.id() == head_commit.tree_id() {
        repo.cleanup_state()?;
        return Err(AppError::NothingToCommit);
    }

    let new = repo.commit(Some("HEAD"), sig, sig, &message, &tree, &[&head_commit])?;
    repo.cleanup_state()?; // removes REVERT_HEAD → state Clean
    Ok(RevertOutcome::Committed {
        oid: new.to_string(),
    })
}

/// Blocking. Reverts `oid` on the current branch. Clean → commit immediately;
/// conflict → pause for the OpBanner/conflict.rs flow. Preconditions all before
/// any mutation (merge/rebase pattern).
pub fn revert_commit(workdir: &Path, oid: &str) -> Result<RevertOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — finish or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git("cannot revert: no commits yet".to_string()));
    }
    if head.detached {
        return Err(AppError::Git("cannot revert: HEAD is detached".to_string()));
    }

    let target = repo.find_commit(
        git2::Oid::from_str(oid).map_err(|_| AppError::Git("invalid commit id".to_string()))?,
    )?;

    // Dirty-index guard (identical to cherry-pick / rebase / merge).
    let mut index = repo.index()?;
    let head_commit = repo.head()?.peel_to_commit()?;
    if index.has_conflicts() || index.write_tree_to(&repo)? != head_commit.tree_id() {
        return Err(AppError::Git(
            "cannot revert: your index contains uncommitted changes — \
             commit or unstage them first"
                .to_string(),
        ));
    }
    drop(index);

    // Identity EARLY (the clean path auto-commits).
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Mutation: sets index/worktree + writes REVERT_HEAD, state → Revert. On
    // failure, guarantee a Clean state (mirror merge_branch).
    if let Err(e) = repo.revert(&target, None) {
        let _ = repo.cleanup_state();
        let mapped = if e.code() == git2::ErrorCode::Conflict {
            AppError::CheckoutConflict(
                "cannot revert: local changes would be overwritten. \
                 Commit or discard them first."
                    .to_string(),
            )
        } else {
            e.into()
        };
        return Err(mapped);
    }

    if repo.index()?.has_conflicts() {
        let paths: Vec<String> = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
        return Ok(RevertOutcome::Conflicts { paths });
    }

    finalize_revert(&repo, &sig)
}

/// Blocking. Finalizes a paused (resolved) revert — commits the resolved index
/// reusing REVERT_HEAD (parallels `commit_merge`). A HARD error leaves the
/// on-disk state intact.
pub fn revert_continue(workdir: &Path) -> Result<RevertOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Revert {
        return Err(AppError::NoOperationInProgress(
            "no revert in progress".to_string(),
        ));
    }
    let index = repo.index()?;
    if index.has_conflicts() {
        let n = index.conflicts()?.count();
        return Err(AppError::UnresolvedConflicts(format!(
            "cannot continue: {n} unresolved conflict(s) remain"
        )));
    }
    drop(index);

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;
    finalize_revert(&repo, &sig)
}

/// Blocking. Aborts a paused revert: reset --hard to HEAD + cleanup_state
/// (git-consistent `revert --abort`; destructive → the UI confirms first).
pub fn revert_abort(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Revert {
        return Err(AppError::NoOperationInProgress(
            "no revert in progress".to_string(),
        ));
    }

    let head_obj = repo.head()?.peel_to_commit()?.into_object();
    repo.reset(&head_obj, git2::ResetType::Hard, None)?;
    repo.cleanup_state()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The serde tag/casing must match the TS `RevertOutcome` union exactly.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(RevertOutcome::Committed {
            oid: "b".repeat(40),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "committed", "oid": "b".repeat(40) })
        );

        let v = serde_json::to_value(RevertOutcome::Conflicts {
            paths: vec!["src/app.ts".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["src/app.ts"] })
        );
    }

    #[test]
    fn revert_preconditions_on_fresh_repo() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        let err = revert_commit(dir.path(), &"0".repeat(40)).expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        let err = revert_continue(dir.path()).expect_err("no op");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
        let err = revert_abort(dir.path()).expect_err("no op");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
    }
}
