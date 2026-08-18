//! T2 Area 1 — P3e-a two-repo isolation (contract §9.1): independent
//! status/commit/branches/op-state across two open repos, close only
//! affecting its target, focus-dedupe on reopen, and in-progress merge state
//! never leaking between repos.

use super::tests_support::*;
use super::*;

// ---- P3e-a two-repo isolation (contract §9.1) --------------------------

/// Committing in A leaves B's status/graph unaffected and A reflects the
/// change.
#[test]
fn isolation_independent_status_and_commit() {
    let state = AppState::default();

    let dir_a = init_repo_with_identity();
    let dir_b = init_repo_with_identity();
    let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
    let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
    assert_ne!(id_a, id_b);

    write_stage_commit(&state, &id_a, dir_a.path(), "a.txt", "hello", "first in A");

    // A now has one commit; its status is clean.
    let graph_a = tauri::async_runtime::block_on(get_graph_inner(&state, &id_a))
        .expect("graph A");
    assert_eq!(graph_a.nodes.len(), 1, "A should have exactly one commit");
    let status_a = tauri::async_runtime::block_on(get_status_inner(&state, &id_a))
        .expect("status A");
    assert!(status_a.staged.is_empty() && status_a.unstaged.is_empty());

    // B is untouched: still unborn, empty graph, no files.
    let graph_b = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
        .expect("graph B");
    assert!(graph_b.nodes.is_empty(), "B must be unaffected by a commit in A");
    let status_b = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
        .expect("status B");
    assert!(
        status_b.staged.is_empty()
            && status_b.unstaged.is_empty()
            && status_b.untracked.is_empty(),
        "B working dir must be empty"
    );
}

/// A branch created in A does not appear in B; B's op-state stays `None`.
#[test]
fn isolation_independent_branches_and_op_state() {
    let state = AppState::default();

    let dir_a = init_repo_with_identity();
    let dir_b = init_repo_with_identity();
    let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
    let id_b = open(&state, dir_b.path()).expect("open B").repo_id;

    // Need a commit before a branch can be created at HEAD.
    write_stage_commit(&state, &id_a, dir_a.path(), "a.txt", "hello", "first in A");
    tauri::async_runtime::block_on(create_branch_inner(&state, &id_a, "x".to_string()))
        .expect("create branch x in A");

    let branches_a = tauri::async_runtime::block_on(list_branches_inner(&state, &id_a))
        .expect("branches A");
    assert!(
        branches_a.local.iter().any(|b| b.name == "x"),
        "A must have branch x"
    );

    let branches_b = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
        .expect("branches B");
    assert!(
        !branches_b.local.iter().any(|b| b.name == "x"),
        "B must NOT have branch x"
    );

    let op_b = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_b))
        .expect("op-state B");
    assert_eq!(op_b, RepoOpState::None, "B op-state must stay None");
}

/// Closing A makes A's commands `NoRepo` while B keeps working; the map
/// then holds exactly one entry.
#[test]
fn isolation_close_only_affects_target() {
    let state = AppState::default();

    let dir_a = init_repo_with_identity();
    let dir_b = init_repo_with_identity();
    let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
    let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
    assert_eq!(repo_count(&state), 2);

    tauri::async_runtime::block_on(close_repo_inner(&state, &id_a)).expect("close A");

    let err = tauri::async_runtime::block_on(get_status_inner(&state, &id_a))
        .expect_err("A must be closed");
    assert!(matches!(err, AppError::NoRepo));

    tauri::async_runtime::block_on(get_status_inner(&state, &id_b)).expect("B still open");
    assert_eq!(repo_count(&state), 1, "exactly one entry after closing A");
}

/// Opening A's path twice (including a case-variant) focuses the same
/// entry: same `repo_id`, one map entry.
#[test]
fn isolation_focus_dedupe_on_reopen() {
    let state = AppState::default();

    let dir_a = init_repo_with_identity();
    let first = open(&state, dir_a.path()).expect("open A").repo_id;
    assert_eq!(repo_count(&state), 1);

    // Re-open the exact same path.
    let again = open(&state, dir_a.path()).expect("re-open A").repo_id;
    assert_eq!(first, again, "re-opening the same path must reuse the id");
    assert_eq!(repo_count(&state), 1, "no duplicate entry on re-open");

    // Re-open via an ASCII case-variant of the path: only meaningful on a
    // case-insensitive filesystem (Windows NTFS, macOS's default APFS).
    // On Linux's case-sensitive ext4 the uppercased path is a genuinely
    // different, nonexistent directory, so `read_repo_info`'s `is_dir()`
    // precheck correctly rejects it before the dedupe scan ever runs —
    // gate this half of the test to the platforms where it can hold.
    #[cfg(any(windows, target_os = "macos"))]
    {
        let variant = path_string(dir_a.path()).to_uppercase();
        let cased = tauri::async_runtime::block_on(open_repo_inner(
            &state,
            variant,
            |_id| Box::new(|| {}),
        ))
        .expect("re-open A (case-variant)")
        .repo_id;
        assert_eq!(
            first, cased,
            "a case-variant path must dedupe to the same id"
        );
        assert_eq!(repo_count(&state), 1, "case-variant must not add an entry");
    }
}

/// Closing an unknown id is a no-op `Ok(())` (idempotent).
#[test]
fn isolation_idempotent_close_of_unknown_id() {
    let state = AppState::default();
    tauri::async_runtime::block_on(close_repo_inner(&state, "does-not-exist"))
        .expect("closing an unknown id must be Ok(())");
    assert_eq!(repo_count(&state), 0);
}

/// Drives the repo `id` (workdir `dir`) into a PAUSED merge with a conflict
/// on `a.txt`, entirely through the command inners + git2 checkout, and
/// returns the base branch name. Post-condition: `merge_branch_inner`
/// returned `Conflicts` and the repo is in `RepoOpState::Merge`.
///
/// Recipe mirrors `merge_cli::script_conflict`: same middle line edited on
/// both the base branch and `topic`, so the true merge is guaranteed to
/// conflict.
fn start_conflicting_merge(state: &AppState, id: &str, dir: &std::path::Path) -> String {
    // Base commit on the default branch.
    write_stage_commit(state, id, dir, "a.txt", "line1\nbase\nline3\n", "base");
    let base_branch = tauri::async_runtime::block_on(list_branches_inner(state, id))
        .expect("branches after base commit")
        .head
        .branch_name
        .expect("HEAD has a branch name after the first commit");

    // topic diverges: edits the middle line differently.
    tauri::async_runtime::block_on(create_branch_inner(state, id, "topic".to_string()))
        .expect("create topic");
    tauri::async_runtime::block_on(checkout_branch_inner(state, id, "topic".to_string()))
        .expect("checkout topic");
    write_stage_commit(state, id, dir, "a.txt", "line1\ntopic\nline3\n", "topic change");

    // Back on the base branch: a conflicting edit to the same line.
    tauri::async_runtime::block_on(checkout_branch_inner(state, id, base_branch.clone()))
        .expect("checkout base branch");
    write_stage_commit(state, id, dir, "a.txt", "line1\nmain\nline3\n", "main change");

    // Merge topic → guaranteed conflict, repo pauses in Merge state.
    let outcome =
        tauri::async_runtime::block_on(merge_branch_inner(state, id, "topic".to_string(), None))
            .expect("merge_branch");
    match outcome {
        MergeOutcome::Conflicts { paths, .. } => {
            assert!(
                paths.iter().any(|p| p == "a.txt"),
                "expected a.txt to be conflicted, got {paths:?}"
            );
        }
        other => panic!("expected a conflicting merge, got {other:?}"),
    }
    base_branch
}

/// An in-progress MERGE in repo A must NOT leak into repo B: B's op-state
/// stays `None`, and B's status/branches are untouched, while A genuinely
/// reflects the paused merge. This strengthens
/// `isolation_independent_branches_and_op_state` (whose op-state half was
/// tautological — it never started an operation). Contract §9.1.
#[test]
fn isolation_in_progress_merge_does_not_leak() {
    let state = AppState::default();

    let dir_a = init_repo_with_identity();
    let dir_b = init_repo_with_identity();
    let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
    let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
    assert_ne!(id_a, id_b);

    // Give B a real (but independent) history so "unaffected" is a
    // meaningful assertion rather than "both are empty".
    write_stage_commit(&state, &id_b, dir_b.path(), "b.txt", "b-only\n", "b base");
    tauri::async_runtime::block_on(create_branch_inner(&state, &id_b, "keep".to_string()))
        .expect("create branch keep in B");

    // Snapshot B before the merge storm in A.
    let branches_b_before = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
        .expect("branches B before");
    let status_b_before = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
        .expect("status B before");
    let graph_b_before = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
        .expect("graph B before");

    // Now drive A into a paused merge.
    start_conflicting_merge(&state, &id_a, dir_a.path());

    // A genuinely reflects the in-progress merge.
    let op_a = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_a))
        .expect("op-state A");
    assert!(
        matches!(op_a, RepoOpState::Merge { .. }),
        "A must be paused in a merge, got {op_a:?}"
    );
    let conflicts_a = tauri::async_runtime::block_on(list_conflicts_inner(&state, &id_a))
        .expect("conflicts A");
    assert!(
        conflicts_a.iter().any(|c| c.path == "a.txt"),
        "A must list a.txt as conflicted, got {conflicts_a:?}"
    );

    // B is entirely unaffected: op-state None, and its branches/status/graph
    // are byte-identical to the pre-merge snapshot.
    let op_b = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_b))
        .expect("op-state B");
    assert_eq!(op_b, RepoOpState::None, "B op-state must stay None");

    let branches_b_after = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
        .expect("branches B after");
    assert_eq!(
        branches_b_after.local.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
        branches_b_before.local.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
        "B's branch set must be unchanged"
    );
    assert!(
        !branches_b_after.local.iter().any(|b| b.name == "topic"),
        "B must not have gained A's 'topic' branch"
    );

    let status_b_after = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
        .expect("status B after");
    assert_eq!(
        (status_b_after.staged.len(), status_b_after.unstaged.len(), status_b_after.untracked.len()),
        (status_b_before.staged.len(), status_b_before.unstaged.len(), status_b_before.untracked.len()),
        "B's working-dir status must be unchanged"
    );

    let graph_b_after = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
        .expect("graph B after");
    assert_eq!(
        graph_b_after.nodes.len(),
        graph_b_before.nodes.len(),
        "B's commit graph must be unchanged"
    );
}

/// Closing repo B while repo A has a PAUSED merge must not disturb A's
/// on-disk operation: A's op-state is still `Merge` and its conflicts are
/// still readable, and the map holds exactly A. Contract §9.1 (close edge).
#[test]
fn isolation_close_preserves_other_repos_in_progress_op() {
    let state = AppState::default();

    let dir_a = init_repo_with_identity();
    let dir_b = init_repo_with_identity();
    let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
    let id_b = open(&state, dir_b.path()).expect("open B").repo_id;

    start_conflicting_merge(&state, &id_a, dir_a.path());

    // Close B while A is mid-merge.
    tauri::async_runtime::block_on(close_repo_inner(&state, &id_b)).expect("close B");
    assert_eq!(repo_count(&state), 1, "only A remains open");

    // A's in-progress merge survives the close of B, fully readable.
    let op_a = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_a))
        .expect("op-state A after closing B");
    assert!(
        matches!(op_a, RepoOpState::Merge { .. }),
        "A must still be paused in a merge after B is closed, got {op_a:?}"
    );
    let conflicts_a = tauri::async_runtime::block_on(list_conflicts_inner(&state, &id_a))
        .expect("conflicts A after closing B");
    assert!(
        conflicts_a.iter().any(|c| c.path == "a.txt"),
        "A must still list a.txt as conflicted after closing B, got {conflicts_a:?}"
    );
}

