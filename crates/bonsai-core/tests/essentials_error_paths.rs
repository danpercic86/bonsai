//! P20 error-path coverage (tester addition). Fills gaps NOT covered by the
//! oracle suite (`essentials_cli.rs`) or the in-module `#[cfg(test)]` blocks:
//!
//!   - cherry-pick / revert REFUSE on a DETACHED HEAD (contract §5.1 step 2 /
//!     §6, OPEN #5 — "attached born HEAD only"). Only the *unborn* branch of
//!     that guard is unit-tested; this exercises the `detached` branch.
//!   - `reset_branch` is ALLOWED on a detached HEAD (contract §3.1 step 3 —
//!     "Detached HEAD is allowed; the UI gates it") and moves the detached HEAD.
//!   - `amend_commit` refuses mid-operation with `OperationInProgress` (the
//!     `repo.state() != Clean` guard, otherwise only reachable via a paused op).
//!
//! These use the `git` CLI only to *build* fixtures (detached checkout, a paused
//! cherry-pick) — the assertions are on Bonsai's core return values / git2
//! state, never on commit oids. Skips (passes with a note) when `git` is absent.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::cherrypick::{cherrypick_commit, CherrypickOutcome};
use bonsai_core::git::commit::amend_commit;
use bonsai_core::git::reset::{reset_branch, ResetMode};
use bonsai_core::git::revert::revert_commit;
use common::{git, git_env, init_repo, FIXED_DATE};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn head_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

fn add_commit(dir: &Path, name: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
    git(dir, &["add", name]);
    git_env(
        dir,
        &["commit", "-m", msg],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

/// Builds base → c2 and returns (dir, base_oid, c2_oid), then detaches HEAD
/// onto `base` so the caller can exercise the detached-HEAD guards.
fn detached_at_base() -> (tempfile::TempDir, String, String) {
    let dir = init_repo();
    let d = dir.path();
    add_commit(d, "a.txt", "one\n", "base");
    let base = head_oid(d);
    add_commit(d, "a.txt", "two\n", "second");
    let c2 = head_oid(d);
    // Detach HEAD onto the base commit.
    git(d, &["checkout", &base]);
    assert_eq!(head_oid(d), base, "HEAD detached at base");
    (dir, base, c2)
}

// ------------------------------------------ cherry-pick / revert refuse detached

#[test]
fn cherrypick_on_detached_head_errors() {
    require_git!();
    let (dir, _base, c2) = detached_at_base();
    let err = cherrypick_commit(dir.path(), &c2, None).expect_err("detached must refuse");
    match err {
        AppError::Git(m) => assert!(m.contains("HEAD is detached"), "got: {m}"),
        other => panic!("expected Git(HEAD is detached), got {other:?}"),
    }
    // Nothing mutated: still detached, state Clean.
    assert_eq!(
        git2::Repository::open(dir.path()).unwrap().state(),
        git2::RepositoryState::Clean
    );
}

#[test]
fn revert_on_detached_head_errors() {
    require_git!();
    let (dir, _base, c2) = detached_at_base();
    let err = revert_commit(dir.path(), &c2).expect_err("detached must refuse");
    match err {
        AppError::Git(m) => assert!(m.contains("HEAD is detached"), "got: {m}"),
        other => panic!("expected Git(HEAD is detached), got {other:?}"),
    }
    assert_eq!(
        git2::Repository::open(dir.path()).unwrap().state(),
        git2::RepositoryState::Clean
    );
}

// ------------------------------------------ reset IS allowed on detached HEAD

/// Contract §3.1 step 3: reset on a detached HEAD is ALLOWED and moves the
/// detached HEAD (the UI, not the core, restricts this). Verify hard reset from
/// a detached c2 back to base matches `git reset --hard`.
#[test]
fn reset_on_detached_head_is_allowed() {
    require_git!();
    // Build twin repos detached at c2, reset each to base (bonsai vs CLI).
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();
    let mut base = String::new();
    for d in [a, b] {
        add_commit(d, "a.txt", "one\n", "base");
        base = head_oid(d);
        add_commit(d, "a.txt", "two\n", "second");
        git(d, &["checkout", &head_oid(d)]); // detach at c2
    }

    reset_branch(a, &base, ResetMode::Hard).expect("bonsai reset on detached HEAD");
    git(b, &["reset", "--hard", &base]);

    assert_eq!(head_oid(a), base, "detached HEAD moved to base");
    assert_eq!(head_oid(a), head_oid(b), "matches CLI");
    // Still detached (reset does not re-attach).
    assert!(
        git2::Repository::open(a).unwrap().head_detached().unwrap(),
        "HEAD stays detached after reset"
    );
    assert_eq!(
        std::fs::read_to_string(a.join("a.txt")).unwrap(),
        "one\n",
        "hard reset restored worktree to base"
    );
}

// ------------------------------------------ amend refuses mid-operation

/// `amend_commit` must refuse with `OperationInProgress` when the repo is
/// paused mid cherry-pick (state != Clean), before touching HEAD.
#[test]
fn amend_during_paused_cherrypick_errors() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    // Construct a guaranteed conflict: feature and main edit the same line.
    add_commit(d, "x.txt", "l1\nbase\nl3\n", "base");
    git(d, &["checkout", "-b", "feature"]);
    add_commit(d, "x.txt", "l1\nfeature\nl3\n", "feature edit");
    let pick = head_oid(d);
    git(d, &["checkout", "main"]);
    add_commit(d, "x.txt", "l1\nmain\nl3\n", "main edit");
    let head_before = head_oid(d);

    let outcome = cherrypick_commit(d, &pick, None).expect("start pick");
    assert!(
        matches!(outcome, CherrypickOutcome::Conflicts { .. }),
        "fixture must conflict"
    );
    assert_eq!(
        git2::Repository::open(d).unwrap().state(),
        git2::RepositoryState::CherryPick
    );

    let err = amend_commit(d, "sneaky amend", None, false).expect_err("amend mid-op must refuse");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
    // HEAD untouched by the refused amend.
    assert_eq!(head_oid(d), head_before, "amend must not move HEAD mid-op");
}
