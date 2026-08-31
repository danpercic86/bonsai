//! T2 Area 1 — open-repo precondition guards, part 1: health, graph,
//! open (non-repo/bare/subfolder), mutation, tag, remote-management, diff,
//! blame, reflog, config, identity, partial-staging, and discard-partial
//! command groups all return `NoRepo` for an unknown id, and opening a
//! non-repo/bare/subfolder path leaves other open entries untouched.
//! Shared fixture helpers hoisted to `tests_support.rs` (T2 Area 1):
//! MISSING_ID, path_string, open, init_repo_with_identity, write_stage_commit,
//! repo_count, …

use super::tests_support::*;
use super::*;

/// Opening a non-repo path inserts NO entry and touches no other open tab
/// (P3e contract §4.2 — there is no single "current repo" to clear).
#[test]
fn failed_open_leaves_other_entries_untouched() {
    let state = AppState::default();

    // Open a real (empty, unborn-HEAD) repo first.
    let repo_dir = tempfile::TempDir::new().expect("create temp dir");
    git2::Repository::init(repo_dir.path()).expect("init repo");
    let a = open(&state, repo_dir.path()).expect("open repo A");
    assert!(a.info.is_repo && !a.info.bare);
    tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
        .expect("status of repo A");

    // Now open a plain directory: not a repo. No entry is created for it…
    let non_repo_dir = tempfile::TempDir::new().expect("create temp dir");
    let n = open(&state, non_repo_dir.path()).expect("open non-repo dir");
    assert!(!n.info.is_repo);
    let err = tauri::async_runtime::block_on(get_status_inner(&state, &n.repo_id))
        .expect_err("a non-repo id must not be open");
    assert!(matches!(err, AppError::NoRepo));

    // …and repo A is still open and usable.
    tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
        .expect("repo A still open after a failed open");
    assert_eq!(repo_count(&state), 1);
}

/// `get_repo_health` errors only for an unknown id (`NoRepo`, P29 §D4);
/// on an open repo it resolves with all four sections carrying data —
/// section-level failures never reject the command.
#[test]
fn get_repo_health_requires_open_repo() {
    let state = AppState::default();
    let err = tauri::async_runtime::block_on(get_repo_health_inner(&state, MISSING_ID))
        .expect_err("unknown id must be NoRepo");
    assert!(matches!(err, AppError::NoRepo));

    let dir = init_repo_with_identity();
    let opened = open(&state, dir.path()).expect("open repo");
    write_stage_commit(&state, &opened.repo_id, dir.path(), "a.txt", "a\n", "C0");
    let health =
        tauri::async_runtime::block_on(get_repo_health_inner(&state, &opened.repo_id))
            .expect("health never errors for an open repo");
    assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
    assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
    assert!(
        health.working_state.data.is_some(),
        "{:?}",
        health.working_state.error
    );
    assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
    assert_eq!(
        health.stats.data.as_ref().map(|s| s.commit_count),
        Some(1)
    );
    assert!(health.generated_at > 0);
}

/// `get_graph` with an unknown id returns `NoRepo`; after opening an
/// unborn-HEAD repo it returns an empty layout (not an error).
#[test]
fn get_graph_no_repo_and_unborn() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(get_graph_inner(&state, MISSING_ID))
        .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo));

    let repo_dir = tempfile::TempDir::new().expect("create temp dir");
    git2::Repository::init(repo_dir.path()).expect("init repo");
    let id = open(&state, repo_dir.path()).expect("open unborn repo").repo_id;

    let layout = tauri::async_runtime::block_on(get_graph_inner(&state, &id))
        .expect("empty layout for unborn repo");
    assert!(layout.nodes.is_empty());
    assert_eq!(layout.head_index, None);
}

/// Bare repos are reported but not kept open; other entries are untouched.
#[test]
fn bare_open_leaves_other_entries_untouched() {
    let state = AppState::default();

    let repo_dir = tempfile::TempDir::new().expect("create temp dir");
    git2::Repository::init(repo_dir.path()).expect("init repo");
    let a = open(&state, repo_dir.path()).expect("open repo A");

    let bare_dir = tempfile::TempDir::new().expect("create temp dir");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare repo");
    let b = open(&state, bare_dir.path()).expect("open bare repo");
    assert!(b.info.is_repo && b.info.bare);

    let err = tauri::async_runtime::block_on(get_status_inner(&state, &b.repo_id))
        .expect_err("bare repo must not be open");
    assert!(matches!(err, AppError::NoRepo));

    tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
        .expect("repo A still open after opening a bare repo");
    assert_eq!(repo_count(&state), 1);
}

/// The M3 mutation commands all return `NoRepo` for an unknown id
/// (empty map + dummy id).
#[test]
fn mutation_commands_require_an_open_repo() {
    let state = AppState::default();
    let paths = vec!["file.txt".to_string()];

    let err = tauri::async_runtime::block_on(stage_inner(&state, MISSING_ID, paths.clone()))
        .expect_err("stage with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(unstage_inner(&state, MISSING_ID, paths))
        .expect_err("unstage with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err =
        tauri::async_runtime::block_on(commit_inner(&state, MISSING_ID, "msg".to_string(), None, None))
            .expect_err("commit with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P22 tag commands all return `NoRepo` for an unknown id (§8.4).
#[test]
fn tag_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(create_tag_inner(
        &state,
        MISSING_ID,
        "v1".to_string(),
        "0".repeat(40),
        None,
        false,
        None,
    ))
    .expect_err("create_tag with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(delete_tag_inner(
        &state,
        MISSING_ID,
        "v1".to_string(),
    ))
    .expect_err("delete_tag with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(push_tag_inner(
        &state,
        MISSING_ID,
        "origin".to_string(),
        "v1".to_string(),
        false,
    ))
    .expect_err("push_tag with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P22 remote-management commands all return `NoRepo` for an unknown
/// id (§8.4).
#[test]
fn remote_mgmt_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(list_remotes_inner(&state, MISSING_ID))
        .expect_err("list_remotes with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(add_remote_inner(
        &state,
        MISSING_ID,
        "backup".to_string(),
        "https://example.com/repo.git".to_string(),
    ))
    .expect_err("add_remote with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(remove_remote_inner(
        &state,
        MISSING_ID,
        "origin".to_string(),
    ))
    .expect_err("remove_remote with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(rename_remote_inner(
        &state,
        MISSING_ID,
        "origin".to_string(),
        "upstream".to_string(),
    ))
    .expect_err("rename_remote with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(set_remote_url_inner(
        &state,
        MISSING_ID,
        "origin".to_string(),
        "https://example.com/other.git".to_string(),
    ))
    .expect_err("set_remote_url with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The M4 diff commands all return `NoRepo` for an unknown id
/// (contract §6.2 scenario 17).
#[test]
fn diff_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(get_workdir_file_diff_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
        None,
        false,
        false,
        false,
    ))
    .expect_err("get_workdir_file_diff with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let oid = "0123456789abcdef0123456789abcdef01234567".to_string();
    let err =
        tauri::async_runtime::block_on(get_commit_diff_inner(&state, MISSING_ID, oid.clone()))
            .expect_err("get_commit_diff with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(get_commit_file_diff_inner(
        &state,
        MISSING_ID,
        oid,
        "file.txt".to_string(),
        None,
        false,
        false,
    ))
    .expect_err("get_commit_file_diff with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P23c blame + file-history commands return `NoRepo` for an unknown id.
#[test]
fn blame_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(blame_file_inner(
        &state,
        MISSING_ID,
        "src/app.ts".to_string(),
        None,
    ))
    .expect_err("blame_file with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(file_history_inner(
        &state,
        MISSING_ID,
        "src/app.ts".to_string(),
        200,
    ))
    .expect_err("file_history with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P38 reflog command returns `NoRepo` for an unknown id.
#[test]
fn read_reflog_requires_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(read_reflog_inner(
        &state,
        MISSING_ID,
        "HEAD".to_string(),
    ))
    .expect_err("read_reflog with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P40 config commands return `NoRepo` for an unknown id (the gate is
/// `repo_path` before any git2 — never touches global config).
#[test]
fn config_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(get_config_inner(
        &state,
        MISSING_ID,
        ConfigLevelArg::Local,
    ))
    .expect_err("get_config with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(set_config_inner(
        &state,
        MISSING_ID,
        ConfigLevelArg::Global,
        "user.name".to_string(),
        "Nobody".to_string(),
    ))
    .expect_err("set_config with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(unset_config_inner(
        &state,
        MISSING_ID,
        ConfigLevelArg::Global,
        "user.name".to_string(),
    ))
    .expect_err("unset_config with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P44 `apply_identity_profile` command returns `NoRepo` for an unknown
/// id. Its only pre-`spawn_blocking` gate is `repo_path` (it has no `_inner`
/// split, and the tauri "test" feature is avoided on this machine — see
/// `config_commands_require_an_open_repo`), so the NoRepo path is exercised
/// at that gate. The gate fails before any identity field reaches git2, so
/// global config stays untouched even for an unknown repo.
#[test]
fn apply_identity_profile_unknown_repo_errors() {
    let state = AppState::default();
    let err = repo_path(&state, MISSING_ID).expect_err("apply with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P17 partial-staging commands return `NoRepo` for an unknown id
/// (contract §6.2 scenario 15) — the gate is `repo_path` before any git2.
#[test]
fn partial_staging_commands_require_an_open_repo() {
    let state = AppState::default();
    let selection = vec![LineSelection {
        kind: bonsai_core::git::diff::LineKind::Add,
        old_no: None,
        new_no: Some(1),
    }];

    let err = tauri::async_runtime::block_on(stage_partial_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
        None,
        selection.clone(),
    ))
    .expect_err("stage_partial with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(unstage_partial_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
        None,
        selection,
    ))
    .expect_err("unstage_partial with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P28 partial-discard command returns `NoRepo` for an unknown id —
/// the gate is `repo_path` before any git2 work.
#[test]
fn discard_partial_command_requires_an_open_repo() {
    let state = AppState::default();
    let selection = vec![LineSelection {
        kind: bonsai_core::git::diff::LineKind::Add,
        old_no: None,
        new_no: Some(1),
    }];
    let err = tauri::async_runtime::block_on(discard_partial_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
        None,
        selection,
    ))
    .expect_err("discard_partial with no repo");
    assert!(matches!(err, AppError::NoRepo));
}
