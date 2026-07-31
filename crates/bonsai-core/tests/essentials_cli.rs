//! P20 CLI-oracle suite (contract §10.2): amend / reset / discard (P20a rows
//! 1, 6, 7). Cherry-pick + revert (rows 2, 3, 4, 5, 8) are added by P20b.
//!
//! Twin-repo pattern (identical to merge_cli.rs / rebase_cli.rs): two scratch
//! repos are built by the IDENTICAL scripted CLI setup (fixed dates → identical
//! base oids). Bonsai's core fns run on one; the real `git` CLI on the other.
//! We compare tree oids / index / worktree — never commit oids (committer time
//! = now() differs). Each test skips (passes with a note) if `git` is absent.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full).

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::amend_commit;
use bonsai_core::git::discard::discard_paths;
use bonsai_core::git::reset::{reset_branch, ResetMode};
use common::{git, git_env, init_repo, FIXED_DATE};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ------------------------------------------------------------ small helpers

fn head_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

fn tree_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD^{tree}"])
}

/// Tree oid of the current INDEX (`git write-tree`) — captures staged content
/// independent of HEAD.
fn index_tree(dir: &Path) -> String {
    git(dir, &["write-tree"])
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read fixture file")
}

/// CLI commit of the CURRENT worktree change to `name` with fixed dates.
fn add_commit(dir: &Path, name: &str, content: &str, msg: &str) {
    write(dir, name, content);
    git(dir, &["add", name]);
    git_env(
        dir,
        &["commit", "-m", msg],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

// ============================================================ Row 1: amend

/// Bonsai `amend_commit` produces the SAME tree as `git commit --amend`, and
/// preserves HEAD's single parent + original author (fresh committer).
#[test]
fn essentials_1_amend_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    // Identical two-commit history on both twins.
    for d in [a, b] {
        add_commit(d, "a.txt", "one\n", "base");
        add_commit(d, "a.txt", "two\n", "second");
    }
    let orig_parent_a = git(a, &["rev-parse", "HEAD^"]);
    let orig_parent_b = git(b, &["rev-parse", "HEAD^"]);
    assert_eq!(orig_parent_a, orig_parent_b, "twin base oids must match");
    // Capture the original author time (%at) to prove amend preserves it.
    let orig_author_at: i64 = git(a, &["show", "-s", "--format=%at", "HEAD"])
        .parse()
        .expect("author epoch");

    // Stage the SAME change on both, then amend each its own way.
    write(a, "a.txt", "three\n");
    write(b, "a.txt", "three\n");
    git(a, &["add", "a.txt"]);
    git(b, &["add", "a.txt"]);

    amend_commit(a, "amended subject").expect("bonsai amend");
    git_env(
        b,
        &["commit", "--amend", "-m", "amended subject"],
        &[("GIT_COMMITTER_DATE", "2026-02-02T00:00:00+0000")],
    );

    assert_eq!(tree_oid(a), tree_oid(b), "amended tree must match the CLI");

    // Parent preserved (still the original base commit, single parent).
    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    assert_eq!(head.parent_count(), 1, "single-parent amend keeps 1 parent");
    assert_eq!(
        head.parent_id(0).expect("parent").to_string(),
        orig_parent_a,
        "amend must preserve HEAD's original parent"
    );
    // Original author preserved (from the fixed-date CLI commit); message updated.
    assert_eq!(head.author().name(), Some("Test User"));
    assert_eq!(head.author().email(), Some("test@example.com"));
    assert_eq!(
        head.author().when().seconds(),
        orig_author_at,
        "amend preserves the original author time"
    );
    assert_eq!(head.message(), Some("amended subject\n"));
}

/// A message-only amend (0 staged) keeps the tree and preserves author.
#[test]
fn essentials_1b_message_only_amend_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    for d in [a, b] {
        add_commit(d, "a.txt", "one\n", "base");
        add_commit(d, "a.txt", "two\n", "second");
    }
    let tree_before = tree_oid(a);

    amend_commit(a, "reworded").expect("bonsai amend");
    git_env(
        b,
        &["commit", "--amend", "-m", "reworded"],
        &[("GIT_COMMITTER_DATE", "2026-02-02T00:00:00+0000")],
    );

    assert_eq!(tree_oid(a), tree_before, "message-only amend keeps the tree");
    assert_eq!(tree_oid(a), tree_oid(b), "tree matches the CLI amend");
}

/// A merge-commit amend preserves BOTH parents (git-consistent).
#[test]
fn essentials_1c_merge_amend_preserves_both_parents() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    add_commit(d, "base.txt", "base\n", "base");
    let base = head_oid(d);
    add_commit(d, "main.txt", "main\n", "main work");

    // topic diverges from base with a different file (no conflict on merge).
    git(d, &["checkout", "-b", "topic", &base]);
    add_commit(d, "topic.txt", "topic\n", "topic work");
    git(d, &["checkout", "main"]);
    git_env(
        d,
        &["merge", "--no-ff", "-m", "merge topic", "topic"],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );

    let p1 = git(d, &["rev-parse", "HEAD^1"]);
    let p2 = git(d, &["rev-parse", "HEAD^2"]);

    // Stage a new change and amend the merge commit.
    write(d, "extra.txt", "extra\n");
    git(d, &["add", "extra.txt"]);
    amend_commit(d, "merge topic (amended)").expect("amend merge");

    let repo = git2::Repository::open(d).expect("open");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    assert_eq!(head.parent_count(), 2, "merge amend must keep both parents");
    assert_eq!(head.parent_id(0).expect("p1").to_string(), p1);
    assert_eq!(head.parent_id(1).expect("p2").to_string(), p2);
    assert!(
        head.tree().expect("tree").get_name("extra.txt").is_some(),
        "the staged change is folded into the amended tree"
    );
}

// ============================================================ Row 6: reset

/// Builds a fresh 3-commit twin history; returns (dir, target_oid) where
/// target == the FIRST commit (HEAD~2).
fn reset_fixture() -> (tempfile::TempDir, String) {
    let dir = init_repo();
    let d = dir.path();
    add_commit(d, "a.txt", "one\n", "c1");
    let target = head_oid(d);
    add_commit(d, "b.txt", "b\n", "c2");
    write(d, "a.txt", "three\n");
    git(d, &["add", "a.txt"]);
    git_env(
        d,
        &["commit", "-m", "c3"],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
    (dir, target)
}

#[test]
fn essentials_6_reset_soft_matches_cli() {
    require_git!();
    let (a_dir, target) = reset_fixture();
    let (b_dir, target_b) = reset_fixture();
    let a = a_dir.path();
    let b = b_dir.path();
    assert_eq!(target, target_b, "twin target oids must match");

    reset_branch(a, &target, ResetMode::Soft).expect("bonsai soft reset");
    git(b, &["reset", "--soft", &target]);

    assert_eq!(head_oid(a), head_oid(b), "HEAD moved to target on both");
    assert_eq!(head_oid(a), target, "soft reset moves HEAD to target");
    // Soft: index + worktree UNCHANGED (still c3 content).
    assert_eq!(index_tree(a), index_tree(b), "soft keeps index unchanged");
    assert_eq!(read(a, "a.txt"), "three\n", "worktree unchanged");
    assert_eq!(read(a, "a.txt"), read(b, "a.txt"));
    assert!(b.join("b.txt").exists() && a.join("b.txt").exists());
}

#[test]
fn essentials_6_reset_mixed_matches_cli() {
    require_git!();
    let (a_dir, target) = reset_fixture();
    let (b_dir, _t) = reset_fixture();
    let a = a_dir.path();
    let b = b_dir.path();

    reset_branch(a, &target, ResetMode::Mixed).expect("bonsai mixed reset");
    git(b, &["reset", "--mixed", &target]);

    assert_eq!(head_oid(a), head_oid(b));
    assert_eq!(head_oid(a), target);
    // Mixed: index reset to target; worktree UNCHANGED.
    assert_eq!(index_tree(a), index_tree(b), "mixed resets the index like CLI");
    assert_eq!(read(a, "a.txt"), "three\n", "worktree unchanged by mixed");
    assert_eq!(read(a, "a.txt"), read(b, "a.txt"));
}

#[test]
fn essentials_6_reset_hard_matches_cli() {
    require_git!();
    let (a_dir, target) = reset_fixture();
    let (b_dir, _t) = reset_fixture();
    let a = a_dir.path();
    let b = b_dir.path();

    reset_branch(a, &target, ResetMode::Hard).expect("bonsai hard reset");
    git(b, &["reset", "--hard", &target]);

    assert_eq!(head_oid(a), head_oid(b));
    assert_eq!(head_oid(a), target);
    // Hard: index + worktree reset to target (a.txt=="one", b.txt gone).
    assert_eq!(index_tree(a), index_tree(b), "hard resets the index like CLI");
    assert_eq!(tree_oid(a), tree_oid(b));
    assert_eq!(read(a, "a.txt"), "one\n", "worktree reset to target content");
    assert_eq!(read(a, "a.txt"), read(b, "a.txt"));
    assert!(!a.join("b.txt").exists(), "hard reset removed the later file");
    assert_eq!(a.join("b.txt").exists(), b.join("b.txt").exists());
}

// ============================================================ Row 7: discard

/// Bonsai `discard_paths` restores the worktree to the INDEX version (staged
/// content preserved), leaving other files untouched — matching `git restore`.
#[test]
fn essentials_7_discard_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    for d in [a, b] {
        add_commit(d, "a.txt", "base\n", "base a");
        add_commit(d, "b.txt", "b-base\n", "base b");
        // Stage a change to a.txt (index != HEAD), then edit further in worktree.
        write(d, "a.txt", "staged\n");
        git(d, &["add", "a.txt"]);
        write(d, "a.txt", "worktree\n");
        // Unstaged edit to a second tracked file (must remain untouched).
        write(d, "b.txt", "b-worktree\n");
    }

    discard_paths(a, &["a.txt".to_string()]).expect("bonsai discard");
    // git restore --worktree restores from the index (default source).
    git(b, &["restore", "--worktree", "a.txt"]);

    // a.txt restored to the INDEX (staged) version on both.
    assert_eq!(read(a, "a.txt"), "staged\n", "discard restores to the index version");
    assert_eq!(read(a, "a.txt"), read(b, "a.txt"));
    // Staged content preserved (index tree unchanged, identical to CLI).
    assert_eq!(index_tree(a), index_tree(b), "staged content preserved");
    // b.txt untouched on both.
    assert_eq!(read(a, "b.txt"), "b-worktree\n", "unrelated file untouched");
    assert_eq!(read(a, "b.txt"), read(b, "b.txt"));
}

/// Discarding an untracked path errors (out of scope, defensive backend guard).
#[test]
fn essentials_7b_discard_untracked_errors() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    add_commit(d, "a.txt", "base\n", "base");
    write(d, "untracked.txt", "new\n");

    let err = discard_paths(d, &["untracked.txt".to_string()]).expect_err("untracked");
    match err {
        AppError::Git(m) => assert!(m.contains("not a tracked file"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}
