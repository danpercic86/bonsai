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
use crate::git::autostash::{self, PopResult};
use crate::git::bisect::require_no_bisect;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

/// Wire: tagged "kind", camelCase (identical recipe to `CherrypickOutcome`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RevertOutcome {
    /// Clean revert, auto-committed. `oid` = the new commit.
    /// `stashed` = an autostash was created AND restored for this revert.
    Committed { oid: String, stashed: bool },
    /// Index/worktree hold conflict markers; REVERT_HEAD written; repo paused
    /// in state Revert. `paths` = sorted conflicted paths. `stashed` = an
    /// autostash was created and is RETAINED on the stack.
    Conflicts { paths: Vec<String>, stashed: bool },
    /// The revert committed cleanly, but re-applying the autostash conflicted.
    /// The stash is RETAINED at stash@{0}. `head` = the new commit oid; `paths`
    /// = conflicted paths from the stash apply.
    StashPopConflicts { head: String, paths: Vec<String> },
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
        stashed: false,
    })
}

/// Blocking. Reverts `oid` on the current branch. Clean → commit immediately;
/// conflict → pause for the OpBanner/conflict.rs flow. Preconditions all before
/// any mutation (merge/rebase pattern).
///
/// A dirty TRACKED worktree is autostashed first (mirrors merge / cherry-pick);
/// the stash is restored after a clean finalize, RETAINED on any conflict /
/// pop-conflict. Revert keeps its deterministic message (no override, F2).
pub fn revert_commit(workdir: &Path, oid: &str) -> Result<RevertOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

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

    // Validate the oid resolves BEFORE any mutation; the borrow ends here so
    // the later &mut autostash calls are legal (mirrors merge_branch).
    let target_id = {
        let oid = git2::Oid::from_str(oid)
            .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
        repo.find_commit(oid)?.id()
    };

    // A conflicted index cannot be stashed safely — refuse before any mutation.
    // (Unstaged/staged tracked changes are now autostashed rather than refused.)
    if repo.index()?.has_conflicts() {
        return Err(AppError::Git(
            "cannot revert: your index has unresolved conflicts".to_string(),
        ));
    }

    // Identity EARLY (the clean path auto-commits). Also the autostash stasher.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Autostash a dirty TRACKED worktree (mirrors merge / cherry-pick).
    let stashed = if autostash::is_dirty(&repo)? {
        autostash::stash_save(&mut repo, &sig, "bonsai: autostash before revert")?;
        true
    } else {
        false
    };

    // Mutation: sets index/worktree + writes REVERT_HEAD, state → Revert. On
    // failure, guarantee a Clean state (mirror merge_branch) and roll back the
    // autostash. Scope the re-found commit so its borrow ends before the &mut
    // rollback/pop calls below.
    let revert_res = {
        let target = repo.find_commit(target_id)?;
        repo.revert(&target, None)
    };
    if let Err(e) = revert_res {
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
        return Err(autostash::rollback_and_map(&mut repo, stashed, mapped));
    }

    if repo.index()?.has_conflicts() {
        let paths: Vec<String> = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
        // PAUSE. Do NOT pop — the autostash (if any) is RETAINED on the stack.
        return Ok(RevertOutcome::Conflicts { paths, stashed });
    }

    let outcome = finalize_revert(&repo, &sig)?;
    let oid = match outcome {
        RevertOutcome::Committed { oid, .. } => oid,
        other => return Ok(other),
    };
    if stashed {
        return Ok(match autostash::pop_after_success(&mut repo, workdir)? {
            PopResult::Restored => RevertOutcome::Committed {
                oid,
                stashed: true,
            },
            PopResult::Conflicted(paths) => RevertOutcome::StashPopConflicts {
                head: oid,
                paths,
            },
        });
    }
    Ok(RevertOutcome::Committed {
        oid,
        stashed: false,
    })
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
            stashed: false,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "committed", "oid": "b".repeat(40), "stashed": false })
        );

        let v = serde_json::to_value(RevertOutcome::Conflicts {
            paths: vec!["src/app.ts".to_string()],
            stashed: true,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["src/app.ts"], "stashed": true })
        );

        let v = serde_json::to_value(RevertOutcome::StashPopConflicts {
            head: "c".repeat(40),
            paths: vec!["src/app.ts".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "stashPopConflicts", "head": "c".repeat(40), "paths": ["src/app.ts"] })
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
