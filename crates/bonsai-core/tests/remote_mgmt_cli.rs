//! P22 CLI-oracle remote-management tests (contract §8.2).
//!
//! All operations are LOCAL config ops — no network, no credentials. Remote
//! URLs are local paths / `file://` / dummy URLs (git never connects for
//! add/remove/rename/set-url). All scratch repos live under
//! `D:\Temp\bonsai-scratch`.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use bonsai_core::error::AppError;
use bonsai_core::git::remote::{
    add_remote, list_remotes, remove_remote, rename_remote, set_remote_url,
};
use common::{git, git_ok, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

const URL_A: &str = "https://example.com/a.git";
const URL_B: &str = "https://example.com/b.git";

// ------------------------------------------------------------------ §8.2.1 add

/// `add_remote` matches `git remote add`; duplicate → Git; bad name →
/// InvalidName.
#[test]
fn add_remote_parity_and_errors() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    add_remote(path, "backup", URL_A).expect("add backup");

    // `git remote -v` lists it with the url.
    let remotes = git(path, &["remote", "-v"]);
    assert!(
        remotes.contains("backup") && remotes.contains(URL_A),
        "git remote -v missing backup+url: {remotes}"
    );
    assert_eq!(git(path, &["remote", "get-url", "backup"]), URL_A);

    // Equals the CLI twin `git remote add`.
    git(path, &["remote", "add", "backup2", URL_A]);
    assert_eq!(
        git(path, &["remote", "get-url", "backup"]),
        git(path, &["remote", "get-url", "backup2"]),
    );

    // Duplicate → Git.
    match add_remote(path, "backup", URL_B) {
        Err(AppError::Git(m)) => assert!(m.contains("already exists"), "{m}"),
        other => panic!("expected Git(already exists), got {other:?}"),
    }

    // Invalid name (whitespace) → InvalidName.
    match add_remote(path, "bad name", URL_A) {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName, got {other:?}"),
    }
}

// ----------------------------------------------------------------- §8.2.2 list

/// `list_remotes` returns all configured remotes sorted (name+url), matching
/// `git remote` / `git remote get-url`; empty repo → `Ok(vec![])`.
#[test]
fn list_remotes_parity_and_empty() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    // Empty repo → no remotes, NOT an error.
    assert_eq!(list_remotes(path).expect("list empty"), vec![]);

    // Add out of order; expect case-insensitive sorted output.
    add_remote(path, "zeta", URL_B).expect("add zeta");
    add_remote(path, "alpha", URL_A).expect("add alpha");

    let listed = list_remotes(path).expect("list");
    let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "zeta"], "sorted by name");

    // Cross-check each name+url against the CLI.
    for r in &listed {
        let cli_url = git(path, &["remote", "get-url", &r.name]);
        assert_eq!(r.url.as_deref(), Some(cli_url.as_str()), "url for {}", r.name);
    }
    // The set of names matches `git remote`.
    let mut cli_names: Vec<String> = git(path, &["remote"])
        .lines()
        .map(|s| s.to_string())
        .collect();
    cli_names.sort();
    assert_eq!(cli_names, vec!["alpha".to_string(), "zeta".to_string()]);
}

// --------------------------------------------------------------- §8.2.3 rename

/// `rename_remote` matches `git remote rename`, moving `refs/remotes/<name>/*`;
/// missing → NoRemote; target exists → Git.
#[test]
fn rename_remote_parity_and_errors() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    // Seed a remote plus a fake remote-tracking ref to prove it moves.
    add_remote(path, "origin", URL_A).expect("add origin");
    std::fs::write(path.join("f.txt"), "x\n").expect("write");
    git(path, &["add", "-A"]);
    common::commit_fixed(path, "c1");
    let head = git(path, &["rev-parse", "HEAD"]);
    git(path, &["update-ref", "refs/remotes/origin/main", &head]);
    assert!(git_ok(path, &["show-ref", "--verify", "refs/remotes/origin/main"]));

    rename_remote(path, "origin", "upstream").expect("rename");

    // `git remote` shows the new name only.
    let remotes = git(path, &["remote"]);
    assert!(remotes.contains("upstream"), "no upstream: {remotes}");
    assert!(!remotes.contains("origin"), "origin still present: {remotes}");
    // Tracking refs moved.
    assert!(
        git_ok(path, &["show-ref", "--verify", "refs/remotes/upstream/main"]),
        "tracking ref not moved to upstream"
    );
    assert!(!git_ok(path, &["show-ref", "--verify", "refs/remotes/origin/main"]));
    // URL preserved (== CLI `git remote rename` semantics).
    assert_eq!(git(path, &["remote", "get-url", "upstream"]), URL_A);

    // Missing → NoRemote.
    match rename_remote(path, "nope", "whatever") {
        Err(AppError::NoRemote(_)) => {}
        other => panic!("expected NoRemote, got {other:?}"),
    }

    // Target exists → Git.
    add_remote(path, "backup", URL_B).expect("add backup");
    match rename_remote(path, "backup", "upstream") {
        Err(AppError::Git(m)) => assert!(m.contains("already exists"), "{m}"),
        other => panic!("expected Git(already exists), got {other:?}"),
    }
}

/// Coverage gap: renaming a remote that has NO remote-tracking refs must still
/// succeed (the git2 rename returns a non-default-refspec "problem" list which
/// the contract says to log-and-ignore, §3.4) — it must not surface as an error.
#[test]
fn rename_remote_without_tracking_refs() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    // Fresh remote, never fetched → zero refs/remotes/origin/* exist.
    add_remote(path, "origin", URL_A).expect("add origin");
    assert!(!git_ok(path, &["show-ref", "--verify", "refs/remotes/origin/main"]));

    rename_remote(path, "origin", "upstream").expect("rename with no tracking refs must succeed");

    let remotes = git(path, &["remote"]);
    assert!(remotes.contains("upstream"), "no upstream: {remotes}");
    assert!(!remotes.contains("origin"), "origin still present: {remotes}");
    assert_eq!(git(path, &["remote", "get-url", "upstream"]), URL_A);
}

/// Coverage gap: remote names and branch names live in DIFFERENT ref namespaces
/// (refs/remotes vs refs/heads), so adding a remote whose name equals an
/// existing branch must NOT be rejected as a collision — parity with the CLI.
#[test]
fn add_remote_name_collides_with_branch() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    // Create a real branch named "main" via a first commit.
    std::fs::write(path.join("f.txt"), "x\n").expect("write");
    git(path, &["add", "-A"]);
    common::commit_fixed(path, "c1");
    assert!(git_ok(path, &["show-ref", "--verify", "refs/heads/main"]));

    // A remote also named "main" is legal (distinct namespace).
    add_remote(path, "main", URL_A).expect("remote named like a branch must be allowed");

    assert_eq!(git(path, &["remote", "get-url", "main"]), URL_A);
    // The branch is untouched.
    assert!(git_ok(path, &["show-ref", "--verify", "refs/heads/main"]));
    // Equals the CLI twin.
    git(path, &["remote", "add", "main2", URL_A]);
    assert_eq!(
        git(path, &["remote", "get-url", "main"]),
        git(path, &["remote", "get-url", "main2"]),
    );
}

// --------------------------------------------------------------- §8.2.4 seturl

/// `set_remote_url` matches `git remote set-url`; missing → NoRemote.
#[test]
fn set_remote_url_parity_and_error() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    add_remote(path, "origin", URL_A).expect("add origin");
    set_remote_url(path, "origin", URL_B).expect("set-url");

    assert_eq!(git(path, &["remote", "get-url", "origin"]), URL_B);
    // list_remotes reflects the new url too.
    let listed = list_remotes(path).expect("list");
    assert_eq!(listed[0].url.as_deref(), Some(URL_B));

    // Missing → NoRemote.
    match set_remote_url(path, "nope", URL_A) {
        Err(AppError::NoRemote(_)) => {}
        other => panic!("expected NoRemote, got {other:?}"),
    }
}

// --------------------------------------------------------------- §8.2.5 remove

/// `remove_remote` matches `git remote remove`, dropping its tracking refs;
/// missing → NoRemote.
#[test]
fn remove_remote_parity_and_error() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();

    add_remote(path, "origin", URL_A).expect("add origin");
    std::fs::write(path.join("f.txt"), "x\n").expect("write");
    git(path, &["add", "-A"]);
    common::commit_fixed(path, "c1");
    let head = git(path, &["rev-parse", "HEAD"]);
    git(path, &["update-ref", "refs/remotes/origin/main", &head]);

    remove_remote(path, "origin").expect("remove");

    // Gone from `git remote`.
    assert!(!git(path, &["remote"]).contains("origin"));
    // Tracking refs gone.
    assert!(!git_ok(path, &["show-ref", "--verify", "refs/remotes/origin/main"]));
    // list_remotes no longer lists it.
    assert!(list_remotes(path).expect("list").is_empty());

    // Missing → NoRemote.
    match remove_remote(path, "origin") {
        Err(AppError::NoRemote(_)) => {}
        other => panic!("expected NoRemote, got {other:?}"),
    }
}
