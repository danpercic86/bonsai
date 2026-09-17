//! M5 CLI-oracle branch tests, §6.3 checkout (split out of `branches_cli.rs`
//! to keep each file under the ~500-line limit). Declared as a child module of
//! `branches_cli`, so it shares that file's `require_git!` macro.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

use std::path::Path;

use crate::common;
use crate::common::{assert_same_status, commit_fixed, git, git_ok, init_repo};
use bonsai_core::error::AppError;
use bonsai_core::git::branches::checkout_branch;

/// Fixture for checkout tests: `main` (file.txt = "main v1", shared.txt) and
/// `side` (file.txt = "side v1"), currently on `main`. Deterministic dates so
/// twin repos are oid-identical.
fn checkout_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "main v1\n").expect("write file.txt");
    std::fs::write(path.join("shared.txt"), "shared v1\n").expect("write shared.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    git(path, &["checkout", "-b", "side"]);
    std::fs::write(path.join("file.txt"), "side v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "side change");
    git(path, &["checkout", "main"]);
    dir
}

fn read(path: &Path, name: &str) -> String {
    std::fs::read_to_string(path.join(name)).expect("read file")
}

/// §6.3.1: clean checkout — HEAD symref, worktree contents, and (empty)
/// porcelain all identical to the CLI twin.
#[test]
fn checkout_clean_matches_cli_twin() {
    require_git!();
    let a = checkout_repo();
    let b = checkout_repo();

    checkout_branch(a.path(), "side").expect("checkout_branch");
    git(b.path(), &["checkout", "side"]);

    assert_eq!(
        git(a.path(), &["symbolic-ref", "HEAD"]),
        git(b.path(), &["symbolic-ref", "HEAD"])
    );
    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), "refs/heads/side");
    assert_eq!(read(a.path(), "file.txt"), read(b.path(), "file.txt"));
    assert_eq!(read(a.path(), "file.txt"), "side v1\n");
    assert_same_status(a.path(), b.path());
    assert!(git(a.path(), &["status", "--porcelain"]).is_empty());
}

/// §6.3.2: checkout carrying a compatible local change (file untouched
/// between branches) succeeds and the modification survives.
#[test]
fn checkout_carries_compatible_changes() {
    require_git!();
    let a = checkout_repo();
    let b = checkout_repo();
    for p in [a.path(), b.path()] {
        std::fs::write(p.join("shared.txt"), "shared modified\n").expect("write shared.txt");
    }

    checkout_branch(a.path(), "side").expect("checkout_branch with compatible changes");
    git(b.path(), &["checkout", "side"]);

    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), "refs/heads/side");
    assert_eq!(read(a.path(), "shared.txt"), "shared modified\n");
    assert_eq!(read(a.path(), "file.txt"), "side v1\n");
    assert_same_status(a.path(), b.path());
}

/// §6.3.3: dirty conflict (modified file DIFFERS between branches) ->
/// CheckoutConflict and NOTHING moved; the CLI twin also refuses.
#[test]
fn checkout_dirty_conflict_changes_nothing() {
    require_git!();
    let a = checkout_repo();
    let b = checkout_repo();
    for p in [a.path(), b.path()] {
        std::fs::write(p.join("file.txt"), "local edit\n").expect("write file.txt");
    }
    let head_before = git(a.path(), &["symbolic-ref", "HEAD"]);
    let porcelain_before = common::porcelain_records(a.path());

    let err = checkout_branch(a.path(), "side").expect_err("conflicting checkout must fail");
    assert!(matches!(err, AppError::CheckoutConflict(_)), "got {err:?}");

    // Twin oracle: git checkout also refuses.
    assert!(!git_ok(b.path(), &["checkout", "side"]));

    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(read(a.path(), "file.txt"), "local edit\n");
    assert_eq!(common::porcelain_records(a.path()), porcelain_before);
}

/// §6.3.4: checkout of the current branch is an Ok no-op.
#[test]
fn checkout_current_branch_is_noop() {
    require_git!();
    let dir = checkout_repo();

    checkout_branch(dir.path(), "main").expect("checkout current branch");
    assert_eq!(
        git(dir.path(), &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert!(git(dir.path(), &["status", "--porcelain"]).is_empty());
}

/// §6.3.5: nonexistent branch -> BranchNotFound.
#[test]
fn checkout_missing_branch() {
    require_git!();
    let dir = checkout_repo();

    let err = checkout_branch(dir.path(), "nope").expect_err("missing branch");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}
