//! M0 cross-check: `read_repo_info`'s view of a repo must match the real `git` CLI.
//!
//! Unlike the git2-only unit tests in `src/git/repo.rs` (contract §9), this test
//! deliberately builds its fixture with the `git` CLI and compares `read_repo_info`
//! output against `git symbolic-ref --short HEAD` and `git rev-parse HEAD`.
//! Skips (passes with a note) if `git` is not on PATH.

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8 git output").trim().to_string()
}

#[test]
fn read_repo_info_matches_git_cli() {
    // Skip cleanly when git is unavailable (CI without git).
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("skipping: `git` CLI not found on PATH");
        return;
    }

    let dir = tempfile::TempDir::new().expect("create temp dir");
    let path = dir.path();

    git(path, &["init"]);
    git(path, &["config", "user.name", "Test User"]);
    git(path, &["config", "user.email", "test@example.com"]);
    std::fs::write(path.join("hello.txt"), "hello bonsai\n").expect("write file");
    git(path, &["add", "hello.txt"]);
    git(path, &["commit", "-m", "initial commit"]);

    let cli_branch = git(path, &["symbolic-ref", "--short", "HEAD"]);
    let cli_oid = git(path, &["rev-parse", "HEAD"]);

    let info = bonsai::git::repo::read_repo_info(path).expect("read_repo_info");
    assert!(info.is_repo, "CLI-created repo must be reported is_repo=true");
    let head = info.head.expect("head present for a repo with a commit");
    assert!(!head.unborn);
    assert!(!head.detached);
    assert_eq!(
        head.branch_name.as_deref(),
        Some(cli_branch.as_str()),
        "branch name must match `git symbolic-ref --short HEAD`"
    );
    assert_eq!(head.oid, cli_oid, "oid must match `git rev-parse HEAD`");
}
