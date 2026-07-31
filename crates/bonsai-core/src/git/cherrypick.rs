//! Cherry-pick a single commit onto the current branch (P20 contract §5).
//! Clean picks auto-commit (reusing the picked commit's message + ORIGINAL
//! author, fresh committer, like git); conflicts pause into
//! `RepoOpState::CherryPick` and flow through the EXISTING conflict.rs /
//! opstate.rs / OpBanner framework — no parallel conflict code.
//!
//! v1 LIMITATION (contract §11 OPEN #9): Bonsai only ever *starts* a SINGLE
//! cherry-pick. git2 has no sequencer support, so a CLI-started multi-commit
//! `git cherry-pick A..B` (whose `.git/sequencer` todo lists several picks) is
//! NOT advanced by `cherrypick_continue` — we commit the one in-progress pick
//! and `cleanup_state`. The banner still lets the user finish/abort the current
//! step.
//!
//! Pure git2, no Tauri types, no network.

use std::path::Path;

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

/// Wire: tagged "kind", camelCase (identical recipe to `MergeOutcome`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CherrypickOutcome {
    /// Clean pick, auto-committed. `oid` = the new commit.
    Committed { oid: String },
    /// Index/worktree hold conflict markers; CHERRY_PICK_HEAD written; repo
    /// paused in state CherryPick. `paths` = sorted conflicted paths (the exact
    /// set `list_conflicts` returns).
    Conflicts { paths: Vec<String> },
}

/// Normalize a reused commit message exactly like `create_commit`: CRLF/CR →
/// `\n`, trim, then a single trailing newline. Empty after trim → empty string
/// (callers treat that as "reuse verbatim"; git never produces an empty pick
/// message so this is defensive only).
fn normalize_message(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    format!("{trimmed}\n")
}

/// Shared finalize for the clean path AND `cherrypick_continue`: read the picked
/// oid from CHERRY_PICK_HEAD, commit the current (resolved) index reusing the
/// picked commit's message + ORIGINAL author with the supplied fresh committer,
/// then `cleanup_state`. Parallels `merge::finalize_merge_commit`.
fn finalize_cherrypick(
    repo: &git2::Repository,
    committer: &git2::Signature,
) -> Result<CherrypickOutcome, AppError> {
    let head_path = repo.path().join("CHERRY_PICK_HEAD");
    let raw = std::fs::read_to_string(&head_path)
        .map_err(|_| AppError::Git("CHERRY_PICK_HEAD missing".to_string()))?;
    let pick_oid = git2::Oid::from_str(raw.trim())
        .map_err(|_| AppError::Git("CHERRY_PICK_HEAD is not a valid oid".to_string()))?;
    let pick = repo.find_commit(pick_oid)?;

    let author = pick.author().to_owned();
    let message = normalize_message(pick.message().unwrap_or(""));

    let head_commit = repo.head()?.peel_to_commit()?;
    let tree = repo.find_tree(repo.index()?.write_tree()?)?;

    // Empty guard (contract §5.1 step 4 / OPEN #6): a pick that produces no net
    // change is git's default refusal — clean up the sequencer state first.
    if tree.id() == head_commit.tree_id() {
        repo.cleanup_state()?;
        return Err(AppError::NothingToCommit);
    }

    let new = repo.commit(
        Some("HEAD"),
        &author,
        committer,
        &message,
        &tree,
        &[&head_commit],
    )?;
    repo.cleanup_state()?; // removes CHERRY_PICK_HEAD → state Clean
    Ok(CherrypickOutcome::Committed {
        oid: new.to_string(),
    })
}

/// Blocking. Cherry-picks `oid` onto the current branch. Clean → commit
/// immediately; conflict → pause for the OpBanner/conflict.rs flow.
/// Preconditions are all checked BEFORE any mutation (merge/rebase pattern).
pub fn cherrypick_commit(workdir: &Path, oid: &str) -> Result<CherrypickOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — finish or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot cherry-pick: no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git(
            "cannot cherry-pick: HEAD is detached".to_string(),
        ));
    }

    let pick = repo.find_commit(
        git2::Oid::from_str(oid).map_err(|_| AppError::Git("invalid commit id".to_string()))?,
    )?;

    // Dirty-index guard (identical to rebase §3.9 / merge §4.1.5): staged
    // changes or a conflicted index refuse the pick. Unstaged worktree changes
    // are OK — they only fail as CheckoutConflict if the pick would clobber.
    let mut index = repo.index()?;
    let head_commit = repo.head()?.peel_to_commit()?;
    if index.has_conflicts() || index.write_tree_to(&repo)? != head_commit.tree_id() {
        return Err(AppError::Git(
            "cannot cherry-pick: your index contains uncommitted changes — \
             commit or unstage them first"
                .to_string(),
        ));
    }
    drop(index);

    // Identity EARLY (the clean path auto-commits) → ConfigMissing before any
    // worktree mutation.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Mutation: sets index/worktree + writes CHERRY_PICK_HEAD, state →
    // CherryPick. On failure, guarantee a Clean state (mirror merge_branch).
    if let Err(e) = repo.cherrypick(&pick, None) {
        let _ = repo.cleanup_state();
        let mapped = if e.code() == git2::ErrorCode::Conflict {
            AppError::CheckoutConflict(
                "cannot cherry-pick: local changes would be overwritten. \
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
        return Ok(CherrypickOutcome::Conflicts { paths });
    }

    finalize_cherrypick(&repo, &sig)
}

/// Blocking. Finalizes a paused (resolved) cherry-pick — commits the resolved
/// index reusing CHERRY_PICK_HEAD's message/author (parallels `commit_merge`).
/// A HARD error leaves the on-disk state intact (no cleanup), same discipline
/// as `rebase_continue`.
pub fn cherrypick_continue(workdir: &Path) -> Result<CherrypickOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::CherryPick {
        return Err(AppError::NoOperationInProgress(
            "no cherry-pick in progress".to_string(),
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
    finalize_cherrypick(&repo, &sig)
}

/// Blocking. Aborts a paused cherry-pick: reset --hard to HEAD + cleanup_state
/// (git-consistent `cherry-pick --abort`; destructive → the UI confirms first).
pub fn cherrypick_abort(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::CherryPick {
        return Err(AppError::NoOperationInProgress(
            "no cherry-pick in progress".to_string(),
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

    /// The serde tag/casing must match the TS `CherrypickOutcome` union exactly.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(CherrypickOutcome::Committed {
            oid: "a".repeat(40),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "committed", "oid": "a".repeat(40) })
        );

        let v = serde_json::to_value(CherrypickOutcome::Conflicts {
            paths: vec!["README.md".to_string(), "src/app.ts".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["README.md", "src/app.ts"] })
        );
    }

    #[test]
    fn cherrypick_preconditions_on_fresh_repo() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        // Unborn HEAD refuses before target resolution.
        let err = cherrypick_commit(dir.path(), &"0".repeat(40)).expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // continue / abort with no cherry-pick in progress.
        let err = cherrypick_continue(dir.path()).expect_err("no op");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
        let err = cherrypick_abort(dir.path()).expect_err("no op");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
    }
}
