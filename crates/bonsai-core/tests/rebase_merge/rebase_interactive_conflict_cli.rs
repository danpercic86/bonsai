//! P23a CLI-oracle interactive-rebase tests — conflict pause / continue / skip /
//! abort flows and the sequencer-state edge cases (contract §13.1).
//!
//! Split out of `rebase_interactive_cli.rs`; shared fixtures and helpers live in
//! `rebase_interactive_support.rs`. Same locked comparison rule (§13): committer
//! time = now(), so REPLAYED commit oids differ from any twin — we compare TREE
//! oids, author identity, messages, and parent topology, never replayed oids.

use std::path::Path;

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::rebase_interactive_support::{
    count_ahead, has_bonsai_dir, read_str, repo_state, require_git, rev, script_conflict,
    symbolic_head, write,
};
use bonsai_core::error::AppError;
use bonsai_core::git::conflict::resolve_conflict_text;
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::rebase::{rebase_abort, rebase_continue, rebase_skip, RebaseOutcome};
use bonsai_core::git::rebase_interactive::{start_interactive_rebase, RebaseAction, RebaseTodoOp};

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
        RebaseOutcome::Conflicts {
            paths,
            current_step,
            total_steps,
        } => (paths, current_step, total_steps),
        other => panic!("expected Conflicts, got {other:?}"),
    };
    assert_eq!(paths, vec!["a.txt".to_string()]);
    assert_eq!(cur, 1);
    assert_eq!(total, 1);

    // The Bonsai sequencer exists and is paused.
    assert!(
        has_bonsai_dir(d),
        ".git/bonsai-rebase/state.json must exist"
    );

    // opstate probe reports Rebase (NOT CherryPick), from the Bonsai file (§4).
    match read_op_state(d).expect("op state") {
        RepoOpState::Rebase {
            head_name,
            onto: onto_field,
            current_step,
            total_steps,
        } => {
            assert_eq!(head_name, Some("topic".to_string()));
            assert_eq!(onto_field, Some(onto.clone()));
            assert_eq!(current_step, 1);
            assert_eq!(total_steps, 1);
        }
        other => panic!("expected Rebase op state, got {other:?}"),
    }

    // Worktree carries real conflict markers.
    let text = read_str(d, "a.txt");
    assert!(
        text.contains("<<<<<<<") && text.contains("=======") && text.contains(">>>>>>>"),
        "expected conflict markers, got: {text}"
    );

    // Continue while conflicts remain -> UnresolvedConflicts.
    assert!(matches!(
        rebase_continue(d).expect_err("still conflicted"),
        AppError::UnresolvedConflicts(_)
    ));

    // Resolve by hand + continue -> completes.
    resolve_conflict_text(d, "a.txt", "line1\nresolved\nline3\n").expect("resolve");
    match rebase_continue(d).expect("continue") {
        RebaseOutcome::Rebased {
            branch,
            head,
            steps,
            ..
        } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, 1);
            assert_eq!(head, rev(d, "HEAD"));
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    assert_eq!(
        read_str(d, "a.txt"),
        "line1\nresolved\nline3\n",
        "resolved content committed"
    );
    assert_eq!(
        rev(d, "HEAD~1"),
        onto,
        "replayed commit sits on the onto tip"
    );
    assert_eq!(count_ahead(d, &onto, "HEAD"), 1);
    assert_eq!(
        repo_state(d),
        git2::RepositoryState::Clean,
        "state Clean after finish"
    );
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
        RebaseTodoOp {
            oid: topic_a,
            action: RebaseAction::Pick,
            new_message: None,
        },
        RebaseTodoOp {
            oid: topic_other,
            action: RebaseAction::Pick,
            new_message: None,
        },
    ];

    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts {
            paths,
            current_step,
            ..
        } => {
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
    assert_eq!(
        read_str(d, "a.txt"),
        "line1\nmain\nline3\n",
        "skipped op dropped"
    );
    assert_eq!(
        read_str(d, "other.txt"),
        "other topic\n",
        "clean op applied"
    );
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

    assert_eq!(
        symbolic_head(d),
        "refs/heads/topic",
        "HEAD re-attached to topic"
    );
    assert_eq!(
        rev(d, "topic"),
        topic_tip,
        "branch tip byte-identical to pre-rebase"
    );
    assert_eq!(rev(d, "HEAD"), topic_tip);
    assert_eq!(
        read_str(d, "a.txt"),
        orig_a,
        "worktree restored to the original tip"
    );
    assert!(
        git(d, &["ls-files", "-u"]).is_empty(),
        "no conflict stages remain"
    );
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d), "sequencer removed on abort");

    // Abort again with nothing in progress -> NoOperationInProgress.
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
    assert!(
        !has_bonsai_dir(d),
        "sequencer removed after the graceful finish"
    );
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
        RebaseTodoOp {
            oid: p,
            action: RebaseAction::Pick,
            new_message: None,
        },
        RebaseTodoOp {
            oid: q,
            action: RebaseAction::Pick,
            new_message: None,
        },
        RebaseTodoOp {
            oid: c,
            action: RebaseAction::Pick,
            new_message: None,
        },
    ];

    // Two clean commits, then a conflict on the third (committed == 2).
    match start_interactive_rebase(d, &onto, todos).expect("start") {
        RebaseOutcome::Conflicts { current_step, .. } => {
            assert_eq!(
                current_step, 3,
                "paused on the third op after two clean commits"
            );
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
    assert_eq!(
        rev(d, "topic"),
        rewritten,
        "ref moved by the simulated partial finish"
    );

    // Abort must force the branch ref back to the exact original tip.
    rebase_abort(d).expect("abort");
    assert_eq!(
        rev(d, "topic"),
        original_tip,
        "abort force-resets the branch ref (M2)"
    );
    assert_eq!(symbolic_head(d), "refs/heads/topic", "HEAD re-attached");
    assert_eq!(rev(d, "HEAD"), original_tip);
    assert_eq!(read_str(d, "a.txt"), orig_a, "worktree restored");
    assert!(
        git(d, &["ls-files", "-u"]).is_empty(),
        "no conflict stages remain"
    );
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
        RebaseTodoOp {
            oid: a,
            action: RebaseAction::Pick,
            new_message: None,
        },
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
    assert_eq!(
        rev(d, "topic"),
        original_tip,
        "branch tip must be unchanged"
    );

    // The engine is still recoverable via abort.
    rebase_abort(d).expect("abort");
    assert_eq!(rev(d, "topic"), original_tip);
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert!(!has_bonsai_dir(d));
}
