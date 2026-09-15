//! `external` commands (P49): launch the OS terminal / file manager / editor at
//! a repo / worktree / submodule / tab path.
//!
//! House shape `X → launch_inner → spawn_blocking(core)`. The path arrives as a
//! raw string the frontend already owns — ANY existing directory the renderer
//! names, not necessarily the opened repo (P49 design; P112 §8 residual 1).
//! Terminal/editor resolve their launch tool from `settings.json` through
//! `tools::picked`, which returns `None` (⇒ the per-OS auto ladder) for anything
//! that is not a currently-resolvable catalog id or a still-valid browsed path;
//! reveal needs neither `AppHandle` nor state.
//! All git-state-free — no `repo_path`, no mutating/opActive gating.
//!
//! **No program STRING reaches here from settings** (P112 §0). `terminal_tool` /
//! `editor_tool` are lookup keys into a compile-time catalog, and the one path
//! involved (`custom_*_path`) is written only by `pick_external_tool` from a
//! native dialog the backend opened.

use super::shared::*;
use bonsai_core::external::{self, SpawnRunner, TargetOs};
use bonsai_core::external_url;
use bonsai_core::tools::{self, BrowsedProgram, ToolKind};
use std::path::PathBuf;

/// Which launch to perform. Keeps `launch_inner` a single spawn_blocking body.
enum Action {
    Terminal,
    Reveal,
    Editor,
}

/// Open the OS terminal at `path`, using the selected `terminalTool` (`""`, an
/// unknown id, or a selection whose tool is gone ⇒ per-OS auto-detect — the OQ1
/// silent fallback). Rejects `externalToolFailed` when no candidate launches,
/// and `io` when `path` is not an accessible directory.
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

/// Open `path` in the selected editor (`editorTool` empty / unknown / gone ⇒
/// auto-detect the VS Code family). Rejects `externalToolFailed` (no candidate
/// launched) / `io`.
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
/// still exists (→ `AppError::Io`); (2) for Terminal/Editor resolve the SELECTED
/// TOOL from settings through `tools::picked`; (3) dispatch to the matching
/// `external::` entry with a real `SpawnRunner` + the host OS.
///
/// **`path` is any existing directory the renderer names** — `is_dir()` is the
/// only check, and it is NOT compared against the opened repo (unchanged P49
/// design, recorded here because P112 §8 residual 1 depends on it: the directory
/// the selected tool is pointed at is not bounded to the repo).
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
            // `picked` is BLOCKING but cheap: it reads the process-wide scan
            // cache (populating it on first use) and rechecks the one selected
            // row. It never re-runs the ladder — an `AppPaths` rung spawns
            // `reg.exe`, and a launch must not.
            Action::Terminal => {
                let picked = settings_file
                    .and_then(|f| picked_tool(&f, ToolKind::Terminal));
                external::open_in_terminal(&runner, os, picked.as_ref(), p)
            }
            Action::Editor => {
                let picked = settings_file.and_then(|f| picked_tool(&f, ToolKind::Editor));
                external::open_in_editor(&runner, os, picked.as_ref(), p)
            }
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Read the selection for `kind` out of `settings.json` and resolve it.
///
/// `None` ⇒ the per-OS auto ladder, for every miss: an empty setting, an id that
/// is not in the catalog, a wrong-kind id, `"custom"` with no stored path, and a
/// known id whose target no longer exists. The miss is SILENT by design (the OQ1
/// ruling: no error toast — "Open in editor" still opens something).
///
/// `custom_*_path` is wrapped in a [`BrowsedProgram`] rather than passed as a
/// `&str`, so a future handler cannot source it out of a request body instead of
/// the settings file.
fn picked_tool(file: &std::path::Path, kind: ToolKind) -> Option<tools::PickedTool> {
    let s = settings::load_from(file);
    let (setting, custom) = match kind {
        ToolKind::Terminal => (&s.terminal_tool, &s.custom_terminal_path),
        ToolKind::Editor => (&s.editor_tool, &s.custom_editor_path),
    };
    tools::picked(setting, kind, BrowsedProgram::from_settings_field(custom))
}
