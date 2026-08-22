//! T2 Area 1 — merge + conflict + rebase (plain & interactive) command inners,
//! runtime-free. Fixtures: divergent branches built through the command layer.

use super::tests_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

fn read(dir: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).expect("read file")
}

/// C0(a.txt "base") ← main; branch "feature" from C0. `conflict:true` → both
/// sides edit a.txt; else they touch different files. Ends checked out on
/// main. Returns (main_name, feature_tip_oid, main_tip_oid).
fn diverge(
    state: &AppState,
    id: &str,
    dir: &std::path::Path,
    conflict: bool,
) -> (String, String, String) {
    let main = head_branch(dir).expect("attached head");
    let c0 = head_oid(dir);
    block_on(create_branch_here_inner(state, id, "feature".into(), c0)).expect("branch feature");
    let f_tip = if conflict {
        write_stage_commit(state, id, dir, "a.txt", "feature\n", "feature edit").oid
    } else {
        write_stage_commit(state, id, dir, "f.txt", "f\n", "feature file").oid
    };
    block_on(checkout_branch_inner(state, id, main.clone())).expect("back to main");
    let m_tip = if conflict {
        write_stage_commit(state, id, dir, "a.txt", "main\n", "main edit").oid
    } else {
        write_stage_commit(state, id, dir, "m.txt", "m\n", "main file").oid
    };
    (main, f_tip, m_tip)
}

/// get_op_state: None on a quiet repo; Merge{incoming} while a conflicted
/// merge is paused.
#[test]
fn op_state_none_and_merge() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    assert_eq!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::None
    );

    diverge(&state, &id, dir.path(), true);
    let out = block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");
    assert!(matches!(out, MergeOutcome::Conflicts { .. }), "{out:?}");

    match block_on(get_op_state_inner(&state, &id)).expect("op state") {
        RepoOpState::Merge { incoming, message } => {
            assert_eq!(incoming, "feature");
            assert!(message.starts_with("Merge branch 'feature'"), "{message}");
        }
        other => panic!("expected Merge, got {other:?}"),
    }
    block_on(abort_merge_inner(&state, &id)).expect("cleanup abort");
}

/// Clean divergent merge auto-commits a 2-parent merge commit.
#[test]
fn merge_branch_clean_happy() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let (_main, f_tip, m_tip) = diverge(&state, &id, dir.path(), false);

    let out = block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");
    let oid = match out {
        MergeOutcome::Merged { oid, stashed } => {
            assert!(!stashed);
            oid
        }
        other => panic!("expected Merged, got {other:?}"),
    };
    assert_eq!(head_oid(dir.path()), oid);
    let repo = git2::Repository::open(dir.path()).expect("open");
    let mc = repo.find_commit(git2::Oid::from_str(&oid).unwrap()).unwrap();
    assert_eq!(mc.parent_count(), 2);
    assert_eq!(mc.parent_id(0).unwrap().to_string(), m_tip);
    assert_eq!(mc.parent_id(1).unwrap().to_string(), f_tip);
    assert!(dir.path().join("f.txt").exists() && dir.path().join("m.txt").exists());
}

/// Conflicted merge pauses; commit_merge is blocked while unresolved; the
/// conflict trio (list/get) exposes ours/theirs; abort restores and a second
/// abort errors.
#[test]
fn merge_conflict_pause_trio_and_abort() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    diverge(&state, &id, dir.path(), true);

    let out = block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");
    match &out {
        MergeOutcome::Conflicts { paths, stashed } => {
            assert_eq!(paths, &vec!["a.txt".to_string()]);
            assert!(!stashed);
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }

    // commit_merge with unresolved conflicts → UnresolvedConflicts.
    let err = block_on(commit_merge_inner(&state, &id, "merge msg".into(), None, None))
        .expect_err("unresolved must block");
    assert!(matches!(err, AppError::UnresolvedConflicts(_)), "{err:?}");

    // list_conflicts / get_conflict content trio.
    let entries = block_on(list_conflicts_inner(&state, &id)).expect("list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "a.txt");
    assert!(entries[0].has_ours && entries[0].has_theirs && entries[0].has_base);
    let file = block_on(get_conflict_inner(&state, &id, "a.txt".into())).expect("get");
    assert!(!file.binary && !file.too_large && !file.missing);
    assert!(file.text.contains("<<<<<<<") && file.text.contains(">>>>>>>"), "{}", file.text);
    assert_eq!(file.ours, "main\n");
    assert_eq!(file.theirs, "feature\n");

    // Abort restores the pre-merge worktree + state; a second abort errors.
    block_on(abort_merge_inner(&state, &id)).expect("abort");
    assert_eq!(read(dir.path(), "a.txt"), "main\n");
    assert_eq!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::None
    );
    let err = block_on(abort_merge_inner(&state, &id)).expect_err("second abort");
    assert!(matches!(err, AppError::NoOperationInProgress(_)), "{err:?}");
}

/// resolve_conflict Ours / Theirs picks the corresponding side, then
/// commit_merge lands the 2-parent commit.
#[test]
fn resolve_conflict_ours_then_theirs() {
    // OURS.
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    diverge(&state, &id, dir.path(), true);
    block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");
    block_on(resolve_conflict_inner(&state, &id, "a.txt".into(), ConflictResolution::Ours))
        .expect("resolve ours");
    assert_eq!(read(dir.path(), "a.txt"), "main\n");
    assert!(block_on(list_conflicts_inner(&state, &id)).expect("list").is_empty());
    let res = block_on(commit_merge_inner(&state, &id, "merged (ours)".into(), None, None))
        .expect("commit merge");
    let repo = git2::Repository::open(dir.path()).expect("open");
    let mc = repo.find_commit(git2::Oid::from_str(&res.oid).unwrap()).unwrap();
    assert_eq!(mc.parent_count(), 2);
    assert_eq!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::None
    );

    // THEIRS (fresh repo).
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    diverge(&state, &id, dir.path(), true);
    block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");
    block_on(resolve_conflict_inner(&state, &id, "a.txt".into(), ConflictResolution::Theirs))
        .expect("resolve theirs");
    assert_eq!(read(dir.path(), "a.txt"), "feature\n");
    assert!(block_on(list_conflicts_inner(&state, &id)).expect("list").is_empty());
}

/// resolve_conflict_text stages hand-merged content; a path-traversal relpath
/// is InvalidName.
#[test]
fn resolve_conflict_text_happy_and_traversal() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    diverge(&state, &id, dir.path(), true);
    block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");

    let err = block_on(resolve_conflict_text_inner(
        &state,
        &id,
        "../evil".into(),
        "pwned\n".into(),
    ))
    .expect_err("traversal must be rejected");
    assert!(matches!(err, AppError::InvalidName(_)), "{err:?}");
    assert!(!dir.path().parent().unwrap().join("evil").exists(), "nothing written outside");

    block_on(resolve_conflict_text_inner(&state, &id, "a.txt".into(), "hand merged\n".into()))
        .expect("resolve text");
    assert_eq!(read(dir.path(), "a.txt"), "hand merged\n");
    assert!(block_on(list_conflicts_inner(&state, &id)).expect("list").is_empty());
    block_on(commit_merge_inner(&state, &id, "merged by hand".into(), None, None))
        .expect("commit merge");
}

/// P68 #7 / H1: ai_apply_resolution re-reads the sides and REFUSES a body with a
/// line present in no version (AiNeedsReview; nothing written, file stays
/// conflicted); a recombination of existing side lines writes through the single
/// `resolve_conflict_text` core writer.
#[test]
fn ai_apply_resolution_gates_novel_but_writes_clean() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    diverge(&state, &id, dir.path(), true);
    block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");

    // ours = "main\n", theirs = "feature\n", base = "base\n". A line in NO version
    // is refused server-side; the worktree keeps its markers and stays conflicted.
    let err = block_on(ai_apply_resolution_inner(
        &state,
        &id,
        "a.txt".into(),
        "totally invented line\n".into(),
    ))
    .expect_err("novel body must be gated");
    assert!(matches!(err, AppError::AiNeedsReview(_)), "{err:?}");
    assert!(read(dir.path(), "a.txt").contains("<<<<<<<"), "worktree stays conflicted");
    assert_eq!(block_on(list_conflicts_inner(&state, &id)).expect("list").len(), 1);

    // A recombination of existing side lines passes the gate and writes stage-0.
    block_on(ai_apply_resolution_inner(&state, &id, "a.txt".into(), "main\nfeature\n".into()))
        .expect("clean body writes");
    assert_eq!(read(dir.path(), "a.txt"), "main\nfeature\n");
    assert!(block_on(list_conflicts_inner(&state, &id)).expect("list").is_empty());
    block_on(commit_merge_inner(&state, &id, "merged by ai".into(), None, None))
        .expect("commit merge");
}

/// Clean rebase replays feature onto main (Rebased, 1 step, new parent = main
/// tip).
#[test]
fn rebase_branch_clean_happy() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let (main, f_tip, m_tip) = diverge(&state, &id, dir.path(), false);
    block_on(checkout_branch_inner(&state, &id, "feature".into())).expect("checkout feature");

    let out = block_on(rebase_branch_inner(&state, &id, main)).expect("rebase");
    match out {
        RebaseOutcome::Rebased { branch, head, steps, warnings } => {
            assert_eq!(branch, "feature");
            assert_eq!(steps, 1);
            assert!(warnings.is_empty());
            assert_ne!(head, f_tip, "commit rewritten");
            assert_eq!(head, head_oid(dir.path()));
            let repo = git2::Repository::open(dir.path()).expect("open");
            let hc = repo.find_commit(git2::Oid::from_str(&head).unwrap()).unwrap();
            assert_eq!(hc.parent_id(0).unwrap().to_string(), m_tip);
        }
        other => panic!("expected Rebased, got {other:?}"),
    }
}

/// Conflicted rebase pauses (Conflicts, step 1/1); resolve + continue finishes
/// it.
#[test]
fn rebase_conflict_then_continue() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let (main, _f_tip, m_tip) = diverge(&state, &id, dir.path(), true);
    block_on(checkout_branch_inner(&state, &id, "feature".into())).expect("checkout feature");

    let out = block_on(rebase_branch_inner(&state, &id, main)).expect("rebase");
    match &out {
        RebaseOutcome::Conflicts { paths, current_step, total_steps } => {
            assert_eq!(paths, &vec!["a.txt".to_string()]);
            assert_eq!((*current_step, *total_steps), (1, 1));
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert!(matches!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::Rebase { .. }
    ));

    block_on(resolve_conflict_text_inner(&state, &id, "a.txt".into(), "resolved\n".into()))
        .expect("resolve");
    let out = block_on(rebase_continue_inner(&state, &id)).expect("continue");
    match out {
        RebaseOutcome::Rebased { branch, head, .. } => {
            assert_eq!(branch, "feature");
            let repo = git2::Repository::open(dir.path()).expect("open");
            let hc = repo.find_commit(git2::Oid::from_str(&head).unwrap()).unwrap();
            assert_eq!(hc.parent_id(0).unwrap().to_string(), m_tip);
            assert_eq!(read(dir.path(), "a.txt"), "resolved\n");
        }
        other => panic!("expected Rebased, got {other:?}"),
    }
    assert_eq!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::None
    );
}

/// rebase_abort restores the pre-rebase branch tip; rebase ops with nothing in
/// progress are clean NoOperationInProgress errors.
#[test]
fn rebase_abort_and_no_op_errors() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    for op in ["continue", "skip", "abort"] {
        let err = match op {
            "continue" => block_on(rebase_continue_inner(&state, &id)).unwrap_err(),
            "skip" => block_on(rebase_skip_inner(&state, &id)).unwrap_err(),
            _ => block_on(rebase_abort_inner(&state, &id)).unwrap_err(),
        };
        assert!(matches!(err, AppError::NoOperationInProgress(_)), "{op}: {err:?}");
    }

    let (main, f_tip, _m_tip) = diverge(&state, &id, dir.path(), true);
    block_on(checkout_branch_inner(&state, &id, "feature".into())).expect("checkout feature");
    let out = block_on(rebase_branch_inner(&state, &id, main)).expect("rebase");
    assert!(matches!(out, RebaseOutcome::Conflicts { .. }), "{out:?}");

    block_on(rebase_abort_inner(&state, &id)).expect("abort");
    assert_eq!(head_branch(dir.path()).as_deref(), Some("feature"));
    assert_eq!(head_oid(dir.path()), f_tip, "tip restored");
    assert_eq!(read(dir.path(), "a.txt"), "feature\n");
    assert_eq!(
        block_on(get_op_state_inner(&state, &id)).expect("op state"),
        RepoOpState::None
    );
}

/// get_interactive_plan returns oldest-first all-Pick todos for base..HEAD;
/// start_interactive_rebase with a Reword rewrites the message in place.
#[test]
fn interactive_plan_and_reword() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let c1 = write_stage_commit(&state, &id, dir.path(), "one.txt", "1\n", "first change").oid;
    let c2 = write_stage_commit(&state, &id, dir.path(), "two.txt", "2\n", "second change").oid;

    let plan = block_on(get_interactive_plan_inner(&state, &id, c0.clone())).expect("plan");
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].oid, c1, "oldest first");
    assert_eq!(plan[1].oid, c2);
    assert!(plan
        .iter()
        .all(|t| t.action == rebase_interactive::RebaseAction::Pick && t.new_message.is_none()));

    let mut todos = plan;
    todos[0].action = rebase_interactive::RebaseAction::Reword;
    todos[0].new_message = Some("first change, reworded".into());
    let out = block_on(start_interactive_rebase_inner(&state, &id, c0, todos)).expect("start");
    match out {
        RebaseOutcome::Rebased { head, steps, .. } => {
            assert_eq!(steps, 2);
            let repo = git2::Repository::open(dir.path()).expect("open");
            let hc = repo.find_commit(git2::Oid::from_str(&head).unwrap()).unwrap();
            assert_eq!(hc.summary().expect("summary"), Some("second change"));
            assert_eq!(
                hc.parent(0).unwrap().summary().expect("summary"),
                Some("first change, reworded")
            );
        }
        other => panic!("expected Rebased, got {other:?}"),
    }
}
