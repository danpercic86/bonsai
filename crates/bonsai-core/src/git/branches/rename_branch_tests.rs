//! P60a (contract §"P60a — Branch rename"): `rename_branch` moves the ref +
//! reflog + `branch.<name>.*` config (upstream SURVIVES), rewrites HEAD when
//! the renamed branch is checked out (`was_head`), and maps the error cases.
//! Fixtures are built with git2 in a scratch dir; a `have_git()` CLI-oracle
//! asserts parity with `git branch -m old new`.

use super::*;
use std::process::Command;

/// Init a scratch repo with a deterministic identity + autocrlf off.
fn rb_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
fn rb_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
    use crate::git::stage::stage_paths;
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    stage_paths(
        dir,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
}

/// Create local branch `name` at the current HEAD commit (no checkout).
fn rb_branch_at_head(repo: &git2::Repository, name: &str) {
    let head_oid = repo.head().expect("HEAD").target().expect("oid");
    let commit = repo.find_commit(head_oid).expect("commit");
    repo.branch(name, &commit, false).expect("create branch");
}

/// Configure `origin/<local>` as `local`'s upstream WITHOUT any network fetch
/// (dummy `origin` remote + a remote-tracking ref + `branch.<local>.remote/
/// merge` config) — identical recipe to `checkout_autostash_tests`.
fn rb_set_upstream(repo: &git2::Repository, local: &str, oid: git2::Oid) {
    if repo.find_remote("origin").is_err() {
        repo.remote("origin", "https://example.invalid/x.git")
            .expect("remote");
    }
    repo.reference(&format!("refs/remotes/origin/{local}"), oid, true, "seed upstream")
        .expect("remote-tracking ref");
    let mut cfg = repo.config().expect("config");
    cfg.set_str(&format!("branch.{local}.remote"), "origin")
        .expect("remote cfg");
    cfg.set_str(
        &format!("branch.{local}.merge"),
        &format!("refs/heads/{local}"),
    )
    .expect("merge cfg");
}

/// The short branch name HEAD points at, or None when detached/unborn.
fn rb_head_branch(dir: &Path) -> Option<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    head.shorthand().ok().map(str::to_string)
}

/// True when local branch `name` does not exist in the repo at `dir`.
fn rb_branch_absent(dir: &Path, name: &str) -> bool {
    let repo = git2::Repository::open(dir).expect("open");
    // Bind before the block ends so the `Result<Branch,_>` temporary (which
    // borrows `repo`) is dropped before `repo` — mirrors `cbh_branch_absent`.
    let absent = repo.find_branch(name, git2::BranchType::Local).is_err();
    absent
}

fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

// ---------------------------------------------- moves the ref, tip preserved

/// Renaming a non-checked-out branch moves the ref to the new name (old gone,
/// same tip) and reports `was_head=false`.
#[test]
fn rename_moves_ref_preserving_tip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");
    rb_branch_at_head(&repo, "old");

    let res = rename_branch(d, "old", "new").expect("rename");
    assert!(!res.was_head, "renaming a non-checked-out branch → was_head=false");

    assert!(rb_branch_absent(d, "old"), "old ref must be gone");
    let repo = git2::Repository::open(d).expect("reopen");
    let new_tip = repo
        .find_branch("new", git2::BranchType::Local)
        .expect("new branch")
        .get()
        .target()
        .expect("tip");
    assert_eq!(new_tip, c0, "renamed branch points at the same tip");
}

// ------------------------------------------------------- upstream survives

/// The `branch.<name>.*` config section moves with the rename, so the
/// upstream shorthand is PRESERVED (surfaced in the result + still resolves
/// via `Branch::upstream()`), and the old config section is gone.
#[test]
fn rename_preserves_upstream() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");
    rb_branch_at_head(&repo, "feature");
    rb_set_upstream(&repo, "feature", c0);

    let res = rename_branch(d, "feature", "feature-renamed").expect("rename");
    assert_eq!(
        res.upstream.as_deref(),
        Some("origin/feature"),
        "upstream shorthand preserved through the rename"
    );

    let repo = git2::Repository::open(d).expect("reopen");
    let cfg = repo.config().expect("config");
    assert_eq!(
        cfg.get_string("branch.feature-renamed.remote").ok().as_deref(),
        Some("origin"),
        "config section moved to the new name"
    );
    assert!(
        cfg.get_string("branch.feature.remote").is_err(),
        "old config section must be gone"
    );
    let renamed = repo
        .find_branch("feature-renamed", git2::BranchType::Local)
        .expect("renamed branch");
    assert_eq!(
        renamed
            .upstream()
            .ok()
            .and_then(|u| u.name().ok().flatten().map(str::to_string))
            .as_deref(),
        Some("origin/feature"),
        "Branch::upstream() still resolves after the rename"
    );
}

// ---------------------------------------- checked-out branch: HEAD follows

/// Renaming the checked-out branch sets `was_head=true` and rewrites the HEAD
/// symref to `refs/heads/<new>`.
#[test]
fn rename_checked_out_branch_moves_head() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    let cur = rb_head_branch(d).expect("head branch");

    let res = rename_branch(d, &cur, "trunk").expect("rename current");
    assert!(res.was_head, "renaming the checked-out branch → was_head=true");
    // HEAD now resolves to refs/heads/trunk: it is an attached symref whose
    // branch is `trunk`, the old ref is gone, and the new ref exists.
    assert_eq!(rb_head_branch(d).as_deref(), Some("trunk"), "HEAD followed the rename");
    assert!(rb_branch_absent(d, &cur), "the old ref must be gone");
    assert!(!rb_branch_absent(d, "trunk"), "the renamed ref must exist");
}

// ----------------------------------------------------------- error cases

/// New name already exists → `BranchExists`; the old ref is untouched.
#[test]
fn rename_to_existing_name_is_branch_exists() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    rb_branch_at_head(&repo, "old");
    rb_branch_at_head(&repo, "taken");

    match rename_branch(d, "old", "taken") {
        Err(AppError::BranchExists(_)) => {}
        other => panic!("expected BranchExists, got {other:?}"),
    }
    assert!(!rb_branch_absent(d, "old"), "old must survive a refused rename");
    assert!(!rb_branch_absent(d, "taken"), "taken must survive");
}

/// Unknown old name → `BranchNotFound`.
#[test]
fn rename_unknown_old_is_branch_not_found() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    match rename_branch(d, "ghost", "new") {
        Err(AppError::BranchNotFound(_)) => {}
        other => panic!("expected BranchNotFound, got {other:?}"),
    }
}

/// Invalid new name (blank / leading '-') → `InvalidName`; old untouched.
#[test]
fn rename_to_invalid_name_is_invalid_name() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    rb_branch_at_head(&repo, "old");
    for bad in ["", "   ", "-x"] {
        match rename_branch(d, "old", bad) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for {bad:?}, got {other:?}"),
        }
    }
    assert!(!rb_branch_absent(d, "old"), "invalid new name must not touch old");
}

// ------------------------------------------------------- CLI-oracle parity

/// State after `rename_branch` matches the documented `git branch -m old new`
/// post-conditions, verified through the git CLI (`git rev-parse` +
/// `git config --get`): the ref moved (feature2 == the old feature tip, old
/// ref gone) and the `branch.<name>.*` tracking section moved with it. A
/// single repo (not a cross-repo compare) so commit oids are stable.
#[test]
fn rename_matches_git_cli_oracle() {
    if !have_git() {
        return;
    }
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = rb_init(d);
    rb_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");
    rb_branch_at_head(&repo, "feature");
    rb_set_upstream(&repo, "feature", c0);
    let tip_before = c0.to_string();

    rename_branch(d, "feature", "feature2").expect("our rename");

    let git = |args: &[&str]| -> Option<String> {
        let o = Command::new("git").args(args).current_dir(d).output().ok()?;
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };

    // Ref moved: feature2 resolves to the old feature tip; feature is gone.
    assert_eq!(
        git(&["rev-parse", "feature2"]).as_deref(),
        Some(tip_before.as_str()),
        "git rev-parse feature2 == the old feature tip"
    );
    assert!(
        git(&["rev-parse", "--verify", "--quiet", "feature"]).is_none(),
        "old feature ref must be gone"
    );
    // Tracking config section moved (git branch -m guarantee).
    assert_eq!(
        git(&["config", "--get", "branch.feature2.remote"]).as_deref(),
        Some("origin"),
        "branch.feature2.remote preserved"
    );
    assert_eq!(
        git(&["config", "--get", "branch.feature2.merge"]).as_deref(),
        Some("refs/heads/feature"),
        "branch.feature2.merge preserved (tracks the same remote branch)"
    );
    assert!(
        git(&["config", "--get", "branch.feature.remote"]).is_none(),
        "old config section must be gone"
    );
}
