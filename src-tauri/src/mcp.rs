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
    // Persist the gate first so the (re)start below — and any future start when
    // stopped — reads the new value.
    let file = settings::settings_file(app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file);
        s.mcp_allow_write = allow_write;
        settings::save_to(&file, &s)
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
            // same port to ride out the brief post-abort release window.
            start(app, mcp_state).await
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
///
/// On a write-gate BOUNCE (D-5) the previous serve task is aborted, which drops
/// its listener and releases the port — but that teardown is asynchronous, so
/// an immediate rebind of the same port can momentarily lose the race on
/// Windows. To keep the persisted port stable (so the user's `claude mcp add`
/// keeps working) the preferred port is retried a handful of times with a short
/// delay BEFORE falling back to a fresh ephemeral port.
async fn bind_listener(preferred: Option<u16>) -> Result<TcpListener, AppError> {
    if let Some(p) = preferred {
        if p != 0 {
            // ~5 attempts over ~250 ms rides out the brief post-abort release
            // window without hanging enable if the port is genuinely taken.
            for attempt in 0..5 {
                if let Ok(l) = TcpListener::bind((Ipv4Addr::LOCALHOST, p)).await {
                    return Ok(l);
                }
                if attempt < 4 {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }
    }
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|e| AppError::Io(format!("bind 127.0.0.1:0: {e}")))
}

/// The transport/network core of the embedded MCP server, extracted from
/// [`start`] so it can be exercised by an integration test WITHOUT a Tauri
/// `AppHandle` (the `test` feature is banned on this machine — see CLAUDE.md).
///
/// This owns everything from listener bind through router assembly, the
/// per-session `service_factory`, `auth_layer` mounting, and the spawned
/// `axum::serve(..).with_graceful_shutdown(..)` task. It touches NO settings,
/// NO `emit`, and NO `AppState` directly — the app-facing glue (loading the
/// persisted token/port/gate, snapshotting `AppState`, persisting the actual
/// port, emitting `mcp-server-changed`) stays in [`start`]. The two closures
/// carry exactly what the core needs from the app:
///
/// - `list_open` — snapshot the currently open repos at tool-call time,
/// - `seed` — the per-session initial selection (the focused tab's repoId).
///
/// Security (P16 §8) is unchanged and lives here with the router: 127.0.0.1-only
/// bind, `auth_layer` on `/mcp` (constant-time token compare, reject-any-Origin,
/// Host allowlist, no CORS). Returns the ACTUAL bound port in [`McpRunning`]; the
/// caller persists it.
pub(crate) async fn spawn_server(
    token: String,
    allow_write: bool,
    preferred_port: Option<u16>,
    list_open: Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync>,
    seed: Arc<dyn Fn() -> Option<String> + Send + Sync>,
) -> Result<McpRunning, AppError> {
    let listener = bind_listener(preferred_port).await?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| AppError::Io(e.to_string()))?
        .port();

    // Per-session service factory (P16 §5): each new MCP session seeds its
    // selection from `seed` (the focused tab) and enumerates the open tabs via
    // `list_open` at call time. The factory closes over the two Arcs (cloning
    // per session) instead of over an `AppHandle`.
    let factory = move || -> Result<BonsaiServer, std::io::Error> {
        let seed_val = (seed)();
        let list_open = Arc::clone(&list_open);
        let list_open_box: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync> =
            Box::new(move || (list_open)());
        let repos = Arc::new(SessionRepos::new(seed_val, list_open_box));
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
        let serve = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(e) = serve.await {
            eprintln!("bonsai: embedded MCP server exited with error: {e}");
        }
    });

    Ok(McpRunning {
        port: actual_port,
        token,
        allow_write,
        shutdown: Some(shutdown_tx),
        task,
    })
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

    let token = token_opt.unwrap_or_else(generate_token);

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

    let running = spawn_server(token, allow_write, port_opt, list_open, seed).await?;
    let actual_port = running.port;
    let token_save = running.token.clone();

    // Persist token + actual port + enabled=true (stable `claude mcp add`, D-4).
    let file_save = file.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file_save);
        s.mcp_token = Some(token_save);
        s.mcp_port = Some(actual_port);
        s.mcp_enabled = true;
        settings::save_to(&file_save, &s)
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
}

/// P16d — embedded-HTTP MCP integration tests (contract §12.1 items 1-7).
///
/// These drive the runtime-free core [`spawn_server`] directly (NO `AppHandle`,
/// NO `AppState`, NO Tauri `test` feature — banned on this machine per CLAUDE.md
/// / STATUS_ENTRYPOINT_NOT_FOUND). The `list_open` / `seed` closures are built
/// straight from fixture [`OpenRepo`]s, so a test can seed / re-point / close
/// repos exactly as the app would.
///
/// HTTP-client choice: a hand-rolled `reqwest` client that speaks streamable-HTTP
/// MCP by hand — the security items (1) are raw POSTs + status-code asserts; the
/// protocol items (2-7) send `initialize` (capturing the `Mcp-Session-Id`
/// response header), then `notifications/initialized`, then `tools/list` /
/// `tools/call`, parsing the `data:` line out of the `text/event-stream` reply.
/// The rmcp streamable-http server runs in LEGACY session mode by default
/// (`legacy_session_mode: true`, `json_response: false`), so every request-scoped
/// reply is an SSE stream that closes after its terminal message — hence
/// `response.text()` (under a timeout) returns the whole body cleanly.
///
/// Runtime note: `spawn_server`'s serve task is spawned via
/// `tauri::async_runtime::spawn` (Tauri's process-global runtime), while the
/// listener is bound inside the awaited `spawn_server` on the per-test
/// `#[tokio::test(flavor = "multi_thread")]` runtime. The listener's IO
/// registration lives with the (still-running) per-test runtime for the whole
/// test, so the global serve task polling it cross-runtime is sound. Each test's
/// server is stopped before the test returns.
#[cfg(test)]
mod http_integration {
    use super::*;

    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HOST, ORIGIN};
    use reqwest::StatusCode;
    use serde_json::{json, Value};

    // -------- read/write tool catalogs (mirror crates/bonsai-mcp/tests) --------

    const READ_TOOLS: &[&str] = &[
        "bonsai_get_graph",
        "bonsai_get_status",
        "bonsai_list_branches",
        "bonsai_get_commit_diff",
        "bonsai_get_commit_file_diff",
        "bonsai_get_workdir_file_diff",
        "bonsai_compare_with_head",
        "bonsai_compare_with_head_file_diff",
        "bonsai_get_op_state",
        "bonsai_list_conflicts",
        "bonsai_get_conflict",
        "bonsai_list_stashes",
        "bonsai_list_repos",
        "bonsai_select_repo",
    ];

    const WRITE_TOOLS: &[&str] = &[
        "bonsai_stage",
        "bonsai_unstage",
        "bonsai_commit",
        "bonsai_resolve_conflict_text",
        "bonsai_resolve_conflict",
        "bonsai_merge_branch",
        "bonsai_commit_merge",
        "bonsai_abort_merge",
        "bonsai_rebase_branch",
        "bonsai_rebase_continue",
        "bonsai_rebase_skip",
        "bonsai_rebase_abort",
        "bonsai_create_branch",
        "bonsai_create_branch_here",
        "bonsai_checkout_branch",
        "bonsai_delete_branch",
        "bonsai_create_stash",
        "bonsai_apply_stash",
        "bonsai_pop_stash",
        "bonsai_drop_stash",
    ];

    const TEST_TOKEN: &str = "s3cr3t-bearer-token-for-p16d-tests";
    const MCP_ACCEPT: &str = "application/json, text/event-stream";
    const READ_TIMEOUT: Duration = Duration::from_secs(30);

    macro_rules! require_git {
        () => {
            if !have_git() {
                eprintln!("skipping: `git` CLI not found on PATH");
                return;
            }
        };
    }

    // ------------------------------------------------------------ git fixtures

    fn have_git() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    /// Scratch dir under `D:\Temp\bonsai-scratch` on Windows (MEMORY rule —
    /// never C:, never the system temp). On macOS/Linux there is no such
    /// constraint, so scratch dirs fall back to
    /// `std::env::temp_dir()/bonsai-scratch`.
    #[cfg(windows)]
    fn scratch_root() -> std::path::PathBuf {
        std::path::PathBuf::from("D:\\Temp\\bonsai-scratch")
    }

    #[cfg(not(windows))]
    fn scratch_root() -> std::path::PathBuf {
        std::env::temp_dir().join("bonsai-scratch")
    }

    fn scratch_dir() -> tempfile::TempDir {
        let root = scratch_root();
        std::fs::create_dir_all(&root).expect("create scratch root");
        tempfile::Builder::new()
            .prefix("bonsai-http-mcp-")
            .tempdir_in(&root)
            .expect("scratch dir")
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Runs `git <args>`; reports only whether it succeeded (a conflicted merge is
    /// EXPECTED to exit non-zero).
    fn git_ok(dir: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = scratch_dir();
        let p = dir.path();
        git(p, &["init", "-b", "main"]);
        git(p, &["config", "user.name", "Test User"]);
        git(p, &["config", "user.email", "test@example.com"]);
        git(p, &["config", "core.autocrlf", "false"]);
        dir
    }

    fn write_file(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).expect("write fixture file");
    }

    /// A linear repo with `n` commits on `main`.
    fn build_linear(dir: &Path, n: usize) {
        for i in 0..n {
            write_file(dir, "f.txt", &format!("line {i}\n"));
            git(dir, &["add", "-A"]);
            git(dir, &["commit", "-m", &format!("commit {i}")]);
        }
    }

    // The `bothModified` two-branch conflict on `a.txt` (identical byte content to
    // crates/bonsai-mcp/tests/mcp_stdio.rs so twin repos resolve to the same tree).
    const BASE_A: &str = "line1\nbase\nline3\n";
    const OURS_A: &str = "line1\nmain\nline3\n";
    const THEIRS_A: &str = "line1\nfeature\nline3\n";
    const MERGED_A: &str = "line1\nmain\nfeature\nline3\n";

    fn build_conflict_fixture(dir: &Path) {
        write_file(dir, "a.txt", BASE_A);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", "base"]);
        git(dir, &["checkout", "-b", "feature"]);
        write_file(dir, "a.txt", THEIRS_A);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", "feature change"]);
        git(dir, &["checkout", "main"]);
        write_file(dir, "a.txt", OURS_A);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", "main change"]);
    }

    /// The repoId used by the embedded server is the canonical workdir path
    /// string; build an [`OpenRepo`] the same way the app's `list_open` does.
    fn open_repo(dir: &Path) -> OpenRepo {
        OpenRepo {
            repo_id: dir.to_string_lossy().into_owned(),
            path: dir.to_path_buf(),
        }
    }

    fn graph_node_count(dir: &Path) -> usize {
        bonsai_core::graph::compute_graph(dir)
            .expect("compute_graph oracle")
            .nodes
            .len()
    }

    fn arc_list(repos: Vec<OpenRepo>) -> Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync> {
        Arc::new(move || repos.clone())
    }

    fn seed_some(id: String) -> Arc<dyn Fn() -> Option<String> + Send + Sync> {
        Arc::new(move || Some(id.clone()))
    }

    fn seed_none() -> Arc<dyn Fn() -> Option<String> + Send + Sync> {
        Arc::new(|| None)
    }

    // ------------------------------------------------------ SSE / JSON-RPC glue

    /// Extract every JSON `data:` payload from a `text/event-stream` body. Non-JSON
    /// SSE lines (priming/retry events, keep-alive comments) are skipped.
    fn parse_sse(body: &str) -> Vec<Value> {
        let mut out = Vec::new();
        for line in body.lines() {
            let line = line.trim_end_matches('\r');
            if let Some(rest) = line.strip_prefix("data:") {
                let rest = rest.strip_prefix(' ').unwrap_or(rest);
                if let Ok(v) = serde_json::from_str::<Value>(rest) {
                    out.push(v);
                }
            }
        }
        out
    }

    fn no_proxy_client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("build reqwest client")
    }

    /// A hand-rolled streamable-HTTP MCP client over `reqwest`. Owns one session
    /// (established in [`connect`]).
    struct HttpMcp {
        client: reqwest::Client,
        url: String,
        session: String,
        next_id: i64,
    }

    impl HttpMcp {
        /// `initialize` + capture `Mcp-Session-Id` + `notifications/initialized`.
        async fn connect(port: u16, token: &str) -> Self {
            let client = no_proxy_client();
            let url = format!("http://127.0.0.1:{port}/mcp");
            let init = json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "bonsai-http-test", "version": "0.0.0" }
                }
            });
            let resp = client
                .post(&url)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header(ACCEPT, MCP_ACCEPT)
                .json(&init)
                .send()
                .await
                .expect("initialize send");
            assert!(
                resp.status().is_success(),
                "initialize HTTP status {}",
                resp.status()
            );
            let session = resp
                .headers()
                .get("mcp-session-id")
                .expect("initialize must return an Mcp-Session-Id header")
                .to_str()
                .expect("session id is ascii")
                .to_string();
            let body = tokio::time::timeout(READ_TIMEOUT, resp.text())
                .await
                .expect("initialize body timed out")
                .expect("initialize body");
            assert!(
                parse_sse(&body).iter().any(|m| m.get("result").is_some()),
                "initialize response had no result: {body}"
            );

            let me = HttpMcp {
                client,
                url,
                session,
                next_id: 1,
            };
            me.notify("notifications/initialized", json!({})).await;
            me
        }

        async fn notify(&self, method: &str, params: Value) {
            let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
            let resp = self
                .client
                .post(&self.url)
                .header(AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
                .header(ACCEPT, MCP_ACCEPT)
                .header("mcp-session-id", &self.session)
                .json(&msg)
                .send()
                .await
                .expect("notify send");
            assert_eq!(
                resp.status(),
                StatusCode::ACCEPTED,
                "notification should be 202 Accepted, got {}",
                resp.status()
            );
        }

        /// Send a JSON-RPC request; return the response message whose `id` matches
        /// (parsed out of the SSE reply).
        async fn rpc(&mut self, method: &str, params: Value) -> Value {
            let id = self.next_id;
            self.next_id += 1;
            let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
            let resp = self
                .client
                .post(&self.url)
                .header(AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
                .header(ACCEPT, MCP_ACCEPT)
                .header("mcp-session-id", &self.session)
                .json(&msg)
                .send()
                .await
                .expect("rpc send");
            assert!(
                resp.status().is_success(),
                "{method} HTTP status {}",
                resp.status()
            );
            let body = tokio::time::timeout(READ_TIMEOUT, resp.text())
                .await
                .unwrap_or_else(|_| panic!("{method} body timed out"))
                .unwrap_or_else(|e| panic!("{method} body error: {e}"));
            parse_sse(&body)
                .into_iter()
                .find(|m| m.get("id").and_then(Value::as_i64) == Some(id))
                .unwrap_or_else(|| panic!("no JSON-RPC reply for id {id} ({method}): {body}"))
        }

        async fn list_tool_names(&mut self) -> Vec<String> {
            let resp = self.rpc("tools/list", json!({})).await;
            let tools = resp
                .get("result")
                .and_then(|r| r.get("tools"))
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("tools/list has no result.tools: {resp}"));
            let mut names: Vec<String> = tools
                .iter()
                .map(|t| {
                    t.get("name")
                        .and_then(Value::as_str)
                        .expect("tool has a name")
                        .to_string()
                })
                .collect();
            names.sort();
            names
        }

        async fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
            self.rpc("tools/call", json!({ "name": name, "arguments": arguments }))
                .await
        }
    }

    /// Assert a `tools/call` succeeded (no JSON-RPC error, `isError` not true) →
    /// its `structuredContent`.
    fn ok_structured(resp: &Value) -> Value {
        assert!(
            resp.get("error").is_none(),
            "unexpected JSON-RPC error: {resp}"
        );
        let result = resp
            .get("result")
            .unwrap_or_else(|| panic!("call had no result: {resp}"));
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(!is_error, "tool reported isError=true: {result}");
        result
            .get("structuredContent")
            .cloned()
            .unwrap_or_else(|| panic!("no structuredContent: {result}"))
    }

    /// Assert a `tools/call` returned a domain error (`isError == true`) → its
    /// `{ kind, message }` structured content.
    fn err_structured(resp: &Value) -> Value {
        assert!(
            resp.get("error").is_none(),
            "expected a tool-domain error (isError), got a JSON-RPC error: {resp}"
        );
        let result = resp
            .get("result")
            .unwrap_or_else(|| panic!("call had no result: {resp}"));
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(is_error, "expected isError=true, got: {result}");
        result
            .get("structuredContent")
            .cloned()
            .unwrap_or_else(|| panic!("error had no structuredContent: {result}"))
    }

    /// Spawn the embedded server for a test. Returns the running handle.
    async fn spawn(
        allow_write: bool,
        preferred_port: Option<u16>,
        list_open: Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync>,
        seed: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    ) -> McpRunning {
        spawn_server(
            TEST_TOKEN.to_string(),
            allow_write,
            preferred_port,
            list_open,
            seed,
        )
        .await
        .expect("spawn embedded MCP server")
    }

    // ===================================================== item 1: security gate

    /// A bare initialize POST with optional auth / Origin / Host overrides,
    /// returning only the HTTP status (the security gate runs before any MCP
    /// processing, so the body is irrelevant).
    async fn probe_status(
        client: &reqwest::Client,
        url: &str,
        auth: Option<&str>,
        origin: Option<&str>,
        host: Option<&str>,
    ) -> StatusCode {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26", "capabilities": {},
                "clientInfo": { "name": "probe", "version": "0.0.0" }
            }
        });
        let mut rb = client
            .post(url)
            .header(ACCEPT, MCP_ACCEPT)
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&body).unwrap());
        if let Some(a) = auth {
            rb = rb.header(AUTHORIZATION, format!("Bearer {a}"));
        }
        if let Some(o) = origin {
            rb = rb.header(ORIGIN, o);
        }
        if let Some(h) = host {
            rb = rb.header(HOST, h);
        }
        rb.send().await.expect("probe send").status()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn item1_security_token_origin_host_gate() {
        require_git!();
        let a = init_repo();
        let b = init_repo();
        build_linear(a.path(), 3);
        build_linear(b.path(), 4);
        let ra = open_repo(a.path());
        let rb = open_repo(b.path());

        let running = spawn(
            false,
            None,
            arc_list(vec![ra.clone(), rb.clone()]),
            seed_some(ra.repo_id.clone()),
        )
        .await;
        let port = running.port;
        let url = format!("http://127.0.0.1:{port}/mcp");
        let client = no_proxy_client();
        let valid_host = format!("127.0.0.1:{port}");

        // No Authorization header -> 401.
        assert_eq!(
            probe_status(&client, &url, None, None, None).await,
            StatusCode::UNAUTHORIZED,
            "missing bearer must be 401"
        );
        // Wrong bearer -> 401.
        assert_eq!(
            probe_status(&client, &url, Some("not-the-token"), None, None).await,
            StatusCode::UNAUTHORIZED,
            "wrong bearer must be 401"
        );
        // Correct bearer but an Origin header present -> 403 (D-3).
        assert_eq!(
            probe_status(&client, &url, Some(TEST_TOKEN), Some("http://evil.test"), None).await,
            StatusCode::FORBIDDEN,
            "any Origin header must be 403"
        );
        // Correct bearer but a disallowed Host -> 403.
        assert_eq!(
            probe_status(&client, &url, Some(TEST_TOKEN), None, Some("evil.test")).await,
            StatusCode::FORBIDDEN,
            "disallowed Host must be 403"
        );
        // Correct bearer + valid Host + no Origin -> reaches the MCP layer (200).
        let ok = probe_status(&client, &url, Some(TEST_TOKEN), None, Some(&valid_host)).await;
        assert!(
            ok != StatusCode::UNAUTHORIZED && ok != StatusCode::FORBIDDEN,
            "authenticated loopback request must pass the gate, got {ok}"
        );
        assert!(ok.is_success(), "authenticated initialize should be 2xx, got {ok}");

        running.stop();
    }

    // ============================================ item 2: read round-trip + count

    #[tokio::test(flavor = "multi_thread")]
    async fn item2_read_round_trip_and_tool_count() {
        require_git!();
        let a = init_repo();
        let b = init_repo();
        build_linear(a.path(), 3);
        build_linear(b.path(), 4);
        let ra = open_repo(a.path());
        let rb = open_repo(b.path());

        let running = spawn(
            false,
            None,
            arc_list(vec![ra.clone(), rb.clone()]),
            seed_some(ra.repo_id.clone()),
        )
        .await;

        let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

        // Exactly the 14 read tools, no mutation tools.
        let names = client.list_tool_names().await;
        assert_eq!(
            names.len(),
            READ_TOOLS.len(),
            "read-only server must advertise exactly 14 tools, got: {names:?}"
        );
        for t in READ_TOOLS {
            assert!(names.contains(&t.to_string()), "missing read tool {t}");
            assert!(t.starts_with("bonsai_"), "read tool must be bonsai_-prefixed: {t}");
        }
        for t in WRITE_TOOLS {
            assert!(
                !names.contains(&t.to_string()),
                "read-only server must NOT advertise mutation tool {t}"
            );
        }

        // get_graph on the seeded repo (A) matches an in-process compute_graph.
        let layout = ok_structured(&client.call_tool("bonsai_get_graph", json!({})).await);
        let nodes = layout
            .get("nodes")
            .and_then(Value::as_array)
            .expect("layout.nodes");
        assert_eq!(
            nodes.len(),
            graph_node_count(a.path()),
            "HTTP get_graph nodes.len() must equal compute_graph(A)"
        );

        running.stop();
    }

    // ================================ item 3: write gating + conflict round-trip

    #[tokio::test(flavor = "multi_thread")]
    async fn item3_write_gating_and_conflict_round_trip() {
        require_git!();
        let repo = init_repo();
        build_conflict_fixture(repo.path());
        let r = open_repo(repo.path());

        let running = spawn(
            true,
            None,
            arc_list(vec![r.clone()]),
            seed_some(r.repo_id.clone()),
        )
        .await;

        let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

        // allow_write=true -> 34 tools.
        let names = client.list_tool_names().await;
        assert_eq!(
            names.len(),
            READ_TOOLS.len() + WRITE_TOOLS.len(),
            "allow-write server must advertise 34 tools, got: {names:?}"
        );
        for t in READ_TOOLS.iter().chain(WRITE_TOOLS.iter()) {
            assert!(names.contains(&t.to_string()), "missing tool {t}");
        }

        // merge feature -> conflicts on a.txt.
        let merge = ok_structured(
            &client
                .call_tool("bonsai_merge_branch", json!({ "name": "feature" }))
                .await,
        );
        assert_eq!(
            merge.get("kind").and_then(Value::as_str),
            Some("conflicts"),
            "expected a conflicts MergeOutcome: {merge}"
        );
        let paths: Vec<String> = merge
            .get("paths")
            .and_then(Value::as_array)
            .expect("conflicts.paths")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(paths.contains(&"a.txt".to_string()), "paths must include a.txt: {paths:?}");

        // get_conflict -> bothModified with the branch versions.
        let conflict = ok_structured(
            &client
                .call_tool("bonsai_get_conflict", json!({ "path": "a.txt" }))
                .await,
        );
        assert_eq!(
            conflict.get("kind").and_then(Value::as_str),
            Some("bothModified"),
            "conflict kind: {conflict}"
        );
        assert_eq!(conflict.get("ours").and_then(Value::as_str), Some(OURS_A));
        assert_eq!(conflict.get("theirs").and_then(Value::as_str), Some(THEIRS_A));

        // resolve + commit_merge.
        ok_structured(
            &client
                .call_tool(
                    "bonsai_resolve_conflict_text",
                    json!({ "path": "a.txt", "content": MERGED_A }),
                )
                .await,
        );
        let commit = ok_structured(
            &client
                .call_tool("bonsai_commit_merge", json!({ "message": "resolve" }))
                .await,
        );
        assert!(
            commit.get("oid").and_then(Value::as_str).is_some(),
            "commit_merge must return an oid: {commit}"
        );
        running.stop();

        let mcp_tree = git(repo.path(), &["rev-parse", "HEAD^{tree}"]);

        // Oracle: identical history hand-resolved by the git CLI.
        let cli = init_repo();
        build_conflict_fixture(cli.path());
        assert!(
            !git_ok(cli.path(), &["merge", "feature"]),
            "git merge feature should conflict"
        );
        write_file(cli.path(), "a.txt", MERGED_A);
        git(cli.path(), &["add", "a.txt"]);
        git(cli.path(), &["commit", "-m", "resolve"]);
        let cli_tree = git(cli.path(), &["rev-parse", "HEAD^{tree}"]);

        assert_eq!(
            mcp_tree, cli_tree,
            "MCP-resolved tree oid ({mcp_tree}) must equal the git-CLI tree oid ({cli_tree})"
        );
    }

    // =============================================== item 4: no selection (seed None)

    #[tokio::test(flavor = "multi_thread")]
    async fn item4_no_selection_is_no_repo_not_panic() {
        require_git!();
        let a = init_repo();
        let b = init_repo();
        build_linear(a.path(), 3);
        build_linear(b.path(), 4);
        let ra = open_repo(a.path());
        let rb = open_repo(b.path());

        // Seed None: this session has no selection.
        let running = spawn(false, None, arc_list(vec![ra, rb]), seed_none()).await;
        let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

        let resp = client.call_tool("bonsai_get_status", json!({})).await;
        let err = err_structured(&resp);
        assert_eq!(
            err.get("kind").and_then(Value::as_str),
            Some("noRepo"),
            "no selection must surface kind=noRepo (no panic / no 500): {err}"
        );

        running.stop();
    }

    // ===================== item 5: list/select + acting on a non-seed repo (B)

    #[tokio::test(flavor = "multi_thread")]
    async fn item5_list_select_and_act_on_non_seed_repo() {
        require_git!();
        let a = init_repo();
        let b = init_repo();
        build_linear(a.path(), 3);
        build_linear(b.path(), 6); // distinct node count from A
        let ra = open_repo(a.path());
        let rb = open_repo(b.path());
        let a_id = ra.repo_id.clone();
        let b_id = rb.repo_id.clone();

        let count_a = graph_node_count(a.path());
        let count_b = graph_node_count(b.path());
        assert_ne!(count_a, count_b, "fixtures A and B must differ in node count");

        let running = spawn(
            false,
            None,
            arc_list(vec![ra, rb]),
            seed_some(a_id.clone()),
        )
        .await;
        let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

        // list_repos: A and B, A marked selected (the seed).
        let list = ok_structured(&client.call_tool("bonsai_list_repos", json!({})).await);
        let arr = list.as_array().expect("list_repos returns an array");
        assert_eq!(arr.len(), 2, "list_repos must return both open tabs: {list}");
        let find = |id: &str| {
            arr.iter()
                .find(|e| e.get("repoId").and_then(Value::as_str) == Some(id))
                .unwrap_or_else(|| panic!("repoId {id} not in list: {list}"))
        };
        assert_eq!(
            find(&a_id).get("selected").and_then(Value::as_bool),
            Some(true),
            "seed repo A must be selected"
        );
        assert_eq!(
            find(&b_id).get("selected").and_then(Value::as_bool),
            Some(false),
            "non-seed repo B must not be selected"
        );

        // Seeded session acts on A.
        let g_a = ok_structured(&client.call_tool("bonsai_get_graph", json!({})).await);
        assert_eq!(
            g_a.get("nodes").and_then(Value::as_array).map(|n| n.len()),
            Some(count_a),
            "before select, get_graph reflects seed A"
        );

        // select B -> succeeds, returns B's summary (selected=true).
        let sel = ok_structured(
            &client
                .call_tool("bonsai_select_repo", json!({ "repoId": b_id }))
                .await,
        );
        assert_eq!(sel.get("repoId").and_then(Value::as_str), Some(b_id.as_str()));
        assert_eq!(sel.get("selected").and_then(Value::as_bool), Some(true));

        // Now get_graph reflects B (the non-focused/non-seed tab) — call-time resolution.
        let g_b = ok_structured(&client.call_tool("bonsai_get_graph", json!({})).await);
        assert_eq!(
            g_b.get("nodes").and_then(Value::as_array).map(|n| n.len()),
            Some(count_b),
            "after select(B), get_graph reflects B, independent of the seed"
        );

        running.stop();
    }

    // ============================= item 6: unknown/closed repoId rejection

    #[tokio::test(flavor = "multi_thread")]
    async fn item6_unknown_and_closed_repo_rejection() {
        require_git!();
        let a = init_repo();
        let b = init_repo();
        build_linear(a.path(), 3);
        build_linear(b.path(), 4);
        let ra = open_repo(a.path());
        let rb = open_repo(b.path());
        let a_id = ra.repo_id.clone();
        let b_id = rb.repo_id.clone();

        // list_open closes over a flag we can flip to "close" B mid-session.
        let include_b = Arc::new(StdMutex::new(true));
        let flag = Arc::clone(&include_b);
        let list_open: Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync> = Arc::new(move || {
            let mut v = vec![ra.clone()];
            if *flag.lock().unwrap() {
                v.push(rb.clone());
            }
            v
        });

        let running = spawn(false, None, list_open, seed_some(a_id.clone())).await;
        let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

        // Unknown repoId -> invalidName.
        let unknown = err_structured(
            &client
                .call_tool("bonsai_select_repo", json!({ "repoId": "C:/not/open" }))
                .await,
        );
        assert_eq!(
            unknown.get("kind").and_then(Value::as_str),
            Some("invalidName"),
            "unknown repoId must be invalidName: {unknown}"
        );

        // Select B (currently open) -> ok.
        ok_structured(
            &client
                .call_tool("bonsai_select_repo", json!({ "repoId": b_id }))
                .await,
        );

        // Close B (stop returning it from list_open), then a git tool -> noRepo.
        *include_b.lock().unwrap() = false;
        let closed = err_structured(&client.call_tool("bonsai_get_status", json!({})).await);
        assert_eq!(
            closed.get("kind").and_then(Value::as_str),
            Some("noRepo"),
            "acting on a since-closed selection must be noRepo: {closed}"
        );

        running.stop();
    }

    // ==================== item 7 (reviewer NIT-2): write->off revoke on bounce

    #[tokio::test(flavor = "multi_thread")]
    async fn item7_write_off_bounce_renegotiates_read_only() {
        require_git!();
        let a = init_repo();
        build_linear(a.path(), 3);
        let ra = open_repo(a.path());
        let a_id = ra.repo_id.clone();

        // Start with write ON (34 tools).
        let running = spawn(
            true,
            None,
            arc_list(vec![ra.clone()]),
            seed_some(a_id.clone()),
        )
        .await;
        let port = running.port;
        {
            let mut client = HttpMcp::connect(port, TEST_TOKEN).await;
            assert_eq!(
                client.list_tool_names().await.len(),
                READ_TOOLS.len() + WRITE_TOOLS.len(),
                "write-on server must advertise 34 tools"
            );
        }

        // Simulate the write->off bounce: stop, then restart read-only on the SAME
        // port (as `set_allow_write` does).
        running.stop();
        let running2 = spawn(
            false,
            Some(port),
            arc_list(vec![ra]),
            seed_some(a_id),
        )
        .await;
        assert_eq!(running2.port, port, "bounce should re-bind the same port");

        // A NEW client session re-negotiates the now-14 tool set.
        let mut client2 = HttpMcp::connect(running2.port, TEST_TOKEN).await;
        let names = client2.list_tool_names().await;
        assert_eq!(
            names.len(),
            READ_TOOLS.len(),
            "after write->off bounce, a fresh session sees exactly 14 tools: {names:?}"
        );
        assert!(
            !names.contains(&"bonsai_create_branch".to_string()),
            "mutation tools must be gone after the bounce"
        );

        // A mutation call is now rejected as an unregistered tool (JSON-RPC error).
        let resp = client2
            .call_tool("bonsai_create_branch", json!({ "name": "x" }))
            .await;
        assert!(
            resp.get("error").is_some(),
            "mutation on a read-only (bounced) server must be a JSON-RPC error: {resp}"
        );

        running2.stop();
    }
}
