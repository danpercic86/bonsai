//! P70 §6.1 #23 — `SpawnGitRunner`'s non-zero-exit message must name the
//! subcommand it ACTUALLY ran.
//!
//! Before P70 the message hard-coded ``failed to run `git log` `` for every
//! consumer of `&dyn GitRunner`, so a failing `commit-graph write` (P52
//! maintenance) reported a command that was never executed — a real bug, and a
//! misleading one, because the runner is shared by commit search, content
//! search and commit-graph maintenance. This pins the fix from the OUTSIDE:
//! `SpawnGitRunner` and `GitRunner` are public, so this needs no test hook and
//! no access to crate internals.
//!
//! Deliberately an integration test rather than a unit test in `search.rs`:
//! that file is on the file-size ratchet baseline and may not grow.

use std::path::Path;
use std::process::Command;

use bonsai_core::error::AppError;
use bonsai_core::git::search::{GitRunner, SpawnGitRunner};

/// Same skip idiom as the rest of the suite: these cases need a real `git` on
/// PATH, and `BONSAI_REQUIRE_GIT_STRICT=1` turns the skip into a hard failure
/// on machines (CI) where git is mandatory.
fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

fn init_repo(dir: &Path) {
    let out = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir)
        .output()
        .expect("spawn git init");
    assert!(out.status.success(), "git init failed");
}

/// Run `git <subcmd> --bonsai-not-a-real-option` and return the error text.
/// An unknown option makes git exit non-zero during option parsing, on every
/// git version and without touching the repository — a stable, fast failure.
fn failing_run(dir: &Path, subcmd: &str) -> String {
    let args = vec![subcmd.to_string(), "--bonsai-not-a-real-option".to_string()];
    match SpawnGitRunner.run(&args, dir) {
        Ok(out) => panic!("`git {subcmd} --bonsai-not-a-real-option` unexpectedly succeeded: {out}"),
        // The spawn itself must have worked — git IS on PATH here. A
        // `GitNotFound` would mean the resolver broke, which is a different
        // (and much louder) failure than the one under test.
        Err(AppError::Git(message)) => message,
        Err(other) => panic!("expected AppError::Git from a non-zero exit, got {other:?}"),
    }
}

#[test]
fn spawn_runner_non_zero_exit_names_the_actual_subcommand() {
    if !have_git() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());

    // The regression that P70 fixed: a commit-graph write reporting "git log".
    let graph = failing_run(dir.path(), "commit-graph");
    assert!(
        graph.contains("`git commit-graph` failed"),
        "the message must name the subcommand actually run, got: {graph}"
    );
    assert!(
        !graph.contains("git log"),
        "a commit-graph failure must never claim `git log` ran: {graph}"
    );

    // …and the original consumer still reports itself correctly (the fix is a
    // generalisation, not a swap).
    let log = failing_run(dir.path(), "log");
    assert!(
        log.contains("`git log` failed"),
        "commit search must still name `git log`, got: {log}"
    );

    // Both carry git's own stderr tail, so the message stays diagnosable.
    assert!(
        graph.len() > "`git commit-graph` failed: ".len(),
        "the stderr tail must be preserved: {graph}"
    );
}

/// The other `GitRunner` consumer shapes: content search runs `git grep`, so a
/// failure there must not be attributed to `log` either. Cheap extra pin over
/// the same code path with a different argv head.
#[test]
fn spawn_runner_names_grep_for_a_failing_content_search() {
    if !have_git() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());

    let message = failing_run(dir.path(), "grep");
    assert!(
        message.contains("`git grep` failed"),
        "expected the grep subcommand named, got: {message}"
    );
    assert!(!message.contains("git log"), "{message}");
}
