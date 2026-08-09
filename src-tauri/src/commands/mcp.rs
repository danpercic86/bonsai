//! `mcp` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Current embedded-MCP server status for the Settings panel (P16 §10.1).
#[tauri::command]
pub async fn get_mcp_status(
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
) -> Result<crate::mcp::McpStatus, AppError> {
    Ok(crate::mcp::status_of(&mcp_state))
}

/// Starts or stops the embedded MCP server (P16 §6). Read-only in P16b (the
/// write-gate is P16c). Returns the resulting status; emits `mcp-server-changed`.
#[tauri::command]
pub async fn set_mcp_enabled(
    app: tauri::AppHandle,
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
    enabled: bool,
) -> Result<crate::mcp::McpStatus, AppError> {
    crate::mcp::set_enabled(&app, &mcp_state, enabled).await
}

/// Flips the embedded-MCP write-gate (P16c §9). Persists `mcp_allow_write` and,
/// if the server is running, BOUNCES it (stop + restart on the same token/port)
/// so the 20 mutation tools (de)register and live sessions re-negotiate.
/// Returns the resulting status; emits `mcp-server-changed`.
#[tauri::command]
pub async fn set_mcp_allow_write(
    app: tauri::AppHandle,
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
    allow_write: bool,
) -> Result<crate::mcp::McpStatus, AppError> {
    crate::mcp::set_allow_write(&app, &mcp_state, allow_write).await
}

/// Registers Bonsai's running embedded MCP server with the local `claude` CLI
/// (P16). Reads the live `url` + `token` from the running server (errors if the
/// server is not enabled). `scope` is `"user"` (register globally) or `"local"`
/// (register in the open repo, private/not committed). cwd = `repo_path` when
/// given (required for a meaningful `local` registration), else the process cwd.
/// The `claude mcp add` argv is built in `bonsai-core` as an argument list, so
/// the variadic `--header` cannot swallow the URL. A provided `repo_path` is
/// prechecked to exist as a directory (T2.1 BUG-3). Errors:
/// `aiUnavailable` | `aiFailed` | `io` (missing repo dir) | `other`.
#[tauri::command]
pub async fn register_mcp_with_claude(
    scope: String,
    repo_path: Option<String>,
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
) -> Result<(), AppError> {
    let status = crate::mcp::status_of(&mcp_state);
    let (url, token) = match (status.url, status.token) {
        (Some(u), Some(t)) => (u, t),
        _ => return Err(AppError::Other("MCP server is not running".to_string())),
    };
    let cwd = resolve_register_cwd(repo_path)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai::register_with_claude(&url, &token, &scope, &cwd)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resolves the cwd for `register_mcp_with_claude` (T2.1 BUG-3): a provided
/// `repo_path` must be an existing directory — a deleted/mistyped path returns
/// a clean `AppError::Io` (same shape as `read_repo_info`'s missing-path
/// error) instead of surfacing a raw OS spawn failure from the `claude` CLI.
/// `None` falls back to the process cwd (unchanged).
pub(crate) fn resolve_register_cwd(
    repo_path: Option<String>,
) -> Result<std::path::PathBuf, AppError> {
    match repo_path {
        Some(p) => {
            let path = std::path::PathBuf::from(p);
            if !path.is_dir() {
                return Err(AppError::Io(format!(
                    "path does not exist or is not a directory: {}",
                    path.display()
                )));
            }
            Ok(path)
        }
        None => std::env::current_dir()
            .map_err(|e| AppError::Other(format!("could not resolve current dir: {e}"))),
    }
}
