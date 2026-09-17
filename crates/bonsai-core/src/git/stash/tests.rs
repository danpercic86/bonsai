//! Wire shapes plus the P9 §8 behavioral stash matrix (rows 1..=7). The P34
//! scope rows live in `tests_scopes` / `tests_staged`, the apply-safety rows in
//! `tests_apply`, and the shared fixtures in `test_support`.

use super::test_support::*;
use super::*;

/// Wire shapes (P9 §8 test 8): serde tag/casing must match the TS mirrors
/// (ApplyStashOutcome union + StashEntry camelCase).
#[test]
fn wire_shapes_are_camel_case_tagged() {
    let v = serde_json::to_value(ApplyStashOutcome::Applied).expect("json");
    assert_eq!(v, serde_json::json!({ "kind": "applied" }));

    let v = serde_json::to_value(ApplyStashOutcome::Conflicts {
        paths: vec!["src/app.ts".to_string(), "README.md".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "conflicts", "paths": ["src/app.ts", "README.md"] })
    );

    let v = serde_json::to_value(StashEntry {
        index: 0,
        message: "WIP on main: 1a2b3c4 summary".to_string(),
        oid: "a".repeat(40),
        base_oid: "b".repeat(40),
        ts: 1_700_000_000,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "index": 0,
            "message": "WIP on main: 1a2b3c4 summary",
            "oid": "a".repeat(40),
            "baseOid": "b".repeat(40),
            "ts": 1_700_000_000
        })
    );

    let v = serde_json::to_value(ApplyStashOutcome::ReservedPaths {
        paths: vec!["src/Aspire.AppHost/NUL".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "reservedPaths", "paths": ["src/Aspire.AppHost/NUL"] })
    );

    let v = serde_json::to_value(ApplyStashOutcome::AppliedSkippingReserved {
        skipped: vec!["src/Aspire.AppHost/NUL".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "appliedSkippingReserved", "skipped": ["src/Aspire.AppHost/NUL"] })
    );

    let v = serde_json::to_value(CreateStashResult { created: true }).expect("json");
    assert_eq!(v, serde_json::json!({ "created": true }));
}

// ============================================================ P9 §8 matrix
// Behavioral stash matrix (one test per §8 row, 1..=7). Each asserts BOTH the
// returned outcome AND the on-disk state via a scratch repo, echoing the
// merge.rs (P8) fixtures. Fixtures are built with git2 — deterministic, no
// network, no CLI.

// ---- Row 1: Round-trip (apply does NOT drop) ---------------------------

#[test]
fn s9_1_round_trip_apply_keeps_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let head = s9_head_oid(d);

    // Dirty: edit a tracked file (unstaged).
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(res.created, "dirty tracked edit must stash");

    // Worktree returned to the committed state (stash reset both index+worktree).
    assert_eq!(
        s9_read(d, "a.txt"),
        "base\n",
        "worktree must be clean after stash"
    );

    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 1, "one entry on the stack");
    assert_eq!(list[0].index, 0);
    assert_eq!(
        list[0].base_oid, head,
        "base_oid must == HEAD at stash time"
    );
    assert!(
        !list[0].message.is_empty(),
        "default message must be non-empty"
    );

    let outcome = apply_stash(d, 0, false, None).expect("apply");
    assert_eq!(outcome, ApplyStashOutcome::Applied, "clean apply");
    assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

    let list = list_stashes(d).expect("list after apply");
    assert_eq!(list.len(), 1, "apply must NOT drop the stash");
}

// ---- Row 2: Pop drops --------------------------------------------------

#[test]
fn s9_2_pop_drops_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "edited\n").expect("edit");
    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(res.created);
    assert_eq!(s9_read(d, "a.txt"), "base\n");

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied, "clean pop");
    assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

    let list = list_stashes(d).expect("list after pop");
    assert_eq!(list.len(), 0, "pop must drop the stash on clean apply");
}

// ---- Row 3: Nothing to stash -------------------------------------------

#[test]
fn s9_3_nothing_to_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    // Clean tree.
    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(!res.created, "clean tree -> created:false, NOT an error");

    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 0, "nothing pushed onto the stack");
}

// ---- Row 4: Include untracked ------------------------------------------

#[test]
fn s9_4_include_untracked_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    // An untracked file present.
    std::fs::write(d.join("untracked.txt"), "new\n").expect("write untracked");

    let res = create_stash(d, None, StashScope::AllWithUntracked)
        .expect("create_stash include_untracked");
    assert!(
        res.created,
        "untracked file must stash under include_untracked"
    );
    assert!(
        !d.join("untracked.txt").exists(),
        "include_untracked must remove the untracked file from the worktree"
    );

    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(
        d.join("untracked.txt").exists(),
        "pop must restore the stashed untracked file"
    );
    assert_eq!(
        s9_read(d, "untracked.txt"),
        "new\n",
        "untracked content restored"
    );
    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "clean pop drops entry"
    );
}

// ---- Row 5: Pop conflict retains (P8-lesson data safety) + apply variant

/// Build a repo where popping/applying stash@{0} necessarily conflicts on X:
/// base has X="base"; stash records X="stashed"; then HEAD advances X="head".
/// Returns the scratch dir (kept alive by the caller).
fn s9_conflict_fixture() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("x.txt", "base\n")]);

    // Stash an unstaged edit to X.
    std::fs::write(d.join("x.txt"), "stashed\n").expect("edit x");
    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(res.created);
    assert_eq!(s9_read(d, "x.txt"), "base\n", "worktree reset after stash");

    // Advance HEAD with a DIFFERENT change to the same file -> guaranteed
    // 3-way conflict on re-apply (base=base, ours=head, theirs=stashed).
    s9_commit(d, "head change", &[("x.txt", "head\n")]);
    dir
}

#[test]
fn s9_5_pop_conflict_retains_stash() {
    let dir = s9_conflict_fixture();
    let d = dir.path();

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(
        outcome,
        ApplyStashOutcome::Conflicts {
            paths: vec!["x.txt".to_string()]
        },
        "conflicting pop must report Conflicts on x.txt"
    );

    // No merge started; only the index carries conflict entries.
    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "stash apply must not enter a Merge state"
    );

    assert!(
        s9_read(d, "x.txt").contains("<<<<<<<"),
        "worktree X must carry conflict markers"
    );

    let list = list_stashes(d).expect("list after conflicting pop");
    assert_eq!(
        list.len(),
        1,
        "DATA SAFETY: a conflicting pop must RETAIN the stash (never lossy)"
    );
}

#[test]
fn s9_5b_apply_conflict_retains_stash() {
    let dir = s9_conflict_fixture();
    let d = dir.path();

    let outcome = apply_stash(d, 0, false, None).expect("apply");
    assert_eq!(
        outcome,
        ApplyStashOutcome::Conflicts {
            paths: vec!["x.txt".to_string()]
        },
        "conflicting apply must report Conflicts on x.txt"
    );

    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    assert!(s9_read(d, "x.txt").contains("<<<<<<<"));

    let list = list_stashes(d).expect("list after conflicting apply");
    assert_eq!(
        list.len(),
        1,
        "apply never drops; stash retained on conflict"
    );
}

// ---- Row 6: Drop re-indexes the stack ----------------------------------

#[test]
fn s9_6_drop_reindexes_stack() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    // Stash A (becomes stash@{1} after the second push).
    std::fs::write(d.join("a.txt"), "edit-A\n").expect("edit A");
    create_stash(d, Some("stash-A"), StashScope::All).expect("stash A");
    // Stash B (most recent, stash@{0}).
    std::fs::write(d.join("a.txt"), "edit-B\n").expect("edit B");
    create_stash(d, Some("stash-B"), StashScope::All).expect("stash B");

    let before = list_stashes(d).expect("list before drop");
    assert_eq!(before.len(), 2);
    assert!(
        before[0].message.contains("stash-B"),
        "stash@{{0}} is the newest"
    );
    assert!(
        before[1].message.contains("stash-A"),
        "stash@{{1}} is the oldest"
    );
    let survivor_oid = before[1].oid.clone();

    // Drop the most recent (index 0). Entries above shift down by one.
    drop_stash(d, 0, None).expect("drop");

    let after = list_stashes(d).expect("list after drop");
    assert_eq!(after.len(), 1, "one entry survives");
    assert_eq!(
        after[0].index, 0,
        "surviving entry re-indexed to 0 (§2.4 shift)"
    );
    assert!(
        after[0].message.contains("stash-A"),
        "the survivor is the entry previously at index 1 (stash-A)"
    );
    assert_eq!(
        after[0].oid, survivor_oid,
        "survivor identity confirmed by oid"
    );
}

// ---- Row 7: Op-state guard (Merge in progress) -------------------------

#[test]
fn s9_7_op_state_guard_blocks_all_but_drop() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);

    s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
    let base = repo
        .find_commit(repo.head().expect("HEAD").target().expect("oid"))
        .expect("base");
    // topic diverges on x.txt.
    s9_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "topic\n")],
        "topic edits x",
    );
    // main diverges on x.txt (guaranteed conflict).
    s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

    // Dirty unrelated file -> merge autostashes it, then pauses in Merge state.
    // This leaves BOTH a Merge state AND a retained stash on the stack, so we
    // can exercise the guard AND prove drop still works.
    std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");

    crate::git::merge::merge_branch(d, "topic", false).expect("merge");
    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Merge,
        "conflicting merge over a dirty tree must pause in Merge state"
    );
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "the retained autostash gives us something to drop"
    );

    // create/apply/pop are all rejected while an operation is in progress.
    match create_stash(d, None, StashScope::All) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("create_stash: expected OperationInProgress, got {other:?}"),
    }
    match apply_stash(d, 0, false, None) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("apply_stash: expected OperationInProgress, got {other:?}"),
    }
    match pop_stash(d, 0, false, None) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("pop_stash: expected OperationInProgress, got {other:?}"),
    }

    // Drop is allowed in ANY repo state (touches only the stash reflog).
    drop_stash(d, 0, None).expect("drop must succeed mid-merge");
    assert_eq!(
        list_stashes(d).expect("list after drop").len(),
        0,
        "drop removed the autostash even though a merge is in progress"
    );
}
