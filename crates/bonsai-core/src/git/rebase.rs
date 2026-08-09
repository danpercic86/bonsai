//! Plain non-interactive rebase of the current branch onto a target (local or
//! remote-tracking). Clean rebases replay + finish automatically; conflicts
//! pause into RepoOpState::Rebase and reuse git/conflict.rs verbatim. Pure
//! git2, no Tauri types, no network (rebasing onto origin/x uses the local
//! remote-tracking ref — Fetch first is the user's job, same as merge). (P3d §3.)

use std::path::Path;

use crate::error::AppError;
use crate::git::bisect::require_no_bisect;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::{ensure_no_untracked_collision, open_workdir_repo};

/// Shared message for a checkout/base conflict (the initial base checkout or a
/// mid-replay checkout that would overwrite local changes). Byte-identical to
/// merge's string save for the operation verb, for the oracle.
const CONFLICT_MSG: &str =
    "cannot rebase: local changes would be overwritten. Commit or discard them first.";

/// Wire: tagged "kind", camelCase (identical recipe to MergeOutcome, P3c §4).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RebaseOutcome {
    /// `onto` is already an ancestor of HEAD (branch already based on it, or
    /// ahead) — nothing to replay. HEAD unmoved.
    UpToDate,
    /// HEAD was an ancestor of `onto`: the branch was fast-forwarded to `onto`
    /// (full oid `to`). No commits were rewritten.
    FastForwarded { branch: String, to: String },
    /// Rebase ran to completion (rebase.finish()). `branch` = the rebased
    /// branch, `head` = its new tip (full oid), `steps` = number of operations
    /// in the plan (rebase.len(); dropped-empty picks are still counted).
    /// `warnings` = non-fatal notes to toast (currently only from interactive
    /// rebase: a Reword whose pick became empty and was dropped). Always empty
    /// for plain rebase; `#[serde(default)]` keeps it optional on the wire.
    Rebased {
        branch: String,
        head: String,
        steps: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        warnings: Vec<String>,
    },
    /// Replay paused on a conflict. Index + worktree hold the conflict markers;
    /// on-disk rebase-merge state persists. `paths` = sorted conflicted paths
    /// (same set list_conflicts returns); `current_step`/`total_steps` mirror
    /// the git msgnum/end (1-based current, total).
    Conflicts {
        paths: Vec<String>,
        current_step: u32,
        total_steps: u32,
    },
}

/// Result of driving the rebase plan to completion or to the next pause.
enum DriveResult {
    Completed {
        head: String,
        steps: u32,
    },
    Paused {
        paths: Vec<String>,
        current_step: u32,
        total_steps: u32,
    },
}

/// Maps a git2 error raised while starting or driving a rebase: a `Conflict`
/// code means the checkout would overwrite local changes (surface the friendly
/// CheckoutConflict), everything else falls through to the generic `From`
/// (`AppError::Git`). Applied at the git2-error origin because `AppError` does
/// not carry git2 codes.
fn map_conflict(e: git2::Error) -> AppError {
    if e.code() == git2::ErrorCode::Conflict {
        AppError::CheckoutConflict(CONFLICT_MSG.to_string())
    } else {
        e.into()
    }
}

/// 1-based current step (== git `msgnum`) and total (== `end`).
fn steps(rebase: &mut git2::Rebase<'_>) -> (u32, u32) {
    let current = rebase
        .operation_current()
        .map(|c| c as u32 + 1)
        .unwrap_or(0);
    let total = rebase.len() as u32;
    (current, total)
}

/// Commits the CURRENT rebase operation, preserving its original author (the
/// `None` author reuses the pick's name/email/author-time) and stamping the
/// resolved `committer`. An `ErrorCode::Applied` means the pick became EMPTY
/// (its change is already on the new base) → DROP it, matching default
/// `git rebase`. (§3.6)
fn commit_current(
    rebase: &mut git2::Rebase<'_>,
    committer: &git2::Signature<'_>,
) -> Result<(), AppError> {
    match rebase.commit(None, committer, None) {
        Ok(_) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Applied => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Drives the plan: apply each patch, pause on the first conflict (KEEPING the
/// on-disk rebase state — that is NOT an error), else commit it and continue;
/// when the plan is exhausted, `finish()` reattaches HEAD and moves the branch
/// ref. `committer` is stamped on every replayed commit and on `finish()`. (§3.4)
fn run_rebase_loop(
    workdir: &Path,
    repo: &git2::Repository,
    rebase: &mut git2::Rebase<'_>,
    committer: &git2::Signature<'_>,
) -> Result<DriveResult, AppError> {
    loop {
        match rebase.next() {
            None => break,                                 // plan exhausted
            Some(Err(e)) => return Err(map_conflict(e)),   // caller decides abort-vs-keep
            Some(Ok(_op)) => {
                // op.kind() is always Pick for plain rebase.
                if repo.index()?.has_conflicts() {
                    let (current_step, total_steps) = steps(rebase);
                    let paths: Vec<String> = list_conflicts(workdir)?
                        .into_iter()
                        .map(|c| c.path)
                        .collect();
                    return Ok(DriveResult::Paused {
                        paths,
                        current_step,
                        total_steps,
                    });
                }
                commit_current(rebase, committer)?;
            }
        }
    }
    // Plan exhausted -> finalize.
    let total = rebase.len() as u32;
    rebase.finish(Some(committer))?;
    let head = repo.head()?.peel_to_commit()?.id().to_string();
    Ok(DriveResult::Completed { head, steps: total })
}

/// START-only cleanup. GUARANTEE: a failed `rebase_branch` restores
/// `RepositoryState::Clean` and leaves no half-initialized rebase. Best-effort,
/// in order; each step ignores its own error. Because §3.1.5 allows a dirty
/// worktree, the only START failure that can touch the worktree is the initial
/// base checkout, which fails atomically as `CheckoutConflict` before any
/// commit is rewritten. (§3.5)
fn cleanup_failed_start(repo: &git2::Repository, head_oid: git2::Oid) {
    if let Ok(mut r) = repo.open_rebase(None) {
        let _ = r.abort(); // normal path: full restore
    }
    if repo.state() != git2::RepositoryState::Clean {
        // belt-and-suspenders
        let _ = repo.cleanup_state();
        if let Ok(commit) = repo.find_commit(head_oid) {
            if let Ok(tree) = commit.tree() {
                if let Ok(mut idx) = repo.index() {
                    let _ = idx.read_tree(&tree);
                    let _ = idx.write();
                }
            }
        }
    }
}

/// True for every on-disk rebase repository state.
fn is_rebase_state(s: git2::RepositoryState) -> bool {
    matches!(
        s,
        git2::RepositoryState::RebaseMerge
            | git2::RepositoryState::Rebase
            | git2::RepositoryState::RebaseInteractive
    )
}

/// Best-effort display name of the branch being rebased: read
/// `rebase-merge/head-name` (strip `refs/heads/`); after a completed `finish()`
/// the file is gone, so fall back to HEAD's branch name. NEVER errors — the
/// `branch` field is display-only (toasts). (§3.7)
fn read_head_from_rebase(repo: &git2::Repository) -> String {
    let head_name_path = repo.path().join("rebase-merge").join("head-name");
    if let Ok(content) = std::fs::read_to_string(&head_name_path) {
        let trimmed = content.trim();
        let name = trimmed.strip_prefix("refs/heads/").unwrap_or(trimmed);
        if !name.is_empty() {
            return name.to_string();
        }
    }
    read_head_info(repo)
        .ok()
        .and_then(|h| h.branch_name)
        .unwrap_or_default()
}

/// Blocking. Starts a rebase of the current branch onto `onto_name` (local
/// shorthand "main" OR remote-tracking shorthand "origin/main").
///
/// Preconditions (§3.1, cheap first, ALL before any mutation): state Clean;
/// HEAD attached + born; onto resolvable (local then remote); index matches
/// HEAD (unstaged worktree changes ARE allowed — they only fail later as
/// CheckoutConflict if the initial base checkout would overwrite them, in
/// which case nothing is left behind, §3.5); git identity configured.
pub fn rebase_branch(workdir: &Path, onto_name: &str) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — commit or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot rebase: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot rebase: HEAD is detached".to_string()));
    }
    let head_branch = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot rebase: HEAD has no branch name".to_string()))?;

    // Resolve onto: local first, then remote-tracking. Rebasing onto the
    // current branch falls out as UpToDate naturally.
    let onto_branch = match repo.find_branch(onto_name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(_) => match repo.find_branch(onto_name, git2::BranchType::Remote) {
            Ok(b) => b,
            Err(_) => {
                return Err(AppError::BranchNotFound(format!(
                    "branch '{onto_name}' not found (local or remote-tracking)"
                )));
            }
        },
    };

    // Dirty-index guard (identical to merge §4.1.5, locked): staged changes or
    // a conflicted index refuse the rebase. Unstaged worktree changes are OK.
    let mut index = repo.index()?;
    let head_commit = repo.head()?.peel_to_commit()?;
    if index.has_conflicts() || index.write_tree_to(&repo)? != head_commit.tree_id() {
        return Err(AppError::Git(
            "cannot rebase: your index contains uncommitted changes — commit or unstage them first"
                .to_string(),
        ));
    }

    // Identity EARLY: replay commits, so ConfigMissing must surface before the
    // worktree is touched. This is the committer for every replayed commit and
    // for rebase.finish().
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Analysis + fast paths (§3.2).
    let onto_commit = onto_branch.get().peel_to_commit()?;
    let head_oid = head_commit.id();
    let onto_oid = onto_commit.id();
    let mb_oid = repo.merge_base(head_oid, onto_oid).ok();

    // Up-to-date: onto == HEAD, or onto is an ancestor of HEAD.
    if onto_oid == head_oid || mb_oid == Some(onto_oid) {
        return Ok(RebaseOutcome::UpToDate);
    }

    // Fast-forward: HEAD is an ancestor of onto -> rebasing yields onto itself.
    // Same safe-FF recipe as merge §4.2 / remote.rs pull_ff (checkout BEFORE
    // set_target).
    if mb_oid == Some(head_oid) {
        let obj = repo.find_object(onto_oid, None)?;
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.safe(); // DEFAULT SAFE MODE — never .force()
        match repo.checkout_tree(&obj, Some(&mut opts)) {
            Ok(()) => {}
            Err(e) if e.code() == git2::ErrorCode::Conflict => {
                return Err(AppError::CheckoutConflict(CONFLICT_MSG.to_string()));
            }
            Err(e) => return Err(e.into()),
        }
        repo.find_reference(&format!("refs/heads/{head_branch}"))?
            .set_target(onto_oid, &format!("rebase {onto_name}: fast-forward"))?;
        return Ok(RebaseOutcome::FastForwarded {
            branch: head_branch,
            to: onto_oid.to_string(),
        });
    }

    // Real rebase (§3.3). branch = HEAD's branch; upstream == onto == onto_name
    // (exactly what `git rebase <onto_name>` does).
    let head_ref = repo.head()?;
    let head_ac = repo.reference_to_annotated_commit(&head_ref)?;
    let onto_ac = repo.reference_to_annotated_commit(onto_branch.get())?;

    // Release every repo-lifetime borrow before the &mut rebase drive (§3.3).
    drop(index);
    drop(head_commit);
    drop(onto_commit);
    drop(onto_branch);
    drop(head_ref);

    // ON-DISK (do NOT set .inmemory(true)) — conflicts must land in the
    // worktree with libgit2's default <<<<<<< markers so conflict.rs sees
    // them, and state must persist across IPC calls.
    let mut opts = git2::RebaseOptions::new();

    let outcome = match repo.rebase(
        Some(&head_ac),
        Some(&onto_ac),
        Some(&onto_ac),
        Some(&mut opts),
    ) {
        Err(e) => {
            // repo.rebase() may have written rebase-merge state before its
            // initial checkout failed. GUARANTEE: a failed START -> Clean.
            cleanup_failed_start(&repo, head_oid);
            Err(map_conflict(e))
        }
        Ok(mut rebase) => match run_rebase_loop(workdir, &repo, &mut rebase, &sig) {
            Ok(DriveResult::Completed { head, steps }) => Ok(RebaseOutcome::Rebased {
                branch: head_branch,
                head,
                steps,
                warnings: Vec::new(),
            }),
            Ok(DriveResult::Paused {
                paths,
                current_step,
                total_steps,
            }) => Ok(RebaseOutcome::Conflicts {
                paths,
                current_step,
                total_steps,
            }),
            Err(e) => {
                let _ = rebase.abort(); // failed START -> Clean
                Err(e)
            }
        },
    };
    outcome
}

/// Blocking. Resumes the paused rebase at `workdir`: commits the current
/// (resolved) operation, then replays until done or the next conflict.
///
/// A HARD error here returns the error and LEAVES the on-disk rebase state
/// intact — it MUST NOT abort or cleanup_state (§3.9): the user has invested
/// resolution work and can retry Continue or explicitly Abort.
pub fn rebase_continue(workdir: &Path) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // Delegate to the Bonsai interactive engine when its sequencer is present
    // (contract §3) — the plain path below is unchanged.
    if crate::git::rebase_interactive::interactive_in_progress(&repo) {
        return crate::git::rebase_interactive::interactive_continue(workdir);
    }

    if !is_rebase_state(repo.state()) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    if repo.index()?.has_conflicts() {
        let n = repo.index()?.conflicts()?.count();
        return Err(AppError::UnresolvedConflicts(format!(
            "cannot continue: {n} unresolved conflict(s) remain"
        )));
    }

    // Valid: Bonsai STARTED this rebase, so open_rebase loads the merge-backend
    // state. Handle is dropped before returning (no Rebase across IPC calls).
    let mut rebase = repo.open_rebase(None)?;
    // Commit the CURRENT resolved op (empty -> drop). HARD error: return Err,
    // do NOT abort (§3.9).
    commit_current(&mut rebase, &sig)?;
    // Capture the branch name BEFORE finish() removes rebase-merge/head-name.
    let head_branch = read_head_from_rebase(&repo);

    match run_rebase_loop(workdir, &repo, &mut rebase, &sig) {
        Ok(DriveResult::Completed { head, steps }) => Ok(RebaseOutcome::Rebased {
            branch: head_branch,
            head,
            steps,
            warnings: Vec::new(),
        }),
        Ok(DriveResult::Paused {
            paths,
            current_step,
            total_steps,
        }) => Ok(RebaseOutcome::Conflicts {
            paths,
            current_step,
            total_steps,
        }),
        Err(e) => Err(e), // leave state intact (§3.9)
    }
}

/// Blocking. Skips the current operation (`git rebase --skip` semantics:
/// discards its changes, does NOT commit it) and resumes.
///
/// Unlike `rebase_continue`, skip does NOT call `commit_current` for the
/// current operation — that operation is dropped from the result. We discard
/// the current op's changes with a LIGHTWEIGHT reset of just the index +
/// worktree to the current in-progress rebase HEAD — NOT `repo.reset(HEAD,
/// Hard)`. The heavy reset rewrites the HEAD ref / reflog / ORIG_HEAD and, on
/// the FIRST op (no commit replayed yet), disturbs `.git/rebase-merge` so the
/// subsequent `rebase.next()` cannot write `msgnum`; it also detaches HEAD, so
/// `read_head_from_rebase` fell through to an empty branch name. Restoring the
/// index to HEAD's tree (stage 0, no conflicts) and force-checking-out the
/// worktree leaves the rebase-merge metadata and the HEAD ref intact.
/// Already-replayed commits are untouched. A HARD error leaves the on-disk
/// rebase state intact (§3.9).
pub fn rebase_skip(workdir: &Path) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // Delegate to the Bonsai interactive engine when its sequencer is present
    // (contract §3) — the plain path below is unchanged.
    if crate::git::rebase_interactive::interactive_in_progress(&repo) {
        return crate::git::rebase_interactive::interactive_skip(workdir);
    }

    if !is_rebase_state(repo.state()) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    let mut rebase = repo.open_rebase(None)?;

    // Discard the current op's changes (conflict stages + partial worktree) WITHOUT
    // touching the HEAD ref/reflog or the rebase-merge metadata: read HEAD's tree
    // into the index (clears all conflict stages, restores stage 0) and force-checkout
    // that index so the worktree matches (drops conflict markers). The next patch then
    // applies cleanly. libgit2 equivalent of `git rebase --skip`: DO NOT commit the
    // current op; just resume the plan.
    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;
    let mut idx = repo.index()?;
    idx.read_tree(&head_tree)?;
    idx.write()?;
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_index(Some(&mut idx), Some(&mut co))?;
    drop(idx);
    drop(head_tree);
    drop(head_commit);

    let head_branch = read_head_from_rebase(&repo);

    match run_rebase_loop(workdir, &repo, &mut rebase, &sig) {
        Ok(DriveResult::Completed { head, steps }) => Ok(RebaseOutcome::Rebased {
            branch: head_branch,
            head,
            steps,
            warnings: Vec::new(),
        }),
        Ok(DriveResult::Paused {
            paths,
            current_step,
            total_steps,
        }) => Ok(RebaseOutcome::Conflicts {
            paths,
            current_step,
            total_steps,
        }),
        Err(e) => Err(e), // leave state intact (§3.9)
    }
}

/// Blocking. Aborts the paused rebase, restoring the original HEAD/branch and
/// worktree (destructive — the UI confirms first; backend guard §4.4).
pub fn rebase_abort(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;

    // Delegate to the Bonsai interactive engine when its sequencer is present
    // (contract §3) — the plain path below is unchanged.
    if crate::git::rebase_interactive::interactive_in_progress(&repo) {
        return crate::git::rebase_interactive::interactive_abort(workdir);
    }

    if !is_rebase_state(repo.state()) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }

    // A git2 error (e.g. a CLI apply-backend rebase open_rebase cannot load)
    // surfaces as AppError::Git -> honest toast.
    let mut rebase = repo.open_rebase(None)?;
    // Data-loss guard (F-A3-1, the 46a34d4 guard bisect/interactive already
    // run): `rebase.abort()` hard-resets the worktree to the original HEAD, so
    // an untracked file whose path exists in the orig-head tree would be
    // silently overwritten. Refuse BEFORE aborting — the rebase state is
    // untouched, so the user can remove/stash the file and retry.
    if let Some(orig) = rebase_orig_head(&repo, &rebase) {
        if let Ok(commit) = repo.find_commit(orig) {
            ensure_no_untracked_collision(&repo, &commit.tree()?)?;
        }
    }
    rebase.abort()?; // restores original HEAD/branch + worktree
    Ok(())
}

/// Oid of the rebase's original HEAD, for the abort clobber guard. Prefer
/// libgit2's `orig_head_id()`; fall back to the on-disk `orig-head` file the
/// git CLI writes (merge and apply backends). `None` = undeterminable — the
/// abort then proceeds unguarded, which is no worse than the pre-F-A3-1
/// behavior.
fn rebase_orig_head(repo: &git2::Repository, rebase: &git2::Rebase<'_>) -> Option<git2::Oid> {
    if let Some(oid) = rebase.orig_head_id() {
        return Some(oid);
    }
    for dir in ["rebase-merge", "rebase-apply"] {
        if let Ok(raw) = std::fs::read_to_string(repo.path().join(dir).join("orig-head")) {
            if let Ok(oid) = git2::Oid::from_str(raw.trim()) {
                return Some(oid);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------- wire shape (TS mirrors)

    /// The serde tag/casing must match the TS RebaseOutcome union exactly.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(RebaseOutcome::UpToDate).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

        let v = serde_json::to_value(RebaseOutcome::FastForwarded {
            branch: "topic".to_string(),
            to: "a".repeat(40),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "fastForwarded", "branch": "topic", "to": "a".repeat(40) })
        );

        let v = serde_json::to_value(RebaseOutcome::Rebased {
            branch: "topic".to_string(),
            head: "b".repeat(40),
            steps: 2,
            warnings: Vec::new(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "rebased", "branch": "topic", "head": "b".repeat(40), "steps": 2 }),
            "empty warnings are omitted from the wire (skip_serializing_if)"
        );

        // A non-empty warnings list surfaces as a `warnings` array (toasted by the UI).
        let v = serde_json::to_value(RebaseOutcome::Rebased {
            branch: "topic".to_string(),
            head: "b".repeat(40),
            steps: 2,
            warnings: vec!["reword of 1234567 was dropped".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "kind": "rebased",
                "branch": "topic",
                "head": "b".repeat(40),
                "steps": 2,
                "warnings": ["reword of 1234567 was dropped"]
            })
        );

        let v = serde_json::to_value(RebaseOutcome::Conflicts {
            paths: vec!["README.md".to_string(), "src/auth.ts".to_string()],
            current_step: 2,
            total_steps: 3,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "kind": "conflicts",
                "paths": ["README.md", "src/auth.ts"],
                "currentStep": 2,
                "totalSteps": 3
            })
        );
    }

    // ------------------------------------------------------- preconditions

    #[test]
    fn rebase_preconditions_on_fresh_repo() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        // Unborn HEAD refuses before onto resolution.
        let err = rebase_branch(dir.path(), "main").expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // continue / skip / abort with no rebase in progress.
        let err = rebase_continue(dir.path()).expect_err("no rebase");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
        let err = rebase_skip(dir.path()).expect_err("no rebase");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
        let err = rebase_abort(dir.path()).expect_err("no rebase");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
    }
}
