//! `external` commands (P49): launch the OS terminal / file manager / editor at
//! a repo / worktree / submodule / tab path.
//!
//! House shape `X → launch_inner → spawn_blocking(core)`. The path arrives as a
//! raw string the frontend already owns. Terminal/editor read their command
//! template from `settings.json`; reveal needs neither `AppHandle` nor state.
//! All git-state-free — no `repo_path`, no mutating/opActive gating.

use super::shared::*;
use bonsai_core::external::{self, SpawnRunner, TargetOs};
use std::path::PathBuf;

/// Which launch to perform. Keeps `launch_inner` a single spawn_blocking body.
enum Action {
    Terminal,
    Reveal,
    Editor,
}

/// Open the OS terminal at `path`, using the configured `terminalCommand`
/// template (empty ⇒ per-OS auto-detect). Rejects `externalToolFailed` when no
/// candidate launches, or `io` when `path` no longer exists.
#[tauri::command]
pub async fn open_in_terminal(app: tauri::AppHandle, path: String) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    launch_inner(Some(file), Action::Terminal, path).await
}

/// Reveal `path` (a directory) in the OS file manager. Rejects
/// `externalToolFailed` / `io`.
#[tauri::command]
pub async fn reveal_in_file_manager(path: String) -> Result<(), AppError> {
    launch_inner(None, Action::Reveal, path).await
}

/// Open `path` in the configured editor (empty `editorCommand` ⇒ auto-detect the
/// VS Code family). Rejects `externalToolFailed` / `io`.
#[tauri::command]
pub async fn open_in_editor(app: tauri::AppHandle, path: String) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    launch_inner(Some(file), Action::Editor, path).await
}

/// spawn_blocking body shared by the three commands: (1) fs-precheck that `path`
/// still exists (→ `AppError::Io`); (2) for Terminal/Editor load the configured
/// template from settings; (3) dispatch to the matching `external::` entry with
/// a real `SpawnRunner` + the host OS.
async fn launch_inner(
    settings_file: Option<PathBuf>,
    action: Action,
    path: String,
) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        if !p.exists() {
            return Err(AppError::Io(format!("path no longer exists: {path}")));
        }
        let os = TargetOs::host();
        let runner = SpawnRunner;
        match action {
            Action::Reveal => external::reveal_in_file_manager(&runner, os, p),
            Action::Terminal => {
                let template = settings_file
                    .map(|f| settings::load_from(&f).terminal_command)
                    .unwrap_or_default();
                external::open_in_terminal(&runner, os, &template, p)
            }
            Action::Editor => {
                let template = settings_file
                    .map(|f| settings::load_from(&f).editor_command)
                    .unwrap_or_default();
                external::open_in_editor(&runner, os, &template, p)
            }
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
