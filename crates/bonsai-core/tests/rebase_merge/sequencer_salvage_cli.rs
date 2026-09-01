//! T2 Area 3 sequencer-hardening tests (FINDINGS F-A3-1..F-A3-4).
//!
//! - F-A3-1: plain `rebase_abort` refuses to clobber an untracked file whose
//!   path exists in the orig-head tree (state intact → retryable).
//! - F-A3-2: a CORRUPT bonsai sequencer state file (bisect / interactive
//!   rebase) no longer deadlocks the app — reset/abort salvages: state dir
//!   cleared, HEAD left in place, distinct explanatory error.
//! - F-A3-3: cross-sequencer start guards (bisect blocks interactive start and
//!   vice versa).
//! - F-A3-4: an UNREADABLE (io-error) bisect state file surfaces the real
//!   error, not "state missing".
//!
//! Adversarial fixtures per the T2 contract: handcrafted/garbage state dirs
//! are constructed directly on disk. Scratch repos live under
//! `D:\Data\Temp\bonsai-scratch`; tests skip with a note when `git` is absent.

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::bisect::{bisect_reset, start_bisect, BisectOutcome};
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::rebase::{rebase_abort, rebase_branch, RebaseOutcome};
use bonsai_core::git::rebase_interactive::start_interactive_rebase;
use bonsai_core::git::stage::stage_paths;
use crate::common;
use crate::common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn rev(dir: &Path, r: &str) -> String {
    git(dir, &["rev-parse", r])
}

/// Linear history c0..c{n-1} on main. Returns oids oldest-first.
fn build_linear(d: &Path, n: usize) -> Vec<String> {
    let mut oids = Vec::new();
    for i in 0..n {
        write(d, &format!("f{i}.txt"), &format!("c{i}\n"));
        git(d, &["add", "-A"]);
        commit_fixed(d, &format!("c{i}"));
        oids.push(rev(d, "HEAD"));
    }
    oids
}

// ===================================================== F-A3-2 bisect salvage

/// F-A3-2: corrupting `.git/bonsai-bisect/state.json` mid-bisect used to make
/// `bisect_reset` fail on parse forever while `require_no_bisect` kept every
/// mutation blocked. The salvage path clears the state dir, leaves HEAD where
/// it is, and returns a distinct explanatory error; the app is unblocked.
#[test]
fn corrupt_bisect_state_is_salvaged_by_reset() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let oids = build_linear(d, 8);

    match start_bisect(d, &oids[7], &[oids[0].clone()]).expect("start bisect") {
        BisectOutcome::Testing { .. } => {}
        other => panic!("expected Testing, got {other:?}"),
    }
    let state_dir = d.join(".git").join("bonsai-bisect");
    assert!(state_dir.join("state.json").exists(), "bisect state present");

    // Corrupt the state (truncated JSON) and record where HEAD sits.
    std::fs::write(state_dir.join("state.json"), "{ \"version\": 1, \"orig").expect("corrupt");
    let head_before = rev(d, "HEAD");

    let err = bisect_reset(d).expect_err("corrupt state -> salvage error");
    let msg = err.to_string();
    assert!(msg.contains("corrupt"), "explains corruption: {msg}");
    assert!(msg.contains("cleared"), "explains the state was cleared: {msg}");
    assert!(msg.contains("HEAD was left"), "explains HEAD untouched: {msg}");

    // State dir gone, HEAD untouched (still on the detached midpoint).
    assert!(!state_dir.exists(), "state dir removed by salvage");
    assert_eq!(rev(d, "HEAD"), head_before, "HEAD left where it was");

    // The deadlock is broken: reset now reports no bisect, and a mutation
    // (commit) passes require_no_bisect again.
    match bisect_reset(d) {
        Err(AppError::NoOperationInProgress(_)) => {}
        other => panic!("expected NoOperationInProgress, got {other:?}"),
    }
    write(d, "unblock.txt", "free\n");
    stage_paths(d, &["unblock.txt".to_string()]).expect("stage");
    create_commit(d, "unblocked after salvage", None, false).expect("commit works again");
}

/// F-A3-4: when the state file exists but cannot be READ (io error — here a
/// directory in its place), the real error is surfaced, not "state missing"
/// and NOT the salvage path (the state may be fine; only reading failed).
#[test]
fn unreadable_bisect_state_surfaces_real_io_error() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    build_linear(d, 3);

    let state_dir = d.join(".git").join("bonsai-bisect");
    std::fs::create_dir_all(state_dir.join("state.json")).expect("state.json as a DIRECTORY");

    let err = bisect_reset(d).expect_err("io error");
    let msg = err.to_string();
    assert!(
        msg.contains("failed to read bisect state"),
        "real io error surfaced: {msg}"
    );
    assert!(!msg.contains("missing"), "must not claim the state is missing: {msg}");
    assert!(
        state_dir.exists(),
        "an io error must NOT trigger salvage deletion"
    );
}

/// Normal missing-state behavior is unchanged by the salvage path.
#[test]
fn missing_bisect_state_still_reports_no_operation() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    build_linear(d, 2);
    match bisect_reset(d) {
        Err(AppError::NoOperationInProgress(_)) => {}
        other => panic!("expected NoOperationInProgress, got {other:?}"),
    }
}

// ========================================= F-A3-2 interactive-rebase salvage

/// F-A3-2 (interactive + the plain-rebase delegation path): a handcrafted,
/// undecodable `.git/bonsai-rebase/state.json` is salvaged by `rebase_abort`
/// (which delegates on state-file existence): dir cleared, HEAD untouched,
/// distinct error; a second abort reports no rebase.
#[test]
fn corrupt_interactive_state_is_salvaged_by_abort() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    build_linear(d, 3);

    let state_dir = d.join(".git").join("bonsai-rebase");
    std::fs::create_dir_all(&state_dir).expect("mkdir");
    std::fs::write(state_dir.join("state.json"), "not json at all \u{0000}").expect("garbage");
    let head_before = rev(d, "HEAD");

    let err = rebase_abort(d).expect_err("corrupt state -> salvage error");
    let msg = err.to_string();
    assert!(msg.contains("corrupt"), "explains corruption: {msg}");
    assert!(msg.contains("cleared"), "explains the state was cleared: {msg}");
    assert!(msg.contains("HEAD was left"), "explains HEAD untouched: {msg}");

    assert!(!state_dir.exists(), "state dir removed by salvage");
    assert_eq!(rev(d, "HEAD"), head_before, "HEAD left where it was");

    match rebase_abort(d) {
        Err(AppError::NoOperationInProgress(_)) => {}
        other => panic!("expected NoOperationInProgress, got {other:?}"),
    }
}

// ================================================= F-A3-3 cross-start guards

/// F-A3-3: `start_bisect` refuses while an interactive-rebase state exists,
/// and `start_interactive_rebase` refuses while a bisect state exists — each
/// with a message naming the OTHER operation. (Reachable via a crash window:
/// both sequencers run detached with `repo.state() == Clean`.)
#[test]
fn cross_sequencer_start_guards_are_symmetric() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let oids = build_linear(d, 4);

    // Interactive state present -> bisect refuses.
    let rebase_dir = d.join(".git").join("bonsai-rebase");
    std::fs::create_dir_all(&rebase_dir).expect("mkdir");
    std::fs::write(rebase_dir.join("state.json"), "{}").expect("seed");
    let err = start_bisect(d, &oids[3], &[oids[0].clone()]).expect_err("blocked");
    match &err {
        AppError::OperationInProgress(m) => {
            assert!(m.contains("interactive rebase"), "names the other op: {m}")
        }
        other => panic!("expected OperationInProgress, got {other:?}"),
    }
    std::fs::remove_dir_all(&rebase_dir).expect("cleanup");

    // Bisect state present -> interactive rebase refuses.
    let bisect_dir = d.join(".git").join("bonsai-bisect");
    std::fs::create_dir_all(&bisect_dir).expect("mkdir");
    std::fs::write(bisect_dir.join("state.json"), "{}").expect("seed");
    let err = start_interactive_rebase(d, &oids[0], Vec::new()).expect_err("blocked");
    match &err {
        AppError::OperationInProgress(m) => {
            assert!(m.contains("bisect"), "names the other op: {m}")
        }
        other => panic!("expected OperationInProgress, got {other:?}"),
    }
}

// ============================================= F-A3-1 plain-abort clobber guard

/// F-A3-1: plain `rebase_abort` runs the untracked-clobber guard against the
/// orig-head tree. Fixture: topic = [t1 conflicts, t2 adds generated.txt];
/// the rebase pauses on t1, so `generated.txt` (present in the orig-head tree
/// via t2) is absent from the worktree. An untracked `generated.txt` created
/// during the pause would be silently overwritten by `rebase.abort()`'s hard
/// reset — the guard must refuse, leave the rebase state intact (retryable),
/// and abort must succeed once the file is removed.
#[test]
fn plain_rebase_abort_refuses_untracked_clobber_then_retries() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "line1\nbase\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n"); // t1: conflicts with main
    git(d, &["add", "-A"]);
    commit_fixed(d, "t1 conflicting");
    write(d, "generated.txt", "from t2\n"); // t2: adds generated.txt
    git(d, &["add", "-A"]);
    commit_fixed(d, "t2 adds generated");
    let topic_tip = rev(d, "HEAD");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main conflicting");
    git(d, &["checkout", "topic"]);

    match rebase_branch(d, "main").expect("rebase") {
        RebaseOutcome::Conflicts { paths, current_step, .. } => {
            assert_eq!(paths, vec!["a.txt".to_string()]);
            assert_eq!(current_step, 1, "paused on t1 — t2 not yet replayed");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert!(!d.join("generated.txt").exists(), "t2's file not in worktree yet");

    // User creates an untracked file colliding with the orig-head tree.
    write(d, "generated.txt", "precious untracked work\n");

    let err = rebase_abort(d).expect_err("guard must refuse the clobber");
    let msg = err.to_string();
    assert!(
        msg.contains("would be overwritten") && msg.contains("generated.txt"),
        "guard names the file: {msg}"
    );
    // Refusal is retryable: rebase state intact, file untouched.
    assert_ne!(
        git2::Repository::open(d).expect("open").state(),
        git2::RepositoryState::Clean,
        "rebase state must survive the refusal"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("generated.txt")).expect("read"),
        "precious untracked work\n",
        "untracked file untouched"
    );

    // Remove the file -> abort succeeds and restores the topic tip (including
    // t2's generated.txt).
    std::fs::remove_file(d.join("generated.txt")).expect("rm");
    rebase_abort(d).expect("abort succeeds after removing the collision");
    assert_eq!(rev(d, "HEAD"), topic_tip, "back on the original tip");
    assert_eq!(
        std::fs::read_to_string(d.join("generated.txt")).expect("read"),
        "from t2\n",
        "orig-head content restored"
    );
    assert_eq!(
        git2::Repository::open(d).expect("open").state(),
        git2::RepositoryState::Clean
    );
}
