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
//! All scratch repos live under `D:\Temp\bonsai-scratch`. Each test skips
//! (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::conflict::resolve_conflict_text;
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::rebase::{rebase_abort, rebase_continue, rebase_skip, RebaseOutcome};
use bonsai_core::git::rebase_interactive::{
    get_interactive_plan, start_interactive_rebase, RebaseAction, RebaseTodoOp,
};
use common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ------------------------------------------------------------ small helpers

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn read_str(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read fixture file")
}

fn rev(dir: &Path, r: &str) -> String {
    git(dir, &["rev-parse", r])
}

fn tree_of(dir: &Path, r: &str) -> String {
    git(dir, &["rev-parse", &format!("{r}^{{tree}}")])
}

fn msg_of(dir: &Path, r: &str) -> String {
    git(dir, &["log", "-1", "--format=%B", r]).trim().to_string()
}

fn author_of(dir: &Path, r: &str) -> String {
    git(dir, &["log", "-1", "--format=%an <%ae> %at", r])
}

fn count_ahead(dir: &Path, base: &str, r: &str) -> usize {
    git(dir, &["rev-list", "--count", &format!("{base}..{r}")])
        .parse()
        .expect("count parse")
}

fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

fn has_bonsai_dir(dir: &Path) -> bool {
    dir.join(".git").join("bonsai-rebase").join("state.json").exists()
}

fn symbolic_head(dir: &Path) -> String {
    git(dir, &["symbolic-ref", "HEAD"])
}

fn tree_files(dir: &Path, r: &str) -> Vec<String> {
    let mut v: Vec<String> = git(dir, &["ls-tree", "-r", "--name-only", r])
        .lines()
        .map(String::from)
        .collect();
    v.sort();
    v
}

// ------------------------------------------------------------ fixtures

/// 3 linear topic commits touching disjoint files a/b/c on top of `base`.
fn script_three_disjoint(d: &Path) {
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c2");
    write(d, "c.txt", "c\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c3");
}

/// 2 linear topic commits touching disjoint files a/b on top of `base`.
fn script_two_disjoint(d: &Path) {
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c2");
}

/// topic edits a.txt one way; main edits the same line differently -> a pick of
/// topic onto main conflicts on a.txt. Ends checked out on `topic`.
fn script_conflict(d: &Path) {
    write(d, "a.txt", "line1\nbase\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
    git(d, &["checkout", "topic"]);
}

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
        RebaseOutcome::Rebased { branch, head, steps } => {
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

// ============================================================ conflict -> continue

#[test]
fn conflict_pauses_then_continue_completes() {
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

    let (paths, cur, total) = match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { paths, current_step, total_steps } => {
            (paths, current_step, total_steps)
        }
        other => panic!("expected Conflicts, got {other:?}"),
    };
    assert_eq!(paths, vec!["a.txt".to_string()]);
    assert_eq!(cur, 1);
    assert_eq!(total, 1);

    // The Bonsai sequencer exists and is paused.
    assert!(has_bonsai_dir(d), ".git/bonsai-rebase/state.json must exist");

    // opstate probe reports Rebase (NOT CherryPick), from the Bonsai file (§4).
    match read_op_state(d).expect("op state") {
        RepoOpState::Rebase { head_name, onto: onto_field, current_step, total_steps } => {
            assert_eq!(head_name, Some("topic".to_string()));
            assert_eq!(onto_field, Some(onto.clone()));
            assert_eq!(current_step, 1);
            assert_eq!(total_steps, 1);
        }
        other => panic!("expected Rebase op state, got {other:?}"),
    }

    // Worktree carries real conflict markers.
    let text = read_str(d, "a.txt");
    assert!(text.contains("<<<<<<<") && text.contains("=======") && text.contains(">>>>>>>"),
        "expected conflict markers, got: {text}");

    // Continue while conflicts remain -> UnresolvedConflicts.
    assert!(matches!(
        rebase_continue(d).expect_err("still conflicted"),
        AppError::UnresolvedConflicts(_)
    ));

    // Resolve by hand + continue -> completes.
    resolve_conflict_text(d, "a.txt", "line1\nresolved\nline3\n").expect("resolve");
    match rebase_continue(d).expect("continue") {
        RebaseOutcome::Rebased { branch, head, steps } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, 1);
            assert_eq!(head, rev(d, "HEAD"));
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    assert_eq!(read_str(d, "a.txt"), "line1\nresolved\nline3\n", "resolved content committed");
    assert_eq!(rev(d, "HEAD~1"), onto, "replayed commit sits on the onto tip");
    assert_eq!(count_ahead(d, &onto, "HEAD"), 1);
    assert_eq!(repo_state(d), git2::RepositoryState::Clean, "state Clean after finish");
    assert!(!has_bonsai_dir(d), "sequencer removed on finish");
    assert_eq!(symbolic_head(d), "refs/heads/topic");
}

// ============================================================ skip

#[test]
fn skip_drops_the_conflicting_op_and_completes() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    // topic = [t_a edits a.txt (conflicts with main), t_other edits other.txt (clean)].
    write(d, "a.txt", "line1\nbase\nline3\n");
    write(d, "other.txt", "other base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic a");
    write(d, "other.txt", "other topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic other");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main a");
    git(d, &["checkout", "topic"]);

    let onto = rev(d, "main");
    let topic_a = rev(d, "topic~1");
    let topic_other = rev(d, "topic");
    let todos = vec![
        RebaseTodoOp { oid: topic_a, action: RebaseAction::Pick, new_message: None },
        RebaseTodoOp { oid: topic_other, action: RebaseAction::Pick, new_message: None },
    ];

    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { paths, current_step, .. } => {
            assert_eq!(paths, vec!["a.txt".to_string()]);
            assert_eq!(current_step, 1, "conflict on the first op");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }

    match rebase_skip(d).expect("skip") {
        RebaseOutcome::Rebased { branch, steps, .. } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, 1, "only the clean op committed");
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    // The skipped op is absent: a.txt stays at onto's content; other.txt applied.
    assert_eq!(read_str(d, "a.txt"), "line1\nmain\nline3\n", "skipped op dropped");
    assert_eq!(read_str(d, "other.txt"), "other topic\n", "clean op applied");
    assert_eq!(count_ahead(d, &onto, "HEAD"), 1);
    assert_eq!(rev(d, "HEAD~1"), onto);
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

// ============================================================ abort

#[test]
fn abort_restores_the_original_branch_tip() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_conflict(d);
    let onto = rev(d, "main");
    let topic_tip = rev(d, "topic");
    let orig_a = read_str(d, "a.txt");

    let todos = vec![RebaseTodoOp {
        oid: topic_tip.clone(),
        action: RebaseAction::Pick,
        new_message: None,
    }];
    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }

    rebase_abort(d).expect("abort");

    assert_eq!(symbolic_head(d), "refs/heads/topic", "HEAD re-attached to topic");
    assert_eq!(rev(d, "topic"), topic_tip, "branch tip byte-identical to pre-rebase");
    assert_eq!(rev(d, "HEAD"), topic_tip);
    assert_eq!(read_str(d, "a.txt"), orig_a, "worktree restored to the original tip");
    assert!(git(d, &["ls-files", "-u"]).is_empty(), "no conflict stages remain");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d), "sequencer removed on abort");

    // Abort again with nothing in progress -> NoOperationInProgress.
    assert!(matches!(
        rebase_abort(d).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
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
    assert!(!has_bonsai_dir(d), "no Bonsai sequencer yet — the git-native one is separate");

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
    assert!(!has_bonsai_dir(d), "rejected start leaves no .git/bonsai-rebase");

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
        AppError::Git(m) => assert!(m.contains("unstaged") || m.contains("uncommitted"), "got: {m}"),
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
    assert!(!has_bonsai_dir(d), "no sequencer left behind by a rejected plan");
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
    assert_eq!(repo_state(d), git2::RepositoryState::Clean, "state stays Clean");
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

// ============================================================ M1 — out-of-range cursor

/// Overwrites one top-level integer field of `.git/bonsai-rebase/state.json`.
fn patch_state_usize(dir: &Path, key: &str, value: usize) {
    let path = dir.join(".git").join("bonsai-rebase").join("state.json");
    let raw = std::fs::read_to_string(&path).expect("read state.json");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("parse state.json");
    v[key] = serde_json::json!(value);
    std::fs::write(&path, serde_json::to_string_pretty(&v).expect("serialize")).expect("write");
}

/// M1: `interactive_continue` with `cursor == todos.len()` (a partial finish or
/// a hand-edited state) must NOT panic on `state.todos[cursor]` — it finishes.
#[test]
fn continue_with_out_of_range_cursor_does_not_panic() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    script_conflict(d);
    let onto = rev(d, "main");
    let topic_tip = rev(d, "topic");

    let todos = vec![RebaseTodoOp {
        oid: topic_tip,
        action: RebaseAction::Pick,
        new_message: None,
    }];
    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }

    // Resolve, then corrupt the cursor to len (1) so no paused op exists.
    resolve_conflict_text(d, "a.txt", "line1\nresolved\nline3\n").expect("resolve");
    patch_state_usize(d, "cursor", 1);

    // Must finish gracefully rather than index out of bounds.
    match rebase_continue(d).expect("continue must not panic") {
        RebaseOutcome::Rebased { .. } => {}
        other => panic!("expected Rebased, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d), "sequencer removed after the graceful finish");
}

// ============================================================ M2 — abort after N commits

/// M2: after several clean commits AND a simulated partial finish that already
/// moved the branch ref, abort must FORCE the branch ref back to the exact
/// original tip (not merely re-attach HEAD).
#[test]
fn abort_after_commits_and_partial_finish_restores_exact_tip() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    // topic = [p (clean), q (clean), c (conflicts with main on a.txt)].
    write(d, "a.txt", "line1\nbase\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "p.txt", "p\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "p clean");
    write(d, "q.txt", "q\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "q clean");
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c conflict");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main a");
    git(d, &["checkout", "topic"]);

    let onto = rev(d, "main");
    let original_tip = rev(d, "topic");
    let orig_a = read_str(d, "a.txt");
    let p = rev(d, "topic~2");
    let q = rev(d, "topic~1");
    let c = rev(d, "topic");
    let todos = vec![
        RebaseTodoOp { oid: p, action: RebaseAction::Pick, new_message: None },
        RebaseTodoOp { oid: q, action: RebaseAction::Pick, new_message: None },
        RebaseTodoOp { oid: c, action: RebaseAction::Pick, new_message: None },
    ];

    // Two clean commits, then a conflict on the third (committed == 2).
    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { current_step, .. } => {
            assert_eq!(current_step, 3, "paused on the third op after two clean commits");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    // The branch ref itself has NOT moved yet (only finish moves it).
    assert_eq!(rev(d, "topic"), original_tip);

    // Simulate a PARTIAL finish that already advanced the branch ref to the
    // rewritten (detached) tip — the exact hazard M2 describes.
    let rewritten = rev(d, "HEAD");
    assert_ne!(rewritten, original_tip);
    git(d, &["update-ref", "refs/heads/topic", &rewritten]);
    assert_eq!(rev(d, "topic"), rewritten, "ref moved by the simulated partial finish");

    // Abort must force the branch ref back to the exact original tip.
    rebase_abort(d).expect("abort");
    assert_eq!(rev(d, "topic"), original_tip, "abort force-resets the branch ref (M2)");
    assert_eq!(symbolic_head(d), "refs/heads/topic", "HEAD re-attached");
    assert_eq!(rev(d, "HEAD"), original_tip);
    assert_eq!(read_str(d, "a.txt"), orig_a, "worktree restored");
    assert!(git(d, &["ls-files", "-u"]).is_empty(), "no conflict stages remain");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}

// ============================================================ S1 — skip -> squash first

/// S1: skipping the first kept op leaves a squash as the first APPLIED op; the
/// engine must refuse (not reparent onto the base's parent) and leave the branch
/// tip unchanged.
#[test]
fn skip_making_squash_first_applied_is_refused() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    // topic = [A edits a.txt (conflicts with main), B adds b.txt]. Plan pick A,
    // squash B; A conflicts, skip A -> B (squash) becomes first-applied.
    write(d, "a.txt", "line1\nbase\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "A");
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "B");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main a");
    git(d, &["checkout", "topic"]);

    let onto = rev(d, "main");
    let original_tip = rev(d, "topic");
    let a = rev(d, "topic~1");
    let b = rev(d, "topic");
    let todos = vec![
        RebaseTodoOp { oid: a, action: RebaseAction::Pick, new_message: None },
        RebaseTodoOp {
            oid: b,
            action: RebaseAction::Squash,
            new_message: Some("squashed".to_string()),
        },
    ];

    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { current_step, .. } => assert_eq!(current_step, 1),
        other => panic!("expected Conflicts, got {other:?}"),
    }

    // Skip A -> the squash B would become the first applied op -> refuse.
    match rebase_skip(d).expect_err("squash-first must be refused") {
        AppError::Git(m) => assert!(m.contains("no preceding commit"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    // No corruption: the branch tip is unchanged (the ref never moved).
    assert_eq!(rev(d, "topic"), original_tip, "branch tip must be unchanged");

    // The engine is still recoverable via abort.
    rebase_abort(d).expect("abort");
    assert_eq!(rev(d, "topic"), original_tip);
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}
