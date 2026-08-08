pub mod commands;
pub mod mcp;
pub mod scheduler;
pub mod settings;
pub mod state;
pub mod watcher;

use tauri::Manager;

pub fn run() {
    bonsai_core::git::relax_odb_hash_verification();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // P42: auto-update (updater is desktop-only) + process (relaunch after
        // install). React drives these ONLY through the IpcApi wrapper (INV-1);
        // no custom updater command — the JS plugin holds the Update handle.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(state::AppState::default())
        .manage(mcp::McpServerState::default())
        .manage(scheduler::SchedulerState::default())
        .setup(|app| {
            // P30: seed the scheduler config from persisted settings, then
            // start the ONE global tick loop (D2). Settings-load failure is
            // non-fatal — the scheduler just starts with defaults (disabled).
            let handle = app.handle().clone();
            let sched = app.state::<scheduler::SchedulerState>();
            match settings::settings_file(&handle) {
                Ok(file) => {
                    let s = settings::load_from(&file);
                    scheduler::apply_config(
                        &sched,
                        scheduler::JobsConfig {
                            auto_fetch: s.auto_fetch,
                            health_refresh: s.health_refresh,
                        },
                    );
                    // P44a: restore the embedded MCP server across restart. The
                    // `mcp_enabled` flag was persisted but nothing read it back at
                    // launch, so the server stayed down (and the UI toggle showed
                    // OFF) after a restart even when the user had turned it on. If
                    // the user consented previously, re-open the loopback listener
                    // here. `set_enabled` is async → drive it off the setup thread
                    // (mirrors the scheduler spawn below). `start` already reads the
                    // persisted `mcp_allow_write`, so enabling alone restores the
                    // correct (read or write) tool set — no separate write-gate call
                    // needed. Start failure (e.g. the persisted port is busy) is
                    // non-fatal: log and continue so the app still launches.
                    if s.mcp_enabled {
                        let mcp_handle = app.handle().clone();
                        tauri::async_runtime::spawn(async move {
                            let mcp_state = mcp_handle.state::<mcp::McpServerState>();
                            if let Err(e) =
                                mcp::set_enabled(&mcp_handle, &mcp_state, true).await
                            {
                                eprintln!("bonsai: MCP auto-start failed (non-fatal): {e}");
                            }
                        });
                    }
                }
                Err(e) => {
                    eprintln!("bonsai: cannot resolve settings file (non-fatal): {e}");
                }
            }
            tauri::async_runtime::spawn(scheduler::run_scheduler(
                handle,
                std::time::Duration::from_secs(scheduler::TICK_SECONDS),
            ));
            Ok(())
        })
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
            commands::rename_branch,
            commands::checkout_remote,
            commands::delete_remote_tracking,
            commands::list_stale_branches,
            commands::delete_branches,
            commands::fetch,
            commands::pull,
            commands::push,
            commands::force_push,
            commands::get_recent_repos,
            commands::remove_recent_repo,
            commands::get_ui_settings,
            commands::set_ui_settings,
            commands::get_session,
            commands::set_session,
            commands::get_op_state,
            commands::get_job_status,
            commands::run_job_now,
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
            commands::ai_changelog,
            commands::ai_compose_commits,
            commands::ai_explain_line,
            commands::ai_suggest_branch_name,
            commands::ai_plan_operation,
            commands::rebase_branch,
            commands::rebase_continue,
            commands::rebase_skip,
            commands::rebase_abort,
            commands::get_interactive_plan,
            commands::start_interactive_rebase,
            commands::start_bisect,
            commands::bisect_mark,
            commands::bisect_skip,
            commands::bisect_reset,
            commands::blame_file,
            commands::file_history,
            commands::read_reflog,
            commands::history_index_build,
            commands::history_index_status,
            commands::history_search,
            commands::ai_search_history,
            commands::search_commits,
            commands::signing_status,
            commands::verify_commits,
            commands::get_config,
            commands::set_config,
            commands::unset_config,
            commands::apply_identity_profile,
            commands::list_stashes,
            commands::create_stash,
            commands::apply_stash,
            commands::pop_stash,
            commands::drop_stash,
            commands::commit_amend,
            commands::apply_composed_commits,
            commands::reset_branch,
            commands::discard_paths,
            commands::discard_paths_force,
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
            commands::register_mcp_with_claude,
            commands::list_submodules,
            commands::init_submodule,
            commands::update_submodule,
            commands::sync_submodule,
            commands::list_worktrees,
            commands::add_worktree,
            commands::remove_worktree,
            commands::lock_worktree,
            commands::unlock_worktree,
            commands::list_copy_candidates,
            commands::preview_worktree_copy,
            commands::add_worktree_with_changes,
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
            commands::list_worktree_contexts,
            commands::preview_worktree_profile,
            commands::activate_worktree_profile,
            commands::ai_generate_asset,
            commands::open_in_terminal,
            commands::reveal_in_file_manager,
            commands::open_in_editor
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
