//! P13c scratch-repo tests for `ai_resolve_conflict` (contract §10.3).
//!
//! Drives real git2 merge conflicts on scratch repos (reusing the `common`
//! `init_repo`/`git`/`commit_fixed` harness), with the local `claude` CLI
//! replaced by the committed stub (`tests/fixtures/claude_stub.cmd`) selected
//! via `BONSAI_CLAUDE_BIN` + `BONSAI_STUB_MODE`. No network, no real CLI.
//!
//! Proves: (1) a proposal is produced and WRITES NOTHING; (2) feeding the
//! proposed text to `resolve_conflict_text` clears the conflict and lets
//! `commit_merge` finalize a clean 2-parent commit; (3) binary/too-large/
//! deletion-kind conflicts short-circuit to `AiFailed` before any CLI call;
//! (4) non-conflicted path → `git`, `../escape` → `invalidName`.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_lib::ai::RunOpts;
use bonsai_lib::error::AppError;
use bonsai_lib::git::ai_resolve::ai_resolve_conflict;
use bonsai_lib::git::conflict::{resolve_conflict_text, MAX_CONFLICT_BYTES};
use bonsai_lib::git::merge::{commit_merge, merge_branch, MergeOutcome};
use common::{commit_fixed, git, init_repo};

const STUB_BODY: &str = "MERGED_BODY_OK";
const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
/// process-global and the stub inherits them, so parallel tests would race.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn stub_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude_stub.cmd")
}

/// Point the AI layer at the committed stub in `success` mode.
fn set_success_stub() {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "success");
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// True iff `path` currently has an index conflict record (any stage).
fn is_conflicted(dir: &Path, path: &str) -> bool {
    let repo = git2::Repository::open(dir).expect("open repo");
    let index = repo.index().expect("index");
    for c in index.conflicts().expect("conflicts") {
        let c = c.expect("conflict record");
        let p = c
            .our
            .as_ref()
            .or(c.their.as_ref())
            .or(c.ancestor.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).into_owned());
        if p.as_deref() == Some(path) {
            return true;
        }
    }
    false
}

/// Stage-0 blob bytes for `path`, or None when the path is still conflicted /
/// absent from the index at stage 0.
fn stage0_blob(dir: &Path, path: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(dir).expect("open repo");
    let index = repo.index().expect("index");
    let entry = index.get_path(Path::new(path), 0)?;
    let blob = repo.find_blob(entry.id).expect("blob");
    Some(blob.content().to_vec())
}

/// Parent oids of HEAD, in order.
fn parents(dir: &Path) -> Vec<String> {
    git(dir, &["log", "-1", "--format=%P"])
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// Builds a scratch repo paused in a `bothModified` merge conflict on `a.txt`
/// (ours = "main", theirs = "topic"), plus an untracked-safe tracked `keep.txt`.
fn both_modified_conflict() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "line1\nbase\nline3\n");
    write(d, "keep.txt", "keep\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");

    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { paths, .. } => {
            assert!(paths.iter().any(|p| p == "a.txt"), "expected a.txt conflict");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    dir
}

/// Builds a scratch repo paused in a `deletedByThem` conflict on `a.txt`
/// (topic deletes it, main modifies it).
fn deleted_by_them_conflict() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "base\n");
    write(d, "keep.txt", "keep\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    git(d, &["rm", "a.txt"]);
    commit_fixed(d, "topic deletes a.txt");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "modified by main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main modifies a.txt");

    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { paths, .. } => {
            assert!(paths.iter().any(|p| p == "a.txt"), "expected a.txt conflict");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    dir
}

// ============================================================ §10.3 (1) proposal writes nothing

#[test]
fn proposal_returns_stub_body_and_writes_nothing() {
    require_git!();
    let _g = env_lock();
    set_success_stub();

    let dir = both_modified_conflict();
    let d = dir.path();
    let before = std::fs::read(d.join("a.txt")).expect("read a.txt before");

    let proposal =
        ai_resolve_conflict(d, "a.txt", RunOpts::default()).expect("proposal on a text conflict");

    assert_eq!(proposal.path, "a.txt");
    assert_eq!(proposal.proposed_text, STUB_BODY, "text must be the stub body");
    assert_eq!(proposal.cost_usd, Some(0.012), "cost parsed from the envelope");

    // WRITES NOTHING: still conflicted, worktree bytes unchanged, no stage-0.
    assert!(is_conflicted(d, "a.txt"), "a.txt must still be conflicted");
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt after"),
        before,
        "worktree bytes must be untouched by a proposal"
    );
    assert!(
        stage0_blob(d, "a.txt").is_none(),
        "a conflicted path must have no stage-0 entry after a proposal"
    );
}

// ============================================================ §10.3 (2) apply + commit_merge

#[test]
fn applying_proposal_clears_conflict_and_commit_merge_finalizes() {
    require_git!();
    let _g = env_lock();
    set_success_stub();

    let dir = both_modified_conflict();
    let d = dir.path();
    let pre_head = git(d, &["rev-parse", "HEAD"]);

    let proposal = ai_resolve_conflict(d, "a.txt", RunOpts::default()).expect("proposal");

    // Apply via the EXISTING resolve_conflict_text primitive (no new command).
    resolve_conflict_text(d, "a.txt", &proposal.proposed_text).expect("apply proposal");

    // Conflict gone; stage-0 blob == the applied bytes; worktree bytes match.
    assert!(!is_conflicted(d, "a.txt"), "a.txt must no longer be conflicted");
    assert_eq!(
        stage0_blob(d, "a.txt").as_deref(),
        Some(proposal.proposed_text.as_bytes()),
        "stage-0 blob must equal the applied proposal bytes"
    );
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt"),
        proposal.proposed_text.as_bytes(),
        "worktree bytes must equal the applied proposal bytes"
    );

    // commit_merge finalizes a clean 2-parent merge commit.
    let result = commit_merge(d, "Merge branch 'topic'").expect("commit_merge");
    assert_eq!(result.oid, git(d, &["rev-parse", "HEAD"]));
    let p = parents(d);
    assert_eq!(p.len(), 2, "merge commit must have 2 parents");
    assert_eq!(p[0], pre_head, "first parent must be the pre-merge HEAD");
    assert_eq!(
        git2::Repository::open(d).expect("open").state(),
        git2::RepositoryState::Clean,
        "repo must be Clean after commit_merge"
    );
}

// ============================================================ §10.3 (3) guards short-circuit to AiFailed

#[test]
fn binary_too_large_and_deletion_kinds_short_circuit_to_ai_failed() {
    require_git!();
    let _g = env_lock();
    // The stub is pointed at even though the guards must fire BEFORE any CLI
    // call — a proposal here would prove the guard leaked.
    set_success_stub();

    // Binary worktree file.
    let dir = both_modified_conflict();
    std::fs::write(dir.path().join("a.txt"), b"\x00\x01binary blob").expect("write binary");
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("binary must be AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "got {err:?}");

    // Too large (> 1 MiB).
    let dir = both_modified_conflict();
    std::fs::write(
        dir.path().join("a.txt"),
        vec![b'a'; MAX_CONFLICT_BYTES as usize + 1],
    )
    .expect("write huge");
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("too_large must be AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "got {err:?}");

    // Deletion-kind conflict (deletedByThem) — no text merge.
    let dir = deleted_by_them_conflict();
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("deletion kind must be AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "got {err:?}");
}

// ============================================================ §10.3 (4) path guards

#[test]
fn non_conflicted_and_escape_paths_error() {
    require_git!();
    let _g = env_lock();
    set_success_stub();

    let dir = both_modified_conflict();
    let d = dir.path();

    // Non-conflicted (but valid) tracked path → git "has no conflict".
    let err = ai_resolve_conflict(d, "keep.txt", RunOpts::default())
        .expect_err("non-conflicted path must error");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // Traversal / absolute paths → invalidName (validated before get_conflict).
    for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
        let err = ai_resolve_conflict(d, bad, RunOpts::default()).expect_err("escape path");
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "path {bad:?}: expected InvalidName, got {err:?}"
        );
    }
}
