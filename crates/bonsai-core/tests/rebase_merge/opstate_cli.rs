//! T2 Area 7 — `read_op_state` against HANDCRAFTED garbage state dirs.
//!
//! `read_op_state` classifies an in-progress op from `repo.state()` + plain file
//! reads. The mandate: on ANY corrupt/impossible on-disk state it must classify
//! or return a clean `AppError` — NEVER panic (no `unwrap` on a bad oid, no
//! `from_utf8().unwrap()` on a non-UTF-8 head-name, no div-by-zero). We forge
//! the marker files libgit2 keys `repo.state()` off of and feed it junk.

use std::path::Path;

use bonsai_core::git::commit::create_commit;
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::stage::stage_paths;
use crate::common;

/// git2-init a repo with ONE commit (so HEAD/oids exist). Returns (dir, head_oid).
fn seeded() -> (tempfile::TempDir, String) {
    let dir = common::scratch_dir();
    let p = dir.path();
    let repo = git2::Repository::init(p).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    std::fs::write(p.join("a.txt"), "a\n").expect("write");
    stage_paths(p, &["a.txt".to_string()]).expect("stage");
    let oid = create_commit(p, "seed", None, false).expect("commit").oid;
    (dir, oid)
}

fn gitdir(p: &Path) -> std::path::PathBuf {
    p.join(".git")
}

/// Junk MERGE_HEAD (not an oid), no MERGE_MSG → Merge with an "(unknown)"
/// incoming and an empty message; the bad-oid `mergehead_foreach` must not panic.
#[test]
fn merge_junk_merge_head_no_msg_classifies_calmly() {
    let (dir, _oid) = seeded();
    let p = dir.path();
    std::fs::write(gitdir(p).join("MERGE_HEAD"), "not-a-valid-oid\n").expect("write");
    let state = read_op_state(p).expect("must not error");
    match state {
        RepoOpState::Merge { incoming, message } => {
            assert_eq!(incoming, "(unknown)", "junk MERGE_HEAD → unknown incoming");
            assert!(message.is_empty(), "no MERGE_MSG → empty message");
        }
        other => panic!("expected Merge, got {other:?}"),
    }
}

/// A valid MERGE_HEAD + a CRLF, multi-MB MERGE_MSG: incoming parses from the
/// first line; the big body is normalized (CRLF→LF, trailing-trimmed) without
/// panicking or truncating the classification.
#[test]
fn merge_crlf_multi_mb_merge_msg() {
    let (dir, oid) = seeded();
    let p = dir.path();
    std::fs::write(gitdir(p).join("MERGE_HEAD"), format!("{oid}\n")).expect("write head");
    let mut msg = String::from("Merge branch 'feature/x'\r\n\r\n");
    msg.push_str(&"padding line\r\n".repeat(150_000)); // ~2 MB
    std::fs::write(gitdir(p).join("MERGE_MSG"), &msg).expect("write msg");
    match read_op_state(p).expect("must not error") {
        RepoOpState::Merge { incoming, message } => {
            assert_eq!(incoming, "feature/x");
            assert!(!message.contains('\r'), "CRLF normalized away");
            assert!(message.len() > 1_000_000, "big body preserved");
        }
        other => panic!("expected Merge, got {other:?}"),
    }
}

/// rebase-merge/ with `msgnum=banana` and NO `end`: the non-numeric step parses
/// to 0 (never an unwrap panic), the missing total to 0; head-name still reads.
#[test]
fn rebase_msgnum_banana_missing_end() {
    let (dir, _oid) = seeded();
    let p = dir.path();
    let rm = gitdir(p).join("rebase-merge");
    std::fs::create_dir_all(&rm).expect("mkdir rebase-merge");
    std::fs::write(rm.join("msgnum"), "banana\n").expect("msgnum");
    std::fs::write(rm.join("head-name"), "refs/heads/topic\n").expect("head-name");
    match read_op_state(p).expect("must not error") {
        RepoOpState::Rebase { head_name, current_step, total_steps, .. } => {
            assert_eq!(head_name.as_deref(), Some("topic"));
            assert_eq!(current_step, 0, "non-numeric msgnum → 0, not a panic");
            assert_eq!(total_steps, 0, "missing end → 0");
        }
        other => panic!("expected Rebase, got {other:?}"),
    }
}

/// rebase-merge/head-name with INVALID UTF-8 bytes: `read_to_string` fails →
/// head_name is None (lossy-safe), never a `from_utf8().unwrap()` panic.
#[test]
fn rebase_non_utf8_head_name() {
    let (dir, _oid) = seeded();
    let p = dir.path();
    let rm = gitdir(p).join("rebase-merge");
    std::fs::create_dir_all(&rm).expect("mkdir");
    std::fs::write(rm.join("head-name"), [0xff, 0xfe, 0x00, b'\n']).expect("head-name");
    std::fs::write(rm.join("msgnum"), "1\n").expect("msgnum");
    std::fs::write(rm.join("end"), "3\n").expect("end");
    match read_op_state(p).expect("must not error") {
        RepoOpState::Rebase { head_name, current_step, total_steps, .. } => {
            assert_eq!(head_name, None, "undecodable head-name → None");
            assert_eq!(current_step, 1);
            assert_eq!(total_steps, 3);
        }
        other => panic!("expected Rebase, got {other:?}"),
    }
}

/// "Impossible" combo: BOTH MERGE_HEAD and rebase-merge/ present. libgit2 picks
/// one deterministically; whichever it is, we classify without panicking.
#[test]
fn impossible_merge_and_rebase_combo() {
    let (dir, oid) = seeded();
    let p = dir.path();
    std::fs::write(gitdir(p).join("MERGE_HEAD"), format!("{oid}\n")).expect("head");
    let rm = gitdir(p).join("rebase-merge");
    std::fs::create_dir_all(&rm).expect("mkdir");
    std::fs::write(rm.join("msgnum"), "1\n").expect("msgnum");
    let state = read_op_state(p).expect("must classify or error, not panic");
    assert!(
        matches!(state, RepoOpState::Merge { .. } | RepoOpState::Rebase { .. }),
        "expected one of the two conflicting states, got {state:?}"
    );
}

/// Junk CHERRY_PICK_HEAD → CherryPick; junk REVERT_HEAD → Revert. No panic even
/// when the head file is garbage.
#[test]
fn junk_cherrypick_and_revert_heads() {
    let (dir, _oid) = seeded();
    let p = dir.path();
    std::fs::write(gitdir(p).join("CHERRY_PICK_HEAD"), "garbage\n").expect("cp");
    assert_eq!(read_op_state(p).expect("ok"), RepoOpState::CherryPick);
    std::fs::remove_file(gitdir(p).join("CHERRY_PICK_HEAD")).expect("rm");

    std::fs::write(gitdir(p).join("REVERT_HEAD"), "garbage\n").expect("rev");
    assert_eq!(read_op_state(p).expect("ok"), RepoOpState::Revert);
}

/// A bare repo has no working tree: `read_op_state` returns a clean `AppError`
/// (via `open_workdir_repo`), never a `workdir().unwrap()` panic.
#[test]
fn bare_repo_errors_cleanly() {
    let dir = common::scratch_dir();
    git2::Repository::init_bare(dir.path()).expect("init bare");
    let err = read_op_state(dir.path()).expect_err("bare repo must be a clean error");
    // Any typed AppError is acceptable; the point is no panic.
    let _ = format!("{err:?}");
}
