//! T2 Area 1 — open-repo precondition guards, part 2: compare, branch,
//! remote, forge, merge, rebase, interactive-rebase, and bisect command
//! groups all return `NoRepo` for an unknown id. Split out of
//! `tests_open_repo_guards.rs` (same fixture helpers) to keep both files
//! under the file-size limit.

use super::tests_support::*;
use super::*;

/// The P5 compare commands also return `NoRepo` for an unknown id
/// (contract §6.2).
#[test]
fn compare_commands_require_an_open_repo() {
    let state = AppState::default();
    let oid = "0123456789abcdef0123456789abcdef01234567".to_string();

    let err = tauri::async_runtime::block_on(compare_with_head_inner(
        &state,
        MISSING_ID,
        oid.clone(),
    ))
    .expect_err("compare_with_head with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(compare_with_head_file_diff_inner(
        &state,
        MISSING_ID,
        oid,
        "file.txt".to_string(),
        None,
        false,
        false,
    ))
    .expect_err("compare_with_head_file_diff with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The M5 branch commands all return `NoRepo` for an unknown id
/// (contract §6.5).
#[test]
fn branch_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(list_branches_inner(&state, MISSING_ID))
        .expect_err("list_branches with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(create_branch_inner(
        &state,
        MISSING_ID,
        "topic".to_string(),
    ))
    .expect_err("create_branch with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(checkout_branch_inner(
        &state,
        MISSING_ID,
        "topic".to_string(),
    ))
    .expect_err("checkout_branch with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(delete_branch_inner(
        &state,
        MISSING_ID,
        "topic".to_string(),
    ))
    .expect_err("delete_branch with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(checkout_remote_inner(
        &state,
        MISSING_ID,
        "origin/topic".to_string(),
    ))
    .expect_err("checkout_remote with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(delete_remote_tracking_inner(
        &state,
        MISSING_ID,
        "origin/topic".to_string(),
    ))
    .expect_err("delete_remote_tracking with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The M6 remote commands all return `NoRepo` for an unknown id
/// (contract §6.7).
#[test]
fn remote_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(fetch_inner(&state, MISSING_ID))
        .expect_err("fetch with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(pull_inner(&state, MISSING_ID))
        .expect_err("pull with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(push_inner(&state, MISSING_ID, None))
        .expect_err("push with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(force_push_inner(&state, MISSING_ID, None))
        .expect_err("force_push with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P62b forge commands all return `NoRepo` for an unknown id. Every
/// inner gates on `repo_path` BEFORE any `spawn_blocking`, so the missing-id
/// path never opens a provider, touches the network, or reads the OS
/// keychain (the tauri "test" feature is avoided on this machine — see
/// `config_commands_require_an_open_repo`). `PrStateFilter` is not among the
/// DTO names `shared` re-exports, so it is referenced fully qualified here.
#[test]
fn forge_commands_require_an_open_repo() {
    let state = AppState::default();

    // P79: the inner fns take a settings-file path for the known-hosts index
    // sync. `repo_path` rejects MISSING_ID before the file is ever touched, so a
    // non-existent temp path is fine here.
    let settings_file = std::path::Path::new("D:/Data/Temp/bonsai-nonexistent-settings.json");

    let err =
        tauri::async_runtime::block_on(forge_repo_context_inner(&state, settings_file, MISSING_ID))
            .expect_err("forge_repo_context with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(forge_list_prs_inner(
        &state,
        settings_file,
        MISSING_ID,
        PrListQuery {
            state: bonsai_forge::PrStateFilter::Open,
            page: 1,
            per_page: 30,
        },
    ))
    .expect_err("forge_list_prs with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err =
        tauri::async_runtime::block_on(forge_get_pr_inner(&state, settings_file, MISSING_ID, 1))
            .expect_err("forge_get_pr with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(forge_create_pr_inner(
        &state,
        settings_file,
        MISSING_ID,
        CreatePrInput {
            title: "t".to_string(),
            body: "b".to_string(),
            source_branch: "feature".to_string(),
            target_branch: "main".to_string(),
            draft: false,
            maintainer_can_modify: true,
        },
    ))
    .expect_err("forge_create_pr with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(forge_list_review_comments_inner(
        &state,
        settings_file,
        MISSING_ID,
        1,
    ))
    .expect_err("forge_list_review_comments with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(forge_set_token_inner(
        &state,
        settings_file,
        MISSING_ID,
        "tok".to_string(),
    ))
    .expect_err("forge_set_token with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err =
        tauri::async_runtime::block_on(forge_clear_token_inner(&state, settings_file, MISSING_ID))
            .expect_err("forge_clear_token with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(forge_commit_statuses_inner(
        &state,
        settings_file,
        MISSING_ID,
        vec!["deadbeef".to_string()],
    ))
    .expect_err("forge_commit_statuses with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P3c merge/conflict commands all return `NoRepo` for an unknown id
/// (contract §6).
#[test]
fn merge_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(get_op_state_inner(&state, MISSING_ID))
        .expect_err("get_op_state with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(merge_branch_inner(
        &state,
        MISSING_ID,
        "topic".to_string(),
        None,
    ))
    .expect_err("merge_branch with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(commit_merge_inner(
        &state,
        MISSING_ID,
        "msg".to_string(),
        None,
        None,
    ))
    .expect_err("commit_merge with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(abort_merge_inner(&state, MISSING_ID))
        .expect_err("abort_merge with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(list_conflicts_inner(&state, MISSING_ID))
        .expect_err("list_conflicts with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(get_conflict_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
    ))
    .expect_err("get_conflict with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(resolve_conflict_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
        ConflictResolution::Ours,
    ))
    .expect_err("resolve_conflict with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(resolve_conflict_text_inner(
        &state,
        MISSING_ID,
        "file.txt".to_string(),
        "resolved\n".to_string(),
    ))
    .expect_err("resolve_conflict_text with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P3d rebase commands all return `NoRepo` for an unknown id
/// (contract §4).
#[test]
fn rebase_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(rebase_branch_inner(
        &state,
        MISSING_ID,
        "main".to_string(),
    ))
    .expect_err("rebase_branch with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(rebase_continue_inner(&state, MISSING_ID))
        .expect_err("rebase_continue with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(rebase_skip_inner(&state, MISSING_ID))
        .expect_err("rebase_skip with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(rebase_abort_inner(&state, MISSING_ID))
        .expect_err("rebase_abort with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P23 interactive-rebase commands return `NoRepo` for an unknown id.
#[test]
fn interactive_rebase_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(get_interactive_plan_inner(
        &state,
        MISSING_ID,
        "a".repeat(40),
    ))
    .expect_err("get_interactive_plan with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(start_interactive_rebase_inner(
        &state,
        MISSING_ID,
        "a".repeat(40),
        Vec::new(),
    ))
    .expect_err("start_interactive_rebase with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

/// The P39 bisect commands return `NoRepo` for an unknown id.
#[test]
fn bisect_commands_require_an_open_repo() {
    let state = AppState::default();

    let err = tauri::async_runtime::block_on(start_bisect_inner(
        &state,
        MISSING_ID,
        "a".repeat(40),
        vec!["b".repeat(40)],
    ))
    .expect_err("start_bisect with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(bisect_mark_inner(&state, MISSING_ID, true))
        .expect_err("bisect_mark with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(bisect_skip_inner(&state, MISSING_ID))
        .expect_err("bisect_skip with no repo");
    assert!(matches!(err, AppError::NoRepo));

    let err = tauri::async_runtime::block_on(bisect_reset_inner(&state, MISSING_ID))
        .expect_err("bisect_reset with no repo");
    assert!(matches!(err, AppError::NoRepo));
}

