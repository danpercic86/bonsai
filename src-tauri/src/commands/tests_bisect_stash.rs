//! T2 Area 1 — bisect + stash command inners, runtime-free.

use super::tests_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

/// Linear history c0..c4 on the default branch. Returns oids oldest-first.
fn linear_history(state: &AppState, id: &str, dir: &std::path::Path, c0: String) -> Vec<String> {
    let mut oids = vec![c0];
    for i in 1..=4 {
        let c = write_stage_commit(state, id, dir, "a.txt", &format!("v{i}\n"), &format!("C{i}"));
        oids.push(c.oid);
    }
    oids
}

fn is_detached(dir: &std::path::Path) -> bool {
    git2::Repository::open(dir).expect("open").head_detached().expect("detached?")
}

/// `commit` is bad iff it IS the culprit or a descendant of it.
fn is_bad(dir: &std::path::Path, commit: &str, culprit: &str) -> bool {
    if commit == culprit {
        return true;
    }
    let repo = git2::Repository::open(dir).expect("open");
    repo.graph_descendant_of(
        git2::Oid::from_str(commit).unwrap(),
        git2::Oid::from_str(culprit).unwrap(),
    )
    .expect("descendant_of")
}

/// start_bisect detaches HEAD at the midpoint and reports Testing; op state is
/// Bisect.
#[test]
fn start_bisect_happy_detaches_at_midpoint() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let oids = linear_history(&state, &id, dir.path(), c0);

    let out = block_on(start_bisect_inner(&state, &id, oids[4].clone(), vec![oids[0].clone()]))
        .expect("start");
    match &out {
        BisectOutcome::Testing { current, revisions_remaining, .. } => {
            assert!(
                oids[1..4].contains(current),
                "midpoint must be strictly between good and bad, got {current}"
            );
            assert!(*revisions_remaining > 0);
            assert!(is_detached(dir.path()), "HEAD detached at midpoint");
            assert_eq!(head_oid(dir.path()), *current);
        }
        other => panic!("expected Testing, got {other:?}"),
    }
    assert!(matches!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::Bisect { .. }
    ));
    block_on(bisect_reset_inner(&state, &id)).expect("cleanup");
}

/// good == bad is rejected up front; starting during a paused merge is
/// OperationInProgress.
#[test]
fn start_bisect_guards() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let oids = linear_history(&state, &id, dir.path(), c0);

    let err = block_on(start_bisect_inner(&state, &id, oids[2].clone(), vec![oids[2].clone()]))
        .expect_err("good == bad must error");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");

    // Pause a conflicting merge, then try to bisect.
    let main = head_branch(dir.path()).expect("head");
    block_on(create_branch_here_inner(&state, &id, "side".into(), oids[4].clone()))
        .expect("branch");
    write_stage_commit(&state, &id, dir.path(), "a.txt", "side\n", "side edit");
    block_on(checkout_branch_inner(&state, &id, main)).expect("back");
    write_stage_commit(&state, &id, dir.path(), "a.txt", "main2\n", "main edit");
    let out = block_on(merge_branch_inner(&state, &id, "side".into())).expect("merge");
    assert!(matches!(out, MergeOutcome::Conflicts { .. }), "{out:?}");

    let err = block_on(start_bisect_inner(&state, &id, oids[4].clone(), vec![oids[0].clone()]))
        .expect_err("mid-merge bisect must be refused");
    assert!(matches!(err, AppError::OperationInProgress(_)), "{err:?}");
    block_on(abort_merge_inner(&state, &id)).expect("cleanup abort");
}

/// Marking good/bad converges on the exact culprit commit.
#[test]
fn bisect_mark_converges_on_culprit() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let oids = linear_history(&state, &id, dir.path(), c0);
    let culprit = oids[3].clone();

    let mut out = block_on(start_bisect_inner(&state, &id, oids[4].clone(), vec![oids[0].clone()]))
        .expect("start");
    let mut steps = 0;
    let verdict = loop {
        match out {
            BisectOutcome::Testing { ref current, .. } => {
                steps += 1;
                assert!(steps <= 10, "bisect did not converge");
                let good = !is_bad(dir.path(), current, &culprit);
                out = block_on(bisect_mark_inner(&state, &id, good)).expect("mark");
            }
            BisectOutcome::Found { first_bad } => break first_bad,
            other => panic!("unexpected outcome {other:?}"),
        }
    };
    assert_eq!(verdict, culprit, "first-bad verdict must be the culprit");
    block_on(bisect_reset_inner(&state, &id)).expect("cleanup");
}

/// bisect_skip moves off the current midpoint without marking it.
#[test]
fn bisect_skip_moves_off_current() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let oids = linear_history(&state, &id, dir.path(), c0);

    let out = block_on(start_bisect_inner(&state, &id, oids[4].clone(), vec![oids[0].clone()]))
        .expect("start");
    let first = match out {
        BisectOutcome::Testing { current, .. } => current,
        other => panic!("expected Testing, got {other:?}"),
    };
    let out = block_on(bisect_skip_inner(&state, &id)).expect("skip");
    match out {
        BisectOutcome::Testing { current, .. } => {
            assert_ne!(current, first, "skip must check out a different candidate")
        }
        BisectOutcome::Found { .. } | BisectOutcome::CannotDetermine { .. } => {}
    }
    block_on(bisect_reset_inner(&state, &id)).expect("cleanup");
}

/// bisect_reset restores the original branch + tip; resetting with no bisect
/// in progress is NoOperationInProgress.
#[test]
fn bisect_reset_restores_branch() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let oids = linear_history(&state, &id, dir.path(), c0);
    let main = head_branch(dir.path()).expect("head");

    block_on(start_bisect_inner(&state, &id, oids[4].clone(), vec![oids[0].clone()]))
        .expect("start");
    assert!(is_detached(dir.path()));

    block_on(bisect_reset_inner(&state, &id)).expect("reset");
    assert_eq!(head_branch(dir.path()).as_deref(), Some(main.as_str()));
    assert_eq!(head_oid(dir.path()), oids[4], "tip restored");
    assert_eq!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::None
    );

    let err = block_on(bisect_reset_inner(&state, &id)).expect_err("no bisect in progress");
    assert!(matches!(err, AppError::NoOperationInProgress(_)), "{err:?}");
}

/// Full stash chain: create (incl. untracked) → list → apply (retained) →
/// second create → drop → pop.
#[test]
fn stash_chain_create_list_apply_drop_pop() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    std::fs::write(dir.path().join("a.txt"), "edited\n").expect("write");
    std::fs::write(dir.path().join("u.txt"), "untracked\n").expect("write");
    let res = block_on(create_stash_inner(
        &state,
        &id,
        Some("my stash".into()),
        StashScope::AllWithUntracked,
    ))
    .expect("create stash");
    assert!(res.created);
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "base\n");
    assert!(!dir.path().join("u.txt").exists(), "untracked captured too");

    let list = block_on(list_stashes_inner(&state, &id)).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].index, 0);
    assert!(list[0].message.contains("my stash"), "{}", list[0].message);

    // Apply keeps the entry.
    let out = block_on(apply_stash_inner(&state, &id, 0, false, None)).expect("apply");
    assert_eq!(out, ApplyStashOutcome::Applied);
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "edited\n");
    assert_eq!(std::fs::read_to_string(dir.path().join("u.txt")).unwrap(), "untracked\n");
    assert_eq!(block_on(list_stashes_inner(&state, &id)).expect("list").len(), 1);

    // Stash the re-applied changes again → two entries; drop the newest.
    block_on(create_stash_inner(&state, &id, Some("second".into()), StashScope::AllWithUntracked))
        .expect("second stash");
    assert_eq!(block_on(list_stashes_inner(&state, &id)).expect("list").len(), 2);
    block_on(drop_stash_inner(&state, &id, 0, None)).expect("drop");
    let list = block_on(list_stashes_inner(&state, &id)).expect("list");
    assert_eq!(list.len(), 1);
    assert!(list[0].message.contains("my stash"), "index shifted: {}", list[0].message);

    // Pop the survivor cleanly → empty stack, changes in the worktree.
    let out = block_on(pop_stash_inner(&state, &id, 0, false, None)).expect("pop");
    assert_eq!(out, ApplyStashOutcome::Applied);
    assert!(block_on(list_stashes_inner(&state, &id)).expect("list").is_empty());
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "edited\n");
}

/// create_stash on a clean tree is created:false (not an error); drop with a
/// huge index is a clean Git error.
#[test]
fn stash_clean_tree_and_bad_index() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    let res = block_on(create_stash_inner(&state, &id, None, StashScope::All))
        .expect("clean tree must not error");
    assert!(!res.created, "created:false on a clean tree");
    assert!(block_on(list_stashes_inner(&state, &id)).expect("list").is_empty());

    let err = block_on(drop_stash_inner(&state, &id, usize::MAX, None)).expect_err("huge index");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
}

/// pop with conflicting worktree/HEAD content reports Conflicts and RETAINS
/// the stash entry.
#[test]
fn pop_with_conflict_retains_stash() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    std::fs::write(dir.path().join("a.txt"), "stashed\n").expect("write");
    let res = block_on(create_stash_inner(&state, &id, None, StashScope::All)).expect("stash");
    assert!(res.created);

    // Commit a DIFFERENT change to the same line so the pop conflicts.
    write_stage_commit(&state, &id, dir.path(), "a.txt", "committed\n", "diverge");

    let out = block_on(pop_stash_inner(&state, &id, 0, false, None)).expect("pop");
    match out {
        ApplyStashOutcome::Conflicts { paths } => assert_eq!(paths, vec!["a.txt".to_string()]),
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert_eq!(
        block_on(list_stashes_inner(&state, &id)).expect("list").len(),
        1,
        "conflicted pop must RETAIN the stash"
    );
}
