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
use crate::git::autostash::{self, PopResult};
use crate::git::bisect::require_no_bisect;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

/// Wire: tagged "kind", camelCase (identical recipe to `MergeOutcome`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CherrypickOutcome {
    /// Clean pick, auto-committed. `oid` = the new commit.
    /// `stashed` = an autostash was created AND restored for this pick.
    Committed { oid: String, stashed: bool },
    /// Index/worktree hold conflict markers; CHERRY_PICK_HEAD (+ MERGE_MSG when
    /// a message override was supplied) written; repo paused in state
    /// CherryPick. `paths` = sorted conflicted paths (the exact set
    /// `list_conflicts` returns). `stashed` = an autostash was created and is
    /// RETAINED on the stack (deferred re-apply, same as merge).
    Conflicts { paths: Vec<String>, stashed: bool },
    /// The pick committed cleanly, but re-applying the autostash conflicted.
    /// The stash is RETAINED at stash@{0}. `head` = the new commit oid; `paths`
    /// = conflicted paths from the stash apply.
    StashPopConflicts { head: String, paths: Vec<String> },
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

/// Read `.git/MERGE_MSG` as an override message, returning `Some(normalized)`
/// only when the file exists and is non-empty after normalization. Used to carry
/// a custom pick message across a conflict pause (persisted at pick time). A
/// missing / empty file → `None` (fall back to the picked commit's message).
fn read_merge_msg_override(repo: &git2::Repository) -> Option<String> {
    let raw = std::fs::read_to_string(repo.path().join("MERGE_MSG")).ok()?;
    let normalized = normalize_message(&raw);
    if normalized.trim().is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// Shared finalize for the clean path AND `cherrypick_continue`: read the picked
/// oid from CHERRY_PICK_HEAD, commit the current (resolved) index preserving the
/// picked commit's ORIGINAL author with the supplied fresh committer, then
/// `cleanup_state`. Parallels `merge::finalize_merge_commit`.
///
/// Message precedence (contract §2.2):
///   1. `message` Some(m)                          -> normalize_message(m)
///   2. else .git/MERGE_MSG present & non-empty     -> that (continue path)
///   3. else the picked commit's original message   -> normalize(pick.message)
fn finalize_cherrypick(
    repo: &git2::Repository,
    committer: &git2::Signature,
    message: Option<&str>,
) -> Result<CherrypickOutcome, AppError> {
    let head_path = repo.path().join("CHERRY_PICK_HEAD");
    let raw = std::fs::read_to_string(&head_path)
        .map_err(|_| AppError::Git("CHERRY_PICK_HEAD missing".to_string()))?;
    let pick_oid = git2::Oid::from_str(raw.trim())
        .map_err(|_| AppError::Git("CHERRY_PICK_HEAD is not a valid oid".to_string()))?;
    let pick = repo.find_commit(pick_oid)?;

    let author = pick.author().to_owned();
    let message = match message {
        Some(m) => normalize_message(m),
        None => read_merge_msg_override(repo)
            .unwrap_or_else(|| normalize_message(pick.message().unwrap_or(""))),
    };

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
        stashed: false,
    })
}

/// Blocking. Cherry-picks `oid` onto the current branch. Clean → commit
/// immediately; conflict → pause for the OpBanner/conflict.rs flow.
///
/// `message`: `None` → reuse the picked commit's message verbatim (P20 behavior,
/// no regression). `Some(m)` → commit with the normalized `m` instead,
/// PRESERVING the original author and using a fresh committer (P20 identity
/// rules). A `Some(m)` override survives a conflict pause via `.git/MERGE_MSG`.
///
/// A dirty TRACKED worktree is autostashed first (mirrors merge); the stash is
/// restored after a clean finalize, RETAINED on any conflict/pop-conflict.
///
/// Preconditions are all checked BEFORE any mutation (merge/rebase pattern).
pub fn cherrypick_commit(
    workdir: &Path,
    oid: &str,
    message: Option<&str>,
) -> Result<CherrypickOutcome, AppError> {
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
        return Err(AppError::Git(
            "cannot cherry-pick: no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git(
            "cannot cherry-pick: HEAD is detached".to_string(),
        ));
    }

    // Validate the oid resolves BEFORE any mutation; the borrow ends here so
    // the later &mut autostash calls are legal (mirrors merge_branch).
    let pick_id = {
        let oid = git2::Oid::from_str(oid)
            .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
        repo.find_commit(oid)?.id()
    };

    // A conflicted index cannot be stashed safely — refuse before any mutation.
    // (Unstaged/staged tracked changes are now autostashed rather than refused.)
    if repo.index()?.has_conflicts() {
        return Err(AppError::Git(
            "cannot cherry-pick: your index has unresolved conflicts".to_string(),
        ));
    }

    // Identity EARLY (the clean path auto-commits) → ConfigMissing before any
    // worktree mutation. `sig` is also the autostash stasher identity.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Autostash a dirty TRACKED worktree (mirrors merge) so the pick checkout
    // cannot clobber the user's edits.
    let stashed = if autostash::is_dirty(&repo)? {
        autostash::stash_save(&mut repo, &sig, "bonsai: autostash before cherry-pick")?;
        true
    } else {
        false
    };

    // Mutation: sets index/worktree + writes CHERRY_PICK_HEAD, state →
    // CherryPick. On failure, guarantee a Clean state (mirror merge_branch) and
    // roll back the autostash. Scope the re-found commit so its borrow ends
    // before the &mut rollback/pop calls below.
    let pick_res = {
        let pick = repo.find_commit(pick_id)?;
        repo.cherrypick(&pick, None)
    };
    if let Err(e) = pick_res {
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
        return Err(autostash::rollback_and_map(&mut repo, stashed, mapped));
    }

    if repo.index()?.has_conflicts() {
        let paths: Vec<String> = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
        // Persist a custom message so it survives the pause and is honored by
        // cherrypick_continue via MERGE_MSG (§2.2). No override → leave whatever
        // libgit2 wrote (the picked commit's message).
        if let Some(m) = message {
            std::fs::write(repo.path().join("MERGE_MSG"), normalize_message(m))?;
        }
        // PAUSE. Do NOT pop — the autostash (if any) is RETAINED on the stack.
        return Ok(CherrypickOutcome::Conflicts { paths, stashed });
    }

    let outcome = finalize_cherrypick(&repo, &sig, message)?;
    let oid = match outcome {
        CherrypickOutcome::Committed { oid, .. } => oid,
        other => return Ok(other),
    };
    if stashed {
        return Ok(match autostash::pop_after_success(&mut repo, workdir)? {
            PopResult::Restored => CherrypickOutcome::Committed {
                oid,
                stashed: true,
            },
            PopResult::Conflicted(paths) => CherrypickOutcome::StashPopConflicts {
                head: oid,
                paths,
            },
        });
    }
    Ok(CherrypickOutcome::Committed {
        oid,
        stashed: false,
    })
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
    // message = None → finalize honors a persisted MERGE_MSG override, else the
    // picked commit's message. Does NOT auto-pop a retained autostash (F5).
    finalize_cherrypick(&repo, &sig, None)
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
            stashed: true,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "committed", "oid": "a".repeat(40), "stashed": true })
        );

        let v = serde_json::to_value(CherrypickOutcome::Conflicts {
            paths: vec!["README.md".to_string(), "src/app.ts".to_string()],
            stashed: false,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["README.md", "src/app.ts"], "stashed": false })
        );

        let v = serde_json::to_value(CherrypickOutcome::StashPopConflicts {
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
    fn cherrypick_preconditions_on_fresh_repo() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        // Unborn HEAD refuses before target resolution.
        let err = cherrypick_commit(dir.path(), &"0".repeat(40), None).expect_err("unborn");
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
