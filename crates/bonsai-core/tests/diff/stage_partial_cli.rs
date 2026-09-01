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

use bonsai_core::error::AppError;
use bonsai_core::git::diff::{workdir_file_diff, FileDiff, LineKind};
use bonsai_core::git::stage_partial::{stage_partial, unstage_partial, LineSelection};
use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write(dir: &Path, name: &str, content: &[u8]) {
    std::fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("write {name}: {e}"));
}

/// Raw staged (stage-0 index) blob bytes for `path` via `git show :path`.
/// With `core.autocrlf=false` this is byte-exact.
fn staged_bytes(dir: &Path, path: &str) -> Vec<u8> {
    git_raw(dir, &["show", &format!(":{path}")], &[])
}

/// `git status --porcelain=v1` XY code for `path`, or `None` if not listed.
/// Reads RAW (untrimmed) output: a leading X-column space (e.g. " M") must not
/// be stripped, so the trimming `git()` helper cannot be used here.
fn xy(dir: &Path, path: &str) -> Option<String> {
    let raw = git_raw(dir, &["status", "--porcelain=v1", "--", path], &[]);
    let out = String::from_utf8_lossy(&raw);
    out.split('\n')
        .find(|l| l.get(3..).map(|p| p == path).unwrap_or(false))
        .map(|l| l[..2].to_string())
}

/// All Add/Del lines of a FileDiff as a selection (whole-file stage).
fn all_changed(fd: &FileDiff) -> Vec<LineSelection> {
    let mut sel = Vec::new();
    for h in &fd.hunks {
        for l in &h.lines {
            if matches!(l.kind, LineKind::Add | LineKind::Del) {
                sel.push(LineSelection {
                    kind: l.kind,
                    old_no: l.old_no,
                    new_no: l.new_no,
                });
            }
        }
    }
    sel
}

/// Add/Del lines of ONE hunk as a selection.
fn hunk_changed(fd: &FileDiff, hunk_idx: usize) -> Vec<LineSelection> {
    let mut sel = Vec::new();
    for l in &fd.hunks[hunk_idx].lines {
        if matches!(l.kind, LineKind::Add | LineKind::Del) {
            sel.push(LineSelection {
                kind: l.kind,
                old_no: l.old_no,
                new_no: l.new_no,
            });
        }
    }
    sel
}

fn numbered(n: usize) -> Vec<u8> {
    (1..=n)
        .map(|i| format!("line {i}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Numbered file with the given 1-based lines replaced by new text.
fn numbered_edited(n: usize, edits: &[(usize, &str)]) -> Vec<u8> {
    let mut lines: Vec<String> = (1..=n).map(|i| format!("line {i}")).collect();
    for (idx, text) in edits {
        lines[*idx - 1] = (*text).to_string();
    }
    (lines.join("\n") + "\n").into_bytes()
}

/// Base repo with a committed multi-line `f.txt` (n lines).
fn repo_with(n: usize) -> tempfile::TempDir {
    let dir = init_repo();
    write(dir.path(), "f.txt", &numbered(n));
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    dir
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
    assert_eq!(xy(p, "f.txt").as_deref(), Some("M "), "nothing left unstaged");
}

// Scenario 6: no-final-newline, stage a change touching the last line
// (byte-level terminator exactness).
#[test]
fn no_newline_stage() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc"); // no trailing newline
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"a\nb\nd"); // still no trailing newline

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    stage_partial(p, "f.txt", None, &all_changed(&fd)).expect("stage last-line change");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nb\nd", "no phantom trailing newline");
}

// Scenario 6b: no-final-newline, UNSTAGE (index -> HEAD) the last-line change.
#[test]
fn no_newline_unstage() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc"); // no trailing newline
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    // Stage a change to the last line.
    write(p, "f.txt", b"a\nb\nd");
    git(p, &["add", "-A"]);

    let fd = workdir_file_diff(p, "f.txt", None, true, false, false).expect("staged diff");
    // Unstage the whole change -> index reverts to HEAD ("a\nb\nc", no newline).
    unstage_partial(p, "f.txt", None, &all_changed(&fd)).expect("unstage last-line change");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nb\nc");
}

// Scenario 7: CRLF file (autocrlf=false); partial stage keeps \r\n on every
// line, no phantom ^M (byte-level).
#[test]
fn crlf() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"one\r\ntwo\r\nthree\r\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"one\r\ntwo CHANGED\r\nthree\r\n");

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    stage_partial(p, "f.txt", None, &all_changed(&fd)).expect("stage crlf change");
    assert_eq!(
        staged_bytes(p, "f.txt"),
        b"one\r\ntwo CHANGED\r\nthree\r\n",
        "CRLF must survive byte-for-byte"
    );
}

// Scenario 7b (gap): CRLF *and* no-final-newline together. Committed file has
// CRLF line separators AND no terminator on the last line; a partial stage of a
// change touching that last line must keep every interior `\r\n` and reproduce
// the missing final terminator exactly (byte-level). The existing `crlf` test
// has a trailing CRLF newline and `no_newline_stage` is LF-only, so the
// combination is otherwise unexercised.
#[test]
fn crlf_no_final_newline() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"one\r\ntwo\r\nthree"); // CRLF, no final newline
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"one\r\ntwo\r\nTHREE"); // change the terminator-less last line

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    stage_partial(p, "f.txt", None, &all_changed(&fd)).expect("stage crlf/no-eol change");
    assert_eq!(
        staged_bytes(p, "f.txt"),
        b"one\r\ntwo\r\nTHREE",
        "CRLF interiors kept and last line still has no trailing newline"
    );
    assert_eq!(xy(p, "f.txt").as_deref(), Some("M "), "nothing left unstaged");
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
    let add_b = vec![LineSelection { kind: LineKind::Add, old_no: None, new_no: Some(2) }];
    stage_partial(p, "f.txt", None, &add_b).expect("stage the add");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nb\nc\n", "add staged");
    assert_eq!(xy(p, "f.txt").as_deref(), Some("M "), "fully staged, workdir clean vs index");

    // Now unstage the SAME line from the staged (HEAD -> index) diff.
    let staged = workdir_file_diff(p, "f.txt", None, true, false, false).expect("staged diff");
    let staged_add = hunk_changed(&staged, 0);
    assert_eq!(staged_add.len(), 1, "one staged add to reverse");
    unstage_partial(p, "f.txt", None, &staged_add).expect("unstage the same add");
    assert_eq!(staged_bytes(p, "f.txt"), b"a\nc\n", "index restored byte-exactly to HEAD");
    assert_eq!(xy(p, "f.txt").as_deref(), Some(" M"), "back to a pure workdir change");
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
        assert_eq!(xy(p, "u.txt").as_deref(), Some("AM"), "added + still-modified");
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
            LineSelection { kind: LineKind::Del, old_no: Some(1), new_no: None },
            LineSelection { kind: LineKind::Del, old_no: Some(3), new_no: None },
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
    assert!(staged_bytes(p, "e.txt").is_empty(), "index restored to empty blob");
    assert_eq!(xy(p, "e.txt").as_deref(), Some(" M"), "no staged deletion");
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
    assert_eq!(staged_bytes(p, "f.txt"), edited, "composed == whole-file stage");
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
        let sel = vec![LineSelection { kind: LineKind::Add, old_no: None, new_no: Some(2) }];
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
    assert_eq!(xy(p, "f.txt").as_deref(), Some(" M"), "still only workdir change");
}

// Scenario 14: rejections.
#[test]
fn rejections() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();

    let add1 = vec![LineSelection { kind: LineKind::Add, old_no: None, new_no: Some(1) }];

    // empty selection -> Ok (no-op), even with no file.
    stage_partial(p, "whatever.txt", None, &[]).expect("empty selection is Ok");

    // invalid / escaping path -> AppError::Other("invalid path...").
    let err = stage_partial(p, "../escape", None, &add1).expect_err("escaping path");
    assert!(matches!(&err, AppError::Other(m) if m.contains("invalid path")), "{err:?}");

    // binary file -> rejected.
    let blob: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
    write(p, "b.bin", &blob);
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    let mut modified = blob.clone();
    modified[10] = 0xAA;
    write(p, "b.bin", &modified);
    let err = stage_partial(p, "b.bin", None, &add1).expect_err("binary");
    assert!(matches!(&err, AppError::Other(m) if m.contains("binary")), "{err:?}");

    // too_large -> rejected. 6000-line deletion busts the 5000 cap.
    write(p, "big.txt", &numbered(6000));
    git(p, &["add", "-A"]);
    commit_fixed(p, "big base");
    std::fs::remove_file(p.join("big.txt")).expect("remove big");
    let del_big = vec![LineSelection { kind: LineKind::Del, old_no: Some(1), new_no: None }];
    let err = stage_partial(p, "big.txt", None, &del_big).expect_err("too_large");
    assert!(matches!(&err, AppError::Other(m) if m.contains("too-large")), "{err:?}");

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
    let staged = workdir_file_diff(p, "new.txt", Some("old.txt"), true, false, false).expect("diff");
    assert_eq!(staged.status, bonsai_core::git::status::FileStatus::Renamed);
    let err = unstage_partial(p, "new.txt", Some("old.txt"), &all_changed(&staged))
        .expect_err("renamed");
    assert!(matches!(&err, AppError::Other(m) if m.contains("renamed")), "{err:?}");

    // stale -> a selection coordinate absent from the recomputed diff.
    write(p, "s.txt", b"a\nb\nc\n");
    git(p, &["add", "-A"]);
    commit_fixed(p, "s base");
    write(p, "s.txt", b"a\nB\nc\n"); // only line 2 changed
    let bogus = vec![LineSelection { kind: LineKind::Add, old_no: None, new_no: Some(99) }];
    let err = stage_partial(p, "s.txt", None, &bogus).expect_err("stale");
    assert!(matches!(&err, AppError::Other(m) if m.contains("stale")), "{err:?}");
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
