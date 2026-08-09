//! T2 Area 1 — discard_paths / discard_paths_force / discard_partial /
//! reset_branch / apply_composed_commits command inners (runtime-free).

use super::tests_support::*;
use super::*;
use bonsai_core::git::diff::LineKind;

fn status_of(state: &AppState, id: &str) -> StatusSnapshot {
    tauri::async_runtime::block_on(get_status_inner(state, id)).expect("status")
}

fn read(dir: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).expect("read file")
}

/// discard_paths restores a tracked file's worktree content to the index
/// version (unstaged edit gone).
#[test]
fn discard_paths_restores_modified_file() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    std::fs::write(dir.path().join("a.txt"), "edited\n").expect("write");
    assert!(!status_of(&state, &id).unstaged.is_empty());

    tauri::async_runtime::block_on(discard_paths_inner(&state, &id, vec!["a.txt".into()]))
        .expect("discard");
    assert_eq!(read(dir.path(), "a.txt"), "base\n", "worktree restored to index");
    assert!(status_of(&state, &id).unstaged.is_empty());
}

/// discard_paths_force removes an untracked file from disk; plain discard of an
/// invalid wire path errors cleanly.
#[test]
fn discard_paths_force_removes_untracked() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    std::fs::write(dir.path().join("junk.txt"), "junk\n").expect("write");
    tauri::async_runtime::block_on(discard_paths_force_inner(&state, &id, vec!["junk.txt".into()]))
        .expect("force discard");
    assert!(!dir.path().join("junk.txt").exists(), "untracked file deleted");

    let err = tauri::async_runtime::block_on(discard_paths_inner(&state, &id, vec!["../evil".into()]))
        .expect_err("escaping path must error");
    assert!(matches!(err, AppError::Other(_)), "{err:?}");
}

/// discard_partial drops only the selected added line from the worktree; an
/// empty selection is a no-op.
#[test]
fn discard_partial_selected_line_and_empty_noop() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "d.txt", "l1\nl2\n", "seed d");

    std::fs::write(dir.path().join("d.txt"), "l1\nl2\nl3\n").expect("write");

    // Empty selection: no-op, file unchanged.
    tauri::async_runtime::block_on(discard_partial_inner(&state, &id, "d.txt".into(), None, vec![]))
        .expect("empty selection no-op");
    assert_eq!(read(dir.path(), "d.txt"), "l1\nl2\nl3\n");

    // Discard the added line 3: worktree returns to the index version.
    tauri::async_runtime::block_on(discard_partial_inner(
        &state,
        &id,
        "d.txt".into(),
        None,
        vec![LineSelection { kind: LineKind::Add, old_no: None, new_no: Some(3) }],
    ))
    .expect("discard_partial");
    assert_eq!(read(dir.path(), "d.txt"), "l1\nl2\n");
}

/// reset_branch soft / mixed / hard against a 2-commit history, asserting the
/// index/workdir state after each mode.
#[test]
fn reset_branch_soft_mixed_hard() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let c1 = write_stage_commit(&state, &id, dir.path(), "a.txt", "v2\n", "C1").oid;

    // Soft: HEAD -> C0, index + workdir keep C1's content (change shows staged).
    tauri::async_runtime::block_on(reset_branch_command_inner(
        &state, &id, c0.clone(), ResetMode::Soft,
    ))
    .expect("soft reset");
    assert_eq!(head_oid(dir.path()), c0);
    let st = status_of(&state, &id);
    assert_eq!(st.staged.len(), 1, "C1 content staged after soft: {st:?}");
    assert!(st.unstaged.is_empty());
    assert_eq!(read(dir.path(), "a.txt"), "v2\n");

    // Mixed: back to C1 first, then mixed-reset to C0 — index matches C0
    // (change shows unstaged), workdir keeps v2.
    tauri::async_runtime::block_on(reset_branch_command_inner(
        &state, &id, c1.clone(), ResetMode::Hard,
    ))
    .expect("restore C1");
    tauri::async_runtime::block_on(reset_branch_command_inner(
        &state, &id, c0.clone(), ResetMode::Mixed,
    ))
    .expect("mixed reset");
    assert_eq!(head_oid(dir.path()), c0);
    let st = status_of(&state, &id);
    assert!(st.staged.is_empty(), "{st:?}");
    assert_eq!(st.unstaged.len(), 1, "C1 content unstaged after mixed: {st:?}");
    assert_eq!(read(dir.path(), "a.txt"), "v2\n");

    // Hard: workdir + index both back to C0.
    tauri::async_runtime::block_on(reset_branch_command_inner(
        &state, &id, c1.clone(), ResetMode::Hard,
    ))
    .expect("restore C1 again");
    tauri::async_runtime::block_on(reset_branch_command_inner(
        &state, &id, c0.clone(), ResetMode::Hard,
    ))
    .expect("hard reset");
    assert_eq!(head_oid(dir.path()), c0);
    let st = status_of(&state, &id);
    assert!(st.staged.is_empty() && st.unstaged.is_empty(), "{st:?}");
    assert_eq!(read(dir.path(), "a.txt"), "base\n");
}

/// reset_branch with a garbage oid is a clean Git error and HEAD is unmoved.
#[test]
fn reset_branch_bad_oid_errors() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    for bad in ["zzzz", "", "0000000000000000000000000000000000000000"] {
        let err = tauri::async_runtime::block_on(reset_branch_command_inner(
            &state,
            &id,
            bad.to_string(),
            ResetMode::Mixed,
        ))
        .expect_err("bad oid must error");
        assert!(matches!(err, AppError::Git(_)), "{bad}: {err:?}");
    }
    assert_eq!(head_oid(dir.path()), c0);
}

/// apply_composed_commits: a 2-group plan creates exactly 2 commits (oldest
/// first) and leaves the unassigned file uncommitted in the working tree.
#[test]
fn compose_two_group_plan_happy() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    std::fs::write(dir.path().join("f1.txt"), "1\n").expect("write");
    std::fs::write(dir.path().join("f2.txt"), "2\n").expect("write");
    std::fs::write(dir.path().join("f3.txt"), "3\n").expect("write");

    let plan = ComposePlan {
        groups: vec![
            ai_compose::ComposeGroup { files: vec!["f1.txt".into()], message: "first group".into() },
            ai_compose::ComposeGroup { files: vec!["f2.txt".into()], message: "second group".into() },
        ],
    };
    let res = tauri::async_runtime::block_on(apply_composed_commits_inner(&state, &id, plan))
        .expect("apply plan");

    assert_eq!(res.commits.len(), 2);
    assert_eq!(res.commits[0].summary, "first group");
    assert_eq!(res.commits[1].summary, "second group");
    assert_eq!(head_oid(dir.path()), res.commits[1].oid, "HEAD = newest group commit");

    let repo = git2::Repository::open(dir.path()).expect("open");
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.parent_id(0).unwrap().to_string(), res.commits[0].oid);
    assert_eq!(head.parent(0).unwrap().parent_id(0).unwrap().to_string(), c0);

    let st = status_of(&state, &id);
    assert_eq!(
        st.untracked.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
        vec!["f3.txt"],
        "unassigned file left untouched"
    );
}

/// A plan with an empty message in group 2 is rejected UP FRONT (whole-plan
/// validation): EmptyMessage, HEAD unchanged, nothing committed, index intact.
#[test]
fn compose_invalid_plan_leaves_head_unchanged() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    std::fs::write(dir.path().join("g1.txt"), "1\n").expect("write");
    std::fs::write(dir.path().join("g2.txt"), "2\n").expect("write");

    let plan = ComposePlan {
        groups: vec![
            ai_compose::ComposeGroup { files: vec!["g1.txt".into()], message: "ok".into() },
            ai_compose::ComposeGroup { files: vec!["g2.txt".into()], message: "   ".into() },
        ],
    };
    let err = tauri::async_runtime::block_on(apply_composed_commits_inner(&state, &id, plan))
        .expect_err("empty group message must reject the plan");
    assert!(matches!(err, AppError::EmptyMessage), "{err:?}");
    assert_eq!(head_oid(dir.path()), c0, "nothing committed");

    // A stale plan (file not in the working changes) is Other, HEAD unchanged.
    let plan = ComposePlan {
        groups: vec![ai_compose::ComposeGroup {
            files: vec!["ghost.txt".into()],
            message: "stale".into(),
        }],
    };
    let err = tauri::async_runtime::block_on(apply_composed_commits_inner(&state, &id, plan))
        .expect_err("stale plan must reject");
    assert!(matches!(err, AppError::Other(_)), "{err:?}");
    assert_eq!(head_oid(dir.path()), c0);

    // An empty plan is NothingToCommit.
    let err = tauri::async_runtime::block_on(apply_composed_commits_inner(
        &state,
        &id,
        ComposePlan { groups: vec![] },
    ))
    .expect_err("empty plan");
    assert!(matches!(err, AppError::NothingToCommit), "{err:?}");
}
