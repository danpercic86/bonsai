pub mod commands;
pub mod error;
pub mod git;
pub mod state;
pub mod watcher;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_repo,
            commands::get_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bonsai");
}
