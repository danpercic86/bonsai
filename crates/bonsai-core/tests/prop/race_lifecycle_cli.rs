//! T5 race / lifecycle suite (contract §4): concurrency and repo-lifecycle
//! robustness. No panics anywhere; the repo stays healthy after concurrent
//! access; operations on a deleted repo return clean errors.
//!
//! NOTE: the `notify` file watcher lives in `src-tauri`, not `bonsai-core`, so
//! scenario 1's "watcher emits ≥1 debounced signal" clause cannot be asserted
//! from a core integration test. It is adapted to a write-STORM-during-commit
//! that still exercises the substance (commit survives a concurrent worktree
//! storm, no thread panics).

use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bonsai_core::git::{commit::create_commit, stage::stage_paths, status::read_status};
use bonsai_core::graph::compute_graph;

use crate::prop_common::common;

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping race/lifecycle suite: `git` CLI not on PATH");
            return;
        }
    };
}

/// A repo with one base commit + identity configured.
fn base_repo() -> tempfile::TempDir {
    let dir = common::init_repo();
    std::fs::write(dir.path().join("base.txt"), "base\n").expect("write");
    common::git(dir.path(), &["add", "-A"]);
    common::commit_fixed(dir.path(), "base");
    dir
}

/// Scenario 1: a worktree write-storm running concurrently with a commit.
#[test]
fn commit_survives_worktree_write_storm() {
    require_git!();
    let dir = base_repo();
    let root = dir.path().to_path_buf();

    // Stage one real change to commit.
    std::fs::write(root.join("tracked.txt"), "v1\n").expect("write");
    stage_paths(&root, &["tracked.txt".to_string()]).expect("stage");

    let stop = Arc::new(AtomicBool::new(false));
    let storm_root = root.clone();
    let storm_stop = stop.clone();
    let storm = std::thread::spawn(move || {
        let mut i = 0u32;
        while !storm_stop.load(Ordering::Relaxed) && i < 200 {
            let _ = std::fs::write(storm_root.join(format!("storm-{i}.tmp")), b"x");
            i += 1;
        }
    });

    let res = create_commit(&root, "storm commit", None, true);
    stop.store(true, Ordering::Relaxed);
    storm.join().expect("storm thread must not panic");

    assert!(res.is_ok(), "commit must succeed during a write storm: {res:?}");
    // Repo remains healthy.
    assert!(read_status(&root).is_ok());
    assert!(compute_graph(&root).is_ok());
    assert!(common::git_ok(&root, &["fsck", "--no-dangling"]), "git fsck clean");
    drop(dir);
}

/// Scenario 2: read_status ×50 concurrent with stage+commit ×10.
#[test]
fn concurrent_status_and_commit_stay_coherent() {
    require_git!();
    let dir = base_repo();
    let root = dir.path().to_path_buf();

    let a_root = root.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..50 {
            // Read-only; may transiently error under contention but never panic.
            let _ = read_status(&a_root);
        }
    });

    let b_root = root.clone();
    let writer = std::thread::spawn(move || {
        let mut clean_errs = 0u32;
        for i in 0..10 {
            let name = format!("c{i}.txt");
            if std::fs::write(b_root.join(&name), format!("v{i}\n")).is_err() {
                continue;
            }
            if stage_paths(&b_root, std::slice::from_ref(&name)).is_err() {
                clean_errs += 1;
                continue;
            }
            match create_commit(&b_root, &format!("commit {i}"), None, true) {
                Ok(_) => {}
                Err(_) => clean_errs += 1, // clean AppError (e.g. index lock) is fine
            }
        }
        clean_errs
    });

    reader.join().expect("reader thread must not panic");
    let _clean_errs = writer.join().expect("writer thread must not panic");

    // Repo is healthy after the joins.
    assert!(read_status(&root).is_ok(), "read_status ok after joins");
    assert!(compute_graph(&root).is_ok(), "compute_graph ok after joins");
    assert!(common::git_ok(&root, &["fsck", "--no-dangling"]), "git fsck clean");
    drop(dir);
}

/// Scenario 3: repo dir deleted while "open" — every surface returns Err, no panic.
#[test]
fn operations_on_deleted_repo_error_cleanly() {
    require_git!();
    let dir = base_repo();
    let root = dir.path().to_path_buf();

    // Baseline succeeds.
    assert!(read_status(&root).is_ok());

    // Remove the whole repo dir (Windows: retry transient sharing violations).
    let mut removed = false;
    for _ in 0..20 {
        if std::fs::remove_dir_all(&root).is_ok() && !root.exists() {
            removed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(removed, "could not remove repo dir for the test");

    let classify = |o: std::thread::Result<bool>| match o {
        Ok(is_err) => is_err, // true == returned Err
        Err(_) => panic!("surface panicked on a deleted repo"),
    };
    let status_err = classify(std::panic::catch_unwind(AssertUnwindSafe(|| {
        read_status(&root).is_err()
    })));
    let graph_err = classify(std::panic::catch_unwind(AssertUnwindSafe(|| {
        compute_graph(&root).is_err()
    })));
    let commit_err = classify(std::panic::catch_unwind(AssertUnwindSafe(|| {
        create_commit(&root, "x", None, true).is_err()
    })));

    assert!(status_err, "read_status errors on a deleted repo");
    assert!(graph_err, "compute_graph errors on a deleted repo");
    assert!(commit_err, "create_commit errors on a deleted repo");
    let _ = dir; // tempdir drop is a no-op now
    let _: &Path = &root;
}
