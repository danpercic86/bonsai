pub mod commands;
pub mod mcp;
pub mod settings;
pub mod state;
pub mod watcher;

use tauri::Manager;

pub fn run() {
    bonsai_core::git::relax_odb_hash_verification();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .manage(mcp::McpServerState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_repo,
            commands::close_repo,
            commands::get_status,
            commands::get_graph,
            commands::stage,
            commands::unstage,
            commands::stage_partial,
            commands::unstage_partial,
            commands::commit,
            commands::get_workdir_file_diff,
            commands::get_commit_diff,
            commands::get_commit_file_diff,
            commands::compare_with_head,
            commands::compare_with_head_file_diff,
            commands::list_branches,
            commands::create_branch,
            commands::create_branch_here,
            commands::checkout_branch,
            commands::delete_branch,
            commands::checkout_remote,
            commands::delete_remote_tracking,
            commands::list_stale_branches,
            commands::delete_branches,
            commands::fetch,
            commands::pull,
            commands::push,
            commands::get_recent_repos,
            commands::remove_recent_repo,
            commands::get_ui_settings,
            commands::set_ui_settings,
            commands::get_session,
            commands::set_session,
            commands::get_op_state,
            commands::merge_branch,
            commands::commit_merge,
            commands::abort_merge,
            commands::list_conflicts,
            commands::get_conflict,
            commands::resolve_conflict,
            commands::resolve_conflict_text,
            commands::check_ai_availability,
            commands::ai_resolve_conflict,
            commands::generate_commit_message,
            commands::ai_analyze_diff,
            commands::ai_summarize_range,
            commands::ai_digest,
            commands::rebase_branch,
            commands::rebase_continue,
            commands::rebase_skip,
            commands::rebase_abort,
            commands::get_interactive_plan,
            commands::start_interactive_rebase,
            commands::blame_file,
            commands::file_history,
            commands::list_stashes,
            commands::create_stash,
            commands::apply_stash,
            commands::pop_stash,
            commands::drop_stash,
            commands::commit_amend,
            commands::reset_branch,
            commands::discard_paths,
            commands::discard_partial,
            commands::cherrypick_commit,
            commands::cherrypick_continue,
            commands::cherrypick_abort,
            commands::revert_commit,
            commands::revert_continue,
            commands::revert_abort,
            commands::set_active_repo,
            commands::get_mcp_status,
            commands::set_mcp_enabled,
            commands::set_mcp_allow_write,
            commands::list_submodules,
            commands::init_submodule,
            commands::update_submodule,
            commands::sync_submodule,
            commands::list_worktrees,
            commands::add_worktree,
            commands::remove_worktree,
            commands::lock_worktree,
            commands::unlock_worktree,
            commands::get_repo_health,
            commands::clone_repo,
            commands::init_repo,
            commands::create_tag,
            commands::delete_tag,
            commands::push_tag,
            commands::list_remotes,
            commands::add_remote,
            commands::remove_remote,
            commands::rename_remote,
            commands::set_remote_url,
            commands::list_ai_assets,
            commands::read_ai_asset,
            commands::list_agent_assets,
            commands::read_agent_asset,
            commands::save_agent_asset,
            commands::delete_agent_asset,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::preview_profile,
            commands::activate_profile,
            commands::ai_generate_asset
        ])
        .build(tauri::generate_context!())
        .expect("error while running Bonsai")
        .run(|app, event| {
            // Release the MCP port on exit (P16 §6.3): stop the embedded server
            // before the app process goes away.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                mcp::shutdown(&app.state::<mcp::McpServerState>());
            }
        });
}
