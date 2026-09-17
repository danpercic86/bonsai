//! P34 `staged` scope rows — the data-safety-critical `create_staged_stash`
//! FOLD path (cases 4..=13) plus its check-in-filter audit. Extracted verbatim
//! from the former inline `mod tests`; continues the P34 matrix banner in
//! `tests_scopes`, shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;

// ---- Case 4: pure-staged modify (the core new path) -----------------------

#[test]
fn p34_staged_pure_modify_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]); // staged, no further unstaged edit

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "staged change must stash");

    // A reverts to HEAD; index == HEAD; one entry.
    assert_eq!(s9_read(d, "a.txt"), "base\n", "worktree reverts to HEAD");
    p34_assert_index_clean(d);
    assert_eq!(
        p34_unstaged_paths(d),
        Vec::<String>::new(),
        "no residual unstaged change"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    // Pop restores the staged content as an UNSTAGED edit (F-1: no reinstate).
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(s9_read(d, "a.txt"), "a-staged\n", "staged content restored");
    p34_assert_index_clean(d);
    assert_eq!(
        p34_unstaged_paths(d),
        vec!["a.txt".to_string()],
        "restored as UNSTAGED, not re-staged"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
}

// ---- Case 5: mixed file FOLD (orchestrator override) ----------------------

#[test]
fn p34_staged_mixed_file_folds_whole() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("b.txt", "base\n")]);

    std::fs::write(d.join("b.txt"), "b-staged\n").expect("stage-edit b");
    p34_stage(d, &["b.txt"]);
    // Further UNSTAGED edit on the SAME path → mixed file.
    std::fs::write(d.join("b.txt"), "b-staged-then-unstaged\n").expect("unstage-edit b");

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "mixed file must fold, not reject");

    // FOLD: B reverts to HEAD, index clean; the FULL worktree content is held.
    assert_eq!(s9_read(d, "b.txt"), "base\n", "b reverted to HEAD");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(
        s9_read(d, "b.txt"),
        "b-staged-then-unstaged\n",
        "stash held the FULL folded worktree content"
    );
}

// ---- Case 6: unstaged-only path is preserved ------------------------------

#[test]
fn p34_staged_preserves_unstaged_only_path() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("c.txt", "c-base\n")]);

    // a.txt staged; c.txt unstaged-only (index == HEAD).
    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("c.txt"), "c-unstaged\n").expect("edit c");

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created);

    assert_eq!(s9_read(d, "a.txt"), "base\n", "staged a reverted");
    assert_eq!(
        s9_read(d, "c.txt"),
        "c-unstaged\n",
        "unstaged-only c must NOT be stashed"
    );
    p34_assert_index_clean(d);
    assert_eq!(
        p34_unstaged_paths(d),
        vec!["c.txt".to_string()],
        "c remains an unstaged change"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1);
}

// ---- Case 7: untracked file is preserved ----------------------------------

#[test]
fn p34_staged_preserves_untracked() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created);

    assert!(
        d.join("u.txt").exists(),
        "untracked survives a staged stash"
    );
    assert_eq!(
        s9_read(d, "u.txt"),
        "untracked\n",
        "untracked content intact"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1);
}

// ---- Case 8: staged ADD ---------------------------------------------------

#[test]
fn p34_staged_add_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("new.txt"), "new\n").expect("write new");
    p34_stage(d, &["new.txt"]); // staged add

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "staged add must stash");

    assert!(
        !d.join("new.txt").exists(),
        "staged add removed from worktree after stash"
    );
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(d.join("new.txt").exists(), "pop restores the added file");
    assert_eq!(s9_read(d, "new.txt"), "new\n", "added content restored");
}

// ---- Case 9: staged DELETE (real worktree deletion) -----------------------

#[test]
fn p34_staged_delete_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("del.txt", "gone\n")]);

    std::fs::remove_file(d.join("del.txt")).expect("rm del");
    p34_stage(d, &["del.txt"]); // stages the deletion (file absent on disk)

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "staged deletion must stash");

    // The staged deletion is reverted → file back on disk == HEAD; index clean.
    assert!(
        d.join("del.txt").exists(),
        "staged deletion reverted: file restored to worktree"
    );
    assert_eq!(s9_read(d, "del.txt"), "gone\n", "restored to HEAD content");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(
        !d.join("del.txt").exists(),
        "pop reintroduces the staged deletion"
    );
}

// ---- Case 10: `git rm --cached` (staged deletion, file still on disk) ------

#[test]
fn p34_staged_rm_cached_deletion_captured() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("rm.txt", "content\n")]);

    p34_rm_cached(d, "rm.txt"); // index deletion; file STAYS on disk w/ HEAD content
    assert_eq!(
        p34_staged_paths(d),
        vec!["rm.txt".to_string()],
        "precondition: rm.txt is a staged deletion"
    );

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(
        res.created,
        "the staged deletion must be captured, not silently dropped"
    );

    // After stash: staged deletion reverted; file present == HEAD; index clean.
    assert!(d.join("rm.txt").exists(), "file back on disk after revert");
    assert_eq!(s9_read(d, "rm.txt"), "content\n");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    // Pop reintroduces the deletion (proves the deletion was in the entry).
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(
        !d.join("rm.txt").exists(),
        "pop reintroduces the staged deletion"
    );
}

// ---- Case 11: STACKING native + staged (no double-log regression) ---------

#[test]
fn p34_stacking_native_then_staged_no_double_log() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "a-base\n"), ("b.txt", "b-base\n")]);

    // 1) Native stash (All) of an unstaged edit to a.txt.
    std::fs::write(d.join("a.txt"), "a-native\n").expect("edit a");
    let r1 = create_stash(d, Some("native-stash"), StashScope::All).expect("native");
    assert!(r1.created);

    // 2) Staged stash of b.txt.
    std::fs::write(d.join("b.txt"), "b-staged\n").expect("edit b");
    p34_stage(d, &["b.txt"]);
    let r2 = create_stash(d, Some("staged-stash"), StashScope::Staged).expect("staged");
    assert!(r2.created);

    // Exactly TWO entries — the hand-rolled push must not double-log.
    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 2, "stacking must yield 2 entries, not 3");
    assert!(
        list[0].message.contains("staged-stash"),
        "stash@{{0}} is the staged one, got {:?}",
        list[0].message
    );
    assert!(
        list[1].message.contains("native-stash"),
        "stash@{{1}} is the older native one, got {:?}",
        list[1].message
    );
    let native_oid = list[1].oid.clone();

    // drop@{0} leaves the native survivor intact and re-indexed to 0.
    drop_stash(d, 0, None).expect("drop 0");
    let after = list_stashes(d).expect("list after drop");
    assert_eq!(after.len(), 1, "native survivor remains");
    assert_eq!(after[0].index, 0, "re-indexed to 0");
    assert_eq!(after[0].oid, native_oid, "survivor is the native entry");
    assert!(after[0].message.contains("native-stash"));

    // apply-by-index resolves the survivor (restores a.txt's native edit).
    let outcome = apply_stash(d, 0, false, None).expect("apply survivor");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(s9_read(d, "a.txt"), "a-native\n", "native edit re-applied");
}

// ---- Case 12: `Staged` with nothing staged (unstaged present) -------------

#[test]
fn p34_staged_nothing_staged_but_unstaged_present() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-unstaged\n").expect("edit a"); // unstaged only

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(!res.created, "nothing staged -> created:false");
    assert_eq!(
        s9_read(d, "a.txt"),
        "a-unstaged\n",
        "unstaged change untouched"
    );
    assert_eq!(
        p34_unstaged_paths(d),
        vec!["a.txt".to_string()],
        "still an unstaged change"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no entry");
}

// ---- Case 13: `Staged` rejected mid-merge (require_clean guard) -----------

#[test]
fn p34_staged_rejected_mid_merge() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);

    s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
    let base = repo
        .find_commit(repo.head().expect("HEAD").target().expect("oid"))
        .expect("base");
    s9_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "topic\n")],
        "topic edits x",
    );
    s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

    // Dirty unrelated file → conflicting merge pauses in Merge state.
    std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");
    crate::git::merge::merge_branch(d, "topic", false).expect("merge");
    assert_eq!(
        git2::Repository::open(d).expect("reopen").state(),
        git2::RepositoryState::Merge,
        "precondition: mid-merge"
    );

    let before = list_stashes(d).expect("list before");
    match create_stash(d, None, StashScope::Staged) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("expected OperationInProgress, got {other:?}"),
    }
    assert_eq!(
        list_stashes(d).expect("list after").len(),
        before.len(),
        "rejected create must not mutate the stash stack"
    );
}

// ===================================================== audit 2026-08-07

/// §2.2: `create_staged_stash` must fold CHECK-IN FILTERED worktree bytes.
/// Under `core.autocrlf=true` a CRLF worktree file must land in the stash
/// tree as an LF blob (what `git add` would stage), never raw CRLF.
#[test]
fn staged_stash_folds_filtered_worktree_bytes_under_autocrlf() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    repo.config()
        .expect("config")
        .set_bool("core.autocrlf", true)
        .expect("autocrlf");
    s9_commit(d, "base", &[("f.txt", "one\r\n")]); // blob is LF via filter

    // Stage a CRLF modification, then edit the worktree AGAIN (CRLF) so the
    // stash fold has fresher worktree content than the staged blob.
    std::fs::write(d.join("f.txt"), "one\r\ntwo\r\n").expect("edit");
    crate::git::stage::stage_paths(d, &["f.txt".to_string()]).expect("stage");
    std::fs::write(d.join("f.txt"), "one\r\ntwo\r\nthree\r\n").expect("edit again");

    let res = create_stash(d, None, StashScope::Staged).expect("staged stash");
    assert!(res.created);

    let entries = list_stashes(d).expect("list");
    let repo2 = git2::Repository::open(d).expect("open");
    let tree = repo2
        .find_commit(git2::Oid::from_str(&entries[0].oid).expect("oid"))
        .expect("stash commit")
        .tree()
        .expect("stash tree");
    let blob_id = tree.get_name("f.txt").expect("f.txt in stash tree").id();
    let content = repo2.find_blob(blob_id).expect("blob").content().to_vec();
    assert_eq!(
        content, b"one\ntwo\nthree\n",
        "stash tree blob must be LF-only (check-in filtered)"
    );
}
