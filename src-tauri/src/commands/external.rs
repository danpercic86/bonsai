//! `external` commands (P49): launch the OS terminal / file manager / editor at
//! a repo / worktree / submodule / tab path.
//!
//! House shape `X → launch_inner → spawn_blocking(core)`. The path arrives as a
//! raw string the frontend already owns — ANY existing directory the renderer
//! names, not necessarily the opened repo (P49 design; see the residual in
//! `bonsai_core::external_cmd`). Terminal/editor read their launch PROGRAM from
//! `settings.json`; reveal needs neither `AppHandle` nor state.
//! All git-state-free — no `repo_path`, no mutating/opActive gating.

use super::shared::*;
use bonsai_core::external::{self, SpawnRunner, TargetOs};
use bonsai_core::external_url;
use std::path::PathBuf;

/// Which launch to perform. Keeps `launch_inner` a single spawn_blocking body.
enum Action {
    Terminal,
    Reveal,
    Editor,
}

/// Open the OS terminal at `path`, using the configured `terminalCommand`
/// program (empty ⇒ per-OS auto-detect). Rejects `externalToolFailed` when the
/// configured program fails the shape rules (audit MEDIUM-2 —
/// `bonsai_core::external_cmd::validate_command_setting`, checked before
/// anything is spawned) or when no candidate launches, and `io` when `path` is
/// not an accessible directory.
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
/// VS Code family). Rejects `externalToolFailed` (invalid `editorCommand` shape,
/// or no candidate launched) / `io`.
#[tauri::command]
pub async fn open_in_editor(app: tauri::AppHandle, path: String) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    launch_inner(Some(file), Action::Editor, path).await
}

/// Open `url` in the user's default browser (P72). Web URLs only —
/// `bonsai_core::external_url::validate_web_url` rejects anything else BEFORE a
/// process is spawned. Deliberately NOT folded into `launch_inner`: it needs no
/// `AppHandle`, reads no settings program, and must SKIP the `path.exists()`
/// precheck (which would reject every URL). Rejects `externalToolFailed` for an
/// invalid URL or when no launcher succeeded.
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), AppError> {
    // spawn_blocking: `Command::spawn`/`status` blocks, and the macOS rung waits
    // on `open`'s exit code.
    tauri::async_runtime::spawn_blocking(move || {
        external_url::open_url(&SpawnRunner, TargetOs::host(), &url)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// spawn_blocking body shared by the three commands: (1) fs-precheck that `path`
/// still exists (→ `AppError::Io`); (2) for Terminal/Editor load the configured
/// PROGRAM from settings; (3) dispatch to the matching `external::` entry with
/// a real `SpawnRunner` + the host OS.
///
/// **`path` is any existing directory the renderer names** — `is_dir()` is the
/// only check, and it is NOT compared against the opened repo (unchanged P49
/// design, recorded here because the `external_cmd` residual depends on it: the
/// directory a configured program is pointed at is not bounded to the repo).
async fn launch_inner(
    settings_file: Option<PathBuf>,
    action: Action,
    path: String,
) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        // EXPLICIT directory precheck (audit 2026-09-03, MEDIUM-1): the target
        // must exist AND be a directory — it becomes the child's `cwd`
        // (`LaunchSpec` invariant). `is_dir()` is false for a missing path, for
        // a file, AND whenever the stat itself fails (permission denied, an
        // unreachable network share, a broken reparse point) — in that last
        // case the folder may exist perfectly well. So it subsumes the old
        // `exists()` check and stops relying on the accidental `NotADirectory`
        // spawn failure a file used to hit, and the message must stay true for
        // all three branches: "not accessible", not "does not exist".
        // LOW-2: the error is CATEGORY-ONLY and never echoes `path` — a
        // repo-authored path can be long / RTL-overridden / a system-message
        // lookalike, exactly the rule `validate_web_url` already documents.
        if !p.is_dir() {
            return Err(AppError::Io(
                "target folder is missing or not accessible".to_string(),
            ));
        }
        let os = TargetOs::host();
        let runner = SpawnRunner;
        match action {
            Action::Reveal => external::reveal_in_file_manager(&runner, os, p),
            Action::Terminal => {
                let program = settings_file
                    .map(|f| settings::load_from(&f).terminal_command)
                    .unwrap_or_default();
                external::open_in_terminal(&runner, os, &program, p)
            }
            Action::Editor => {
                let program = settings_file
                    .map(|f| settings::load_from(&f).editor_command)
                    .unwrap_or_default();
                external::open_in_editor(&runner, os, &program, p)
            }
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
