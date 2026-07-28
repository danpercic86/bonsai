//! Shared helpers for the M3 CLI-oracle integration tests.
//!
//! HARD RULE (M3 contract §6.0): C: is critically full — every scratch repo
//! lives under `D:\Temp\bonsai-scratch`, never the system temp. This mirrors
//! `src/testutil.rs` (a `#[cfg(test)]` lib module cannot be linked from
//! integration binaries, so the helper is duplicated here).

#![allow(dead_code)] // each test binary uses a subset of these helpers

use std::path::Path;
use std::process::Command;

/// Fixed dates for base-history CLI commits so twin repos produce identical
/// base oids (M3 contract §6.2).
pub const FIXED_DATE: &str = "2026-01-02T03:04:05+0000";

/// Creates a scratch temp dir under `D:\Temp\bonsai-scratch` (created if
/// absent). Use this — never `TempDir::new()` — for every fixture.
pub fn scratch_dir() -> tempfile::TempDir {
    let root = Path::new("D:\\Temp\\bonsai-scratch");
    std::fs::create_dir_all(root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-")
        .tempdir_in(root)
        .expect("scratch dir")
}

pub fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

/// Runs `git <args>` in `dir`, asserting success; returns trimmed stdout.
pub fn git(dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&git_raw(dir, args, &[])).trim().to_string()
}

/// Runs `git <args>` in `dir` with extra env vars, asserting success;
/// returns trimmed stdout.
pub fn git_env(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> String {
    String::from_utf8_lossy(&git_raw(dir, args, envs)).trim().to_string()
}

/// Runs `git <args>` in `dir`, asserting success; returns RAW stdout bytes
/// (no trimming — needed for byte-exact commit-object comparison).
pub fn git_raw(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Vec<u8> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// Runs `git <args>` in `dir` and reports only whether it succeeded.
pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `git init -b main` + deterministic local config in a fresh scratch dir.
pub fn init_repo() -> tempfile::TempDir {
    let dir = scratch_dir();
    let path = dir.path();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "user.name", "Test User"]);
    git(path, &["config", "user.email", "test@example.com"]);
    git(path, &["config", "status.renames", "true"]);
    git(path, &["config", "core.autocrlf", "false"]);
    dir
}

/// CLI commit with fixed author/committer dates (deterministic oid across
/// twin repos built by the same script).
pub fn commit_fixed(dir: &Path, msg: &str) {
    git_env(
        dir,
        &["commit", "-m", msg],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

/// `git status --porcelain=v1 -z --untracked-files=all` parsed into sorted
/// records: `(XY + ' ' + path, Some(orig_path) for renames)`. Sorted so
/// ordering differences never matter; byte-identical content is the oracle.
pub fn porcelain_records(dir: &Path) -> Vec<(String, Option<String>)> {
    let raw = git_raw(
        dir,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        &[],
    );
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let mut tokens = raw.split('\0').filter(|t| !t.is_empty());

    let mut records = Vec::new();
    while let Some(token) = tokens.next() {
        let mut chars = token.chars();
        let x = chars.next().expect("X column");
        let y = chars.next().expect("Y column");
        // Rename entries carry the ORIG path as the next NUL token.
        let orig = if x == 'R' || y == 'R' {
            Some(tokens.next().expect("rename orig path token").to_string())
        } else {
            None
        };
        records.push((token.to_string(), orig));
    }
    records.sort();
    records
}

/// Asserts that repos `a` and `b` have byte-identical porcelain status.
pub fn assert_same_status(a: &Path, b: &Path) {
    assert_eq!(
        porcelain_records(a),
        porcelain_records(b),
        "porcelain status of git2 repo (left) differs from CLI twin (right)"
    );
}
