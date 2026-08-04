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
use bonsai_core::git::cherrypick::{cherrypick_abort, cherrypick_commit, cherrypick_continue, CherrypickOutcome};
use bonsai_core::git::commit::amend_commit;
use bonsai_core::git::conflict::resolve_conflict_text;
use bonsai_core::git::discard::discard_paths;
use bonsai_core::git::reset::{reset_branch, ResetMode};
use bonsai_core::git::revert::{revert_abort, revert_commit, revert_continue, RevertOutcome};
use common::{git, git_env, git_ok, init_repo, FIXED_DATE};

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
    assert_eq!(head.author().name().ok(), Some("Test User"));
    assert_eq!(head.author().email().ok(), Some("test@example.com"));
    assert_eq!(
        head.author().when().seconds(),
        orig_author_at,
        "amend preserves the original author time"
    );
    assert_eq!(head.message().ok(), Some("amended subject\n"));
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

fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

// ==================================================== Row 2: cherry-pick clean

/// Bonsai `cherrypick_commit` on a divergent branch produces the SAME tree as
/// `git cherry-pick`, advances HEAD by one, reuses the picked message, and
/// preserves the original author.
#[test]
fn essentials_2_cherrypick_clean_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    // Identical divergent history: base → feature adds feature.txt (the pick);
    // main then advances with an unrelated file so the pick is NOT a fast path.
    let build = |d: &Path| -> (String, String) {
        add_commit(d, "base.txt", "base\n", "base");
        let base = head_oid(d);
        git(d, &["checkout", "-b", "feature"]);
        add_commit(d, "feature.txt", "feature\n", "add feature");
        let pick = head_oid(d);
        git(d, &["checkout", "main"]);
        add_commit(d, "main.txt", "main\n", "main work");
        let _ = base;
        (pick, head_oid(d))
    };
    let (pick_a, main_a) = build(a);
    let (pick_b, main_b) = build(b);
    assert_eq!(pick_a, pick_b, "twin pick oids must match (fixed dates)");
    assert_eq!(main_a, main_b, "twin main-tip oids must match");

    // Author epoch of the picked commit, to prove preservation.
    let pick_author_at: i64 = git(a, &["show", "-s", "--format=%at", &pick_a])
        .parse()
        .expect("author epoch");

    let outcome = cherrypick_commit(a, &pick_a).expect("bonsai cherry-pick");
    match outcome {
        CherrypickOutcome::Committed { .. } => {}
        other => panic!("expected Committed, got {other:?}"),
    }
    git(b, &["cherry-pick", &pick_b]);

    assert_eq!(tree_oid(a), tree_oid(b), "cherry-picked tree must match the CLI");
    assert_eq!(read(a, "feature.txt"), "feature\n");

    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    assert_eq!(head.parent_count(), 1, "pick has a single parent");
    assert_eq!(
        head.parent_id(0).expect("parent").to_string(),
        main_a,
        "HEAD advanced onto the former main tip"
    );
    assert_eq!(head.message().ok(), Some("add feature\n"), "picked message reused");
    assert_eq!(head.author().name().ok(), Some("Test User"));
    assert_eq!(
        head.author().when().seconds(),
        pick_author_at,
        "the ORIGINAL author (and author time) is preserved"
    );
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
}

// ================================================= Row 3: cherry-pick conflict

/// A conflicting cherry-pick pauses (Conflicts + state CherryPick); resolving
/// the index then `cherrypick_continue` yields the SAME tree as the CLI's
/// hand-resolved cherry-pick.
#[test]
fn essentials_3_cherrypick_conflict_resolve_continue_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    // base x.txt → feature edits it one way → main edits the SAME line another
    // way. Cherry-picking feature onto main conflicts on x.txt.
    let build = |d: &Path| -> String {
        add_commit(d, "x.txt", "line1\nbase\nline3\n", "base");
        git(d, &["checkout", "-b", "feature"]);
        add_commit(d, "x.txt", "line1\nfeature\nline3\n", "feature edit");
        let pick = head_oid(d);
        git(d, &["checkout", "main"]);
        add_commit(d, "x.txt", "line1\nmain\nline3\n", "main edit");
        pick
    };
    let pick_a = build(a);
    let pick_b = build(b);
    assert_eq!(pick_a, pick_b, "twin pick oids must match");

    let outcome = cherrypick_commit(a, &pick_a).expect("bonsai cherry-pick");
    match outcome {
        CherrypickOutcome::Conflicts { paths } => {
            assert_eq!(paths, vec!["x.txt".to_string()], "x.txt must be conflicted");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert_eq!(repo_state(a), git2::RepositoryState::CherryPick);

    // CLI twin: the same cherry-pick conflicts (non-zero exit).
    assert!(!git_ok(b, &["cherry-pick", &pick_b]), "CLI cherry-pick must conflict");

    // Both resolve x.txt to the SAME hand-merged content.
    let resolved = "line1\nresolved\nline3\n";
    resolve_conflict_text(a, "x.txt", resolved).expect("resolve index");
    let out = cherrypick_continue(a).expect("bonsai continue");
    match out {
        CherrypickOutcome::Committed { .. } => {}
        other => panic!("expected Committed after resolve, got {other:?}"),
    }

    write(b, "x.txt", resolved);
    git(b, &["add", "x.txt"]);
    git_env(b, &["cherry-pick", "--continue"], &[("GIT_EDITOR", "true")]);

    assert_eq!(tree_oid(a), tree_oid(b), "resolved cherry-pick tree must match CLI");
    assert_eq!(read(a, "x.txt"), resolved);
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
}

// ========================================================= Row 4: revert clean

/// Bonsai `revert_commit` produces the SAME tree as `git revert --no-edit` and
/// writes the byte-exact `Revert "<subject>"\n\nThis reverts commit <oid>.\n`
/// message, authored as the current signature.
#[test]
fn essentials_4_revert_clean_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    for d in [a, b] {
        add_commit(d, "x.txt", "base\n", "base");
        add_commit(d, "x.txt", "v2\n", "second");
    }
    let c2_a = head_oid(a);
    let c2_b = head_oid(b);
    assert_eq!(c2_a, c2_b, "twin target oids must match");

    let outcome = revert_commit(a, &c2_a).expect("bonsai revert");
    match outcome {
        RevertOutcome::Committed { .. } => {}
        other => panic!("expected Committed, got {other:?}"),
    }
    git(b, &["revert", "--no-edit", &c2_b]);

    assert_eq!(tree_oid(a), tree_oid(b), "reverted tree must match the CLI");
    assert_eq!(read(a, "x.txt"), "base\n", "revert undoes the second commit");

    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    let expected = format!("Revert \"second\"\n\nThis reverts commit {c2_a}.\n");
    assert_eq!(head.message().ok(), Some(expected.as_str()), "byte-exact revert message");
    // The revert is authored as YOU (current signature), not the reverted author.
    assert_eq!(head.author().name().ok(), Some("Test User"));
    assert_eq!(head.committer().name().ok(), Some("Test User"));
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
}

// ====================================================== Row 5: revert conflict

/// A conflicting revert pauses (Conflicts + state Revert); resolving then
/// `revert_continue` yields the SAME tree as the CLI's hand-resolved revert.
#[test]
fn essentials_5_revert_conflict_resolve_continue_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    // base → c2 edits line2 → c3 edits the SAME line again. Reverting c2 tries
    // to undo c2's change on a line c3 has since changed → conflict.
    let build = |d: &Path| -> String {
        add_commit(d, "x.txt", "line1\nbase\nline3\n", "base");
        add_commit(d, "x.txt", "line1\nv2\nline3\n", "second");
        let c2 = head_oid(d);
        add_commit(d, "x.txt", "line1\nv3\nline3\n", "third");
        c2
    };
    let c2_a = build(a);
    let c2_b = build(b);
    assert_eq!(c2_a, c2_b, "twin target oids must match");

    let outcome = revert_commit(a, &c2_a).expect("bonsai revert");
    match outcome {
        RevertOutcome::Conflicts { paths } => {
            assert_eq!(paths, vec!["x.txt".to_string()]);
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert_eq!(repo_state(a), git2::RepositoryState::Revert);

    assert!(!git_ok(b, &["revert", "--no-edit", &c2_b]), "CLI revert must conflict");

    let resolved = "line1\nresolved\nline3\n";
    resolve_conflict_text(a, "x.txt", resolved).expect("resolve index");
    let out = revert_continue(a).expect("bonsai continue");
    match out {
        RevertOutcome::Committed { .. } => {}
        other => panic!("expected Committed after resolve, got {other:?}"),
    }

    write(b, "x.txt", resolved);
    git(b, &["add", "x.txt"]);
    git_env(b, &["revert", "--continue"], &[("GIT_EDITOR", "true")]);

    assert_eq!(tree_oid(a), tree_oid(b), "resolved revert tree must match CLI");
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
}

// =============================================================== Row 8: abort

/// Aborting a conflicting cherry-pick restores state Clean, HEAD, and worktree.
#[test]
fn essentials_8_cherrypick_abort_restores_head() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    add_commit(d, "x.txt", "line1\nbase\nline3\n", "base");
    git(d, &["checkout", "-b", "feature"]);
    add_commit(d, "x.txt", "line1\nfeature\nline3\n", "feature edit");
    let pick = head_oid(d);
    git(d, &["checkout", "main"]);
    add_commit(d, "x.txt", "line1\nmain\nline3\n", "main edit");
    let head_before = head_oid(d);
    let worktree_before = read(d, "x.txt");

    let outcome = cherrypick_commit(d, &pick).expect("cherry-pick");
    assert!(matches!(outcome, CherrypickOutcome::Conflicts { .. }));
    assert_eq!(repo_state(d), git2::RepositoryState::CherryPick);

    cherrypick_abort(d).expect("abort");

    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(head_oid(d), head_before, "HEAD unchanged after abort");
    assert_eq!(read(d, "x.txt"), worktree_before, "worktree back at HEAD");
}

/// Aborting a conflicting revert restores state Clean, HEAD, and worktree.
#[test]
fn essentials_8b_revert_abort_restores_head() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    add_commit(d, "x.txt", "line1\nbase\nline3\n", "base");
    add_commit(d, "x.txt", "line1\nv2\nline3\n", "second");
    let c2 = head_oid(d);
    add_commit(d, "x.txt", "line1\nv3\nline3\n", "third");
    let head_before = head_oid(d);
    let worktree_before = read(d, "x.txt");

    let outcome = revert_commit(d, &c2).expect("revert");
    assert!(matches!(outcome, RevertOutcome::Conflicts { .. }));
    assert_eq!(repo_state(d), git2::RepositoryState::Revert);

    revert_abort(d).expect("abort");

    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(head_oid(d), head_before, "HEAD unchanged after abort");
    assert_eq!(read(d, "x.txt"), worktree_before, "worktree back at HEAD");
}
