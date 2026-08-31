//! The transport/network core of the embedded MCP server (P16 §5/§8): listener
//! binding, the per-session `service_factory`, router + auth-layer assembly, and
//! the spawned `axum::serve` task. Kept free of Tauri glue (no `AppHandle`, no
//! settings, no `AppState`) so it is exercisable by the integration tests
//! without the banned `test` feature — the app-facing wiring stays in `super`.

use std::net::Ipv4Addr;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;

use bonsai_core::error::AppError;
use bonsai_mcp::{BonsaiServer, OpenRepo, SessionRepos};

use super::McpRunning;

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
/// caller persists it. `make_token` receives that actual port AFTER binding so
/// the caller can rotate the token on an ephemeral-port fallback (audit §3.7);
/// tests pass `|_| TEST_TOKEN.to_string()`.
pub(crate) async fn spawn_server(
    make_token: impl FnOnce(u16) -> String + Send,
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
    let token = make_token(actual_port);

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
