//! P28 CLI-oracle integration tests for `discard_partial` (contract §6.2).
//!
//! Oracles:
//! - **Index invariant**: `git diff --cached` raw bytes must be identical
//!   before/after every call — the command never touches the index.
//! - **Remainder equivalence**: `git apply --reverse` of the minimal
//!   single-hunk patch on a twin repo must yield byte-identical worktree
//!   content to ours.
//! - **All-hunks discard**: byte-identical to `git checkout -- <path>` on a
//!   twin, and the porcelain row disappears.
//! - **CRLF / no-EOF-newline**: byte-level assertions (autocrlf=false pinned
//!   for byte-exact cases; a dedicated autocrlf=true fixture for the
//!   LF-index/CRLF-worktree splice).
//!
//! HARD RULE: every scratch repo lives on D: via `common::init_repo`
//! (`scratch_dir()`). Twin repos use `commit_fixed` so base oids match. Each
//! test skips (passes with a note) if `git` is not on PATH.

use bonsai_core::git::diff::workdir_file_diff;
use bonsai_core::git::discard_partial::discard_partial;
use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo};
use crate::discard_partial_helpers::{
    all_changed, cached_diff, git_apply_stdin, hunk_changed, numbered_edited, read, repo_with,
    single_hunk_patch, single_hunk_patch_bytes, write, xy,
};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// §6.2.1 — Index invariant
// ---------------------------------------------------------------------------

// Staged + unstaged edits to the SAME file; discarding one unstaged hunk must
// leave `git diff --cached` byte-identical (and the staged edit intact).
#[test]
fn index_invariant_with_staged_and_unstaged_edits() {
    require_git!();
    let dir = repo_with(40);
    let p = dir.path();

    // Staged edit at line 5.
    write(p, "f.txt", &numbered_edited(40, &[(5, "line 5 STAGED")]));
    git(p, &["add", "f.txt"]);
    // Two additional UNSTAGED edits at lines 20 and 35.
    write(
        p,
        "f.txt",
        &numbered_edited(40, &[(5, "line 5 STAGED"), (20, "line 20 W"), (35, "line 35 W")]),
    );

    let cached_before = cached_diff(p);
    assert!(!cached_before.is_empty(), "fixture must have a staged edit");
    assert_eq!(xy(p, "f.txt").as_deref(), Some("MM"));

    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 2, "two unstaged edits => two hunks");

    // Discard the FIRST unstaged hunk (the line-20 edit).
    discard_partial(p, "f.txt", None, &hunk_changed(&fd, 0)).expect("discard hunk 0");

    assert_eq!(
        cached_diff(p),
        cached_before,
        "`git diff --cached` must be byte-identical: the index is never touched"
    );
    // Worktree: staged edit + line-35 edit survive; line-20 edit reverted.
    assert_eq!(
        read(p, "f.txt"),
        numbered_edited(40, &[(5, "line 5 STAGED"), (35, "line 35 W")])
    );
    assert_eq!(xy(p, "f.txt").as_deref(), Some("MM"), "surviving hunk still unstaged");
}

// ---------------------------------------------------------------------------
// §6.2.2 — Remainder equivalence vs `git apply --reverse`
// ---------------------------------------------------------------------------

// Discarding the middle of three hunks must equal `git apply --reverse` of
// that hunk's minimal patch applied to a twin worktree.
#[test]
fn remainder_matches_git_apply_reverse() {
    require_git!();
    let edited = numbered_edited(40, &[(3, "line 3 X"), (20, "line 20 X"), (37, "line 37 X")]);

    // Ours.
    let dir = repo_with(40);
    let p = dir.path();
    write(p, "f.txt", &edited);
    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 3, "three separated edits => three hunks");
    discard_partial(p, "f.txt", None, &hunk_changed(&fd, 1)).expect("discard middle hunk");

    // Oracle twin: `git apply --reverse` of the same single hunk in the worktree.
    let twin = repo_with(40);
    let tp = twin.path();
    write(tp, "f.txt", &edited);
    let full = String::from_utf8(git_raw(tp, &["diff", "--no-color", "-U3", "--", "f.txt"], &[]))
        .expect("utf8 patch");
    let minimal = single_hunk_patch(&full, 1);
    git_apply_stdin(tp, &["--reverse"], minimal.as_bytes());

    assert_eq!(
        read(p, "f.txt"),
        read(tp, "f.txt"),
        "discard_partial(middle hunk) must equal `git apply --reverse` of that hunk"
    );
    // Explicit expectation too: hunks 1 & 3 survive, hunk 2 reverted.
    assert_eq!(read(p, "f.txt"), numbered_edited(40, &[(3, "line 3 X"), (37, "line 37 X")]));
    assert_eq!(cached_diff(p), cached_before, "index invariant");
    assert_eq!(xy(p, "f.txt").as_deref(), Some(" M"), "remainder still unstaged");
}

// Same equivalence for a pure-deletion hunk (Del lines restored from the index).
#[test]
fn remainder_matches_git_apply_reverse_deletion_hunk() {
    require_git!();
    let dir = repo_with(30);
    let p = dir.path();
    // Delete lines 5-6 and modify line 20 -> two hunks.
    let mut lines: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
    lines[19] = "line 20 X".to_string();
    lines.remove(5); // "line 6"
    lines.remove(4); // "line 5"
    let edited = (lines.join("\n") + "\n").into_bytes();
    write(p, "f.txt", &edited);

    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 2);
    // Discard the deletion hunk (hunk 0) -> lines 5-6 restored from the index.
    discard_partial(p, "f.txt", None, &hunk_changed(&fd, 0)).expect("discard del hunk");

    let twin = repo_with(30);
    let tp = twin.path();
    write(tp, "f.txt", &edited);
    let full = String::from_utf8(git_raw(tp, &["diff", "--no-color", "-U3", "--", "f.txt"], &[]))
        .expect("utf8 patch");
    git_apply_stdin(tp, &["--reverse"], single_hunk_patch(&full, 0).as_bytes());

    assert_eq!(read(p, "f.txt"), read(tp, "f.txt"));
    assert_eq!(read(p, "f.txt"), numbered_edited(30, &[(20, "line 20 X")]));
    assert_eq!(cached_diff(p), cached_before, "index invariant");
}

// ---------------------------------------------------------------------------
// §6.2.3 — CRLF / no-newline-at-EOF byte cases
// ---------------------------------------------------------------------------

// autocrlf=false (pinned by init_repo): every \r\n preserved byte-exact after
// a partial discard; the discarded hunk's line reverts to its CRLF original.
#[test]
fn crlf_autocrlf_false_byte_exact() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    let base: Vec<u8> = (1..=20).map(|i| format!("line {i}\r\n")).collect::<String>().into_bytes();
    write(p, "f.txt", &base);
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    // Edit lines 3 and 12 (still CRLF) -> two hunks.
    let mut lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    lines[2] = "line 3 X".to_string();
    lines[11] = "line 12 X".to_string();
    let edited = (lines.join("\r\n") + "\r\n").into_bytes();
    write(p, "f.txt", &edited);

    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 2);
    discard_partial(p, "f.txt", None, &hunk_changed(&fd, 0)).expect("discard hunk 0");

    // Byte-exact expectation: line 3 reverted, line 12 edit kept, all CRLF.
    let mut expect: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    expect[11] = "line 12 X".to_string();
    let expect = (expect.join("\r\n") + "\r\n").into_bytes();
    let got = read(p, "f.txt");
    assert_eq!(got, expect, "CRLF must survive byte-for-byte");
    assert!(!got.windows(2).any(|w| w[1] == b'\n' && w[0] != b'\r'), "no bare LF introduced");
    assert_eq!(cached_diff(p), cached_before, "index invariant");

    // CLI cross-check on a twin: apply --reverse of the same hunk. NOTE: the
    // patch body carries the file's \r bytes, so it must be sliced at the BYTE
    // level (str::lines() would strip \r and corrupt the patch).
    let twin = init_repo();
    let tp = twin.path();
    write(tp, "f.txt", &base);
    git(tp, &["add", "-A"]);
    commit_fixed(tp, "base");
    write(tp, "f.txt", &edited);
    let full = git_raw(tp, &["diff", "--no-color", "-U3", "--", "f.txt"], &[]);
    git_apply_stdin(tp, &["--reverse"], &single_hunk_patch_bytes(&full, 0));
    assert_eq!(got, read(tp, "f.txt"), "matches git apply --reverse byte-for-byte");
}

// autocrlf=true: index blob is LF, worktree is CRLF. Restored Del lines must
// come out CRLF (no mixed endings); git must report only the surviving hunk.
#[test]
fn crlf_autocrlf_true_restored_lines_normalized() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    git(p, &["config", "core.autocrlf", "true"]);
    let base: Vec<u8> = (1..=20).map(|i| format!("line {i}\r\n")).collect::<String>().into_bytes();
    write(p, "f.txt", &base);
    git(p, &["add", "-A"]); // index blob stored LF-normalized
    commit_fixed(p, "base");
    // Edit lines 3 and 12 in the CRLF worktree -> two hunks.
    let mut lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    lines[2] = "line 3 X".to_string();
    lines[11] = "line 12 X".to_string();
    let edited = (lines.join("\r\n") + "\r\n").into_bytes();
    write(p, "f.txt", &edited);

    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 2, "two edits => two hunks");
    // Discard hunk 0: its Del line ("line 3") is restored FROM THE LF INDEX
    // BLOB and must be spliced back as CRLF.
    discard_partial(p, "f.txt", None, &hunk_changed(&fd, 0)).expect("discard hunk 0");

    let got = read(p, "f.txt");
    assert!(
        !got.windows(2).any(|w| w[1] == b'\n' && w[0] != b'\r'),
        "restored line must be CRLF — no mixed endings: {:?}",
        String::from_utf8_lossy(&got)
    );
    let mut expect: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    expect[11] = "line 12 X".to_string();
    assert_eq!(got, (expect.join("\r\n") + "\r\n").into_bytes());
    assert_eq!(cached_diff(p), cached_before, "index invariant");
    // git sees only the surviving edit as modified.
    assert_eq!(xy(p, "f.txt").as_deref(), Some(" M"));
    let remaining = git(p, &["diff", "--no-color", "--", "f.txt"]);
    assert!(remaining.contains("line 12 X"), "surviving hunk present:\n{remaining}");
    assert!(!remaining.contains("line 3 X"), "discarded hunk gone:\n{remaining}");

    // Discard the rest -> file clean (no perpetual-modified CRLF artifact).
    let fd2 = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff 2");
    discard_partial(p, "f.txt", None, &all_changed(&fd2)).expect("discard remainder");
    assert_eq!(xy(p, "f.txt"), None, "file clean after discarding everything");
}

// No trailing newline: discarding the last-line edit must round-trip the
// terminator state byte-exactly (no phantom trailing newline).
#[test]
fn no_final_newline_roundtrip() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc"); // no trailing newline
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"a\nb\nd"); // edit the terminator-less last line

    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    discard_partial(p, "f.txt", None, &all_changed(&fd)).expect("discard last-line edit");
    assert_eq!(read(p, "f.txt"), b"a\nb\nc", "restored byte-exactly, still no trailing newline");
    assert_eq!(cached_diff(p), cached_before, "index invariant");
    assert_eq!(xy(p, "f.txt"), None, "file clean");
}

// Inverse terminator case: worktree ADDED a trailing newline; discarding must
// remove it again (index has none).
#[test]
fn no_final_newline_added_terminator_discarded() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    write(p, "f.txt", b"a\nb\nc"); // committed WITHOUT trailing newline
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    write(p, "f.txt", b"a\nb\nc\n"); // worktree adds the terminator

    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert!(!all_changed(&fd).is_empty(), "terminator change must diff");
    discard_partial(p, "f.txt", None, &all_changed(&fd)).expect("discard terminator add");
    assert_eq!(read(p, "f.txt"), b"a\nb\nc", "trailing newline removed again");
    assert_eq!(cached_diff(p), cached_before, "index invariant");
    assert_eq!(xy(p, "f.txt"), None, "file clean");
}

// ---------------------------------------------------------------------------
// §6.2.4 — All-hunks discard == `git checkout -- <path>`
// ---------------------------------------------------------------------------

#[test]
fn all_hunks_equals_git_checkout() {
    require_git!();
    let edited = numbered_edited(40, &[(3, "line 3 X"), (20, "line 20 X"), (37, "line 37 X")]);

    // Ours: discard every hunk.
    let dir = repo_with(40);
    let p = dir.path();
    write(p, "f.txt", &edited);
    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    assert_eq!(fd.hunks.len(), 3);
    discard_partial(p, "f.txt", None, &all_changed(&fd)).expect("discard all hunks");

    // Oracle twin: `git checkout -- f.txt`.
    let twin = repo_with(40);
    let tp = twin.path();
    write(tp, "f.txt", &edited);
    git(tp, &["checkout", "--", "f.txt"]);

    assert_eq!(
        read(p, "f.txt"),
        read(tp, "f.txt"),
        "all-hunks discard must equal `git checkout -- <path>` byte-for-byte"
    );
    assert_eq!(cached_diff(p), cached_before, "index invariant");
    assert_eq!(xy(p, "f.txt"), None, "porcelain row gone");
    assert_eq!(xy(tp, "f.txt"), None, "twin row gone too");
}

// Same cross-check when a STAGED edit exists: `git checkout -- <path>` on the
// twin restores the INDEX (not HEAD), which is exactly what an all-hunks
// discard of the unstaged diff does. Staged half must survive in both.
#[test]
fn all_hunks_equals_git_checkout_with_staged_edit() {
    require_git!();
    let staged = numbered_edited(40, &[(5, "line 5 STAGED")]);
    let edited = numbered_edited(40, &[(5, "line 5 STAGED"), (20, "line 20 W")]);

    let dir = repo_with(40);
    let p = dir.path();
    write(p, "f.txt", &staged);
    git(p, &["add", "f.txt"]);
    write(p, "f.txt", &edited);
    let cached_before = cached_diff(p);
    let fd = workdir_file_diff(p, "f.txt", None, false, false, false).expect("diff");
    discard_partial(p, "f.txt", None, &all_changed(&fd)).expect("discard all unstaged");

    let twin = repo_with(40);
    let tp = twin.path();
    write(tp, "f.txt", &staged);
    git(tp, &["add", "f.txt"]);
    write(tp, "f.txt", &edited);
    git(tp, &["checkout", "--", "f.txt"]);

    assert_eq!(read(p, "f.txt"), read(tp, "f.txt"), "worktree == index restore");
    assert_eq!(read(p, "f.txt"), staged, "staged edit survives in the worktree");
    assert_eq!(cached_diff(p), cached_before, "index invariant — staged diff untouched");
    assert_eq!(xy(p, "f.txt").as_deref(), Some("M "), "only the staged half remains");
}