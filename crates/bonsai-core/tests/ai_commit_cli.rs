//! P15a scratch-repo tests for `generate_commit_message` (contract §8.2).
//!
//! Drives a real staged git2 index on scratch repos, with the local `claude`
//! CLI replaced by the committed stub (`tests/fixtures/claude_stub.cmd`)
//! selected via `BONSAI_CLAUDE_BIN` + `BONSAI_STUB_MODE`. No network, no real
//! CLI. Mirrors the `ai_resolve_cli.rs` harness.
//!
//! Proves: (1) a staged repo → the stub body is returned, the cost is parsed,
//! and NOTHING is written (index/worktree unchanged); (2) an empty staged set
//! (index matches HEAD) → `NothingToCommit` before any CLI call; (3) the staged
//! file's added/deleted lines actually reach the CLI's stdin payload (via the
//! `dump_stdin` stub mode that captures its stdin to a file).
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_commit::generate_commit_message;
use common::{commit_fixed, git, init_repo};

const STUB_BODY: &str = "MERGED_BODY_OK";
const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
const STDIN_DUMP_ENV: &str = "BONSAI_STUB_STDIN_DUMP";

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
    common::claude_stub_path()
}

/// Point the AI layer at the committed stub in an arbitrary `BONSAI_STUB_MODE`.
fn set_stub_mode(mode: &str) {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, mode);
    std::env::remove_var(STDIN_DUMP_ENV);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// A blob of the index tree (staged, HEAD-independent snapshot) for `path`, or
/// None if `path` is not in the index. Used to prove a proposal writes nothing.
fn index_blob(dir: &Path, path: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(dir).expect("open repo");
    let index = repo.index().expect("index");
    let entry = index.get_path(Path::new(path), 0)?;
    let blob = repo.find_blob(entry.id).expect("blob");
    Some(blob.content().to_vec())
}

/// A scratch repo with an initial commit, then `a.txt` MODIFIED and STAGED
/// (index != HEAD, so there is something to commit). Returns the temp dir.
fn staged_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "line1\nbase\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    // Stage a modification: an added line and a removed line vs HEAD.
    write(d, "a.txt", "line1\nSTAGED_ADDED_LINE\nline3\n");
    git(d, &["add", "a.txt"]);
    dir
}

// ============================================================ §8.2 (1) proposal + writes nothing

#[test]
fn generate_returns_stub_body_and_writes_nothing() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("success");

    let dir = staged_repo();
    let d = dir.path();

    let worktree_before = std::fs::read(d.join("a.txt")).expect("read a.txt before");
    let index_before = index_blob(d, "a.txt").expect("a.txt staged before");
    let head_before = git(d, &["rev-parse", "HEAD"]);
    let status_before = git(d, &["status", "--porcelain=v1"]);

    let proposal = generate_commit_message(d, RunOpts::default()).expect("proposal on staged repo");

    assert_eq!(proposal.message, STUB_BODY, "message must be the stub body");
    assert_eq!(proposal.cost_usd, Some(0.012), "cost parsed from the envelope");

    // WRITES NOTHING: no commit created, index & worktree bytes unchanged.
    assert_eq!(git(d, &["rev-parse", "HEAD"]), head_before, "HEAD must not move");
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt after"),
        worktree_before,
        "worktree bytes must be untouched"
    );
    assert_eq!(
        index_blob(d, "a.txt").expect("a.txt still staged"),
        index_before,
        "the staged index blob must be untouched"
    );
    assert_eq!(
        git(d, &["status", "--porcelain=v1"]),
        status_before,
        "porcelain status must be identical before/after (nothing staged/unstaged)"
    );
}

// ============================================================ §8.2 (2) empty staged → NothingToCommit

#[test]
fn empty_staged_index_maps_to_nothing_to_commit_no_cli_call() {
    require_git!();
    let _g = env_lock();
    // Point at a mode that WOULD blow up loudly if it ran, to prove no CLI call:
    // `nonzero` exits 1 with stderr → would surface as AiFailed, not
    // NothingToCommit. Getting NothingToCommit proves the guard fired first.
    set_stub_mode("nonzero");

    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "only committed content\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    // Clean index: nothing staged (index == HEAD).

    let err = generate_commit_message(d, RunOpts::default())
        .expect_err("clean index must be NothingToCommit");
    assert!(
        matches!(err, AppError::NothingToCommit),
        "expected NothingToCommit (before any CLI call), got {err:?}"
    );
}

// ============================================================ §8.2 (3) payload carries staged lines

#[test]
fn payload_contains_staged_added_and_removed_lines() {
    require_git!();
    let _g = env_lock();

    let dir = init_repo();
    let d = dir.path();
    // Committed baseline for BOTH files.
    write(d, "a.txt", "line1\nbase\nline3\n");
    write(d, "b.txt", "keepme\nDELETED_STAGED_LINE\ntail\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    // Stage an ADD in a.txt and a DELETE in b.txt simultaneously (no commit
    // between — both must appear in one `git diff --cached` payload).
    write(d, "a.txt", "line1\nSTAGED_ADDED_LINE\nline3\n");
    write(d, "b.txt", "keepme\ntail\n");
    git(d, &["add", "-A"]);

    // Capture the stdin the stub receives.
    let dump = d.join("stdin_dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);

    let proposal =
        generate_commit_message(d, RunOpts::default()).expect("proposal (dump_stdin stub)");
    // dump_stdin still emits the success envelope body.
    assert_eq!(proposal.message, STUB_BODY);

    std::env::remove_var(STDIN_DUMP_ENV);

    let payload = std::fs::read_to_string(&dump).expect("stub must have written the stdin dump");

    // The one-line payload header from ai_commit.rs.
    assert!(
        payload.contains("STAGED CHANGES (git diff --cached):"),
        "payload should carry the staged-changes header; got:\n{payload}"
    );
    // The added line reaches stdin as a `+`-prefixed diff line.
    assert!(
        payload.contains("+STAGED_ADDED_LINE"),
        "payload should contain the staged ADDED line; got:\n{payload}"
    );
    // The removed line reaches stdin as a `-`-prefixed diff line.
    assert!(
        payload.contains("-DELETED_STAGED_LINE"),
        "payload should contain the staged DELETED line; got:\n{payload}"
    );
    // Both staged file paths appear as FILE labels.
    assert!(
        payload.contains("a.txt") && payload.contains("b.txt"),
        "payload should label both staged files; got:\n{payload}"
    );
}
