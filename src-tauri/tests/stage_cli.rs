//! M3 CLI-oracle staging tests (contract §6.1).
//!
//! Twin-repo pattern: two identical scratch repos built by the same script
//! (fixed commit dates so base oids match). Our git2 op runs on repo A, the
//! equivalent CLI op on repo B, then `git status --porcelain=v1 -z
//! --untracked-files=all` must be byte-identical between the two (records
//! sorted to dodge ordering differences).
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_lib::error::AppError;
use bonsai_lib::git::stage::{stage_paths, unstage_paths};
use common::{assert_same_status, commit_fixed, git, init_repo, porcelain_records};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

const TRACKED: &str = "line one\nline two\nline three\nline four\nline five\n";

/// Base fixture: committed `tracked.txt` (multi-line, rename-detectable) and
/// `other.txt`, committed with fixed dates.
fn base_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("tracked.txt"), TRACKED).expect("write tracked.txt");
    std::fs::write(path.join("other.txt"), "other content\n").expect("write other.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    dir
}

/// Two identical repos: same base, same worktree mutations.
fn twins(setup: impl Fn(&Path)) -> (tempfile::TempDir, tempfile::TempDir) {
    let a = base_repo();
    let b = base_repo();
    setup(a.path());
    setup(b.path());
    (a, b)
}

fn strings(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

// Scenario 1: stage untracked files, incl. one nested in a new directory.
#[test]
fn stage_untracked_including_nested() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::write(p.join("loose.txt"), "loose\n").expect("write loose.txt");
        std::fs::create_dir(p.join("newdir")).expect("create newdir");
        std::fs::write(p.join("newdir").join("nested.txt"), "nested\n").expect("write nested");
    });

    stage_paths(a.path(), &strings(&["loose.txt", "newdir/nested.txt"])).expect("stage_paths");
    git(b.path(), &["add", "--", "loose.txt", "newdir/nested.txt"]);

    assert_same_status(a.path(), b.path());
    let records = porcelain_records(a.path());
    assert!(records.iter().any(|(r, _)| r == "A  loose.txt"), "{records:?}");
    assert!(
        records.iter().any(|(r, _)| r == "A  newdir/nested.txt"),
        "{records:?}"
    );
}

// Scenario 2: stage a modified tracked file.
#[test]
fn stage_modified() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::write(p.join("tracked.txt"), "changed content\n").expect("modify tracked.txt");
    });

    stage_paths(a.path(), &strings(&["tracked.txt"])).expect("stage_paths");
    git(b.path(), &["add", "--", "tracked.txt"]);

    assert_same_status(a.path(), b.path());
    assert!(porcelain_records(a.path())
        .iter()
        .any(|(r, _)| r == "M  tracked.txt"));
}

// Scenario 3: stage an fs-deleted file (our remove_path branch vs `git add -A`).
#[test]
fn stage_deleted() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::remove_file(p.join("tracked.txt")).expect("delete tracked.txt");
    });

    stage_paths(a.path(), &strings(&["tracked.txt"])).expect("stage_paths");
    git(b.path(), &["add", "-A", "--", "tracked.txt"]);

    assert_same_status(a.path(), b.path());
    assert!(porcelain_records(a.path())
        .iter()
        .any(|(r, _)| r == "D  tracked.txt"));
}

// Scenario 4: stage a worktree rename by sending BOTH sides in one call.
#[test]
fn stage_rename_both_sides() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::rename(p.join("tracked.txt"), p.join("renamed.txt")).expect("fs rename");
    });

    stage_paths(a.path(), &strings(&["tracked.txt", "renamed.txt"])).expect("stage_paths");
    git(b.path(), &["add", "-A", "--", "tracked.txt", "renamed.txt"]);

    assert_same_status(a.path(), b.path());
    let records = porcelain_records(a.path());
    assert!(
        records
            .iter()
            .any(|(r, orig)| r == "R  renamed.txt" && orig.as_deref() == Some("tracked.txt")),
        "expected staged rename, got: {records:?}"
    );
}

// Scenario 5: batch — untracked + modified + deleted staged in ONE call.
#[test]
fn stage_batch_mixed() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::write(p.join("u.txt"), "new file\n").expect("write u.txt");
        std::fs::write(p.join("tracked.txt"), "modified\n").expect("modify tracked.txt");
        std::fs::remove_file(p.join("other.txt")).expect("delete other.txt");
    });

    stage_paths(a.path(), &strings(&["u.txt", "tracked.txt", "other.txt"])).expect("stage_paths");
    git(b.path(), &["add", "-A", "--", "u.txt", "tracked.txt", "other.txt"]);

    assert_same_status(a.path(), b.path());
    let records = porcelain_records(a.path());
    assert!(records.iter().any(|(r, _)| r == "A  u.txt"));
    assert!(records.iter().any(|(r, _)| r == "M  tracked.txt"));
    assert!(records.iter().any(|(r, _)| r == "D  other.txt"));
}

// Scenario 6: atomicity — one invalid path aborts the whole batch, index unchanged.
#[test]
fn stage_atomic_on_invalid_path() {
    require_git!();
    let a = base_repo();
    std::fs::write(a.path().join("u.txt"), "new file\n").expect("write u.txt");

    let before = porcelain_records(a.path());
    let err = stage_paths(a.path(), &strings(&["u.txt", "../escape"]))
        .expect_err("path escaping the worktree must be rejected");
    assert!(matches!(err, AppError::Other(_)), "got: {err:?}");
    assert_eq!(
        porcelain_records(a.path()),
        before,
        "failed batch must leave the index untouched"
    );
}

// Scenario 7: unstage a staged modification (parity with `git restore --staged`).
#[test]
fn unstage_modified() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::write(p.join("tracked.txt"), "staged change\n").expect("modify");
        git(p, &["add", "--", "tracked.txt"]);
    });

    unstage_paths(a.path(), &strings(&["tracked.txt"])).expect("unstage_paths");
    git(b.path(), &["restore", "--staged", "--", "tracked.txt"]);

    assert_same_status(a.path(), b.path());
    assert!(porcelain_records(a.path())
        .iter()
        .any(|(r, _)| r == " M tracked.txt"));
}

// Scenario 8: unstage a staged deletion — index entry restored from HEAD.
#[test]
fn unstage_deleted() {
    require_git!();
    let (a, b) = twins(|p| {
        git(p, &["rm", "--", "tracked.txt"]);
    });

    unstage_paths(a.path(), &strings(&["tracked.txt"])).expect("unstage_paths");
    git(b.path(), &["restore", "--staged", "--", "tracked.txt"]);

    assert_same_status(a.path(), b.path());
    assert!(porcelain_records(a.path())
        .iter()
        .any(|(r, _)| r == " D tracked.txt"));
}

// Scenario 9: unstage a staged rename (both paths) — back to worktree-rename state.
#[test]
fn unstage_rename_both_sides() {
    require_git!();
    let (a, b) = twins(|p| {
        std::fs::rename(p.join("tracked.txt"), p.join("renamed.txt")).expect("fs rename");
        git(p, &["add", "-A", "--", "tracked.txt", "renamed.txt"]);
    });

    unstage_paths(a.path(), &strings(&["tracked.txt", "renamed.txt"])).expect("unstage_paths");
    git(b.path(), &["restore", "--staged", "--", "tracked.txt", "renamed.txt"]);

    assert_same_status(a.path(), b.path());
}

// Scenario 10: unborn repo — stage -> `A `, unstage -> `??` (remove_path branch).
#[test]
fn unborn_stage_then_unstage() {
    require_git!();
    let dir = init_repo(); // no commits: unborn HEAD
    let path = dir.path();
    std::fs::write(path.join("first.txt"), "first\n").expect("write first.txt");

    stage_paths(path, &strings(&["first.txt"])).expect("stage on unborn");
    assert_eq!(
        porcelain_records(path),
        vec![("A  first.txt".to_string(), None)]
    );

    unstage_paths(path, &strings(&["first.txt"])).expect("unstage on unborn");
    assert_eq!(
        porcelain_records(path),
        vec![("?? first.txt".to_string(), None)]
    );
}

// Scenario 11 (empty paths vec) is covered by the unit test in git/stage.rs.
