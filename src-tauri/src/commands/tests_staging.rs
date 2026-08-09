//! T2 Area 1 — happy + failure paths for the `staging` command inners
//! (stage / unstage / commit / commit_amend / stage_partial / unstage_partial),
//! runtime-free per the `tests.rs` pattern (broken tauri `test` feature).

use super::tests_support::*;
use super::*;
use bonsai_core::git::diff::LineKind;

fn status_of(state: &AppState, id: &str) -> StatusSnapshot {
    tauri::async_runtime::block_on(get_status_inner(state, id)).expect("status")
}

fn paths(entries: &[bonsai_core::git::status::StatusEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.path.as_str()).collect()
}

fn sel(kind: LineKind, old_no: Option<u32>, new_no: Option<u32>) -> LineSelection {
    LineSelection { kind, old_no, new_no }
}

/// stage puts a new file into the index; unstage takes it back out — the
/// worktree file is never touched.
#[test]
fn stage_then_unstage_round_trip() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    std::fs::write(dir.path().join("b.txt"), "new\n").expect("write");
    let before = status_of(&state, &id);
    assert!(paths(&before.untracked).contains(&"b.txt"));

    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["b.txt".into()]))
        .expect("stage");
    let staged = status_of(&state, &id);
    assert!(paths(&staged.staged).contains(&"b.txt"), "index must reflect the stage");
    assert!(!paths(&staged.untracked).contains(&"b.txt"));

    tauri::async_runtime::block_on(unstage_inner(&state, &id, vec!["b.txt".into()]))
        .expect("unstage");
    let after = status_of(&state, &id);
    assert!(paths(&after.untracked).contains(&"b.txt"), "back to untracked");
    assert!(after.staged.is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).expect("read"),
        "new\n",
        "worktree never touched"
    );
}

/// commit advances HEAD and the message round-trips byte-exactly, including
/// unicode + a multi-line body.
#[test]
fn commit_message_round_trip_unicode_multiline() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    let msg = "féature: ünïcode ✨ 日本語\n\nbody line 1\nbody line 2\n";
    std::fs::write(dir.path().join("u.txt"), "u\n").expect("write");
    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["u.txt".into()])).expect("stage");
    let res = tauri::async_runtime::block_on(commit_inner(&state, &id, msg.into(), None, None))
        .expect("commit");

    assert_eq!(res.summary, "féature: ünïcode ✨ 日本語");
    assert_eq!(res.branch.as_deref(), head_branch(dir.path()).as_deref());
    let new_head = head_oid(dir.path());
    assert_eq!(new_head, res.oid, "HEAD advanced to the new commit");
    assert_ne!(new_head, c0);

    let repo = git2::Repository::open(dir.path()).expect("open");
    let commit = repo
        .find_commit(git2::Oid::from_str(&res.oid).expect("oid"))
        .expect("find commit");
    assert_eq!(commit.message().expect("utf8 message"), msg);
    assert_eq!(commit.parent_id(0).expect("parent").to_string(), c0);
}

/// commit with a clean index → NothingToCommit; empty message → EmptyMessage.
#[test]
fn commit_failure_paths() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    let err = tauri::async_runtime::block_on(commit_inner(&state, &id, "noop".into(), None, None))
        .expect_err("nothing staged must error");
    assert!(matches!(err, AppError::NothingToCommit), "{err:?}");

    std::fs::write(dir.path().join("e.txt"), "e\n").expect("write");
    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["e.txt".into()])).expect("stage");
    let err = tauri::async_runtime::block_on(commit_inner(&state, &id, "  \n ".into(), None, None))
        .expect_err("blank message must error");
    assert!(matches!(err, AppError::EmptyMessage), "{err:?}");

    assert_eq!(head_oid(dir.path()), c0, "failed commits must not move HEAD");
}

/// commit_amend replaces HEAD with the new message + current index, preserving
/// the parent set (here: the root commit's zero parents).
#[test]
fn commit_amend_happy() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    std::fs::write(dir.path().join("extra.txt"), "x\n").expect("write");
    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["extra.txt".into()]))
        .expect("stage");
    let res =
        tauri::async_runtime::block_on(commit_amend_inner(&state, &id, "C0 amended".into(), None, None))
            .expect("amend");

    assert_ne!(res.oid, c0, "amend rewrites the commit oid");
    assert_eq!(head_oid(dir.path()), res.oid);
    let repo = git2::Repository::open(dir.path()).expect("open");
    let commit = repo
        .find_commit(git2::Oid::from_str(&res.oid).expect("oid"))
        .expect("commit");
    assert_eq!(commit.parent_count(), 0, "root commit keeps zero parents");
    assert_eq!(commit.summary().expect("summary"), Some("C0 amended"));
    assert!(commit.tree().expect("tree").get_name("extra.txt").is_some());
}

/// Amending an unborn-HEAD repo is a clean Git error (nothing to amend).
#[test]
fn commit_amend_unborn_head_errors() {
    let state = AppState::default();
    let dir = init_repo_with_identity();
    let opened = open(&state, dir.path()).expect("open");

    let err = tauri::async_runtime::block_on(commit_amend_inner(
        &state,
        &opened.repo_id,
        "msg".into(),
        None,
        None,
    ))
    .expect_err("unborn HEAD must not amend");
    match err {
        AppError::Git(m) => assert!(m.contains("nothing to amend"), "{m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

/// stage_partial with a single selected added line stages only that line: the
/// file shows up BOTH staged (line 4) and still unstaged (line 5).
#[test]
fn stage_partial_single_line_selection() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "p.txt", "l1\nl2\nl3\n", "seed p");

    std::fs::write(dir.path().join("p.txt"), "l1\nl2\nl3\nl4\nl5\n").expect("write");
    tauri::async_runtime::block_on(stage_partial_inner(
        &state,
        &id,
        "p.txt".into(),
        None,
        vec![sel(LineKind::Add, None, Some(4))],
    ))
    .expect("stage_partial");

    let st = status_of(&state, &id);
    assert!(paths(&st.staged).contains(&"p.txt"), "line 4 staged");
    assert!(paths(&st.unstaged).contains(&"p.txt"), "line 5 still unstaged");
}

/// Empty selection is a documented no-op; a stale coordinate (not in the fresh
/// diff) is the typed stale-selection error.
#[test]
fn stage_partial_empty_and_stale_selection() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "q.txt", "one\n", "seed q");
    std::fs::write(dir.path().join("q.txt"), "one\ntwo\n").expect("write");

    tauri::async_runtime::block_on(stage_partial_inner(&state, &id, "q.txt".into(), None, vec![]))
        .expect("empty selection is a no-op");
    let st = status_of(&state, &id);
    assert!(st.staged.is_empty(), "no-op must not stage anything");

    let err = tauri::async_runtime::block_on(stage_partial_inner(
        &state,
        &id,
        "q.txt".into(),
        None,
        vec![sel(LineKind::Add, None, Some(999))],
    ))
    .expect_err("stale coordinate must error");
    match err {
        AppError::Other(m) => assert!(m.contains("stale"), "{m}"),
        other => panic!("expected Other(stale), got {other:?}"),
    }
}

/// unstage_partial moves one selected staged line back toward HEAD.
#[test]
fn unstage_partial_single_line_selection() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "r.txt", "l1\n", "seed r");

    std::fs::write(dir.path().join("r.txt"), "l1\nl2\nl3\n").expect("write");
    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["r.txt".into()])).expect("stage");
    assert!(status_of(&state, &id).unstaged.is_empty(), "fully staged");

    tauri::async_runtime::block_on(unstage_partial_inner(
        &state,
        &id,
        "r.txt".into(),
        None,
        vec![sel(LineKind::Add, None, Some(2))],
    ))
    .expect("unstage_partial");

    let st = status_of(&state, &id);
    assert!(paths(&st.staged).contains(&"r.txt"), "line 3 still staged");
    assert!(paths(&st.unstaged).contains(&"r.txt"), "line 2 back to unstaged");

    // Empty selection: no-op.
    tauri::async_runtime::block_on(unstage_partial_inner(&state, &id, "r.txt".into(), None, vec![]))
        .expect("empty selection is a no-op");
}

/// The wire path format is forward-slash only: a backslash-separated Windows
/// relpath is rejected up front (Other "invalid path"), as are `..` escapes.
/// (Adapted from "Windows seam proof": backslashes never reach libgit2.)
#[test]
fn stage_rejects_backslash_and_escaping_paths() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    std::fs::create_dir_all(dir.path().join("dir")).expect("mkdir");
    std::fs::write(dir.path().join("dir").join("f.txt"), "f\n").expect("write");

    for bad in ["dir\\f.txt", "../evil", "", "/abs", "C:/abs"] {
        let err =
            tauri::async_runtime::block_on(stage_inner(&state, &id, vec![bad.to_string()]))
                .expect_err("invalid wire path must error");
        match err {
            AppError::Other(m) => assert!(m.contains("invalid path"), "{bad}: {m}"),
            other => panic!("{bad}: expected Other(invalid path), got {other:?}"),
        }
    }
    // The forward-slash form of the same file stages fine.
    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["dir/f.txt".into()]))
        .expect("forward-slash path stages");
}

/// Adapted: staging a valid-shaped path that exists nowhere (worktree or
/// index) is a SILENT NO-OP, not an error — a missing worktree path routes to
/// `index.remove_path` (the "stage a deletion" seam), and libgit2's
/// remove-by-path is remove-if-present. Assert it neither errors nor stages
/// anything.
#[test]
fn stage_nonexistent_path_is_a_noop() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["no/such/file.txt".into()]))
        .expect("missing path is a no-op, never a panic");
    let st = status_of(&state, &id);
    assert!(
        st.staged.is_empty() && st.unstaged.is_empty() && st.untracked.is_empty(),
        "no-op must not touch the index: {st:?}"
    );
}

/// A foreign `.git/index.lock` blocks the index write with a clean AppError;
/// the lock file is NOT deleted by us, and the operation succeeds once the
/// lock is gone. (Adapted: `commit` only reads the index — `write_tree` needs
/// no index lock — so the contended write is `stage`.)
#[test]
fn index_lock_contention_is_clean_error() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    std::fs::write(dir.path().join("l.txt"), "l\n").expect("write");

    let lock = dir.path().join(".git").join("index.lock");
    std::fs::write(&lock, "").expect("plant lock");

    let err = tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["l.txt".into()]))
        .expect_err("locked index must fail the stage");
    assert!(matches!(err, AppError::Git(_) | AppError::Io(_)), "{err:?}");
    assert!(lock.exists(), "we must not delete a foreign lock");

    std::fs::remove_file(&lock).expect("remove lock");
    tauri::async_runtime::block_on(stage_inner(&state, &id, vec!["l.txt".into()]))
        .expect("stage succeeds after the lock is gone");
    tauri::async_runtime::block_on(commit_inner(&state, &id, "after lock".into(), None, None))
        .expect("commit succeeds after the lock is gone");
}
