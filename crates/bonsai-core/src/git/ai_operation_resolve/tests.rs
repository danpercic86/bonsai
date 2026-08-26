//! P55 resolution/preview unit tests (§11.3–§11.8). These assert Rust's
//! resolution of a PARSED intent (the model's text transform is exercised
//! end-to-end in `tests/ai_operation_cli.rs`); the read-only guarantee for
//! ALL ten intents is proven by `plan_never_mutates` in `ai_operation`.
//!
//! Extracted verbatim from the former inline `mod tests`; shared fixtures live
//! in `test_support`.

use super::test_support::{
    expect_proposed, expect_unsupported, linear_repo, merge_repo, oid,
};
use super::*;

// ------------------------------------------------------- §11.3 undoLastCommit

/// §11.3: undoLastCommit targets HEAD's parent; Mixed (keep) vs Hard
/// (discard) by `keepChanges`; dropped = [HEAD].
#[test]
fn undo_last_commit_targets_head_parent() {
    let (dir, a, b) = linear_repo();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let short_b: String = b.chars().take(7).collect();

    // keepChanges=true → Mixed, Caution, no worktree warning.
    let op = expect_proposed(
        resolve_intent(&repo, AiOpIntent::UndoLastCommit { keep_changes: true }, None)
            .expect("Ok"),
    );
    match &op.op {
        SafeOp::Reset {
            target_oid, mode, ..
        } => {
            assert_eq!(target_oid, &a, "target = HEAD's parent (A)");
            assert_eq!(*mode, ResetMode::Mixed);
        }
        other => panic!("expected Reset, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Caution));
    assert!(op.preview.worktree_warning.is_none());
    assert_eq!(op.preview.dropped_commits.len(), 1, "dropped = [HEAD]");
    assert_eq!(op.preview.dropped_commits[0].short, short_b);

    // keepChanges=false → Hard, Destructive, worktree warning present.
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::UndoLastCommit { keep_changes: false },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::Reset { mode, .. } => assert_eq!(*mode, ResetMode::Hard),
        other => panic!("expected Reset, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Destructive));
    assert!(op.preview.worktree_warning.is_some());
}

// -------------------------------------------------------- §11.4 undoLastMerge

/// §11.4: undoLastMerge on a merge HEAD → Reset{first parent, Mixed},
/// Destructive, with the upstream shared-history warning when an upstream
/// exists; a non-merge HEAD → Unsupported.
#[test]
fn undo_last_merge_requires_merge_head() {
    let (dir, a, m, head_branch) = merge_repo();
    let p = dir.path();
    let repo = git2::Repository::open(p).expect("open");
    let short_m: String = m.chars().take(7).collect();

    let op =
        expect_proposed(resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok"));
    match &op.op {
        SafeOp::Reset {
            target_oid, mode, ..
        } => {
            assert_eq!(target_oid, &a, "target = merge's FIRST parent (A)");
            assert_eq!(*mode, ResetMode::Mixed);
        }
        other => panic!("expected Reset, got {other:?}"),
    }
    assert!(
        matches!(op.preview.danger, DangerLevel::Destructive),
        "undoLastMerge is always Destructive (OQ2)"
    );
    assert!(
        op.preview.dropped_commits.iter().any(|c| c.short == short_m),
        "the merge commit leaves the branch"
    );
    // No upstream yet → no shared-history warning.
    assert!(op.preview.worktree_warning.is_none());

    // Add an upstream → the shared-history warning appears.
    repo.remote("origin", "https://example.invalid/x.git").expect("remote");
    repo.reference(
        &format!("refs/remotes/origin/{head_branch}"),
        oid(&m),
        true,
        "seed upstream",
    )
    .expect("remote-tracking ref");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str(&format!("branch.{head_branch}.remote"), "origin")
            .expect("remote cfg");
        cfg.set_str(
            &format!("branch.{head_branch}.merge"),
            &format!("refs/heads/{head_branch}"),
        )
        .expect("merge cfg");
    }
    let op =
        expect_proposed(resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok"));
    let warn = op.preview.worktree_warning.expect("upstream warning present");
    assert!(warn.contains("rewrites history"), "got: {warn}");
    assert!(warn.contains(&format!("origin/{head_branch}")), "got: {warn}");

    // A non-merge HEAD → Unsupported.
    let (dir2, _a2, _b2) = linear_repo();
    let repo2 = git2::Repository::open(dir2.path()).expect("open");
    let reason =
        expect_unsupported(resolve_intent(&repo2, AiOpIntent::UndoLastMerge, None).expect("Ok"));
    assert!(reason.contains("isn't a merge"), "got: {reason}");
}

// ------------------------------------------------------ §11.5 resetToCommit

/// §11.5: resetToCommit resolves a SHORT hash from the state to a full oid;
/// a bad ref → Unsupported.
#[test]
fn reset_to_commit_resolves_short_hash() {
    let (dir, a, _b) = linear_repo();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let short_a: String = a.chars().take(7).collect();

    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::ResetToCommit {
                commit: short_a.clone(),
                keep_changes: true,
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::Reset {
            target_oid,
            target_short,
            mode,
        } => {
            assert_eq!(target_oid, &a, "short hash resolved to A's FULL oid");
            assert_eq!(target_short, &short_a);
            assert_eq!(*mode, ResetMode::Mixed);
        }
        other => panic!("expected Reset, got {other:?}"),
    }

    let bad = resolve_intent(
        &repo,
        AiOpIntent::ResetToCommit {
            commit: "deadbeefdeadbeef".to_string(),
            keep_changes: false,
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(bad).contains("couldn't find a commit"));
}

// -------------------------------------------------- §11.6 switch local/remote

/// §11.6: switchBranch — a LOCAL branch → `remote:false`; a name matching
/// ONLY a remote-tracking branch → `remote:true`; no match → Unsupported.
#[test]
fn switch_branch_local_vs_remote() {
    let (dir, _a, b) = linear_repo();
    let repo = git2::Repository::open(dir.path()).expect("open");

    // A second LOCAL branch (non-current) at HEAD.
    let head_c = repo.find_commit(oid(&b)).expect("B");
    repo.branch("other", &head_c, false).expect("local branch");
    // A remote-tracking ref with NO matching local branch.
    repo.reference("refs/remotes/origin/feature", oid(&b), true, "seed remote")
        .expect("remote-tracking ref");

    // Local → remote:false.
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::SwitchBranch {
                branch: "other".to_string(),
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::SwitchBranch { name, remote } => {
            assert_eq!(name, "other");
            assert!(!remote, "a local branch resolves to remote:false");
        }
        other => panic!("expected SwitchBranch, got {other:?}"),
    }

    // Only-remote match → remote:true.
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::SwitchBranch {
                branch: "origin/feature".to_string(),
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::SwitchBranch { name, remote } => {
            assert_eq!(name, "origin/feature");
            assert!(remote, "an only-remote match resolves to remote:true");
        }
        other => panic!("expected SwitchBranch, got {other:?}"),
    }

    // No match → Unsupported.
    let reason = expect_unsupported(
        resolve_intent(
            &repo,
            AiOpIntent::SwitchBranch {
                branch: "does-not-exist".to_string(),
            },
            None,
        )
        .expect("Ok"),
    );
    assert!(reason.contains("couldn't find a branch"), "got: {reason}");
}

// --------------------------------------- §11.7 discard → tracked-modified only

/// §11.7: discardChanges intersects with the tracked-modified (unstaged) set;
/// unknown/clean paths are dropped; none valid → Unsupported; a valid
/// tracked-modified path → `Discard` (Destructive, worktree warning).
#[test]
fn discard_filters_to_tracked_modified() {
    let (dir, _a, _b) = linear_repo();
    let p = dir.path();
    let repo = git2::Repository::open(p).expect("open");
    // a.txt tracked+committed → modify it unstaged so it is tracked-modified.
    std::fs::write(p.join("a.txt"), "changed\n").expect("edit a.txt");

    // Unknown + clean paths dropped; only a.txt kept.
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::DiscardChanges {
                paths: vec![
                    "a.txt".to_string(),
                    "b.txt".to_string(),        // tracked but clean → dropped
                    "no-such.txt".to_string(),  // unknown → dropped
                ],
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::Discard { paths } => assert_eq!(paths, &vec!["a.txt".to_string()]),
        other => panic!("expected Discard, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Destructive));
    assert!(op.preview.worktree_warning.is_some(), "discard warns");

    // None valid → Unsupported.
    let reason = expect_unsupported(
        resolve_intent(
            &repo,
            AiOpIntent::DiscardChanges {
                paths: vec!["b.txt".to_string(), "no-such.txt".to_string()],
            },
            None,
        )
        .expect("Ok"),
    );
    assert!(reason.contains("uncommitted changes to discard"), "got: {reason}");
}

// ------------------------------ §11.8 op-in-progress blocks ALL mutating intents

/// §11.8: with a mid-flight op (a written MERGE_HEAD ⇒ `repo.state()` ==
/// Merge), EACH of the ten mutating intents resolves to Unsupported (the
/// global precondition, §4). The `unsupported` escape hatch is not a
/// mutating intent and is excluded.
#[test]
fn op_in_progress_blocks_all_mutating_intents() {
    let (dir, a, _b) = linear_repo();
    let p = dir.path();
    // Force a Merge state: libgit2 derives RepositoryState::Merge from the
    // presence of .git/MERGE_HEAD (no real conflict needed).
    {
        let repo = git2::Repository::open(p).expect("open");
        std::fs::write(repo.path().join("MERGE_HEAD"), format!("{a}\n"))
            .expect("write MERGE_HEAD");
    }
    let repo = git2::Repository::open(p).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Merge,
        "MERGE_HEAD must put the repo in Merge state"
    );

    let short_a: String = a.chars().take(7).collect();
    let intents = vec![
        AiOpIntent::UndoLastCommit { keep_changes: true },
        AiOpIntent::UndoLastMerge,
        AiOpIntent::ResetToCommit {
            commit: short_a,
            keep_changes: true,
        },
        AiOpIntent::RevertCommit {
            commit: a.clone(),
        },
        AiOpIntent::SwitchBranch {
            branch: "whatever".to_string(),
        },
        AiOpIntent::CreateBranch {
            name: "new-branch".to_string(),
            at_commit: None,
        },
        AiOpIntent::DeleteBranch {
            branch: "whatever".to_string(),
        },
        AiOpIntent::StashChanges {
            message: None,
            include_untracked: true,
        },
        AiOpIntent::DiscardChanges {
            paths: vec!["a.txt".to_string()],
        },
        AiOpIntent::MergeBranch {
            branch: "whatever".to_string(),
        },
    ];
    assert_eq!(intents.len(), 10, "all ten mutating intents are enumerated");

    for intent in intents {
        let label = format!("{intent:?}");
        let reason = expect_unsupported(
            resolve_intent(&repo, intent, None).expect("Ok(Unsupported)"),
        );
        assert!(
            reason.contains("in-progress"),
            "{label} must be blocked by the op-in-progress guard, got: {reason}"
        );
    }
}
