//! `tools` commands (P112 §6): the external-tool picker's data, and the
//! **native** program-browse dialog.
//!
//! Two commands, both git-state-free (no `repo_path`, no mutating/opActive
//! gating):
//!
//! * [`list_external_tools`] — one round trip carrying both detected lists, both
//!   AMEND-1 label maps and `scannedAtMs`. It NEVER rejects for detection state:
//!   an empty scan is a normal result (the `check_git_availability` precedent).
//! * [`pick_external_tool`] — opens the OS file dialog **itself**, validates the
//!   result, and writes both the path and the selection.
//!
//! ## Why the dialog is opened here and not by the renderer
//!
//! This is the F4 rule already forced on observability export destinations
//! (`commands/obs.rs`: "the path must come from a Tauri dialog invoked by the
//! BACKEND, whose result the webview never chooses"). The renderer supplies only
//! a [`ToolKind`] — it *asks for* a dialog, it does not *supply a result* — so a
//! renderer-written program path is not "rejected by a validator", it is
//! **unrepresentable**: `UiSettingsPatch` has no field that can carry one, and
//! this command takes no path argument.
//!
//! The `openRepo` shape (frontend calls `@tauri-apps/plugin-dialog`, sends the
//! path down) is deliberately NOT used here. It makes the dialog advisory and
//! the value arriving at the backend a renderer-supplied string again. A repo
//! path is data; a program path is code.
//!
//! **Stated honestly:** a browsed `.exe` is arbitrary code. The property bought
//! is "a human chose it in a native dialog the backend opened", not "it is
//! harmless".

use super::shared::*;
use bonsai_core::external::TargetOs;
use bonsai_core::tools::{self, BrowsedProgram, DetectedTool, ExternalToolScan, ToolKind};
use tauri_plugin_dialog::DialogExt;

/// Detected tools + the remembered browsed row + the AMEND-1 label maps.
///
/// `refresh: true` re-probes and replaces the process cache (the picker's
/// Rescan), so `scannedAtMs` advances and a tool installed since the last scan
/// appears. Rejects only `other` (app-config-dir resolution / task join) — never
/// for detection state.
///
/// Repeating the request does not multiply the work: `bonsai_core::tools`
/// coalesces concurrent probes onto one, so N in-flight refreshes hold N
/// blocking threads but run a single `reg.exe` sweep between them.
#[tauri::command]
pub async fn list_external_tools(
    app: tauri::AppHandle,
    refresh: bool,
) -> Result<ExternalToolScan, AppError> {
    let file = settings::settings_file(&app)?;
    // spawn_blocking: the probe ladders stat the filesystem and run `reg.exe` on
    // Windows, and `load_from` reads settings.json.
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        // Typed, so a request-body `&str` cannot type-check into a scan.
        let terminal = BrowsedProgram::from_settings_field(&s.custom_terminal_path);
        let editor = BrowsedProgram::from_settings_field(&s.custom_editor_path);
        if refresh {
            tools::refresh_tool_scan(terminal, editor)
        } else {
            tools::tool_scan(terminal, editor)
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Open the NATIVE program picker and, on confirm, write BOTH
/// `custom_<kind>_path` AND `<kind>_tool = "custom"`.
///
/// `Ok(None)` = the user cancelled: nothing is written. A path that fails
/// `tools::browsed_tool_row` writes NOTHING either — not the path, and not the
/// selection — and rejects with the category-only refusal, which never echoes
/// the path (the UI shows its own copy, `P112-ui.md` §8 `BROWSE_ERR`).
///
/// After this resolves non-null the frontend re-reads settings; it must **not**
/// also patch `{ editorTool: 'custom' }`, or it races this write.
#[tauri::command]
pub async fn pick_external_tool(
    app: tauri::AppHandle,
    kind: ToolKind,
) -> Result<Option<DetectedTool>, AppError> {
    let file = settings::settings_file(&app)?;
    let os = TargetOs::host();
    let Some(chosen) = open_program_dialog(&app, kind, os).await else {
        return Ok(None);
    };
    // spawn_blocking: validation stats the filesystem — and since ruling #26
    // admits UNC here, that stat can block for a full SMB timeout on a
    // disconnected share. Synchronously it would freeze the command loop.
    tauri::async_runtime::spawn_blocking(move || {
        super::tools_pick::commit_browsed_tool(&file, kind, &chosen, os).map(Some)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// The one untestable step: the OS file dialog. Bridged to `async` through a
/// `oneshot`, because the blocking variant must not run on the async runtime and
/// the dialog must be created on the main thread on some platforms.
///
/// A dropped sender (the callback never ran) reads as a cancel rather than
/// panicking: `Ok(None)` is already the "nothing happened" answer, and a
/// `.expect()` here would crash the app on a platform quirk.
///
/// **Only the native window can confirm this function** (UC6 / UC-UI-2): that
/// the dialog appears, that the filter reads as DEC-1 specifies, that a `.app`
/// bundle is selectable as one item on macOS, and that focus returns to the
/// Browse button afterwards.
async fn open_program_dialog(
    app: &tauri::AppHandle,
    kind: ToolKind,
    os: TargetOs,
) -> Option<std::path::PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut builder = app.dialog().file().set_title(match kind {
        ToolKind::Terminal => "Choose a terminal program",
        ToolKind::Editor => "Choose an editor program",
    });
    builder = match os {
        // DEC-1: `.exe` only, excluding `.cmd`/`.bat` — Rust's `Command` routes
        // those through `cmd.exe`, which performs `%VAR%` expansion on the argv
        // it receives (the CVE-2024-24576 path). The filter is NOT the gate
        // (the user can type a name in the box, and a filter is one "add All
        // files" change away from gone): `validate_custom_program` refuses a
        // non-`.exe` extension, and that check is what the tests pin.
        TargetOs::Windows => builder.add_filter("Programs (*.exe)", &["exe"]),
        // `.app` bundles plus extensionless unix binaries, which no extension
        // filter can express — the execute-bit check is the real gate.
        TargetOs::MacOs => builder
            .add_filter("Applications (*.app)", &["app"])
            .add_filter("All files", &["*"]),
        // No filter: a Linux program has no conventional extension.
        TargetOs::Linux => builder,
    };
    builder.pick_file(move |picked| {
        let _ = tx.send(picked);
    });
    rx.await.ok().flatten().and_then(|f| f.into_path().ok())
}
