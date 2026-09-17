//! P17 CLI-oracle partial-staging tests — line-terminator byte-exactness
//! (contract §6.2 scenarios 6, 6b, 7, 7b): missing final newline, CRLF with
//! `core.autocrlf=false`, and the two combined.
//!
//! Moved verbatim out of `stage_partial_cli.rs`; see that module for the
//! oracle rules and `stage_partial_helpers` for the shared helpers.

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::stage_partial_helpers::{all_changed, staged_bytes, write, xy};
use bonsai_core::git::diff::workdir_file_diff;
use bonsai_core::git::stage_partial::{stage_partial, unstage_partial};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
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
    assert_eq!(
        staged_bytes(p, "f.txt"),
        b"a\nb\nd",
        "no phantom trailing newline"
    );
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
    assert_eq!(
        xy(p, "f.txt").as_deref(),
        Some("M "),
        "nothing left unstaged"
    );
}
