//! Embedded MCP server (P16). Runs an MCP server *inside* the Tauri app over
//! streamable-HTTP bound to `127.0.0.1:<port>`, so an external client (Claude
//! Code) operates on the SAME live repos the user has open (`AppState.repos`),
//! and the GUI live-updates via the existing `repo-changed` watcher.
//!
//! The write-gate (`mcp_allow_write`) is a real, consented, default-OFF toggle
//! (P16c): when OFF the server registers only the 14 read tools (P14's 12 + the
//! two P16 repo-selection tools); when ON it also merges the 20 mutation tools
//! (34 total). Any change to the gate BOUNCES the server (stop + restart on the
//! SAME persisted token + port, D-5) so already-connected sessions are dropped
//! and re-negotiate the new tool set — this is what revokes write from live
//! sessions when the gate is turned OFF.
//!
//! Security (P16 §8, LOAD-BEARING): a bearer token (persisted, 32 CSPRNG bytes,
//! base64url) is required on every request, ANY request carrying an `Origin`
//! header is rejected (D-3), the `Host` must be exactly `127.0.0.1:<port>` /
//! `localhost:<port>` (DNS-rebinding), and no CORS headers are ever emitted. The
//! transport wiring (`StreamableHttpService` + axum) lives here so `bonsai-mcp`
//! stays transport-agnostic (D-1).
//!
//! Launch behaviour (P44a): the server is auto-started at launch ONLY when the
//! persisted `mcp_enabled` flag is true — i.e. the user consented and turned it
//! on in a prior session; it is still never started without that prior explicit
//! opt-in. Privacy posture: this re-opens the 127.0.0.1, bearer-token-gated
//! listener at launch purely by the user's earlier choice (never silently on a
//! fresh, never-enabled install). Turning it OFF persists `mcp_enabled=false`,
//! so the next launch stays down.


use std::sync::{Arc, Mutex};

use bonsai_core::error::AppError;
use bonsai_mcp::{BonsaiServer, OpenRepo};
use tauri::{AppHandle, Emitter, Manager};

use crate::settings;
use crate::state::AppState;

mod server;
mod token;

pub(crate) use server::spawn_server;
pub(crate) use token::token_for_bound_port;
// base64url/token generation are exercised only by the sibling test module.
#[cfg(test)]
pub(crate) use token::{base64url_nopad, generate_token};

/// Read-tool count (P14's 12 + the two P16 repo-selection tools), DERIVED from
/// the live `bonsai-mcp` read router so it can never drift from the actual tool
/// set (F-A8-b — previously a hand-maintained `const 14`).
fn read_tool_count() -> u32 {
    BonsaiServer::read_tool_count() as u32
}

/// Mutation-tool count (registered only when the write-gate is on — P16c),
/// derived from the live write router.
fn write_tool_count() -> u32 {
    BonsaiServer::write_tool_count() as u32
}

fn tool_count(allow_write: bool) -> u32 {
    if allow_write {
        read_tool_count() + write_tool_count()
    } else {
        read_tool_count()
    }
}

/// Map a poisoned lock to a domain error rather than panicking.
fn pois<T>(_: std::sync::PoisonError<T>) -> AppError {
    AppError::Other("MCP state lock poisoned".to_string())
}

/// A running embedded MCP server (P16 §6.1). `None` in [`McpServerState`] means
/// stopped.
pub struct McpRunning {
    pub port: u16,
    pub token: String,
    pub allow_write: bool,
    /// Fires graceful shutdown of the axum serve task. `Option` so `stop` can
    /// take it exactly once.
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    /// The axum serve task.
    task: tauri::async_runtime::JoinHandle<()>,
}

impl McpRunning {
    /// Signal graceful shutdown, then abort the serve task as a backstop.
    fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        self.task.abort();
    }
}

/// Managed handle for the embedded MCP server, separate from [`AppState`].
#[derive(Default)]
pub struct McpServerState {
    pub inner: Mutex<Option<McpRunning>>,
    /// Serializes the WHOLE enable/disable/bounce path (audit §3.6). The sync
    /// `inner` lock cannot be held across `start().await`, so without this
    /// guard the P44a launch auto-start and a user toggle could both pass the
    /// "already running" check (which only sees `inner` — populated at the END
    /// of `start`) and bind two servers, persisting the dead one's token/port.
    start_guard: tokio::sync::Mutex<()>,
}

/// The server status surfaced to the Settings panel (P16 §10.3). `camelCase`
/// mirrors the TS `McpStatus`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// Server running?
    pub enabled: bool,
    /// Write tools registered? Reflects the running server's live gate (P16c).
    pub allow_write: bool,
    /// Bound port when running, else `None`.
    pub port: Option<u16>,
    /// e.g. `"http://127.0.0.1:8765/mcp"`; `None` when stopped.
    pub url: Option<String>,
    /// Persisted bearer token; `None` when stopped.
    pub token: Option<String>,
    /// 14 (read-only) or 34 (write enabled).
    pub tool_count: u32,
}

fn endpoint_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

fn running_status(r: &McpRunning) -> McpStatus {
    McpStatus {
        enabled: true,
        allow_write: r.allow_write,
        port: Some(r.port),
        url: Some(endpoint_url(r.port)),
        token: Some(r.token.clone()),
        tool_count: tool_count(r.allow_write),
    }
}

fn stopped_status() -> McpStatus {
    McpStatus {
        enabled: false,
        allow_write: false,
        port: None,
        url: None,
        token: None,
        tool_count: read_tool_count(),
    }
}

/// Current status (for `get_mcp_status`). A poisoned lock degrades to "stopped".
pub fn status_of(state: &McpServerState) -> McpStatus {
    match state.inner.lock() {
        Ok(g) => match g.as_ref() {
            Some(r) => running_status(r),
            None => stopped_status(),
        },
        Err(_) => stopped_status(),
    }
}

/// Start or stop the server (for `set_mcp_enabled`). Returns the resulting
/// status; emits `mcp-server-changed`.
pub async fn set_enabled(
    app: &AppHandle,
    mcp_state: &McpServerState,
    enabled: bool,
) -> Result<McpStatus, AppError> {
    // Hold the start-guard across the whole path (check → bind → insert) so a
    // racing enable (launch auto-start vs user toggle, audit §3.6) cannot pass
    // the "already running" check twice and start two servers.
    let _start = mcp_state.start_guard.lock().await;
    if enabled {
        // Already running → return current status (idempotent enable). Checked
        // AFTER acquiring the guard: a racer that just finished `start` has
        // inserted into `inner` by the time we get here.
        {
            let g = mcp_state.inner.lock().map_err(pois)?;
            if let Some(r) = g.as_ref() {
                return Ok(running_status(r));
            }
        }
        start_or_signal_stopped(app, mcp_state).await
    } else {
        stop(app, mcp_state).await
    }
}

/// Run [`start`], but on failure leave a CONSISTENT stopped state before
/// surfacing the error (F-A8-c). A failed start (or a failed restart during a
/// bounce) leaves the server DOWN — `inner` is `None` and no listener is bound —
/// yet without this the persisted `mcp_enabled` would stay `true` and no
/// `mcp-server-changed` would fire, so the Settings UI would keep showing
/// "running" over a dead server (and P44a would try to auto-start it again next
/// launch). We persist `mcp_enabled = false` (best-effort) and emit a stopped
/// status, then return the original error so the caller still sees the failure.
async fn start_or_signal_stopped(
    app: &AppHandle,
    mcp_state: &McpServerState,
) -> Result<McpStatus, AppError> {
    match start(app, mcp_state).await {
        Ok(status) => Ok(status),
        Err(e) => {
            if let Ok(file) = settings::settings_file(app) {
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    let _ = settings::update(&file, |s| s.mcp_enabled = false);
                })
                .await;
            }
            let _ = app.emit("mcp-server-changed", stopped_status());
            Err(e)
        }
    }
}

/// Flip the write-gate (for `set_mcp_allow_write`, P16 §9 / D-5). Persists
/// `mcp_allow_write`, then — if the server is running — BOUNCES it: tears down
/// the current serve task (which drops its listener and all live sessions) and
/// restarts on the SAME persisted token + port with the new gate, so clients
/// reconnect and re-negotiate the now-14-or-34 tool set. When stopped, only the
/// setting is persisted (the next `start` picks it up). Returns the resulting
/// status; emits `mcp-server-changed`.
///
/// Turning the gate OFF is the security-relevant direction: the bounce fully
/// tears down the old server (abort drops its listener + sessions) BEFORE the
/// new read-only server binds, so no already-connected session keeps write.
pub async fn set_allow_write(
    app: &AppHandle,
    mcp_state: &McpServerState,
    allow_write: bool,
) -> Result<McpStatus, AppError> {
    // Same start-guard as `set_enabled` (audit §3.6): the bounce below is a
    // stop+start and must not interleave with a concurrent enable.
    let _start = mcp_state.start_guard.lock().await;

    // Persist the gate first so the (re)start below — and any future start when
    // stopped — reads the new value. Serialized load→mutate→save (audit §2.3).
    let file = settings::settings_file(app)?;
    tauri::async_runtime::spawn_blocking(move || {
        settings::update(&file, |s| s.mcp_allow_write = allow_write).map(|_| ())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    // Take the running server (if any) out of the mutex and tear it down BEFORE
    // rebinding — abort drops its listener + sessions.
    let running = {
        let mut g = mcp_state.inner.lock().map_err(pois)?;
        g.take()
    };
    match running {
        Some(r) => {
            r.stop();
            // Restart reuses the persisted token + port (D-4); `start` reads the
            // just-persisted `mcp_allow_write` and `bind_listener` retries the
            // same port to ride out the brief post-abort release window. If the
            // restart FAILS the old server is already gone, so signal a stopped
            // state rather than leaving the UI showing a dead-but-enabled server
            // (F-A8-c).
            start_or_signal_stopped(app, mcp_state).await
        }
        None => {
            // Stopped: nothing to bounce; the setting is persisted for next start.
            let status = stopped_status();
            let _ = app.emit("mcp-server-changed", status.clone());
            Ok(status)
        }
    }
}

/// Stop the server synchronously (app-exit hook — P16 §6.3). No settings write,
/// no event: the app is going away.
pub fn shutdown(mcp_state: &McpServerState) {
    if let Ok(mut g) = mcp_state.inner.lock() {
        if let Some(r) = g.take() {
            r.stop();
        }
    }
}

/// Thin wrapper over [`spawn_server`]: adds only the app-facing glue — read the
/// persisted token/port/write-gate from settings, snapshot `AppState` into the
/// `list_open` + `seed` closures, then (after the core binds) persist the actual
/// port + `mcp_enabled=true` and emit `mcp-server-changed`.
async fn start(app: &AppHandle, mcp_state: &McpServerState) -> Result<McpStatus, AppError> {
    let file = settings::settings_file(app)?;

    // Load the persisted token + preferred port + write-gate (D-4 / P16c).
    let file_load = file.clone();
    let (token_opt, port_opt, allow_write) = tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file_load);
        (s.mcp_token, s.mcp_port, s.mcp_allow_write)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?;

    // AppState glue: seed the per-session selection from the focused tab
    // (`active_repo`) and snapshot the open tabs (`repos`) at call time. A
    // poisoned lock degrades to "no selection" / "no repos" rather than failing.
    let seed_app = app.clone();
    let seed: Arc<dyn Fn() -> Option<String> + Send + Sync> = Arc::new(move || {
        seed_app
            .state::<AppState>()
            .active_repo
            .lock()
            .ok()
            .and_then(|g| g.clone())
    });
    let list_app = app.clone();
    let list_open: Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync> = Arc::new(move || {
        let st = list_app.state::<AppState>();
        // Poison recovery on AppState.repos (audit §3.8) — the map is
        // structurally valid at every point; degrading to "no repos" forever
        // would silently hide every open tab from connected sessions.
        let open: Vec<OpenRepo> = st
            .repos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(id, e)| OpenRepo {
                repo_id: id.clone(),
                path: e.path.clone(),
            })
            .collect();
        open
    });

    let running = spawn_server(
        move |actual| token_for_bound_port(token_opt, port_opt, actual),
        allow_write,
        port_opt,
        list_open,
        seed,
    )
    .await?;
    let actual_port = running.port;
    let token_save = running.token.clone();

    // Persist token + actual port + enabled=true (stable `claude mcp add`, D-4).
    // Serialized load→mutate→save (audit §2.3).
    let file_save = file.clone();
    tauri::async_runtime::spawn_blocking(move || {
        settings::update(&file_save, |s| {
            s.mcp_token = Some(token_save);
            s.mcp_port = Some(actual_port);
            s.mcp_enabled = true;
        })
        .map(|_| ())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    let status = running_status(&running);
    {
        let mut g = mcp_state.inner.lock().map_err(pois)?;
        *g = Some(running);
    }
    let _ = app.emit("mcp-server-changed", status.clone());
    Ok(status)
}

async fn stop(app: &AppHandle, mcp_state: &McpServerState) -> Result<McpStatus, AppError> {
    let running = {
        let mut g = mcp_state.inner.lock().map_err(pois)?;
        g.take()
    };
    if let Some(r) = running {
        r.stop();
    }

    // Persist enabled=false (best-effort — a failed write must not block stop).
    // Serialized load→mutate→save (audit §2.3).
    if let Ok(file) = settings::settings_file(app) {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let _ = settings::update(&file, |s| s.mcp_enabled = false);
        })
        .await;
    }

    let status = stopped_status();
    let _ = app.emit("mcp-server-changed", status.clone());
    Ok(status)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod http_support;
#[cfg(test)]
mod http_tests;
