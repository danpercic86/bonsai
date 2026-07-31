//! Embedded MCP server (P16). Runs an MCP server *inside* the Tauri app over
//! streamable-HTTP bound to `127.0.0.1:<port>`, so an external client (Claude
//! Code) operates on the SAME live repos the user has open (`AppState.repos`),
//! and the GUI live-updates via the existing `repo-changed` watcher.
//!
//! P16b is **read-only**: the write-gate is forced OFF here regardless of the
//! persisted `mcp_allow_write` setting (write tools + the toggle are P16c). The
//! server exposes the 14 read tools (P14's 12 + the two P16 repo-selection
//! tools).
//!
//! Security (P16 §8, LOAD-BEARING): a bearer token (persisted, 32 CSPRNG bytes,
//! base64url) is required on every request, ANY request carrying an `Origin`
//! header is rejected (D-3), the `Host` must be exactly `127.0.0.1:<port>` /
//! `localhost:<port>` (DNS-rebinding), and no CORS headers are ever emitted. The
//! transport wiring (`StreamableHttpService` + axum) lives here so `bonsai-mcp`
//! stays transport-agnostic (D-1).

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    Router,
};
use rand::RngCore;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;

use bonsai_core::error::AppError;
use bonsai_mcp::{BonsaiServer, OpenRepo, SessionRepos};
use tauri::{AppHandle, Emitter, Manager};

use crate::settings;
use crate::state::AppState;

/// Read-tool count (P14's 12 + the two P16 repo-selection tools).
const READ_TOOL_COUNT: u32 = 14;
/// Mutation-tool count (registered only when the write-gate is on — P16c).
const WRITE_TOOL_COUNT: u32 = 20;

fn tool_count(allow_write: bool) -> u32 {
    if allow_write {
        READ_TOOL_COUNT + WRITE_TOOL_COUNT
    } else {
        READ_TOOL_COUNT
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
}

/// The server status surfaced to the Settings panel (P16 §10.3). `camelCase`
/// mirrors the TS `McpStatus`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// Server running?
    pub enabled: bool,
    /// Write tools registered? (Always `false` in P16b.)
    pub allow_write: bool,
    /// Bound port when running, else `None`.
    pub port: Option<u16>,
    /// e.g. `"http://127.0.0.1:8765/mcp"`; `None` when stopped.
    pub url: Option<String>,
    /// Persisted bearer token; `None` when stopped.
    pub token: Option<String>,
    /// Ready-to-paste `claude mcp add` line; `None` when stopped.
    pub claude_add_command: Option<String>,
    /// 14 (read-only) or 34 (write enabled).
    pub tool_count: u32,
}

fn endpoint_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

/// The one-time `claude mcp add` registration line (P16 §10.3).
fn claude_add_command(port: u16, token: &str) -> String {
    format!(
        "claude mcp add bonsai --transport http --header \"Authorization: Bearer {token}\" {}",
        endpoint_url(port)
    )
}

fn running_status(r: &McpRunning) -> McpStatus {
    McpStatus {
        enabled: true,
        allow_write: r.allow_write,
        port: Some(r.port),
        url: Some(endpoint_url(r.port)),
        token: Some(r.token.clone()),
        claude_add_command: Some(claude_add_command(r.port, &r.token)),
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
        claude_add_command: None,
        tool_count: READ_TOOL_COUNT,
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
    if enabled {
        // Already running → return current status (idempotent enable).
        {
            let g = mcp_state.inner.lock().map_err(pois)?;
            if let Some(r) = g.as_ref() {
                return Ok(running_status(r));
            }
        }
        start(app, mcp_state).await
    } else {
        stop(app, mcp_state).await
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

/// 32 CSPRNG bytes, base64url (no padding) — ~256 bits (P16 §8.2).
fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    base64url_nopad(&buf)
}

/// Minimal base64url (RFC 4648 §5) encoder, no padding. Avoids a base64 dep for
/// the single token-encoding use.
fn base64url_nopad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 63) as usize] as char);
        }
    }
    out
}

/// Bind `127.0.0.1:<preferred>`, falling back to an OS-chosen ephemeral port if
/// the preferred one is busy (P16 §8.5).
async fn bind_listener(preferred: Option<u16>) -> Result<TcpListener, AppError> {
    if let Some(p) = preferred {
        if p != 0 {
            if let Ok(l) = TcpListener::bind((Ipv4Addr::LOCALHOST, p)).await {
                return Ok(l);
            }
        }
    }
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|e| AppError::Io(format!("bind 127.0.0.1:0: {e}")))
}

fn io_err(msg: &str) -> std::io::Error {
    std::io::Error::other(msg)
}

async fn start(app: &AppHandle, mcp_state: &McpServerState) -> Result<McpStatus, AppError> {
    let file = settings::settings_file(app)?;

    // Load the persisted token + preferred port (D-4).
    let file_load = file.clone();
    let (token_opt, port_opt) = tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file_load);
        (s.mcp_token, s.mcp_port)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?;

    let token = token_opt.unwrap_or_else(generate_token);
    // P16b: write-gate forced OFF regardless of the persisted setting.
    let allow_write = false;

    let listener = bind_listener(port_opt).await?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| AppError::Io(e.to_string()))?
        .port();

    // Persist token + actual port + enabled=true (stable `claude mcp add`, D-4).
    let file_save = file.clone();
    let token_save = token.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file_save);
        s.mcp_token = Some(token_save);
        s.mcp_port = Some(actual_port);
        s.mcp_enabled = true;
        settings::save_to(&file_save, &s)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    // Per-session service factory (P16 §5): each new MCP session seeds its
    // selection from the app's focused tab (`active_repo`) and enumerates the
    // open tabs (`repos`) at call time via an AppHandle-scoped closure.
    let factory_app = app.clone();
    let factory = move || -> Result<BonsaiServer, std::io::Error> {
        let seed = {
            let st = factory_app.state::<AppState>();
            let g = st
                .active_repo
                .lock()
                .map_err(|_| io_err("active_repo lock poisoned"))?;
            g.clone()
        };
        let list_app = factory_app.clone();
        let list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync> = Box::new(move || {
            let st = list_app.state::<AppState>();
            let open: Vec<OpenRepo> = match st.repos.lock() {
                Ok(m) => m
                    .iter()
                    .map(|(id, e)| OpenRepo {
                        repo_id: id.clone(),
                        path: e.path.clone(),
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
            open
        });
        let repos = Arc::new(SessionRepos::new(seed, list_open));
        Ok(BonsaiServer::with_session(repos, allow_write))
    };

    let mcp_service = StreamableHttpService::new(
        factory,
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let auth_cfg = AuthConfig {
        token: Arc::new(token.clone()),
        allowed_hosts: Arc::new(vec![
            format!("127.0.0.1:{actual_port}"),
            format!("localhost:{actual_port}"),
        ]),
    };

    let router = Router::new()
        .nest_service("/mcp", mcp_service)
        .layer(from_fn_with_state(auth_cfg, auth_layer));

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let task = tauri::async_runtime::spawn(async move {
        let serve = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
        if let Err(e) = serve.await {
            eprintln!("bonsai: embedded MCP server exited with error: {e}");
        }
    });

    let running = McpRunning {
        port: actual_port,
        token,
        allow_write,
        shutdown: Some(shutdown_tx),
        task,
    };
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
    if let Ok(file) = settings::settings_file(app) {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let mut s = settings::load_from(&file);
            s.mcp_enabled = false;
            let _ = settings::save_to(&file, &s);
        })
        .await;
    }

    let status = stopped_status();
    let _ = app.emit("mcp-server-changed", status.clone());
    Ok(status)
}

/// Auth-middleware state (P16 §8): the expected bearer token and the exact
/// allowed `Host` authorities.
#[derive(Clone)]
struct AuthConfig {
    token: Arc<String>,
    allowed_hosts: Arc<Vec<String>>,
}

/// The LOAD-BEARING gate (P16 §8). Reject ANY `Origin`-bearing request (403,
/// D-3), any `Host` not in the loopback allowlist (403), and any missing /
/// malformed / mismatched bearer token (401, constant-time compare). No CORS
/// headers are ever added, and rejection bodies are empty.
async fn auth_layer(State(cfg): State<AuthConfig>, req: Request, next: Next) -> Response {
    let headers = req.headers();

    // D-3: no browser origin is ever legitimate for this endpoint.
    if headers.contains_key(header::ORIGIN) {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Host allowlist (DNS-rebinding): exact `127.0.0.1:<port>` / `localhost:<port>`.
    let host_ok = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| cfg.allowed_hosts.iter().any(|a| a == h))
        .unwrap_or(false);
    if !host_ok {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Bearer token, constant-time compare.
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let ok = match provided {
        Some(tok) => bool::from(tok.as_bytes().ct_eq(cfg.token.as_bytes())),
        None => false,
    };
    if !ok {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_encodes_known_vectors() {
        // RFC 4648 test vectors, url-safe, no padding.
        assert_eq!(base64url_nopad(b""), "");
        assert_eq!(base64url_nopad(b"f"), "Zg");
        assert_eq!(base64url_nopad(b"fo"), "Zm8");
        assert_eq!(base64url_nopad(b"foo"), "Zm9v");
        assert_eq!(base64url_nopad(b"foob"), "Zm9vYg");
        assert_eq!(base64url_nopad(b"fooba"), "Zm9vYmE");
        assert_eq!(base64url_nopad(b"foobar"), "Zm9vYmFy");
        // url-safe alphabet uses '-' and '_' (0xfb,0xff -> "-_" region).
        assert_eq!(base64url_nopad(&[0xff, 0xff, 0xff]), "____");
        assert_eq!(base64url_nopad(&[0xfb, 0xff, 0xbf]), "-_-_");
    }

    #[test]
    fn generated_token_is_43_chars_no_padding() {
        // 32 bytes -> ceil(32/3)*4 = 44 raw, minus 1 for the last partial group.
        let t = generate_token();
        assert_eq!(t.len(), 43, "32 bytes base64url (no pad) is 43 chars: {t}");
        assert!(!t.contains('='));
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn two_generated_tokens_differ() {
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn tool_count_reflects_write_gate() {
        assert_eq!(tool_count(false), 14);
        assert_eq!(tool_count(true), 34);
    }

    #[test]
    fn claude_add_command_shape() {
        let cmd = claude_add_command(8765, "TOK");
        assert!(cmd.contains("--transport http"));
        assert!(cmd.contains("Authorization: Bearer TOK"));
        assert!(cmd.contains("http://127.0.0.1:8765/mcp"));
    }
}
