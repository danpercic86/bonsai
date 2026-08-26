//! Bisect state-machine unit tests. Extracted verbatim from the former
//! inline `mod tests` (file-size discipline).

use super::*;
use crate::testutil::scratch_dir;

fn sig() -> git2::Signature<'static> {
    git2::Signature::now("Test", "test@example.com").expect("sig")
}

/// Linear repo of `n` commits on the default branch (each adds `f{i}.txt`);
/// from commit index `bug_at` onward each also writes `bug.txt` (the marker
/// the predicate greps). Returns (dir, oids oldest-first).
fn linear_repo_with_bug(n: usize, bug_at: usize) -> (tempfile::TempDir, Vec<String>) {
    let dir = scratch_dir();
    // Pins the initial branch to "main" via `initial_head` rather than
    // relying on `init.defaultBranch` — libgit2 falls back to "master"
    // when that config is unset, which `reset_restores_original_branch` assumes.
    let repo = git2::Repository::init_opts(dir.path(), git2::RepositoryInitOptions::new().initial_head("main"))
        .expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
    }
    let s = sig();
    let mut oids = Vec::new();
    let mut parents: Vec<git2::Commit> = Vec::new();
    for i in 0..n {
        std::fs::write(dir.path().join(format!("f{i}.txt")), format!("c{i}\n")).expect("write");
        if i >= bug_at {
            std::fs::write(dir.path().join("bug.txt"), "boom\n").expect("write bug");
        }
        let mut idx = repo.index().expect("index");
        idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("add");
        idx.write().expect("write index");
        let tree = repo
            .find_tree(idx.write_tree().expect("tree"))
            .expect("find tree");
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let commit_oid = repo
            .commit(Some("HEAD"), &s, &s, &format!("c{i}"), &tree, &parent_refs)
            .expect("commit");
        oids.push(commit_oid.to_string());
        parents = vec![repo.find_commit(commit_oid).expect("find commit")];
    }
    (dir, oids)
}

/// True iff the commit's tree carries the `bug.txt` marker.
fn has_bug(repo: &git2::Repository, oid_str: &str) -> bool {
    let c = repo
        .find_commit(git2::Oid::from_str(oid_str).expect("oid"))
        .expect("commit");
    c.tree().expect("tree").get_name("bug.txt").is_some()
}

// ------------------------------------------------ wire shapes (TS mirrors)

#[test]
fn bisect_outcome_wire_shape_is_camel_case() {
    let v = serde_json::to_value(BisectOutcome::Testing {
        current: "a".repeat(40),
        revisions_remaining: 4,
        estimated_steps: 2,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "testing",
            "current": "a".repeat(40),
            "revisionsRemaining": 4,
            "estimatedSteps": 2
        })
    );

    let v = serde_json::to_value(BisectOutcome::Found {
        first_bad: "b".repeat(40),
    })
    .expect("json");
    assert_eq!(v, serde_json::json!({ "kind": "found", "firstBad": "b".repeat(40) }));

    let v = serde_json::to_value(BisectOutcome::CannotDetermine {
        skipped: vec!["c".repeat(40)],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "cannotDetermine", "skipped": ["c".repeat(40)] })
    );
}

#[test]
fn estimated_steps_is_ceil_log2() {
    assert_eq!(estimated_steps(0), 0);
    assert_eq!(estimated_steps(1), 0);
    assert_eq!(estimated_steps(2), 1);
    assert_eq!(estimated_steps(3), 2);
    assert_eq!(estimated_steps(4), 2);
    assert_eq!(estimated_steps(5), 3);
    assert_eq!(estimated_steps(8), 3);
    assert_eq!(estimated_steps(9), 4);
}

#[test]
fn state_round_trips_on_disk() {
    let (dir, oids) = linear_repo_with_bug(3, 2);
    let repo = git2::Repository::open(dir.path()).expect("open");
    let state = BisectState {
        version: 1,
        original_head: oids[2].clone(),
        original_branch: Some("main".to_string()),
        bad: oids[2].clone(),
        good: vec![oids[0].clone()],
        skipped: Vec::new(),
        current: Some(oids[1].clone()),
        first_bad: None,
    };
    assert!(!bisect_in_progress(&repo));
    write_state(&repo, &state).expect("write");
    assert!(bisect_in_progress(&repo));
    assert_eq!(read_state(&repo).expect("read"), state);
    remove_state(&repo);
    assert!(!bisect_in_progress(&repo));
}

// ------------------------------------------------------- preconditions

#[test]
fn start_rejects_unborn() {
    let dir = scratch_dir();
    git2::Repository::init(dir.path()).expect("init");
    match start_bisect(dir.path(), &"0".repeat(40), &["0".repeat(40)]).expect_err("unborn") {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn start_rejects_blank_args() {
    let (dir, oids) = linear_repo_with_bug(3, 2);
    // Blank bad.
    assert!(matches!(
        start_bisect(dir.path(), "", &[oids[0].clone()]).expect_err("blank bad"),
        AppError::Git(_)
    ));
    // Empty good list.
    assert!(matches!(
        start_bisect(dir.path(), &oids[2], &[]).expect_err("no good"),
        AppError::Git(_)
    ));
    // Blank good entry.
    assert!(matches!(
        start_bisect(dir.path(), &oids[2], &["".to_string()]).expect_err("blank good"),
        AppError::Git(_)
    ));
}

#[test]
fn start_rejects_same_good_bad() {
    let (dir, oids) = linear_repo_with_bug(3, 2);
    match start_bisect(dir.path(), &oids[2], &[oids[2].clone()]).expect_err("same") {
        AppError::Git(m) => assert!(m.contains("same commit"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn start_rejects_non_ancestor_good() {
    // Two independent roots: good on a sibling branch is not an ancestor of bad.
    let dir = scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
    }
    let s = sig();
    // main: one commit.
    std::fs::write(dir.path().join("a.txt"), "a\n").expect("write");
    let mut idx = repo.index().expect("index");
    idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).expect("add");
    idx.write().expect("write");
    let tree = repo.find_tree(idx.write_tree().expect("t")).expect("ft");
    let bad = repo.commit(Some("HEAD"), &s, &s, "bad", &tree, &[]).expect("c");
    // orphan branch with a disjoint root commit.
    std::fs::write(dir.path().join("b.txt"), "b\n").expect("write");
    let mut idx2 = repo.index().expect("index");
    idx2.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).expect("add");
    idx2.write().expect("write");
    let tree2 = repo.find_tree(idx2.write_tree().expect("t")).expect("ft");
    let good = repo
        .commit(Some("refs/heads/other"), &s, &s, "other", &tree2, &[])
        .expect("c2");
    match start_bisect(dir.path(), &bad.to_string(), &[good.to_string()])
        .expect_err("non-ancestor")
    {
        AppError::Git(m) => assert!(m.contains("not an ancestor"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn mark_and_skip_without_start_are_no_op_in_progress() {
    let (dir, _oids) = linear_repo_with_bug(2, 1);
    assert!(matches!(
        bisect_mark(dir.path(), true).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        bisect_skip(dir.path()).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        bisect_reset(dir.path()).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
}

// ------------------------------------------------------- convergence

#[test]
fn linear_bisect_converges() {
    let n = 12;
    let bug_at = 7;
    let (dir, oids) = linear_repo_with_bug(n, bug_at);
    let repo = git2::Repository::open(dir.path()).expect("open");

    let mut outcome =
        start_bisect(dir.path(), &oids[n - 1], &[oids[0].clone()]).expect("start");
    let mut guard = 0;
    loop {
        guard += 1;
        assert!(guard < 50, "bisect did not converge");
        match outcome {
            BisectOutcome::Testing { current, .. } => {
                let bad = has_bug(&repo, &current);
                outcome = bisect_mark(dir.path(), !bad).expect("mark");
            }
            BisectOutcome::Found { first_bad } => {
                assert_eq!(first_bad, oids[bug_at], "culprit is the bug-introducing commit");
                // HEAD is detached at the culprit.
                let re = git2::Repository::open(dir.path()).expect("open");
                assert!(re.head_detached().expect("detached"));
                assert_eq!(
                    re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
                    oids[bug_at]
                );
                break;
            }
            BisectOutcome::CannotDetermine { .. } => panic!("unexpected cannotDetermine"),
        }
    }
}

#[test]
fn skip_picks_adjacent() {
    let (dir, oids) = linear_repo_with_bug(8, 5);
    let first = match start_bisect(dir.path(), &oids[7], &[oids[0].clone()]).expect("start") {
        BisectOutcome::Testing { current, .. } => current,
        other => panic!("expected Testing, got {other:?}"),
    };
    match bisect_skip(dir.path()).expect("skip") {
        BisectOutcome::Testing { current, .. } => {
            assert_ne!(current, first, "skip picks a different candidate");
        }
        other => panic!("expected Testing after skip, got {other:?}"),
    }
}

#[test]
fn all_skipped_cannot_determine() {
    // 3 commits: good=c0, bad=c2 → only c1 is testable. Skipping it exhausts
    // the testable set with an unresolved skipped candidate.
    let (dir, oids) = linear_repo_with_bug(3, 2);
    match start_bisect(dir.path(), &oids[2], &[oids[0].clone()]).expect("start") {
        BisectOutcome::Testing { current, .. } => assert_eq!(current, oids[1]),
        other => panic!("expected Testing, got {other:?}"),
    }
    match bisect_skip(dir.path()).expect("skip") {
        BisectOutcome::CannotDetermine { skipped } => {
            assert_eq!(skipped, vec![oids[1].clone()]);
        }
        other => panic!("expected CannotDetermine, got {other:?}"),
    }
}

#[test]
fn mark_and_skip_reject_when_head_moved_off_midpoint() {
    let (dir, oids) = linear_repo_with_bug(8, 5);
    let repo = git2::Repository::open(dir.path()).expect("open");

    let midpoint = match start_bisect(dir.path(), &oids[7], &[oids[0].clone()]).expect("start") {
        BisectOutcome::Testing { current, .. } => current,
        other => panic!("expected Testing, got {other:?}"),
    };
    let before = read_state(&repo).expect("read");

    // Move HEAD to a DIFFERENT (clean) commit externally via a clean checkout
    // so `ensure_clean` still passes but HEAD != state.current.
    let other = if midpoint == oids[0] { &oids[7] } else { &oids[0] };
    checkout_commit(&repo, git2::Oid::from_str(other).expect("oid")).expect("checkout");

    match bisect_mark(dir.path(), false).expect_err("head moved") {
        AppError::Git(m) => assert!(m.contains("not on the bisect commit"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    match bisect_skip(dir.path()).expect_err("head moved") {
        AppError::Git(m) => assert!(m.contains("not on the bisect commit"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // The rejected mark/skip left the bisect state byte-identical (no verdict
    // recorded against the wrong commit).
    assert_eq!(read_state(&repo).expect("read"), before);
    assert_eq!(before.current.as_deref(), Some(midpoint.as_str()));
}

// ------------------------------------------------------- reset

#[test]
fn reset_restores_original_branch() {
    let (dir, oids) = linear_repo_with_bug(6, 3);
    let repo = git2::Repository::open(dir.path()).expect("open");
    let orig_tip = repo.head().expect("head").peel_to_commit().expect("c").id().to_string();

    start_bisect(dir.path(), &oids[5], &[oids[0].clone()]).expect("start");
    // Now HEAD is detached on some midpoint.
    assert!(bisect_in_progress(&repo));

    bisect_reset(dir.path()).expect("reset");
    let re = git2::Repository::open(dir.path()).expect("open");
    assert!(!bisect_in_progress(&re), "state dir removed");
    assert!(!re.head_detached().expect("detached"), "HEAD re-attached");
    assert_eq!(re.head().expect("head").shorthand().ok(), Some("main"));
    assert_eq!(
        re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
        orig_tip
    );
}

// ---------------------------------------------- user-mutation guard (Fix 1)

/// While a Bonsai bisect is active the HEAD is detached and `repo.state()`
/// is Clean, so the ordinary `state() != Clean` checks can't see it. The
/// user-facing mutating cores must therefore refuse via `require_no_bisect`.
#[test]
fn active_bisect_blocks_user_mutations() {
    use crate::git::commit::create_commit;
    use crate::git::reset::{reset_branch, ResetMode};
    use crate::git::stash::{create_stash, StashScope};

    let (dir, oids) = linear_repo_with_bug(8, 5);
    let d = dir.path();

    match start_bisect(d, &oids[7], &[oids[0].clone()]).expect("start") {
        BisectOutcome::Testing { .. } => {}
        other => panic!("expected Testing, got {other:?}"),
    }
    let repo = git2::Repository::open(d).expect("open");
    assert!(bisect_in_progress(&repo));
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "a Bonsai bisect keeps libgit2 op-state Clean"
    );

    // A worktree edit so create_stash would otherwise have work to do.
    std::fs::write(d.join("f0.txt"), "dirty\n").expect("edit");

    assert!(
        matches!(
            create_commit(d, "blocked", None, false).expect_err("commit blocked"),
            AppError::OperationInProgress(_)
        ),
        "commit must be refused mid-bisect"
    );
    assert!(
        matches!(
            reset_branch(d, &oids[0], ResetMode::Soft).expect_err("reset blocked"),
            AppError::OperationInProgress(_)
        ),
        "reset must be refused mid-bisect"
    );
    assert!(
        matches!(
            create_stash(d, None, StashScope::All).expect_err("stash blocked"),
            AppError::OperationInProgress(_)
        ),
        "stash-create must be refused mid-bisect"
    );

    // The bisect state is intact — the refusals mutated nothing.
    assert!(bisect_in_progress(&repo), "bisect still active after refusals");
}

/// Audit 2026-08-07 §3.1: the restore path shares the untracked-clobber
/// guard. An untracked file (e.g. generated during a bisect test run)
/// colliding with a tracked path at the ORIGINAL head makes `bisect_reset`
/// refuse: file content preserved, bisect state intact and retryable.
#[test]
fn reset_refuses_untracked_collision_and_stays_retryable() {
    let (dir, oids) = linear_repo_with_bug(6, 3);
    let d = dir.path();
    start_bisect(d, &oids[5], &[oids[0].clone()]).expect("start");
    let repo = git2::Repository::open(d).expect("open");
    assert!(bisect_in_progress(&repo));

    // The midpoint predates c5, so its checkout removed f5.txt. Plant an
    // UNTRACKED f5.txt that the restore's force checkout would clobber.
    assert!(!d.join("f5.txt").exists(), "midpoint must predate c5");
    std::fs::write(d.join("f5.txt"), "precious build artifact\n").expect("plant");

    let err = bisect_reset(d).expect_err("must refuse the clobber");
    assert!(
        matches!(&err, AppError::Git(m) if m.contains("f5.txt")
            && m.contains("would be overwritten")),
        "got {err:?}"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("f5.txt")).expect("read"),
        "precious build artifact\n",
        "untracked file untouched"
    );
    assert!(
        bisect_in_progress(&repo),
        "state intact — the refusal must leave the bisect retryable"
    );

    // Clear the collision → the retry succeeds and restores the branch.
    std::fs::remove_file(d.join("f5.txt")).expect("remove");
    bisect_reset(d).expect("retry succeeds");
    let re = git2::Repository::open(d).expect("open");
    assert!(!bisect_in_progress(&re));
    assert_eq!(
        re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
        oids[5]
    );
}

#[test]
fn reset_restores_detached_start() {
    let (dir, oids) = linear_repo_with_bug(6, 3);
    let repo = git2::Repository::open(dir.path()).expect("open");
    // Detach HEAD at the tip before starting.
    let tip = git2::Oid::from_str(&oids[5]).expect("oid");
    repo.set_head_detached(tip).expect("detach");

    start_bisect(dir.path(), &oids[5], &[oids[0].clone()]).expect("start");
    bisect_reset(dir.path()).expect("reset");

    let re = git2::Repository::open(dir.path()).expect("open");
    assert!(!bisect_in_progress(&re));
    assert!(re.head_detached().expect("detached"), "detached start stays detached");
    assert_eq!(
        re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
        oids[5]
    );
}
