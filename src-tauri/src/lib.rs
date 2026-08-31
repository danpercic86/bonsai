pub mod commands;
pub mod mcp;
/// P71 guards on `tauri.conf.json` (strict JSON, so it cannot carry comments).
#[cfg(test)]
#[path = "bundle_config_tests.rs"]
mod bundle_config_tests;
/// P69 §3.2 (OQ-2) guard: Rust `Settings::default()` ⟷ the TS-owned
/// `src/settings/uiSettingsDefaults.json` oracle. Declared here rather than as a
/// child of `settings` only because `settings.rs` is over the file-size ratchet's
/// limit and may not grow; it reaches into `settings` + `commands` by path.
#[cfg(test)]
#[path = "settings_defaults_parity_tests.rs"]
mod settings_defaults_parity_tests;
pub mod graph_cache;
pub mod perf;
pub mod repo_handle;
pub mod scheduler;
pub mod settings;
pub mod state;
pub mod watcher;

use tauri::Manager;

pub fn run() {
    // P71 R2 (BACKSTOP, not the fix — the fix is shipping NSIS only): repair a
    // PATH inherited from an installer before anything spawns a child or caches
    // a resolution. MUST be the first statement — it calls `std::env::set_var`,
    // which is only sound while the process is still single-threaded, so it has
    // to precede every thread spawn, the async runtime, and the Tauri builder.
    // It must also precede `gitbin`'s process-lifetime cache so the P70 ladder
    // sees the repaired PATH (and reports `source: "path"`, the C-1 oracle).
    // Silent no-op on non-Windows and on any registry read failure.
    //
    // The one debug line the contract permits (§5.4 constraint 5): it is the
    // only oracle acceptance criterion C-2 has for "R2 repaired this client in
    // place". COUNTS ONLY — the recovered directory names are user paths and
    // must never reach a log.
    let rehydration = bonsai_core::winenv::rehydrate_path_once();
    if rehydration.applied {
        eprintln!(
            "bonsai: rehydrated PATH from the registry ({} entries appended)",
            rehydration.added.len()
        );
    }
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
        // P68 §B/D7: the streaming-AI run registry (run id -> cancel/reply
        // handles). Managed state because `ai_cancel_run` / `ai_reply_run` are
        // SEPARATE commands from the run they control — a Tauri command cannot be
        // aborted from JS. Cleared on exit below.
        .manage(bonsai_core::ai::AiRunRegistry::default())
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
            commands::stream_graph,
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
            commands::get_image_diff,
            commands::list_branches,
            commands::create_branch,
            commands::create_branch_here,
            commands::checkout_branch,
            commands::checkout_commit,
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
            commands::git_activity_subscribe,
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
            commands::ai_apply_resolution,
            commands::check_ai_availability,
            commands::ai_resolve_conflict,
            commands::ai_resolve_conflict_stream,
            commands::ai_cancel_run,
            commands::ai_reply_run,
            commands::generate_commit_message,
            commands::ai_analyze_diff,
            commands::ai_summarize_range,
            commands::ai_digest,
            commands::ai_changelog,
            commands::ai_generate_pr_description,
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
            commands::describe_last_undo,
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
            commands::add_submodule,
            commands::deinit_submodule,
            commands::remove_submodule,
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
            commands::list_tag_sync,
            commands::auto_sync_tags,
            commands::force_refresh_tag,
            commands::delete_remote_tag,
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
            commands::open_in_editor,
            commands::open_url,
            commands::check_git_availability,
            commands::forge_repo_context,
            commands::forge_list_prs,
            commands::forge_get_pr,
            commands::forge_pr_diff,
            commands::forge_pr_file_diff,
            commands::forge_create_pr,
            commands::forge_merge_pr,
            commands::forge_close_pr,
            commands::forge_list_review_comments,
            commands::forge_set_token,
            commands::forge_clear_token,
            commands::forge_commit_statuses,
            commands::forge_list_accounts,
            commands::forge_set_token_for_host,
            commands::forge_add_account,
            commands::forge_remove_account,
            commands::forge_set_host_default,
            commands::forge_set_repo_account,
            commands::forge_clear_token_for_host,
            commands::forge_invalidate_viewer,
            commands::get_repo_hooks_disclosure,
            commands::ack_repo_hooks,
            commands::debug_perf_counters,
            commands::debug_reset_perf_counters
        ])
        .build(tauri::generate_context!())
        .expect("error while running Bonsai")
        .run(|app, event| {
            // Release the MCP port on exit (P16 §6.3): stop the embedded server
            // before the app process goes away.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                mcp::shutdown(&app.state::<mcp::McpServerState>());
                // P68 §B/D7: flip every cancel flag AND kill the recorded child
                // TREES. A streaming run has NO wall-clock deadline by design, so
                // without this a `claude` child (and the node process behind the
                // npm shim) could outlive the window indefinitely.
                app.state::<bonsai_core::ai::AiRunRegistry>().cancel_all();
            }
        });
}
