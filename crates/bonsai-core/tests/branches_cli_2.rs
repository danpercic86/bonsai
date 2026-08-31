//! M5 CLI-oracle branch tests, part 2 (contract §6.1 tip / §6.2 / §6.3):
//! the remote-tracking checkout + delete group. Split off `branches_cli.rs`
//! (file-size discipline) — same fixtures, same oracle methodology.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::branches::{checkout_remote, delete_remote_tracking, list_refs};
use common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Lines of trimmed `git` stdout (empty output -> empty vec).
fn lines(out: &str) -> Vec<String> {
    if out.is_empty() {
        Vec::new()
    } else {
        out.lines().map(str::to_string).collect()
    }
}

fn read(path: &Path, name: &str) -> String {
    std::fs::read_to_string(path.join(name)).expect("read file")
}

// ---------------------------------------------------- P6 §2.1 tip / §2.2 / §2.3

/// Repo with a `file://` bare remote `origin` and a remote-tracking
/// `origin/topic` advanced one commit past `main` (topic changes file.txt so a
/// checkout touches the worktree). Currently on `main`, with NO local `topic`.
/// Returns `(working dir, bare remote dir)` — keep both alive.
fn remote_topic_repo() -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "main v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");

    let bare = common::scratch_dir();
    git(bare.path(), &["init", "--bare"]);
    let bare_url = bare.path().to_string_lossy().replace('\\', "/");
    git(path, &["remote", "add", "origin", &bare_url]);

    // topic advances past main, changing file.txt.
    git(path, &["checkout", "-b", "topic"]);
    std::fs::write(path.join("file.txt"), "topic v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic change");

    git(path, &["push", "origin", "main", "topic"]);
    git(path, &["checkout", "main"]);
    git(path, &["fetch", "origin"]);
    // Drop the local topic so tests exercise the create / collision paths.
    git(path, &["branch", "-D", "topic"]);
    (dir, bare)
}

/// P6 §2.1: BranchInfo.tip / RemoteBranchInfo.tip equal `git rev-parse <ref>`.
#[test]
fn list_refs_tip_matches_cli() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();

    let snap = list_refs(path).expect("list_refs");

    let main = snap.local.iter().find(|b| b.name == "main").expect("main");
    assert_eq!(main.tip, git(path, &["rev-parse", "refs/heads/main"]));

    let origin_topic = snap
        .remote
        .iter()
        .find(|r| r.name == "origin/topic")
        .expect("origin/topic");
    assert_eq!(
        origin_topic.tip,
        git(path, &["rev-parse", "refs/remotes/origin/topic"])
    );
    // Tips are full 40-char hex oids.
    assert_eq!(main.tip.len(), 40);
    assert_eq!(origin_topic.tip.len(), 40);
}

/// P6 §2.2 create path: `origin/topic` with NO local `topic` -> creates and
/// switches to a local tracking branch at the remote tip, upstream configured.
#[test]
fn checkout_remote_creates_and_tracks() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();

    checkout_remote(path, "origin/topic").expect("checkout_remote create path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    assert_eq!(
        git(path, &["rev-parse", "topic"]),
        git(path, &["rev-parse", "refs/remotes/origin/topic"])
    );
    // Upstream configured to origin/refs/heads/topic.
    assert_eq!(git(path, &["config", "branch.topic.remote"]), "origin");
    assert_eq!(
        git(path, &["config", "branch.topic.merge"]),
        "refs/heads/topic"
    );
    // Worktree now has topic's content.
    assert_eq!(read(path, "file.txt"), "topic v1\n");
}

/// P6 §2.2 fast-forward path: a local `topic` exists strictly BEHIND
/// `origin/topic` (at main's oid, an ancestor of the remote tip) -> checkout
/// fast-forwards the local ref onto the remote tip and ends on local `topic`.
#[test]
fn checkout_remote_fast_forwards_behind_local() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    // Local topic at main's oid — a strict ancestor of origin/topic's tip.
    git(path, &["branch", "topic", "main"]);
    let local_before = git(path, &["rev-parse", "topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);
    assert_ne!(local_before, remote_tip, "fixture must be behind");

    checkout_remote(path, "origin/topic").expect("checkout_remote fast-forward path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    // Ref fast-forwarded onto the remote tip.
    assert_eq!(git(path, &["rev-parse", "topic"]), remote_tip);
    // Worktree now has the remote's content.
    assert_eq!(read(path, "file.txt"), "topic v1\n");
}

/// P6 §2.2 ahead path: a local `topic` strictly AHEAD of `origin/topic` (the
/// remote tip is an ancestor of the local tip) -> check out local as-is, ref
/// NOT moved.
#[test]
fn checkout_remote_ahead_local_not_moved() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    // Recreate local topic at the remote tip, then advance it one commit.
    git(path, &["branch", "topic", "refs/remotes/origin/topic"]);
    git(path, &["checkout", "topic"]);
    std::fs::write(path.join("file.txt"), "topic v2\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic ahead");
    git(path, &["checkout", "main"]);
    let local_before = git(path, &["rev-parse", "topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);
    assert_ne!(local_before, remote_tip, "fixture must be ahead");

    checkout_remote(path, "origin/topic").expect("checkout_remote ahead path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    // Ref NOT moved: local retains its extra commit.
    assert_eq!(git(path, &["rev-parse", "topic"]), local_before);
    assert_eq!(read(path, "file.txt"), "topic v2\n");
}

/// P6 §2.2 equal path: a local `topic` at the SAME oid as `origin/topic` ->
/// check out as-is, no ref move, no error.
#[test]
fn checkout_remote_equal_tips_no_move() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    git(path, &["branch", "topic", "refs/remotes/origin/topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);

    checkout_remote(path, "origin/topic").expect("checkout_remote equal path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    assert_eq!(git(path, &["rev-parse", "topic"]), remote_tip);
    assert_eq!(read(path, "file.txt"), "topic v1\n");
}

/// P6 §2.2 diverged path: a local `topic` that has diverged from `origin/topic`
/// (neither tip is an ancestor of the other) -> error, and HEAD + branch tip +
/// worktree are all untouched.
#[test]
fn checkout_remote_diverged_changes_nothing() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    // Local topic branches off main with its OWN divergent commit.
    git(path, &["branch", "topic", "main"]);
    git(path, &["checkout", "topic"]);
    std::fs::write(path.join("file.txt"), "topic divergent\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic divergent");
    git(path, &["checkout", "main"]);
    let local_before = git(path, &["rev-parse", "topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);
    let head_before = git(path, &["symbolic-ref", "HEAD"]);
    let file_before = read(path, "file.txt");
    assert_ne!(local_before, remote_tip, "fixture must diverge");

    let err = checkout_remote(path, "origin/topic").expect_err("diverged checkout must fail");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");

    // Nothing changed.
    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(git(path, &["rev-parse", "topic"]), local_before);
    assert_eq!(read(path, "file.txt"), file_before);
}

/// P6 §2.2 conflict: a dirty worktree a safe checkout would overwrite ->
/// CheckoutConflict, HEAD + worktree unchanged, and NO new local branch.
#[test]
fn checkout_remote_conflict_changes_nothing() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "local edit\n").expect("write file.txt");
    let head_before = git(path, &["symbolic-ref", "HEAD"]);

    let err = checkout_remote(path, "origin/topic").expect_err("conflicting checkout must fail");
    assert!(matches!(err, AppError::CheckoutConflict(_)), "got {err:?}");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(read(path, "file.txt"), "local edit\n");
    // No local branch was created.
    assert!(lines(&git(path, &["branch", "--list", "topic"])).is_empty());
}

/// P6 §2.2 errors: no '/' -> InvalidName; unknown remote ref -> BranchNotFound.
#[test]
fn checkout_remote_error_taxonomy() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();

    let err = checkout_remote(path, "nope").expect_err("no slash must fail");
    assert!(matches!(err, AppError::InvalidName(_)), "got {err:?}");

    let err = checkout_remote(path, "origin/ghost").expect_err("unknown remote ref must fail");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}

/// P6 §2.3: deletes only the LOCAL remote-tracking ref; the server's own refs
/// are untouched. Unknown ref -> BranchNotFound.
#[test]
fn delete_remote_tracking_local_only() {
    require_git!();
    let (dir, bare) = remote_topic_repo();
    let path = dir.path();

    // Sanity: the remote-tracking ref exists before deletion.
    assert!(!lines(&git(path, &["branch", "-r", "--list", "origin/topic"])).is_empty());

    delete_remote_tracking(path, "origin/topic").expect("delete_remote_tracking");

    assert!(lines(&git(path, &["branch", "-r", "--list", "origin/topic"])).is_empty());
    // The server's own branch is untouched.
    assert!(
        git(bare.path(), &["show-ref"]).contains("refs/heads/topic"),
        "server ref refs/heads/topic must survive a local remote-tracking delete"
    );

    // Unknown ref -> BranchNotFound.
    let err = delete_remote_tracking(path, "origin/ghost").expect_err("unknown ref must fail");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}
