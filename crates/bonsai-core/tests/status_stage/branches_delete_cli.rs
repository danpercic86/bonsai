//! M5 CLI-oracle branch tests, §6.4 delete (split out of `branches_cli.rs` to
//! keep each file under the ~500-line limit). Declared as a child module of
//! `branches_cli`, so it shares that file's `require_git!` macro and its
//! `base_repo` fixture.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

use super::base_repo;
use crate::common;
use crate::common::{commit_fixed, git, git_ok};
use bonsai_core::error::AppError;
use bonsai_core::git::branches::delete_branch;

/// §6.4.1: merged branch deletes; twin `git branch -d` agrees.
#[test]
fn delete_merged_branch() {
    require_git!();
    let a = base_repo();
    let b = base_repo();
    git(a.path(), &["branch", "merged"]);
    git(b.path(), &["branch", "merged"]);

    delete_branch(a.path(), "merged").expect("delete merged branch");
    assert!(!git_ok(
        a.path(),
        &["rev-parse", "--verify", "refs/heads/merged"]
    ));
    assert!(git_ok(b.path(), &["branch", "-d", "merged"]));
}

/// §6.4.2: unmerged branch -> UnmergedBranch, ref still present; twin
/// `git branch -d` also fails.
#[test]
fn delete_unmerged_branch_blocked() {
    require_git!();
    let build = || {
        let dir = base_repo();
        let path = dir.path();
        git(path, &["checkout", "-b", "topic"]);
        std::fs::write(path.join("topic.txt"), "topic\n").expect("write topic.txt");
        git(path, &["add", "-A"]);
        commit_fixed(path, "topic commit");
        git(path, &["checkout", "main"]);
        dir
    };
    let a = build();
    let b = build();

    let err = delete_branch(a.path(), "topic").expect_err("unmerged delete must fail");
    match &err {
        AppError::UnmergedBranch(m) => {
            assert!(m.contains("not fully merged into HEAD"), "message: {m}");
            assert!(m.contains("git branch -D topic"), "message: {m}");
        }
        other => panic!("expected UnmergedBranch, got {other:?}"),
    }
    assert!(git_ok(
        a.path(),
        &["rev-parse", "--verify", "refs/heads/topic"]
    ));
    assert!(!git_ok(b.path(), &["branch", "-d", "topic"]));
}

/// §6.4.3: current branch -> AppError::Git with the contract message.
#[test]
fn delete_current_branch_blocked() {
    require_git!();
    let dir = base_repo();

    let err = delete_branch(dir.path(), "main").expect_err("delete current must fail");
    match err {
        AppError::Git(m) => {
            assert_eq!(
                m,
                "cannot delete 'main': it is the currently checked-out branch"
            )
        }
        other => panic!("expected Git error, got {other:?}"),
    }
    assert!(git_ok(
        dir.path(),
        &["rev-parse", "--verify", "refs/heads/main"]
    ));
}

/// §6.4.4: nonexistent -> BranchNotFound.
#[test]
fn delete_missing_branch() {
    require_git!();
    let dir = base_repo();

    let err = delete_branch(dir.path(), "nope").expect_err("missing branch");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}

/// §6.4.5: detached on the merged tip -> delete succeeds (merged relative to
/// the detached HEAD commit).
#[test]
fn delete_merged_branch_while_detached() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();
    git(path, &["branch", "extra"]);
    git(path, &["checkout", "--detach", "HEAD"]);

    delete_branch(path, "extra").expect("delete merged branch while detached");
    assert!(!git_ok(
        path,
        &["rev-parse", "--verify", "refs/heads/extra"]
    ));
}
