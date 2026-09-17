//! P17 CLI-oracle partial-staging tests (contract §6.1–§6.2).
//!
//! The load-bearing oracle is byte-exactness of the reconstructed index blob:
//! after `stage_partial`/`unstage_partial` we read the staged content via
//! `git show :path` (raw bytes, `core.autocrlf=false`) and assert it equals an
//! independently constructed expectation. Because the workdir is never touched
//! by staging, a byte-exact index fully determines BOTH the staged side (HEAD →
//! index) AND the unstaged remainder (index → workdir); porcelain status
//! confirms the file lands in the expected section(s).
//!
//! One scenario additionally proves equivalence to git's own partial apply:
//! a single hunk fed to `git apply --cached` on a twin repo must yield the same
//! `git write-tree` as our line-selected `stage_partial`.
//!
//! HARD RULE: every scratch repo lives on D: via `common::init_repo` (which
//! calls `scratch_dir()`); fixtures pin `core.autocrlf=false`,
//! `init.defaultBranch=main`, and a repo-local identity. Each test skips
//! (passes with a note) if `git` is not on PATH.

use std::path::Path;

use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo};
use crate::stage_partial_helpers::{
    all_changed, hunk_changed, numbered, numbered_edited, repo_with, staged_bytes, write, xy,
};
use bonsai_core::error::AppError;
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

// ---------------------------------------------------------------------------
// §6.2 scenarios
// ---------------------------------------------------------------------------

// Scenario 1: 40-line file edited in 3 places; stage the MIDDLE hunk only.
// Proven against git's own `git apply --cached` (single-hunk patch) on a twin.
#[test]
fn one_hunk_of_many() {
    require_git!();
    let dir = repo_with(40);
    let p = dir.path();
    let edited = numbered_edited(40, &[(3, "line 3 X"), (20, "line 20 X"), (37, "line 37 X")]);
    write(p, "f.txt", &edited);

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 3, "three separated edits => three hunks");

    // Our op: stage exactly the middle hunk's changed lines.
    stage_partial(p, "f.txt", None, &hunk_changed(&fd, 1)).expect("stage middle hunk");
    let ours_tree = git(p, &["write-tree"]);

    // Oracle: on a twin, feed the same single hunk to `git apply --cached`.
    let twin = repo_with(40);
    let tp = twin.path();
    write(tp, "f.txt", &edited);
    // Full unified patch, then slice out the diff header + the 2nd hunk.
    let full = String::from_utf8(git_raw(
        tp,
        &["diff", "--no-color", "-U3", "--", "f.txt"],
        &[],
    ))
    .expect("utf8 patch");
    let minimal = single_hunk_patch(&full, 1);
    apply_cached(tp, &minimal);
    let oracle_tree = git(tp, &["write-tree"]);

    assert_eq!(
        ours_tree, oracle_tree,
        "stage_partial(middle hunk) must equal `git apply --cached` of that hunk"
    );

    // Byte-exact: staged index has ONLY line 20 changed.
    let expect = numbered_edited(40, &[(20, "line 20 X")]);
    assert_eq!(staged_bytes(p, "f.txt"), expect);
    // Remainder still unstaged (workdir differs from index).
    assert_eq!(xy(p, "f.txt").as_deref(), Some("MM"));
}

/// Extracts the `diff --git` header + hunk number `idx` (0-based) from a full
/// unified patch as a standalone single-file, single-hunk patch.
fn single_hunk_patch(full: &str, idx: usize) -> String {
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    let mut cur: Option<String> = None;
    for line in full.lines() {
        if line.starts_with("@@") {
            if let Some(h) = cur.take() {
                hunks.push(h);
            }
            cur = Some(format!("{line}\n"));
        } else if let Some(h) = cur.as_mut() {
            h.push_str(line);
            h.push('\n');
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }
    if let Some(h) = cur.take() {
        hunks.push(h);
    }
    format!("{header}{}", hunks[idx])
}

fn apply_cached(dir: &Path, patch: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .args(["apply", "--cached", "-"])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git apply");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(patch.as_bytes())
        .expect("write patch");
    let out = child.wait_with_output().expect("wait git apply");
    assert!(
        out.status.success(),
        "git apply --cached failed: {}\n--- patch ---\n{patch}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// Scenario 2: stage exactly one ADDED line out of several adds.
#[test]
fn single_add() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    // Insert two new lines after "a".
    write(p, "f.txt", b"a\nNEW1\nNEW2\nb\nc\n");

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    // Pick only the first added line (new_no of "NEW1" == 2).
    let sel = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(2),
    }];
    stage_partial(p, "f.txt", None, &sel).expect("stage one add");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nNEW1\nb\nc\n");
    assert_eq!(xy(p, "f.txt").as_deref(), Some("MM"));
    // Sanity: fd had two adds.
    assert_eq!(all_changed(&fd).len(), 2);
}

// Scenario 3: stage exactly one DELETED line (removed from the index).
#[test]
fn del_only() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc\nd\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    // Delete "b" and "c" in the workdir.
    write(p, "f.txt", b"a\nd\n");

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    // Stage only the deletion of "b" (old_no 2), leaving "c" in the index.
    let sel = vec![LineSelection {
        kind: LineKind::Del,
        old_no: Some(2),
        new_no: None,
    }];
    stage_partial(p, "f.txt", None, &sel).expect("stage one del");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nc\nd\n");
    assert_eq!(all_changed(&fd).len(), 2, "two deletions available");
}

// Scenario 4: a modification (del+add pair): stage only the add, then (fresh)
// only the del — each independent.
#[test]
fn mixed_add_del_each_side() {
    require_git!();
    // Variant A: stage only the ADD half of the modification.
    {
        let dir = init_repo();
        let p = dir.path();
        write(p, "f.txt", b"a\nb\nc\n");
        git(p, &["add", "-A"]);
        commit_fixed(p, "base");
        write(p, "f.txt", b"a\nB\nc\n"); // modify line 2

        let sel = vec![LineSelection {
            kind: LineKind::Add,
            old_no: None,
            new_no: Some(2),
        }];
        stage_partial(p, "f.txt", None, &sel).expect("stage add half");
        // Add "B" without removing "b". Reconstruction emits hunk lines in
        // order: the unselected del keeps old "b", then the selected add "B".
        assert_eq!(staged_bytes(p, "f.txt"), b"a\nb\nB\nc\n");
    }
    // Variant B: stage only the DEL half.
    {
        let dir = init_repo();
        let p = dir.path();
        write(p, "f.txt", b"a\nb\nc\n");
        git(p, &["add", "-A"]);
        commit_fixed(p, "base");
        write(p, "f.txt", b"a\nB\nc\n");

        let sel = vec![LineSelection {
            kind: LineKind::Del,
            old_no: Some(2),
            new_no: None,
        }];
        stage_partial(p, "f.txt", None, &sel).expect("stage del half");
        // Remove "b" without adding "B".
        assert_eq!(staged_bytes(p, "f.txt"), b"a\nc\n");
    }
}

// Scenario 5: a selection spanning changed lines in two adjacent hunks.
#[test]
fn range_across_two_hunks() {
    require_git!();
    let dir = repo_with(20);
    let p = dir.path();
    // Edits at line 3 and line 8 -> two hunks (separated by >6 context lines? 8-3=5, may merge). Use 3 and 12.
    let edited = numbered_edited(20, &[(3, "line 3 X"), (12, "line 12 X")]);
    write(p, "f.txt", &edited);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 2, "edits at 3 and 12 must be two hunks");

    // Select the changed lines from BOTH hunks (whole-file, via all_changed).
    stage_partial(p, "f.txt", None, &all_changed(&fd)).expect("stage across hunks");
    assert_eq!(staged_bytes(p, "f.txt"), edited, "both edits now staged");
    assert_eq!(
        xy(p, "f.txt").as_deref(),
        Some("M "),
        "nothing left unstaged"
    );
}

// Scenario 14: rejections.
#[test]
fn rejections() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();

    let add1 = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(1),
    }];

    // empty selection -> Ok (no-op), even with no file.
    stage_partial(p, "whatever.txt", None, &[]).expect("empty selection is Ok");

    // invalid / escaping path -> AppError::Other("invalid path...").
    let err = stage_partial(p, "../escape", None, &add1).expect_err("escaping path");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("invalid path")),
        "{err:?}"
    );

    // binary file -> rejected.
    let blob: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
    write(p, "b.bin", &blob);
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    let mut modified = blob.clone();
    modified[10] = 0xAA;
    write(p, "b.bin", &modified);
    let err = stage_partial(p, "b.bin", None, &add1).expect_err("binary");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("binary")),
        "{err:?}"
    );

    // too_large -> rejected. 6000-line deletion busts the 5000 cap.
    write(p, "big.txt", &numbered(6000));
    git(p, &["add", "-A"]);
    commit_fixed(p, "big base");
    std::fs::remove_file(p.join("big.txt")).expect("remove big");
    let del_big = vec![LineSelection {
        kind: LineKind::Del,
        old_no: Some(1),
        new_no: None,
    }];
    let err = stage_partial(p, "big.txt", None, &del_big).expect_err("too_large");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("too-large")),
        "{err:?}"
    );

    // renamed -> rejected. A STAGED rename is detectable in the HEAD->index
    // (unstage) diff via find_similar; the stage direction (index->workdir)
    // does not detect renames of untracked new sides, so the reachable guard is
    // on unstage_partial.
    write(p, "old.txt", &numbered(20));
    git(p, &["add", "-A"]);
    commit_fixed(p, "rename base");
    git(p, &["mv", "old.txt", "new.txt"]);
    write(p, "new.txt", &numbered_edited(20, &[(10, "line 10 X")]));
    git(p, &["add", "-A"]);
    let staged =
        workdir_file_diff(p, "new.txt", Some("old.txt"), true, false, false).expect("diff");
    assert_eq!(staged.status, bonsai_core::git::status::FileStatus::Renamed);
    let err =
        unstage_partial(p, "new.txt", Some("old.txt"), &all_changed(&staged)).expect_err("renamed");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("renamed")),
        "{err:?}"
    );

    // stale -> a selection coordinate absent from the recomputed diff.
    write(p, "s.txt", b"a\nb\nc\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "s base");
    write(p, "s.txt", b"a\nB\nc\n"); // only line 2 changed
    let bogus = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(99),
    }];
    let err = stage_partial(p, "s.txt", None, &bogus).expect_err("stale");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("stale")),
        "{err:?}"
    );
}

// Scenario 16: full-context regression (§6.2 #16).
#[test]
fn full_context_regression() {
    require_git!();
    let dir = repo_with(20);
    let p = dir.path();
    let edited = numbered_edited(20, &[(3, "line 3 X"), (12, "line 12 X")]);
    write(p, "f.txt", &edited);

    // full_context = false -> the M4 3-context multi-hunk view.
    let three = workdir_file_diff(p, "f.txt", None, false, false, false).expect("3-context");
    assert_eq!(three.hunks.len(), 2, "two separated edits -> two hunks");

    // full_context = true -> exactly one whole-file hunk covering all 20 lines.
    let full = workdir_file_diff(p, "f.txt", None, false, true, false).expect("full-context");
    assert_eq!(full.hunks.len(), 1, "whole file is one hunk");
    let h = &full.hunks[0];
    assert_eq!((h.old_start, h.old_lines), (1, 20));
    assert_eq!((h.new_start, h.new_lines), (1, 20));
    // Same changed content in both views (add/del numbering is context-free).
    assert_eq!(all_changed(&three).len(), all_changed(&full).len());

    // A 6000-line change with full_context=true still trips too_large.
    write(p, "big.txt", &numbered(6000));
    git(p, &["add", "-A"]);
    commit_fixed(p, "big");
    std::fs::remove_file(p.join("big.txt")).expect("remove big");
    let big = workdir_file_diff(p, "big.txt", None, false, true, false).expect("big full-context");
    assert!(big.too_large, "cap enforced regardless of context");
    assert!(big.hunks.is_empty());
}
