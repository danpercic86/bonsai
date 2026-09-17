//! P3d CLI-oracle rebase tests — the start/continue/skip precondition matrix
//! and the backend commit guard (contract §9.9–§9.10).
//!
//! Split out of `rebase_cli.rs`; the twin-repo scaffolding, helpers, and
//! fixtures live in `rebase_support.rs`.

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::rebase_support::{
    checkout, has_rebase_dir, repo_state, require_git, script_clean_linear, script_conflict_one,
    twin_pair, write,
};
use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::conflict::{resolve_conflict, ConflictResolution};
use bonsai_core::git::merge::{merge_branch, MergeOutcome};
use bonsai_core::git::rebase::{rebase_branch, rebase_continue, rebase_skip, RebaseOutcome};

// ============================================================ §9.9 precondition matrix

#[test]
fn precondition_detached_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["branch", "topic"]);
    git(d, &["checkout", "--detach"]);

    match rebase_branch(d, "topic").expect_err("detached") {
        AppError::Git(m) => assert!(m.contains("detached"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

#[test]
fn precondition_unborn_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    match rebase_branch(repo.path(), "main").expect_err("unborn") {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn precondition_dirty_index_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_clean_linear);
    let d = bonsai.path();
    checkout(d, "topic");
    write(d, "t1.txt", "staged edit\n");
    git(d, &["add", "t1.txt"]);

    match rebase_branch(d, "main").expect_err("dirty index") {
        AppError::Git(m) => assert!(m.contains("uncommitted changes"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

#[test]
fn precondition_rebase_during_op_is_rejected() {
    require_git!();
    // Start a merge into a conflict (state != Clean), then attempt a rebase.
    let (bonsai, _twin) = twin_pair(script_conflict_one);
    let d = bonsai.path();
    git(d, &["branch", "other"]); // a second candidate onto
    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected merge Conflicts, got {other:?}"),
    }

    let err = rebase_branch(d, "other").expect_err("rebase during op");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
}

#[test]
fn precondition_unknown_onto_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = rebase_branch(d, "no-such-branch").expect_err("unknown");
    assert!(
        matches!(err, AppError::BranchNotFound(_)),
        "expected BranchNotFound, got {err:?}"
    );
}

#[test]
fn precondition_missing_identity_is_config_missing_before_worktree() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_clean_linear);
    let d = bonsai.path();
    checkout(d, "topic");
    // Blank the repo-local identity (an explicit empty value overrides any
    // global identity for this repo -> resolve_signature reports it missing).
    git(d, &["config", "user.name", ""]);
    git(d, &["config", "user.email", ""]);

    let err = rebase_branch(d, "main").expect_err("no identity");
    assert!(
        matches!(err, AppError::ConfigMissing(_)),
        "expected ConfigMissing, got {err:?}"
    );
    // Surfaces BEFORE the worktree is touched: nothing left behind.
    assert_eq!(
        repo_state(d),
        git2::RepositoryState::Clean,
        "state must stay Clean"
    );
    assert!(!has_rebase_dir(d), "no rebase-merge dir left behind");
}

// ============================================================ §9.10 backend commit guard

#[test]
fn plain_commit_during_paused_rebase_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict_one);
    let d = bonsai.path();
    checkout(d, "topic");
    match rebase_branch(d, "main").expect("rebase") {
        RebaseOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    // Resolve so the only thing blocking a plain commit is the op-state guard.
    resolve_conflict(d, "a.txt", ConflictResolution::Ours).expect("resolve");

    let err = create_commit(d, "sneaky plain commit", None, false).expect_err("gated");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
    // The rebase state is untouched by the refused commit.
    assert!(
        has_rebase_dir(d),
        "rebase state must persist after the refused commit"
    );
}

#[test]
fn continue_and_skip_without_a_rebase_are_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    assert!(matches!(
        rebase_continue(d).expect_err("no rebase"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        rebase_skip(d).expect_err("no rebase"),
        AppError::NoOperationInProgress(_)
    ));
}
