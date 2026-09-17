//! P23a CLI-oracle interactive-rebase tests — the start/continue/skip/abort
//! precondition matrix and the untracked-collision data-loss guards
//! (contract §13.1).
//!
//! Split out of `rebase_interactive_cli.rs`; shared fixtures and helpers live in
//! `rebase_interactive_support.rs`.

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::rebase_interactive_support::{
    has_bonsai_dir, read_str, repo_state, require_git, rev, script_conflict, script_two_disjoint,
    symbolic_head, write,
};
use bonsai_core::error::AppError;
use bonsai_core::git::rebase::{rebase_abort, rebase_continue, rebase_skip, RebaseOutcome};
use bonsai_core::git::rebase_interactive::{
    get_interactive_plan, start_interactive_rebase, RebaseAction, RebaseTodoOp,
};

// ============================================================ precondition matrix

#[test]
fn precondition_interactive_already_in_progress() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_conflict(d);
    let onto = rev(d, "main");
    let topic_tip = rev(d, "topic");
    let todos = vec![RebaseTodoOp {
        oid: topic_tip.clone(),
        action: RebaseAction::Pick,
        new_message: None,
    }];
    match start_interactive_rebase(d, &onto, todos.clone()).expect("start") {
        RebaseOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    // A second start refuses.
    assert!(matches!(
        start_interactive_rebase(d, &onto, todos).expect_err("already in progress"),
        AppError::OperationInProgress(_)
    ));
    rebase_abort(d).expect("abort cleanup");
}

/// A git-NATIVE rebase already in progress (`repo.state() != Clean`, its own
/// `.git/rebase-merge` sequencer, NOT the Bonsai one) must block a Bonsai
/// interactive-rebase START with `OperationInProgress` (contract §2.4 step 3).
/// This is the sibling of `precondition_interactive_already_in_progress`, which
/// exercises the Bonsai-sequencer branch; here the guard is `repo.state()`.
#[test]
fn precondition_git_native_rebase_in_progress_is_refused() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_conflict(d); // on `topic`; replaying it onto `main` conflicts on a.txt
    let onto = rev(d, "main");
    let topic_tip = rev(d, "topic");

    // Kick off a git-native rebase that stops on a conflict, leaving a
    // git-owned sequencer + a non-Clean repo state.
    assert!(
        !common::git_ok(d, &["rebase", "main"]),
        "git rebase should stop with a conflict"
    );
    assert_ne!(
        repo_state(d),
        git2::RepositoryState::Clean,
        "a git-native rebase must be in progress"
    );
    assert!(
        !has_bonsai_dir(d),
        "no Bonsai sequencer yet — the git-native one is separate"
    );

    // Bonsai interactive start must refuse via the repo.state() guard (§2.4 step 3).
    let todos = vec![RebaseTodoOp {
        oid: topic_tip,
        action: RebaseAction::Pick,
        new_message: None,
    }];
    assert!(matches!(
        start_interactive_rebase(d, &onto, todos).expect_err("git-native op in progress"),
        AppError::OperationInProgress(_)
    ));
    // A rejected start must not have written a Bonsai sequencer over the git one.
    assert!(
        !has_bonsai_dir(d),
        "rejected start leaves no .git/bonsai-rebase"
    );

    // Clean up the git-native rebase (tempdir is dropped anyway).
    let _ = common::git_ok(d, &["rebase", "--abort"]);
}

#[test]
fn precondition_dirty_worktree_is_rejected() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");
    // Unstaged edit to a tracked file.
    write(d, "a.txt", "dirty\n");
    let todos = get_interactive_plan(d, &base).expect("plan");
    match start_interactive_rebase(d, &base, todos).expect_err("dirty") {
        AppError::Git(m) => assert!(
            m.contains("unstaged") || m.contains("uncommitted"),
            "got: {m}"
        ),
        other => panic!("expected Git, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

#[test]
fn precondition_detached_head_is_rejected() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");
    git(d, &["checkout", "--detach"]);
    let todos = vec![RebaseTodoOp {
        oid: rev(d, "HEAD"),
        action: RebaseAction::Pick,
        new_message: None,
    }];
    match start_interactive_rebase(d, &base, todos).expect_err("detached") {
        AppError::Git(m) => assert!(m.contains("detached"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn precondition_unborn_head_is_rejected() {
    require_git!();
    let dir = init_repo();
    match start_interactive_rebase(dir.path(), &"0".repeat(40), Vec::new()).expect_err("unborn") {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn precondition_bad_plan_is_rejected() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");

    // Empty plan.
    assert!(matches!(
        start_interactive_rebase(d, &base, Vec::new()).expect_err("empty"),
        AppError::Git(_)
    ));

    // Squash as the first (only kept) op.
    let squash_first = vec![RebaseTodoOp {
        oid: rev(d, "topic"),
        action: RebaseAction::Squash,
        new_message: None,
    }];
    assert!(matches!(
        start_interactive_rebase(d, &base, squash_first).expect_err("squash first"),
        AppError::Git(_)
    ));
    assert!(
        !has_bonsai_dir(d),
        "no sequencer left behind by a rejected plan"
    );
}

#[test]
fn precondition_missing_identity_is_config_missing_before_mutation() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");
    // Blank the repo-local identity.
    git(d, &["config", "user.name", ""]);
    git(d, &["config", "user.email", ""]);
    let todos = get_interactive_plan(d, &base).expect("plan");
    match start_interactive_rebase(d, &base, todos).expect_err("no identity") {
        AppError::ConfigMissing(_) => {}
        other => panic!("expected ConfigMissing, got {other:?}"),
    }
    assert_eq!(
        repo_state(d),
        git2::RepositoryState::Clean,
        "state stays Clean"
    );
    assert!(!has_bonsai_dir(d), "no sequencer left behind");
}

#[test]
fn continue_skip_abort_without_a_rebase_are_rejected() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    assert!(matches!(
        rebase_continue(d).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        rebase_skip(d).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        rebase_abort(d).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
}

// ------------------------------------------------ untracked-collision data-loss guard

/// DATA-LOSS SAFETY: an untracked, non-ignored worktree file whose path collides
/// with a file present in the `onto` tree must NOT be silently clobbered by the
/// force checkout that seeds the replay. Start must refuse, preserve the
/// untracked file byte-for-byte, and write no rebase state (HEAD/branch intact).
#[test]
fn start_refuses_when_untracked_file_would_be_clobbered_by_onto_checkout() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    // c0 tracks foo.txt; c1 removes it. onto = c0, whose tree still carries foo.txt.
    write(d, "a.txt", "a\n");
    write(d, "foo.txt", "from-onto\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c0");
    let c0 = rev(d, "HEAD");

    git(d, &["rm", "foo.txt"]);
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");
    let c1 = rev(d, "HEAD");

    // Untracked foo.txt in the worktree collides with c0's tree.
    write(d, "foo.txt", "UNTRACKED-LOCAL\n");

    let todos = vec![RebaseTodoOp {
        oid: c1.clone(),
        action: RebaseAction::Pick,
        new_message: None,
    }];
    match start_interactive_rebase(d, &c0, todos).expect_err("collision must refuse") {
        AppError::Git(m) => assert!(
            m.contains("would be overwritten by checkout") && m.contains("foo.txt"),
            "got: {m}"
        ),
        other => panic!("expected Git, got {other:?}"),
    }

    // The untracked file is untouched, no rebase started, branch/HEAD unmoved.
    assert_eq!(
        read_str(d, "foo.txt"),
        "UNTRACKED-LOCAL\n",
        "untracked file was clobbered"
    );
    assert!(
        !has_bonsai_dir(d),
        "a refused start must leave no rebase state"
    );
    assert_eq!(rev(d, "HEAD"), c1, "HEAD still at the original tip");
    assert_eq!(
        symbolic_head(d),
        "refs/heads/main",
        "still on the original branch"
    );
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

/// DATA-LOSS SAFETY (type-swap): an untracked file nested UNDER a directory whose
/// path is a BLOB in the `onto` tree must also be caught. The force checkout
/// replaces the directory with the file, deleting the untracked file inside it —
/// yet `get_path("foo/bar.txt")` traverses the blob `foo` and returns Err, so the
/// naive direct check would miss it. Start must still refuse.
#[test]
fn start_refuses_when_untracked_file_nested_under_a_target_blob() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    // c0 tracks a BLOB at `foo`; c1 removes it. onto = c0.
    write(d, "a.txt", "a\n");
    write(d, "foo", "i-am-a-file\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c0");
    let c0 = rev(d, "HEAD");

    git(d, &["rm", "foo"]);
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");
    let c1 = rev(d, "HEAD");

    // Untracked file nested in a NEW dir `foo/` — collides with c0's blob `foo`.
    std::fs::create_dir(d.join("foo")).expect("mkdir foo");
    write(d, "foo/bar.txt", "UNTRACKED-NESTED\n");

    let todos = vec![RebaseTodoOp {
        oid: c1.clone(),
        action: RebaseAction::Pick,
        new_message: None,
    }];
    match start_interactive_rebase(d, &c0, todos).expect_err("type-swap collision must refuse") {
        AppError::Git(m) => assert!(
            m.contains("would be overwritten by checkout") && m.contains("foo/bar.txt"),
            "got: {m}"
        ),
        other => panic!("expected Git, got {other:?}"),
    }

    // The nested untracked file is untouched, no rebase started, branch/HEAD unmoved.
    assert_eq!(
        read_str(d, "foo/bar.txt"),
        "UNTRACKED-NESTED\n",
        "nested untracked file was clobbered"
    );
    assert!(
        !has_bonsai_dir(d),
        "a refused start must leave no rebase state"
    );
    assert_eq!(rev(d, "HEAD"), c1, "HEAD still at the original tip");
    assert_eq!(
        symbolic_head(d),
        "refs/heads/main",
        "still on the original branch"
    );
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}
