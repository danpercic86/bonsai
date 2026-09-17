//! P17 CLI-oracle partial-staging tests — file-state edges (contract §6.2
//! scenarios 8, 9 and SF-1): untracked files, fs-deleted files, a tracked
//! file emptied to zero bytes, and a committed empty file.
//!
//! Moved verbatim out of `stage_partial_cli.rs`; see that module for the
//! oracle rules and `stage_partial_helpers` for the shared helpers.

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::stage_partial_helpers::{all_changed, repo_with, staged_bytes, write, xy};
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

// Scenario 8: untracked partial (index gains a partial blob) + full.
#[test]
fn untracked_partial_and_full() {
    require_git!();
    // partial
    {
        let dir = repo_with(3);
        let p = dir.path();
        write(p, "u.txt", b"x\ny\nz\n"); // untracked
        let fd = workdir_file_diff(p, "u.txt", None, false, false, false).expect("diff");
        assert_eq!(fd.hunks.len(), 1);
        // Stage only the first added line.
        let sel = vec![LineSelection {
            kind: LineKind::Add,
            old_no: None,
            new_no: Some(1),
        }];
        stage_partial(p, "u.txt", None, &sel).expect("stage partial untracked");
        assert_eq!(staged_bytes(p, "u.txt"), b"x\n");
        assert_eq!(
            xy(p, "u.txt").as_deref(),
            Some("AM"),
            "added + still-modified"
        );
    }
    // full
    {
        let dir = repo_with(3);
        let p = dir.path();
        write(p, "u.txt", b"x\ny\nz\n");
        let fd = workdir_file_diff(p, "u.txt", None, false, false, false).expect("diff");
        stage_partial(p, "u.txt", None, &all_changed(&fd)).expect("stage whole untracked");
        assert_eq!(staged_bytes(p, "u.txt"), b"x\ny\nz\n");
        assert_eq!(xy(p, "u.txt").as_deref(), Some("A "));
    }
}

// Scenario 9: deleted-file partial (index keeps unselected lines) + full
// (index.remove_path -> staged deletion).
#[test]
fn deleted_partial_and_full() {
    require_git!();
    // partial: stage some del lines only.
    {
        let dir = init_repo();
        let p = dir.path();
        write(p, "f.txt", b"a\nb\nc\n");
        git(p, &["add", "-A"]);
        commit_fixed(p, "base");
        std::fs::remove_file(p.join("f.txt")).expect("delete f.txt");

        let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
        assert_eq!(fd.status, bonsai_core::git::status::FileStatus::Deleted);
        // Stage the deletion of "a" and "c" only; "b" stays in the index.
        let sel = vec![
            LineSelection {
                kind: LineKind::Del,
                old_no: Some(1),
                new_no: None,
            },
            LineSelection {
                kind: LineKind::Del,
                old_no: Some(3),
                new_no: None,
            },
        ];
        stage_partial(p, "f.txt", None, &sel).expect("stage partial deletion");
        assert_eq!(staged_bytes(p, "f.txt"), b"b\n");
    }
    // full: stage ALL del lines -> index.remove_path.
    {
        let dir = init_repo();
        let p = dir.path();
        write(p, "f.txt", b"a\nb\nc\n");
        git(p, &["add", "-A"]);
        commit_fixed(p, "base");
        std::fs::remove_file(p.join("f.txt")).expect("delete f.txt");

        let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
        stage_partial(p, "f.txt", None, &all_changed(&fd)).expect("stage full deletion");
        assert_eq!(xy(p, "f.txt").as_deref(), Some("D "), "staged deletion");
    }
}

// SF-1 (a): emptying a TRACKED file to zero bytes and staging all deletions
// stages an EMPTY BLOB (status `M`), NOT a deletion (`D`). `git add` of an
// emptied-but-present file yields `M `, and the removal discriminator must key
// on presence (status Deleted), not byte-emptiness.
#[test]
fn stage_emptied_tracked_file_is_modified_not_deleted() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b""); // truncate to zero bytes; file still exists

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(
        fd.status,
        bonsai_core::git::status::FileStatus::Modified,
        "emptied-but-present file is Modified, not Deleted"
    );
    stage_partial(p, "f.txt", None, &all_changed(&fd)).expect("stage all dels");
    assert!(staged_bytes(p, "f.txt").is_empty(), "staged an empty blob");
    assert_eq!(xy(p, "f.txt").as_deref(), Some("M "), "staged M, not D");
}

// SF-1 (b): fully unstaging a change to a COMMITTED EMPTY file restores the
// empty blob (present in HEAD), NOT a staged deletion.
#[test]
fn unstage_committed_empty_file_restores_empty_blob() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "e.txt", b""); // commit an empty file
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "e.txt", b"x\ny\n"); // add content
    git(p, &["add", "-A"]); // staged

    let staged = workdir_file_diff(p, "e.txt", None, true, false, false).expect("staged diff");
    unstage_partial(p, "e.txt", None, &all_changed(&staged)).expect("unstage all");
    // Index restored to HEAD's empty blob -> no staged deletion; workdir still
    // has content -> unstaged M.
    assert!(
        staged_bytes(p, "e.txt").is_empty(),
        "index restored to empty blob"
    );
    assert_eq!(xy(p, "e.txt").as_deref(), Some(" M"), "no staged deletion");
}
