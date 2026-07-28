pub mod commands;
pub mod error;
#[doc(hidden)]
pub mod fixture;
pub mod git;
pub mod graph;
pub mod state;
#[cfg(test)]
pub mod testutil;
pub mod watcher;

pub fn run() {
    git::relax_odb_hash_verification();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_repo,
            commands::get_status,
            commands::get_graph,
            commands::stage,
            commands::unstage,
            commands::commit,
            commands::get_workdir_file_diff,
            commands::get_commit_diff,
            commands::get_commit_file_diff
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bonsai");
}
