//! P23a CLI-oracle interactive-rebase tests (contract §13.1).
//!
//! Fixtures are built with the `git` CLI (fixed dates -> deterministic base
//! oids), then Bonsai's interactive engine runs the rebase and the result is
//! asserted against a hand-built git-equivalent expectation. Because the
//! fixtures touch DISJOINT files (reorder/squash/fixup/reword/drop), the final
//! tree is deterministic and is computed directly from the fixture instead of
//! scripting `git rebase -i` (which is fiddly and non-portable) — this is the
//! contract's allowed "hand-built git-equivalent expectation" (§13.1).
//!
//! Locked comparison rule (§13): committer time = now(), so REPLAYED commit oids
//! differ from any twin. We compare TREE oids, author identity, messages, and
//! parent topology — never replayed commit oids.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch`. Each test skips
//! (passes with a note) if `git` is not on PATH.

use bonsai_core::git::rebase::RebaseOutcome;
use bonsai_core::git::rebase_interactive::{
    get_interactive_plan, start_interactive_rebase, RebaseAction, RebaseTodoOp,
};
use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::rebase_interactive_support::{
    author_of, count_ahead, has_bonsai_dir, msg_of, repo_state, require_git, rev,
    script_three_disjoint, script_two_disjoint, symbolic_head, tree_files, tree_of, write,
};

// ============================================================ reorder

#[test]
fn reorder_swaps_top_two_commits() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_three_disjoint(d);
    let base = rev(d, "main");
    let orig_tree = tree_of(d, "topic");
    let orig_c1_author = author_of(d, "topic~2"); // c1

    let mut todos = get_interactive_plan(d, &base).expect("plan");
    assert_eq!(todos.len(), 3);
    todos.swap(1, 2); // [c1, c2, c3] -> [c1, c3, c2]

    match start_interactive_rebase(d, &base, todos).expect("start") {
        RebaseOutcome::Rebased { branch, head, steps, .. } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, 3);
            assert_eq!(head, rev(d, "HEAD"));
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    // Disjoint files -> final tree unchanged; only ORDER differs.
    assert_eq!(tree_of(d, "HEAD"), orig_tree, "final tree must match original");
    assert_eq!(msg_of(d, "HEAD~2"), "c1");
    assert_eq!(msg_of(d, "HEAD~1"), "c3", "swapped: c3 now precedes c2");
    assert_eq!(msg_of(d, "HEAD"), "c2");
    assert_eq!(count_ahead(d, &base, "HEAD"), 3);
    assert_eq!(rev(d, "HEAD~3"), base, "chain roots at the onto base");
    assert_eq!(
        author_of(d, "HEAD~2"),
        orig_c1_author,
        "author identity + author time preserved"
    );
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
    assert_eq!(symbolic_head(d), "refs/heads/topic", "HEAD re-attached to topic");
}

// ============================================================ squash

#[test]
fn squash_combines_two_into_one() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");
    let orig_tree = tree_of(d, "topic");
    let orig_c1_author = author_of(d, "topic~1"); // predecessor c1

    let mut todos = get_interactive_plan(d, &base).expect("plan");
    todos[1].action = RebaseAction::Squash;
    todos[1].new_message = Some("combined squash".to_string());

    match start_interactive_rebase(d, &base, todos).expect("start") {
        // `steps` counts ops APPLIED (pick + squash = 2); the resulting TOPOLOGY
        // is one commit (the squash replaces the pick).
        RebaseOutcome::Rebased { steps, .. } => assert_eq!(steps, 2, "two ops applied"),
        other => panic!("expected Rebased, got {other:?}"),
    }

    assert_eq!(count_ahead(d, &base, "HEAD"), 1, "commit count dropped by one");
    assert_eq!(tree_of(d, "HEAD"), orig_tree, "combined tree == original tree");
    assert_eq!(msg_of(d, "HEAD"), "combined squash", "combined message");
    assert_eq!(author_of(d, "HEAD"), orig_c1_author, "squash keeps the predecessor's author (N3)");
    assert_eq!(rev(d, "HEAD~1"), base, "parent == the onto base");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

// ============================================================ fixup

#[test]
fn fixup_discards_message_keeps_tree() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");
    let orig_tree = tree_of(d, "topic");
    let orig_c1_author = author_of(d, "topic~1"); // predecessor c1

    let mut todos = get_interactive_plan(d, &base).expect("plan");
    todos[1].action = RebaseAction::Fixup; // no message

    match start_interactive_rebase(d, &base, todos).expect("start") {
        // pick + fixup = 2 ops applied; topology collapses to one commit.
        RebaseOutcome::Rebased { steps, .. } => assert_eq!(steps, 2, "two ops applied"),
        other => panic!("expected Rebased, got {other:?}"),
    }

    assert_eq!(count_ahead(d, &base, "HEAD"), 1);
    assert_eq!(tree_of(d, "HEAD"), orig_tree, "same tree as squash");
    assert_eq!(msg_of(d, "HEAD"), "c1", "fixup keeps the predecessor's message");
    assert_eq!(author_of(d, "HEAD"), orig_c1_author, "fixup keeps the predecessor's author (N3)");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

// ============================================================ reword

#[test]
fn reword_changes_message_keeps_tree() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_two_disjoint(d);
    let base = rev(d, "main");
    let orig_tree = tree_of(d, "topic");
    let orig_c2_author = author_of(d, "topic"); // c2

    let mut todos = get_interactive_plan(d, &base).expect("plan");
    todos[1].action = RebaseAction::Reword;
    todos[1].new_message = Some("reworded c2".to_string());

    match start_interactive_rebase(d, &base, todos).expect("start") {
        RebaseOutcome::Rebased { steps, .. } => assert_eq!(steps, 2),
        other => panic!("expected Rebased, got {other:?}"),
    }

    assert_eq!(count_ahead(d, &base, "HEAD"), 2, "reword keeps both commits");
    assert_eq!(tree_of(d, "HEAD"), orig_tree, "reword leaves the tree unchanged");
    assert_eq!(msg_of(d, "HEAD"), "reworded c2");
    assert_eq!(msg_of(d, "HEAD~1"), "c1");
    assert_eq!(author_of(d, "HEAD"), orig_c2_author, "author preserved on reword");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

/// A reworded commit whose change is ALREADY on the new base becomes an empty
/// pick and is dropped (like `git rebase`). Because a Reword is a message-only
/// intent, dropping it would silently discard the user's new message — so the
/// engine surfaces a `warnings` note on the final Rebased outcome instead of
/// losing it quietly. (Fix 3: dropped-reword warning.)
#[test]
fn reword_dropped_when_empty_emits_warning() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    // topic adds feat.txt="x"; main independently adds an IDENTICAL feat.txt, so
    // replaying topic onto main is an empty pick (mirrors already_applied_pick).
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "feat.txt", "x\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "add feat");
    git(d, &["checkout", "main"]);
    write(d, "feat.txt", "x\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main adds feat too");
    git(d, &["checkout", "topic"]);

    let onto = rev(d, "main");
    let topic_tip = rev(d, "topic");
    // REWORD (not Pick): the empty-drop must not silently swallow the new message.
    let todos = vec![RebaseTodoOp {
        oid: topic_tip,
        action: RebaseAction::Reword,
        new_message: Some("reworded but doomed".to_string()),
    }];

    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Rebased { steps, warnings, .. } => {
            assert_eq!(steps, 0, "the empty pick produced no commit");
            assert_eq!(warnings.len(), 1, "exactly one dropped-reword warning");
            assert!(
                warnings[0].contains("reword") && warnings[0].contains("dropped"),
                "warning names the dropped reword, got: {}",
                warnings[0]
            );
        }
        other => panic!("expected Rebased, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

// ============================================================ drop

#[test]
fn drop_removes_the_middle_commit() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_three_disjoint(d);
    let base = rev(d, "main");

    let mut todos = get_interactive_plan(d, &base).expect("plan");
    todos[1].action = RebaseAction::Drop; // drop c2 (adds b.txt)

    match start_interactive_rebase(d, &base, todos).expect("start") {
        RebaseOutcome::Rebased { steps, .. } => assert_eq!(steps, 2),
        other => panic!("expected Rebased, got {other:?}"),
    }

    assert_eq!(count_ahead(d, &base, "HEAD"), 2, "one commit dropped");
    let files = tree_files(d, "HEAD");
    assert!(files.contains(&"a.txt".to_string()), "a.txt survives");
    assert!(files.contains(&"c.txt".to_string()), "c.txt survives");
    assert!(!files.contains(&"b.txt".to_string()), "dropped commit's file is gone");
    assert_eq!(msg_of(d, "HEAD~1"), "c1");
    assert_eq!(msg_of(d, "HEAD"), "c3");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

// ============================================================ empty-pick drop

#[test]
fn already_applied_pick_is_dropped() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    // topic adds feat.txt="x"; main independently adds an IDENTICAL feat.txt.
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "feat.txt", "x\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "add feat");
    git(d, &["checkout", "main"]);
    write(d, "feat.txt", "x\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main adds feat too");
    git(d, &["checkout", "topic"]);

    let onto = rev(d, "main");
    let topic_tip = rev(d, "topic");
    let onto_tree = tree_of(d, "main");
    let todos = vec![RebaseTodoOp {
        oid: topic_tip,
        action: RebaseAction::Pick,
        new_message: None,
    }];

    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Rebased { steps, .. } => assert_eq!(steps, 0, "empty pick dropped"),
        other => panic!("expected Rebased, got {other:?}"),
    }
    assert_eq!(count_ahead(d, &onto, "HEAD"), 0, "no commit replayed");
    assert_eq!(tree_of(d, "HEAD"), onto_tree, "HEAD tree == onto tree");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}
