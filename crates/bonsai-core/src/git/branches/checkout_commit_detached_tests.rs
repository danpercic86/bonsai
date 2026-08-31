//! Contract §6 (docs/contracts/checkout-commit-backend.md): `checkout_commit_detached`
//! detaches HEAD onto an arbitrary commit oid, dirty-safe (auto-stash -> safe
//! checkout_tree -> set_head_detached -> re-apply stash), NEVER lossy. Every
//! test asserts the observable git state (returned `CheckoutResult`, HEAD
//! detached + target, worktree contents, stash stack), not just the return value.
//!
//! Fixtures are built with git2 in a scratch `TempDir` (deterministic, no
//! network, no CLI), mirroring `checkout_autostash_tests`.

use super::*;
use crate::git::stash::{list_stashes, ApplyStashOutcome};

/// Init a scratch repo with a deterministic identity + autocrlf off, HEAD "main".
fn cd_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
        .expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
fn cd_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
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

fn cd_head_oid(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let oid = repo
        .head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("peel")
        .id()
        .to_string();
    oid
}

fn cd_head_detached(dir: &Path) -> bool {
    let repo = git2::Repository::open(dir).expect("open");
    repo.head_detached().expect("head_detached")
}

/// The short branch name HEAD points at, or None when detached/unborn.
fn cd_head_branch(dir: &Path) -> Option<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    head.shorthand().ok().map(str::to_string)
}

fn cd_read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

// ---------------------------------------- Case 1: clean tree, detach on a commit

/// §6: clean worktree, detach onto a non-tip commit → HEAD detached at the oid,
/// `{ stashed:false, fast_forwarded:false, apply:None }`, worktree matches the
/// target tree, no stash created.
#[test]
fn cd_1_clean_detach_at_oid() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = cd_init(d);
    cd_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid").to_string();
    // Advance so C0 is a non-tip commit.
    cd_commit(d, "C1", &[("b.txt", "b1\n")]);
    assert_eq!(cd_head_branch(d).as_deref(), Some("main"));

    let res = checkout_commit_detached(d, &c0).expect("detach");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        },
        "clean detach → no stash, no FF, no apply"
    );

    assert!(cd_head_detached(d), "HEAD must be detached");
    assert_eq!(cd_head_branch(d), None, "no branch attached");
    assert_eq!(cd_head_oid(d), c0, "HEAD points at the target oid");
    assert_eq!(cd_read(d, "a.txt"), "base\n", "target tree checked out");
    assert!(!d.join("b.txt").exists(), "later commit's file gone");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
}

// -------------------------------------- Case 2: dirty tree, clean re-apply

/// §6: uncommitted edit that does NOT conflict at the target → auto-stashed and
/// re-applied cleanly: `{ stashed:true, .., apply:Some(Applied) }`; the edit is
/// present at the detached target; the stash was DROPPED.
#[test]
fn cd_2_dirty_clean_reapply() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = cd_init(d);
    // a.txt is set at C0 and untouched at C1, so the stashed edit re-applies clean.
    cd_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid").to_string();
    cd_commit(d, "C1", &[("b.txt", "b1\n")]);

    // Dirty: unstaged edit to a.txt (unchanged at C0 → clean carry-over).
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    let res = checkout_commit_detached(d, &c0).expect("detach");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: true,
            fast_forwarded: false,
            apply: Some(ApplyStashOutcome::Applied)
        },
        "dirty tree carries cleanly across detach → Applied"
    );

    assert!(cd_head_detached(d), "HEAD detached");
    assert_eq!(cd_head_oid(d), c0, "HEAD at target");
    assert_eq!(cd_read(d, "a.txt"), "edited\n", "carried edit preserved");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "clean pop dropped the stash"
    );
}

// ------------------------------- Case 3: dirty tree, conflicting re-apply

/// §6 (KEY DATA-SAFETY CASE): edit to a file that differs at the target so the
/// 3-way re-apply conflicts → SUCCESS with `apply:Some(Conflicts{paths})` (NOT
/// Err); the stash is RETAINED at stash@{0}; repo state stays Clean (not Merge).
#[test]
fn cd_3_dirty_conflicting_reapply_retains_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = cd_init(d);
    // a.txt differs between C0 and C1; the dirty edit vs both differs → conflict.
    cd_commit(d, "C0", &[("a.txt", "target-side\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid").to_string();
    cd_commit(d, "C1", &[("a.txt", "main-side\n")]);

    // Dirty edit to a.txt (stash base == C1 "main-side").
    std::fs::write(d.join("a.txt"), "dirty\n").expect("edit a.txt");

    let res = checkout_commit_detached(d, &c0).expect("detach is Ok");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: true,
            fast_forwarded: false,
            apply: Some(ApplyStashOutcome::Conflicts {
                paths: vec!["a.txt".to_string()]
            })
        },
        "conflicting carry-over reports Conflicts on a.txt as a SUCCESS"
    );

    assert!(cd_head_detached(d), "detach happened");
    assert_eq!(cd_head_oid(d), c0, "HEAD at target");

    let repo = git2::Repository::open(d).expect("reopen");
    assert!(
        repo.index().expect("index").has_conflicts(),
        "index must carry conflict entries"
    );
    assert!(
        cd_read(d, "a.txt").contains("<<<<<<<"),
        "worktree a.txt must carry conflict markers"
    );
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "a conflicted stash pop must NOT leave the repo in Merge state"
    );
    // DATA SAFETY: the user's work must be recoverable.
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "conflicting carry-over must RETAIN the stash (never lossy)"
    );
}

// --------------------------------------- Case 4: no-op when already detached

/// §6: HEAD already detached AT the exact oid → `{ false, false, None }`, no
/// side effects (even with a dirty tree: no spurious stash).
#[test]
fn cd_4_already_detached_at_oid_noop() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = cd_init(d);
    cd_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid").to_string();
    cd_commit(d, "C1", &[("b.txt", "b1\n")]);

    // First detach (clean) onto C0.
    checkout_commit_detached(d, &c0).expect("first detach");
    assert!(cd_head_detached(d));

    // Now dirty the tree so a stray auto-stash would be observable.
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    let res = checkout_commit_detached(d, &c0).expect("no-op detach");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        },
        "re-detaching to the current detached oid is a no-op"
    );
    assert!(cd_head_detached(d), "still detached");
    assert_eq!(cd_head_oid(d), c0, "still at C0");
    assert_eq!(cd_read(d, "a.txt"), "edited\n", "dirty edit untouched");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no spurious stash");
}

// -------------------------------------- Case 5a: bad oid — unparseable name

/// §6: an oid that is not a parseable git oid → `Err(InvalidName)`, HEAD
/// unchanged, no stash.
#[test]
fn cd_5a_bad_oid_invalid_name() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cd_init(d);
    cd_commit(d, "C0", &[("a.txt", "base\n")]);
    let head_before = cd_head_oid(d);
    let branch_before = cd_head_branch(d);

    match checkout_commit_detached(d, "not-a-valid-oid") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName, got {other:?}"),
    }
    assert_eq!(cd_head_oid(d), head_before, "HEAD oid unchanged");
    assert_eq!(cd_head_branch(d), branch_before, "still on the branch");
    assert!(!cd_head_detached(d), "not detached");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
}

// ----------------------------- Case 5b: bad oid — well-formed but missing

/// §6: a well-formed 40-hex oid that does not exist in the repo → `Err(Git)`,
/// HEAD unchanged, no stash.
#[test]
fn cd_5b_bad_oid_missing_commit() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cd_init(d);
    cd_commit(d, "C0", &[("a.txt", "base\n")]);
    let head_before = cd_head_oid(d);
    let branch_before = cd_head_branch(d);

    // Valid hex, but no such object.
    let missing = "0123456789abcdef0123456789abcdef01234567";
    match checkout_commit_detached(d, missing) {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git error for missing oid, got {other:?}"),
    }
    assert_eq!(cd_head_oid(d), head_before, "HEAD oid unchanged");
    assert_eq!(cd_head_branch(d), branch_before, "still on the branch");
    assert!(!cd_head_detached(d), "not detached");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
}

// --------------------- Case 6: detach from current branch tip is a real change

/// §5 case (e): detaching onto the CURRENT branch tip oid is NOT a no-op — it
/// converts attached HEAD into detached HEAD at the same commit.
#[test]
fn cd_6_detach_at_current_branch_tip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cd_init(d);
    cd_commit(d, "C0", &[("a.txt", "base\n")]);
    let tip = cd_head_oid(d);
    assert_eq!(cd_head_branch(d).as_deref(), Some("main"));

    let res = checkout_commit_detached(d, &tip).expect("detach at tip");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        }
    );
    assert!(cd_head_detached(d), "HEAD now detached");
    assert_eq!(cd_head_branch(d), None, "no branch attached");
    assert_eq!(cd_head_oid(d), tip, "same commit oid");
}
