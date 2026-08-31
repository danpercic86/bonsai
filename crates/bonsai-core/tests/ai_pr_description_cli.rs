//! P64b scratch-repo tests for `generate_pr_description` (contract §4a/§6).
//!
//! Drives the real `generate_pr_description` pipeline against a git-CLI-built
//! scratch repo (two branches) with the committed `claude` stub selected via
//! `BONSAI_CLAUDE_BIN` — NO real CLI, NO network:
//!   - a canned multi-line stub reply parses into `PrDescription{title, body}`
//!     with the resolved base/head echoed and the range commit count;
//!   - the captured stdin payload carries a `COMMITS (head since base):` block
//!     and a `NET CHANGES (diffstat):` section (grounding assembly);
//!   - `base == head` (empty range) => `AiFailed` BEFORE any CLI call (a fake bin
//!     path would surface as `AiUnavailable` if spawned).
//!
//! Lives in its OWN test binary so the process-global `BONSAI_CLAUDE_BIN` cannot
//! race the lib unit tests (mirrors `ai_changelog_cli.rs`). All scratch repos
//! live under `D:\Data\Temp\bonsai-scratch` (C: is full). Each test skips (passes with
//! a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_pr_description::{generate_pr_description, PrDescription};
use common::{git, init_repo};

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

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn stub_path() -> std::path::PathBuf {
    common::claude_stub_path()
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// `main` = A -> B, then `feature` = B -> C -> D (branched off B). `main..feature`
/// is exactly {C, D}. HEAD is left on `feature`.
fn two_branch_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "f.txt", "base\n");
    git(d, &["add", "-A"]);
    git(d, &["commit", "-m", "A base"]);
    write(d, "f.txt", "base\nb\n");
    git(d, &["add", "-A"]);
    git(d, &["commit", "-m", "B main work"]);
    git(d, &["checkout", "-b", "feature"]);
    write(d, "f.txt", "base\nb\nc\n");
    git(d, &["add", "-A"]);
    git(d, &["commit", "-m", "feat: add C"]);
    write(d, "f.txt", "base\nb\nc\nd\n");
    git(d, &["add", "-A"]);
    git(d, &["commit", "-m", "fix: fix D"]);
    dir
}

// ============================================================ §6 stub echo

/// A canned multi-line stub reply parses into title + body; the resolved
/// base/head are echoed and the range's two unique commits are counted. The
/// `success_crlf` stub emits CRLF line endings — proving the parser is
/// CRLF-tolerant (it re-joins the body with LF).
#[test]
fn stub_reply_parses_title_and_body() {
    require_git!();
    let _g = env_lock();

    let dir = two_branch_repo();
    let d = dir.path();

    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "success_crlf");
    let out = generate_pr_description(d, "main", "feature", RunOpts::default());
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);

    let pr: PrDescription = out.unwrap_or_else(|e| panic!("generate should succeed: {e:?}"));
    // success_crlf result body = "L1\r\nL2\r\nL3\r\n" → title "L1", body "L2\nL3".
    assert_eq!(pr.title, "L1");
    assert_eq!(pr.body, "L2\nL3");
    assert_eq!(pr.base, "main");
    assert_eq!(pr.head, "feature");
    assert_eq!(pr.commit_count, 2, "main..feature = {{C, D}}");
    assert_eq!(pr.cost_usd, Some(0.02));
}

// ============================================================ §4a payload grounding

/// The stdin payload the CLI receives carries a `COMMITS (head since base):`
/// block (the two range commits, newest first) and a `NET CHANGES (diffstat):`
/// section listing the changed file.
#[test]
fn payload_carries_commits_and_diffstat() {
    require_git!();
    let _g = env_lock();

    let dir = two_branch_repo();
    let d = dir.path();
    let dump = d.join("pr_dump.txt");

    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);
    let out = generate_pr_description(d, "main", "feature", RunOpts::default());
    std::env::remove_var(STDIN_DUMP_ENV);
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);
    out.unwrap_or_else(|e| panic!("generate should succeed: {e:?}"));

    // find.exe re-emits the payload with CRLF; normalize to LF for matching.
    let payload = std::fs::read_to_string(&dump)
        .expect("stub wrote stdin dump")
        .replace("\r\n", "\n");
    assert!(
        payload.contains("COMMITS (head since base):\n"),
        "payload lacks COMMITS block:\n{payload}"
    );
    assert!(
        payload.contains("\nNET CHANGES (diffstat):\n"),
        "payload lacks NET CHANGES diffstat:\n{payload}"
    );
    assert!(payload.contains("f.txt"), "diffstat lacks the changed file:\n{payload}");
    // Both unique commit summaries reach the grounding.
    assert!(payload.contains("feat: add C"), "COMMITS lacks C:\n{payload}");
    assert!(payload.contains("fix: fix D"), "COMMITS lacks D:\n{payload}");
}

// ============================================================ §4a empty-range guard

/// `base == head` (no unique commits AND no changed files) => `AiFailed` BEFORE
/// any CLI call. `BONSAI_CLAUDE_BIN` points at a nonexistent path: a regressed
/// spawn would return `AiUnavailable` (a DIFFERENT variant).
#[test]
fn empty_range_fails_before_cli() {
    require_git!();
    let _g = env_lock();

    let dir = two_branch_repo();
    let d = dir.path();

    std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/claude-must-not-spawn.exe");
    let out = generate_pr_description(d, "feature", "feature", RunOpts::default());
    std::env::remove_var(CLAUDE_BIN_ENV);

    match out.expect_err("empty range must fail") {
        AppError::AiFailed(m) => {
            assert_eq!(m, "nothing to describe: feature has no commits beyond feature");
        }
        other => {
            panic!("expected AiFailed (pre-CLI), got {other:?} — a spawn would be AiUnavailable")
        }
    }
}
