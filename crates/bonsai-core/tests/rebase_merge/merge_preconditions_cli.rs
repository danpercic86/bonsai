//! P3c CLI-oracle merge tests — the precondition matrix (including the P8
//! autostash paths) and the `create_commit` gate (contract §9.6, §9.9).
//!
//! Split out of `merge_cli.rs`; the twin-repo scaffolding, helpers, and
//! fixtures live in `merge_support.rs`.

use std::process::Command;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::conflict::{resolve_conflict, ConflictResolution};
use bonsai_core::git::merge::{merge_branch, MergeOutcome};
use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::merge_support::{
    head_oid, parents, repo_state, require_git, script_clean_diverged, script_conflict,
    stash_count, tree_oid, twin_pair, write,
};

// ============================================================ §9.6 preconditions

#[test]
fn detached_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["branch", "topic"]);
    git(d, &["checkout", "--detach"]);

    let err = merge_branch(d, "topic", false).expect_err("detached");
    match err {
        AppError::Git(m) => assert!(m.contains("detached"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn unborn_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    let err = merge_branch(repo.path(), "topic", false).expect_err("unborn");
    match err {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

/// P8 §2.1 removed the pre-P8 dirty-INDEX refusal: a STAGED change to a file
/// the merge does not touch is now AUTOSTASHED, the (non-FF, clean) merge
/// proceeds and auto-commits, then the stash is re-applied. Matrix row #3
/// (dirty + clean normal merge + clean pop) -> `Merged { stashed: true }`.
/// Per OPEN Q#1 (no REINSTATE_INDEX) the change comes back UNSTAGED.
#[test]
fn staged_change_is_autostashed_and_merge_proceeds() {
    require_git!();
    // script_clean_diverged: topic edits b.txt, main edits a.txt -> non-FF
    // clean merge on disjoint files. The staged edit below is to a.txt, which
    // the merge leaves at main's version, so the pop applies cleanly.
    let (bonsai, _twin) = twin_pair(script_clean_diverged);
    let d = bonsai.path();
    write(d, "a.txt", "staged edit\n");
    git(d, &["add", "a.txt"]);

    let outcome = merge_branch(d, "topic", false).expect("merge");
    let oid = match &outcome {
        MergeOutcome::Merged { oid, stashed } => {
            assert!(*stashed, "staged change must be autostashed -> stashed:true");
            oid.clone()
        }
        other => panic!("expected Merged{{stashed:true}}, got {other:?}"),
    };

    // A real 2-parent merge commit landed.
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(oid, head_oid(d), "returned oid must be HEAD");
    assert_eq!(parents(d).len(), 2, "normal merge -> 2-parent commit");
    // Disjoint clean merge kept both sides.
    assert_eq!(std::fs::read_to_string(d.join("b.txt")).expect("b"), "b topic\n");

    // The staged change's CONTENT survives...
    assert_eq!(
        std::fs::read_to_string(d.join("a.txt")).expect("a"),
        "staged edit\n",
        "the autostashed change content must be restored"
    );
    // ...and returns UNSTAGED (OPEN Q#1: no REINSTATE_INDEX). Nothing staged;
    // a.txt shows only as a worktree modification.
    assert!(
        git(d, &["diff", "--cached", "--name-only"]).trim().is_empty(),
        "OPEN Q#1: the restored change must NOT be re-staged"
    );
    assert_eq!(
        git(d, &["diff", "--name-only"]).trim(),
        "a.txt",
        "the restored change must be an UNSTAGED worktree modification"
    );
    assert_eq!(stash_count(d), 0, "clean pop -> stash applied and dropped");
}

#[test]
fn merge_during_merge_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict);
    let d = bonsai.path();
    git(d, &["branch", "other"]); // second candidate branch
    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }

    let err = merge_branch(d, "other", false).expect_err("nested merge");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
}

#[test]
fn unknown_branch_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = merge_branch(d, "no-such-branch", false).expect_err("unknown");
    assert!(
        matches!(err, AppError::BranchNotFound(_)),
        "expected BranchNotFound, got {err:?}"
    );
}

/// P8: an UNSTAGED edit to a merge-touched file is no longer a pre-flight
/// CheckoutConflict. The edit is autostashed, the (non-FF, clean) merge runs
/// and auto-commits, then re-applying the stash onto the merged tree conflicts
/// on that same file. Matrix row #4 -> `StashPopConflicts { head, paths }`:
/// state Clean (a conflicted stash-apply is not a merge op), worktree has
/// markers, the stash is RETAINED. Pinned against real `git merge --autostash`.
#[test]
fn unstaged_edit_to_merge_touched_file_autostashes_then_pop_conflicts() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_clean_diverged);
    let d = bonsai.path();
    // topic changes b.txt; make an UNSTAGED local edit to the SAME file.
    let local = "b local unstaged\n";
    write(d, "b.txt", local);

    let (head, paths) = match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::StashPopConflicts { head, paths } => (head, paths),
        other => panic!("expected StashPopConflicts, got {other:?}"),
    };
    assert_eq!(paths, vec!["b.txt".to_string()], "b.txt conflicted on the pop");
    assert_eq!(head, head_oid(d), "head = the new merge-commit oid");

    // A conflicted stash-apply is NOT a merge op: state stays Clean.
    assert_eq!(repo_state(d), git2::RepositoryState::Clean, "state must be Clean");
    assert!(!d.join(".git").join("MERGE_HEAD").exists(), "no MERGE_HEAD");
    assert_eq!(parents(d).len(), 2, "the merge itself committed (2 parents)");
    // a.txt is main's side (merge untouched); b.txt has conflict markers.
    assert_eq!(std::fs::read_to_string(d.join("a.txt")).expect("a"), "a main\n");
    let b = std::fs::read_to_string(d.join("b.txt")).expect("b");
    assert!(
        b.contains("<<<<<<<") && b.contains(">>>>>>>"),
        "b.txt must carry conflict markers, got:\n{b}"
    );
    assert_eq!(stash_count(d), 1, "conflicting pop RETAINS the stash");

    // Oracle: real `git merge --autostash topic` on the twin with the same
    // unstaged edit. The COMMITTED merge tree is stable regardless of the
    // post-commit pop, so compare HEAD trees; also confirm git likewise keeps
    // a stash and leaves markers. (Commit oids differ: timestamps differ.)
    write(twin.path(), "b.txt", local);
    let _ = Command::new("git")
        .args(["merge", "--autostash", "--no-edit", "topic"])
        .current_dir(twin.path())
        .output()
        .expect("run git merge --autostash");
    assert_eq!(
        tree_oid(d),
        tree_oid(twin.path()),
        "our committed merge tree must equal `git merge --autostash`'s"
    );
    assert_eq!(
        stash_count(twin.path()),
        1,
        "real git also RETAINS the autostash on a conflicting re-apply"
    );
    let twin_b = std::fs::read_to_string(twin.path().join("b.txt")).expect("twin b");
    assert!(
        twin_b.contains("<<<<<<<") && twin_b.contains(">>>>>>>"),
        "real git also leaves conflict markers in b.txt"
    );
}

// ============================================================ §9.9 create_commit gate

#[test]
fn plain_commit_during_paused_merge_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict);
    let d = bonsai.path();
    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    resolve_conflict(d, "a.txt", ConflictResolution::Ours).expect("resolve");

    let err = create_commit(d, "sneaky plain commit", None, false).expect_err("gated");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
    // Merge state untouched by the refused commit.
    assert_eq!(repo_state(d), git2::RepositoryState::Merge);
}
