//! P17 CLI-oracle partial-staging tests — composition and round-trips
//! (contract §6.2 scenarios 10–13 plus the cross-direction gap scenario):
//! stage-then-stage-the-rest, symmetric unstage, unborn HEAD, and the no-op
//! selection whose reconstruction equals the current index.
//!
//! Moved verbatim out of `stage_partial_cli.rs`; see that module for the
//! oracle rules and `stage_partial_helpers` for the shared helpers.

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::stage_partial_helpers::{
    all_changed, hunk_changed, numbered_edited, repo_with, staged_bytes, write, xy,
};
use bonsai_core::git::diff::{workdir_file_diff, LineKind};
use bonsai_core::git::stage_partial::{stage_partial, unstage_partial, LineSelection};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// Scenario (gap): cross-direction round-trip on the SAME line. Stage one added
// line with `stage_partial`, then unstage that exact line with
// `unstage_partial`; the index must return byte-exactly to HEAD and the file
// drops back to a pure workdir change. No existing test composes the two
// directions on the same coordinate.
#[test]
fn stage_then_unstage_same_line_round_trips() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nc\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"a\nb\nc\n"); // add "b" at new line 2

    // Stage exactly the added line.
    let add_b = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(2),
    }];
    stage_partial(p, "f.txt", None, &add_b).expect("stage the add");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nb\nc\n", "add staged");
    assert_eq!(
        xy(p, "f.txt").as_deref(),
        Some("M "),
        "fully staged, workdir clean vs index"
    );

    // Now unstage the SAME line from the staged (HEAD -> index) diff.
    let staged = workdir_file_diff(p, "f.txt", None, true, false, false).expect("staged diff");
    let staged_add = hunk_changed(&staged, 0);
    assert_eq!(staged_add.len(), 1, "one staged add to reverse");
    unstage_partial(p, "f.txt", None, &staged_add).expect("unstage the same add");
    assert_eq!(
        staged_bytes(p, "f.txt"),
        b"a\nc\n",
        "index restored byte-exactly to HEAD"
    );
    assert_eq!(
        xy(p, "f.txt").as_deref(),
        Some(" M"),
        "back to a pure workdir change"
    );
}

// Scenario 10: stage half, then the rest -> final index == whole-file stage.
#[test]
fn compose_on_partial() {
    require_git!();
    let dir = repo_with(20);
    let p = dir.path();
    let edited = numbered_edited(20, &[(3, "line 3 X"), (12, "line 12 X")]);
    write(p, "f.txt", &edited);

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    // Stage hunk 0 first.
    stage_partial(p, "f.txt", None, &hunk_changed(&fd, 0)).expect("stage hunk 0");
    assert_eq!(
        staged_bytes(p, "f.txt"),
        numbered_edited(20, &[(3, "line 3 X")]),
        "only first edit staged"
    );
    // Recompute against the CURRENT index and stage the remainder.
    let fd2 = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff 2");
    stage_partial(p, "f.txt", None, &all_changed(&fd2)).expect("stage remainder");
    assert_eq!(
        staged_bytes(p, "f.txt"),
        edited,
        "composed == whole-file stage"
    );
    assert_eq!(xy(p, "f.txt").as_deref(), Some("M "));
}

// Scenario 11: fully stage, then unstage_partial a subset -> exactly those
// lines revert toward HEAD, the rest stay staged.
#[test]
fn symmetric_unstage() {
    require_git!();
    let dir = repo_with(20);
    let p = dir.path();
    let edited = numbered_edited(20, &[(3, "line 3 X"), (12, "line 12 X")]);
    write(p, "f.txt", &edited);
    git(p, &["add", "-A"]); // fully staged

    let staged = workdir_file_diff(p, "f.txt", None, true, false, false).expect("staged diff");
    assert_eq!(staged.hunks.len(), 2);
    // Unstage only hunk 0 (line 3 reverts to original; line 12 stays changed).
    unstage_partial(p, "f.txt", None, &hunk_changed(&staged, 0)).expect("unstage hunk 0");
    assert_eq!(
        staged_bytes(p, "f.txt"),
        numbered_edited(20, &[(12, "line 12 X")]),
        "line 3 reverted, line 12 still staged"
    );
    assert_eq!(xy(p, "f.txt").as_deref(), Some("MM"));
}

// Scenario 12: unborn HEAD, staged file, unstage some added lines; then all.
#[test]
fn unborn_head_unstage() {
    require_git!();
    // partial unstage on unborn HEAD.
    {
        let dir = init_repo(); // no commit: unborn HEAD
        let p = dir.path();
        write(p, "f.txt", b"a\nb\nc\n");
        git(p, &["add", "-A"]); // staged, all Add vs empty tree

        let staged = workdir_file_diff(p, "f.txt", None, true, false, false).expect("staged diff");
        // Unstage just the middle added line (new_no 2).
        let sel = vec![LineSelection {
            kind: LineKind::Add,
            old_no: None,
            new_no: Some(2),
        }];
        unstage_partial(p, "f.txt", None, &sel).expect("unstage one add (unborn)");
        assert_eq!(staged_bytes(p, "f.txt"), b"a\nc\n");
        // Sanity: staged had 3 adds.
        assert_eq!(all_changed(&staged).len(), 3);
    }
    // unstage ALL -> index.remove_path.
    {
        let dir = init_repo();
        let p = dir.path();
        write(p, "f.txt", b"a\nb\nc\n");
        git(p, &["add", "-A"]);

        let staged = workdir_file_diff(p, "f.txt", None, true, false, false).expect("staged diff");
        unstage_partial(p, "f.txt", None, &all_changed(&staged)).expect("unstage all (unborn)");
        assert_eq!(xy(p, "f.txt").as_deref(), Some("??"), "back to untracked");
    }
}

// Scenario 13: a selection whose reconstruction equals the current index ->
// Ok, no index change, blob oid unchanged.
#[test]
fn noop_result_equals_index() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"a\nB\nc\n"); // modify line 2

    let before = staged_bytes(p, "f.txt"); // == HEAD content
                                           // Select ONLY the del half. Staging a del of "b" without the add: index
                                           // becomes "a\nc\n" -> that's a change, not a noop. Instead select nothing
                                           // meaningful: an Add coordinate is required, so use the del+add pair but
                                           // note the true noop is "select nothing" — exercise via a Context-only
                                           // selection which is ignored, leaving the index untouched.
    let sel = vec![LineSelection {
        kind: LineKind::Context,
        old_no: Some(1),
        new_no: Some(1),
    }];
    stage_partial(p, "f.txt", None, &sel).expect("noop stage");
    assert_eq!(staged_bytes(p, "f.txt"), before, "index blob unchanged");
    assert_eq!(
        xy(p, "f.txt").as_deref(),
        Some(" M"),
        "still only workdir change"
    );
}
