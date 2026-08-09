//! Shared helpers for the M3 CLI-oracle integration tests.
//!
//! HARD RULE (M3 contract §6.0): on Windows, C: is critically full — every
//! scratch repo lives under `D:\Temp\bonsai-scratch`, never the system temp.
//! On macOS/Linux there is no such constraint, so scratch dirs fall back to
//! `std::env::temp_dir()/bonsai-scratch`. This mirrors `src/testutil.rs` (a
//! `#[cfg(test)]` lib module cannot be linked from integration binaries, so
//! the helper is duplicated here).

#![allow(dead_code)] // each test binary uses a subset of these helpers

use std::path::Path;
use std::process::Command;

/// Fixed dates for base-history CLI commits so twin repos produce identical
/// base oids (M3 contract §6.2).
pub const FIXED_DATE: &str = "2026-01-02T03:04:05+0000";

#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    Path::new("D:\\Temp\\bonsai-scratch").to_path_buf()
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// Creates a scratch temp dir under the platform scratch root (created if
/// absent). Use this — never `TempDir::new()` — for every fixture.
pub fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Builds a `file://` URL for a local path. On POSIX the path already starts
/// with `/`, so `file://` + path gives the correct 3-slash form; prepending a
/// bare `file:///` unconditionally (as Windows drive paths need) double-slashes
/// it into `file:////...`, which libgit2 rejects as "not a valid local file
/// URI" even though the real `git` CLI tolerates it.
pub fn file_url(path: &Path) -> String {
    let s = path.display().to_string().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

/// Path to the committed `claude` CLI stub used by the P13/P15 AI-integration
/// tests, selected via `BONSAI_CLAUDE_BIN`. Windows runs the `.cmd` stub
/// directly (`Command::new` routes `.cmd` through cmd.exe automatically);
/// macOS/Linux use the POSIX `.sh` twin, with the executable bit forced on at
/// test time — git doesn't reliably preserve the mode bit across
/// clones/platforms.
pub fn claude_stub_path() -> std::path::PathBuf {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if cfg!(windows) {
        fixtures.join("claude_stub.cmd")
    } else {
        let path = fixtures.join("claude_stub.sh");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(perms.mode() | 0o111);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }
        path
    }
}

pub fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

/// True when the git on PATH is ≥ `(major, minor)`. The P59a-2 `pre-push` hook
/// oracle needs `git hook run` (Git ≥ 2.36); tests skip below that.
pub fn git_version_at_least(major: u32, minor: u32) -> bool {
    let out = match Command::new("git").arg("--version").output() {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    let s = String::from_utf8_lossy(&out.stdout);
    let ver = s.split_whitespace().nth(2).unwrap_or("");
    let mut it = ver.split('.');
    let maj: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let min: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    maj > major || (maj == major && min >= minor)
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

/// Write an executable `#!/bin/sh` `pre-push` hook into `repo/.git/hooks` with
/// LF endings (git's bundled `sh` runs it on Windows too — the point of
/// `git hook run`). `body` is the script AFTER the shebang. The P59a-2 pre-push
/// oracle uses this against a local bare remote.
pub fn write_pre_push_hook(repo: &Path, body: &str) {
    let hooks = repo.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).expect("mkdir hooks");
    let path = hooks.join("pre-push");
    std::fs::write(&path, format!("#!/bin/sh\n{body}").replace("\r\n", "\n")).expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
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
