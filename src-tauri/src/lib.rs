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
            commands::rebase_branch,
            commands::rebase_continue,
            commands::rebase_skip,
            commands::rebase_abort,
            commands::list_stashes,
            commands::create_stash,
            commands::apply_stash,
            commands::pop_stash,
            commands::drop_stash,
            commands::set_active_repo,
            commands::get_mcp_status,
            commands::set_mcp_enabled,
            commands::set_mcp_allow_write,
            commands::list_submodules,
            commands::init_submodule,
            commands::update_submodule,
            commands::sync_submodule
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
