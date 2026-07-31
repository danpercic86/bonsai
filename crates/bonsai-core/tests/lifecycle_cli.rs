//! P21 CLI-oracle repo-lifecycle tests (contract §5).
//!
//! Every "remote" is a LOCAL bare repo (`git init --bare`) referenced by a
//! plain path / `file://` URL — the local transport needs NO network and NO
//! credentials (the credential callback is never invoked; that path is covered
//! only by the USER CHECKPOINT, contract §5.2 coverage note). All scratch
//! repos live under `D:\Temp\bonsai-scratch` via `common::scratch_dir()`.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bonsai_core::error::AppError;
use bonsai_core::git::clone::{clone_repo, init_repo, CloneProgress};
use bonsai_core::git::repo::read_repo_info;
use common::{commit_fixed, git};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn configure_identity(repo: &Path) {
    git(repo, &["config", "user.name", "Test User"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "core.autocrlf", "false"]);
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    git(repo, &["rev-parse", rev])
}

/// bare "origin.git" on `main` with two commits (A then B) pushed via a seed
/// clone. Returns (scratch dir, root, bare path).
fn setup_origin() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();

    git(&root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");

    // Seed clone publishes commit A then commit B on main.
    git(&root, &["clone", &path_str(&bare), "seed"]);
    let seed = root.join("seed");
    configure_identity(&seed);
    git(&seed, &["checkout", "-B", "main"]);

    std::fs::write(seed.join("hello.txt"), "hello A\n").expect("write hello.txt");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "commit A");

    std::fs::write(seed.join("shared.txt"), "content B\n").expect("write shared.txt");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "commit B");

    git(&seed, &["push", "-u", "origin", "main"]);

    (dir, root, bare)
}

/// bare "origin.git" carrying MULTIPLE branches (`main`, `feature`) and
/// MULTIPLE annotated tags (`v1.0` on main, `v2.0` on feature), all pushed via
/// a seed clone. Returns (scratch dir, root, bare path). Used to prove a clone
/// brings ALL refs, not just the default branch.
fn setup_multi_ref_origin() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();

    git(&root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");

    git(&root, &["clone", &path_str(&bare), "seed"]);
    let seed = root.join("seed");
    configure_identity(&seed);
    git(&seed, &["checkout", "-B", "main"]);

    // main: commit A, annotated tag v1.0.
    std::fs::write(seed.join("hello.txt"), "hello A\n").expect("write hello.txt");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "commit A on main");
    git(&seed, &["tag", "-a", "v1.0", "-m", "release 1.0"]);

    // feature branch off main: commit C, annotated tag v2.0.
    git(&seed, &["checkout", "-b", "feature"]);
    std::fs::write(seed.join("feature.txt"), "feature C\n").expect("write feature.txt");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "commit C on feature");
    git(&seed, &["tag", "-a", "v2.0", "-m", "release 2.0"]);

    // Publish both branches and all tags.
    git(&seed, &["push", "origin", "main", "feature"]);
    git(&seed, &["push", "origin", "--tags"]);

    (dir, root, bare)
}

/// A collecting progress sink: pushes each tick into a shared Vec. Uses a
/// `Mutex` (not `RefCell`) so the closure satisfies the `Send` bound on the
/// `clone_repo` sink even though the test drives it on a single thread.
fn collect_sink(ticks: &Mutex<Vec<CloneProgress>>) -> impl FnMut(CloneProgress) + Send + '_ {
    move |p: CloneProgress| ticks.lock().expect("lock").push(p)
}

// ------------------------------------------------------- §5.2 #1,#2,#3 clone

/// §5.2 #1/#2/#3: clone round-trip — HEAD oid, refs, remote wired, worktree
/// clean; and the progress callback fires monotonically to a final total.
#[test]
fn clone_round_trip_and_progress() {
    require_git!();
    let (_dir, root, bare) = setup_origin();
    let url = format!("file:///{}", path_str(&bare).replace('\\', "/"));
    let work = root.join("work");

    let ticks = Mutex::new(Vec::new());
    let out = clone_repo(&url, &work, collect_sink(&ticks)).expect("clone_repo");

    // Returned path is the cloned workdir; read_repo_info agrees.
    let info = read_repo_info(Path::new(&out)).expect("read_repo_info");
    assert!(info.is_repo, "clone must be a repo");
    assert!(!info.bare, "clone must not be bare");
    let head = info.head.expect("head present");
    assert!(!head.unborn, "cloned HEAD must not be unborn");

    // §5.2 #1 cross-checks: work HEAD == origin main tip (B); tree clean.
    let origin_tip = rev_parse(&bare, "main");
    assert_eq!(rev_parse(&work, "HEAD"), origin_tip, "work HEAD != origin main");
    assert_eq!(head.oid, origin_tip, "read_repo_info oid != origin tip");
    // Compare the committed BLOB (via git object, line-ending-neutral) rather
    // than the checked-out file, whose EOLs depend on global autocrlf config.
    assert_eq!(
        git(&work, &["show", "HEAD:shared.txt"]),
        git(&bare, &["show", "main:shared.txt"]),
        "blob content must match source",
    );

    // §5.2 #2: refs/remote wired — proves a real clone, not a copy.
    assert_eq!(
        rev_parse(&work, "refs/remotes/origin/main"),
        origin_tip,
        "remote-tracking ref must equal origin tip",
    );
    assert_eq!(
        git(&work, &["remote", "get-url", "origin"]),
        url,
        "origin remote url must be the clone source",
    );

    // §5.2 #3: progress callback fired monotonically to a final full total.
    let ticks = ticks.into_inner().expect("into_inner");
    assert!(!ticks.is_empty(), "progress callback must fire at least once");
    let mut prev = 0u32;
    for t in &ticks {
        assert!(
            t.received_objects >= prev,
            "received_objects must be non-decreasing: {prev} -> {}",
            t.received_objects
        );
        prev = t.received_objects;
    }
    let total = ticks.last().expect("final tick").total_objects;
    assert!(total > 0, "non-empty origin must report total_objects > 0");
    assert_eq!(
        ticks.last().expect("final tick").received_objects,
        total,
        "final tick must have received all objects",
    );
}

/// Coverage gap: cloning a multi-branch / multi-tag origin brings ALL refs
/// (every remote-tracking branch + every tag), not just the default branch.
#[test]
fn clone_brings_all_branches_and_tags() {
    require_git!();
    let (_dir, root, bare) = setup_multi_ref_origin();
    let url = format!("file:///{}", path_str(&bare).replace('\\', "/"));
    let work = root.join("work");

    let out = clone_repo(&url, &work, |_p| {}).expect("clone_repo");
    let info = read_repo_info(Path::new(&out)).expect("read_repo_info");
    assert!(info.is_repo && !info.bare, "clone must be a usable repo");

    // Both remote-tracking branches exist and point at the origin tips.
    assert_eq!(
        rev_parse(&work, "refs/remotes/origin/main"),
        rev_parse(&bare, "main"),
        "origin/main tracking ref must match origin",
    );
    assert_eq!(
        rev_parse(&work, "refs/remotes/origin/feature"),
        rev_parse(&bare, "feature"),
        "origin/feature tracking ref must match origin",
    );

    // Both tags were fetched and resolve to the origin's tag objects.
    let tags = git(&work, &["tag", "--list"]);
    assert!(tags.contains("v1.0"), "tag v1.0 must be cloned: {tags:?}");
    assert!(tags.contains("v2.0"), "tag v2.0 must be cloned: {tags:?}");
    assert_eq!(
        rev_parse(&work, "refs/tags/v1.0"),
        rev_parse(&bare, "refs/tags/v1.0"),
        "cloned tag v1.0 must match origin",
    );
    assert_eq!(
        rev_parse(&work, "refs/tags/v2.0"),
        rev_parse(&bare, "refs/tags/v2.0"),
        "cloned tag v2.0 must match origin",
    );

    // The checked-out HEAD is the default branch (main), not feature.
    assert_eq!(
        rev_parse(&work, "HEAD"),
        rev_parse(&bare, "main"),
        "clone must check out the default branch (main)",
    );
}

/// Coverage gap: cloning into a destination path that CONTAINS SPACES succeeds
/// (Windows path-quoting / libgit2 path handling regression guard).
#[test]
fn clone_into_path_with_spaces_succeeds() {
    require_git!();
    let (_dir, root, bare) = setup_origin();
    let url = format!("file:///{}", path_str(&bare).replace('\\', "/"));

    let dest = root.join("my cloned repo");
    let out = clone_repo(&url, &dest, |_p| {}).expect("clone into spaced path");

    assert!(out.contains(' '), "returned workdir path must retain the space: {out:?}");
    let info = read_repo_info(Path::new(&out)).expect("read_repo_info");
    assert!(info.is_repo && !info.bare, "spaced-path clone must be a usable repo");
    assert_eq!(
        rev_parse(&dest, "HEAD"),
        rev_parse(&bare, "main"),
        "spaced-path clone HEAD must match origin main",
    );
    assert_eq!(
        git(&dest, &["status", "--porcelain"]),
        "",
        "spaced-path clone worktree must be clean",
    );
}

// ----------------------------------------------------------- §5.2 #4 dest err

/// §5.2 #4: dest is an existing NON-EMPTY dir → Io; nothing written into it.
#[test]
fn clone_into_non_empty_dir_is_io_error() {
    require_git!();
    let (_dir, root, bare) = setup_origin();
    let url = format!("file:///{}", path_str(&bare).replace('\\', "/"));

    let dest = root.join("occupied");
    std::fs::create_dir(&dest).expect("mkdir occupied");
    std::fs::write(dest.join("keep.txt"), "existing\n").expect("write existing file");

    let err = clone_repo(&url, &dest, |_p| {}).expect_err("must reject non-empty dest");
    assert!(matches!(err, AppError::Io(_)), "got {err:?}");

    // The existing file survived and no .git was created.
    assert!(dest.join("keep.txt").exists(), "existing content must survive");
    assert!(!dest.join(".git").exists(), "no repo must be created");
}

/// §5.2 #4: dest is an existing FILE → Io.
#[test]
fn clone_into_a_file_is_io_error() {
    require_git!();
    let (_dir, root, bare) = setup_origin();
    let url = format!("file:///{}", path_str(&bare).replace('\\', "/"));

    let dest = root.join("afile");
    std::fs::write(&dest, "i am a file\n").expect("write file");

    let err = clone_repo(&url, &dest, |_p| {}).expect_err("must reject file dest");
    assert!(matches!(err, AppError::Io(_)), "got {err:?}");
}

/// §5.2 #4: an existing EMPTY dir clones fine.
#[test]
fn clone_into_empty_dir_succeeds() {
    require_git!();
    let (_dir, root, bare) = setup_origin();
    let url = format!("file:///{}", path_str(&bare).replace('\\', "/"));

    let dest = root.join("empty");
    std::fs::create_dir(&dest).expect("mkdir empty");

    let out = clone_repo(&url, &dest, |_p| {}).expect("clone into empty dir");
    let info = read_repo_info(Path::new(&out)).expect("read_repo_info");
    assert!(info.is_repo && !info.bare, "empty-dir clone must be a usable repo");
    assert_eq!(rev_parse(&dest, "HEAD"), rev_parse(&bare, "main"));
}

// ---------------------------------------------------------- §5.2 #5 init repo

/// §5.2 #5: init produces a valid, unborn repo that read_repo_info opens; a
/// subsequent stage+commit succeeds (repo is usable).
#[test]
fn init_produces_valid_unborn_repo() {
    require_git!();
    let dir = common::scratch_dir();
    let fresh = dir.path().join("fresh");

    let out = init_repo(&fresh).expect("init_repo");
    let info = read_repo_info(Path::new(&out)).expect("read_repo_info");
    assert!(info.is_repo, "init must produce a repo");
    assert!(!info.bare, "init must be non-bare");
    let head = info.head.expect("head present");
    assert!(head.unborn, "fresh init must have unborn HEAD");
    assert_eq!(head.oid, "", "unborn HEAD oid must be empty");

    // CLI cross-checks.
    assert_eq!(git(&fresh, &["rev-parse", "--is-inside-work-tree"]), "true");
    assert!(
        git(&fresh, &["status"]).contains("No commits yet"),
        "fresh repo status must report 'No commits yet'",
    );

    // The repo is usable: stage + commit succeeds.
    configure_identity(&fresh);
    std::fs::write(fresh.join("first.txt"), "first\n").expect("write first.txt");
    git(&fresh, &["add", "-A"]);
    commit_fixed(&fresh, "first commit");
    let info = read_repo_info(&fresh).expect("read_repo_info after commit");
    let head = info.head.expect("head present");
    assert!(!head.unborn, "HEAD must be born after the first commit");
}

/// §5.2 #6: init on a path that is ALREADY a repo returns that repo's workdir
/// and does not disturb its HEAD/refs (idempotent, §OPEN-3).
#[test]
fn init_on_existing_repo_is_idempotent() {
    require_git!();
    let dir = common::scratch_dir();
    let existing = dir.path().join("existing");
    std::fs::create_dir(&existing).expect("mkdir existing");

    git(&existing, &["init", "-b", "main"]);
    configure_identity(&existing);
    std::fs::write(existing.join("a.txt"), "a\n").expect("write a.txt");
    git(&existing, &["add", "-A"]);
    commit_fixed(&existing, "base");
    let tip_before = rev_parse(&existing, "HEAD");

    let out = init_repo(&existing).expect("init on existing repo");
    // Same repo workdir returned, HEAD unchanged.
    assert_eq!(rev_parse(&existing, "HEAD"), tip_before, "HEAD must not move");
    let info = read_repo_info(Path::new(&out)).expect("read_repo_info");
    assert!(info.is_repo && !info.bare);
    assert_eq!(info.head.expect("head").oid, tip_before);

    // A second init call is likewise idempotent.
    let out2 = init_repo(Path::new(&out)).expect("second init");
    assert_eq!(rev_parse(&existing, "HEAD"), tip_before, "HEAD still unchanged");
    assert_eq!(
        Path::new(&out).canonicalize().ok(),
        Path::new(&out2).canonicalize().ok(),
        "idempotent init must return the same workdir",
    );
}

/// §5.2 #5 (error): init where the path is an existing FILE → Io.
#[test]
fn init_on_a_file_is_io_error() {
    require_git!();
    let dir = common::scratch_dir();
    let file = dir.path().join("afile");
    std::fs::write(&file, "not a dir\n").expect("write file");

    let err = init_repo(&file).expect_err("init on a file must fail");
    assert!(matches!(err, AppError::Io(_)), "got {err:?}");
}
