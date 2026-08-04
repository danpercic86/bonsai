# P16 — Embedded MCP server (Tier 3: shared live workspace)

Status: contract (design only). Author: architect. All 7 open decisions are **RESOLVED** (see §14).
Depends on P14 (`docs/contracts/P14-mcp-server.md`) — the `bonsai-mcp` tool layer, the `AppError`
mapping, the curated tool set, and §7.2 exclusions are **reused verbatim**. No new *Git* logic; the only
tool additions are two repo-management (non-git-mutation) read tools mandated by D-2 (§4b).

## 0. Goal & scope

Run an MCP server **inside the running Bonsai Tauri app** so an external client (Claude Code, HTTP
transport) operates on the SAME live repos the user has open, and the GUI live-updates as the AI acts (via
the existing `repo-changed` watcher). This is Tier 3 ("shared live workspace") of the AI-integration
analysis.

Delta vs P14 (standalone stdio server):
1. **Transport** changes from stdio to **streamable-HTTP** bound to `127.0.0.1:<port>` (external client
   attaches to the long-lived app process).
2. **Repo targeting** changes from a fixed `--repo` to the app's **open tabs**: per D-2 the AI enumerates
   the open repos (`bonsai_list_repos`) and picks one for its session (`bonsai_select_repo`); the choice
   is resolved to a workdir at each tool call, so it can act on any open tab, not just the focused one.
3. New **security** surface (a localhost mutating HTTP port): a **bearer token** (persisted, D-4) +
   Origin/Host validation.
4. New **UI surface**: enable toggle, write-gate toggle, status, URL+token, the `claude mcp add` line.

Tool catalog: the P14 32-tool catalog (12 read + 20 write) is reused verbatim; D-2 adds **2 repo-management
read tools** (`bonsai_list_repos`, `bonsai_select_repo`), giving **14 read / 34 with write**. Network +
AI-helper tools stay excluded (P14 §7.2). Every git2 call runs in `spawn_blocking` and opens the repo fresh
from the workdir path.

**rmcp stays pinned `3.0.1`.** The streamable-HTTP server transport **exists at 3.0.1** — verified, no
version bump needed (see §2).

---

## 1. Contract sections
§2 rmcp HTTP transport (verified) · §3 crate/layout changes · §4 shared tool-layer factoring
(WorkdirSource + per-session selection) · §4b new repo-selection tools (D-2) · §5 active-repo seed +
`set_active_repo` · §6 embedded server transport & lifecycle · §7 concurrency analysis · §8 security model
(load-bearing) · §9 write-gate UI + settings · §10 frontend IPC surface + mock · §11 sub-increments ·
§12 acceptance criteria · §13 risks · §14 resolved decisions · Appendix A signatures.

---

## 2. rmcp streamable-HTTP transport — VERIFIED at 3.0.1

Feature flag: **`transport-streamable-http-server`** (transitively enables `server-side-http`,
`transport-streamable-http-server-session`, `transport-worker`). Confirmed present in rmcp 3.0.1.

Public API (module `rmcp::transport::streamable_http_server`):
- `StreamableHttpService<S, M>` — a **`tower::Service<http::Request<..>>`**; mount into an axum `Router`.
- `session::local::LocalSessionManager` — the in-process `SessionManager` impl (in-memory sessions).
- `StreamableHttpServerConfig` — transport config (stateful/SSE, keep-alive).
- `SessionManager` trait, `SessionId` type.

Constructor (verified signature):
```rust
StreamableHttpService::<BonsaiServer, LocalSessionManager>::new(
    service_factory: impl Fn() -> Result<BonsaiServer, std::io::Error> + Send + Sync + 'static,
    session_manager: Arc<LocalSessionManager>,
    config: StreamableHttpServerConfig,
) -> Self
```
`S: ServerHandler + Send + 'static` — satisfied by `BonsaiServer` (its `#[tool_handler] impl ServerHandler`
from P14 is transport-agnostic; only the serve wiring differs). The **`service_factory` runs per new MCP
session**, returning a fresh `BonsaiServer` — this is the seam we exploit for **per-session repo selection**
(§4) and the write-gate (§9).

Serve wiring (in `src-tauri`, on the app's existing tokio runtime):
```rust
let mcp = StreamableHttpService::new(factory, Arc::new(LocalSessionManager::default()), cfg);
let router = axum::Router::new()
    .nest_service("/mcp", mcp)                     // MCP endpoint
    .layer(axum::middleware::from_fn(auth_layer)); // §8: token + Origin/Host gate
let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
let actual_port = listener.local_addr()?.port();   // read back when port==0 (ephemeral)
axum::serve(listener, router)
    .with_graceful_shutdown(async move { shutdown_rx.await.ok(); })
    .await
```

Deps required by `src-tauri`:
```toml
rmcp = { version = "3.0.1", features = ["server", "macros", "transport-streamable-http-server"] }
axum = "0.8"           # match the http/tower-service versions rmcp 3.0.1 resolves; verify at implement time
tokio  # already present via tauri (rt-multi-thread)
```

Cargo verification is still MANDATORY at implement time (P14 §3 rmcp-churn discipline): run
`cargo tree -p rmcp` to confirm the resolved `http`/`axum`/`tower-service` versions align, and adapt the
`StreamableHttpServerConfig` field names / `nest_service` vs `fallback_service` mount to the resolved API.
Docs: <https://docs.rs/rmcp/3.0.1/rmcp/transport/streamable_http_server/index.html> and
<https://docs.rs/rmcp/3.0.1/rmcp/transport/streamable_http_server/tower/struct.StreamableHttpService.html>.

---

## 3. Crate / layout changes

### 3.1 `bonsai-mcp` becomes lib + bin (P16a)
Today `bonsai-mcp` is bin-only with `server.rs` as a private `mod`. Split so the tool layer is reusable:

```
crates/bonsai-mcp/
  Cargo.toml        # EDITED: add [lib]; keep [[bin]]
  src/lib.rs        # NEW: `pub mod server;` + re-exports (BonsaiServer, WorkdirSource, SessionRepos, OpenRepo)
  src/server.rs     # EDITED: WorkdirSource + per-session selection (§4); P14 tool bodies UNCHANGED
  src/main.rs       # EDITED: `use bonsai_mcp::server::BonsaiServer;` (stdio path unchanged)
```
`Cargo.toml`:
```toml
[lib]
name = "bonsai_mcp"
path = "src/lib.rs"

[[bin]]
name = "bonsai-mcp"
path = "src/main.rs"
```
The stdio bin keeps its current deps (`rmcp` with `server,transport-io,macros`; `tokio io-std`). The
HTTP/axum deps do **not** belong to the bin.

### 3.2 RESOLVED (D-1) — axum/token/lifecycle wiring lives in `src-tauri/src/mcp.rs`
`bonsai-mcp` stays **transport-agnostic** (exposes only `BonsaiServer`, `WorkdirSource`, `SessionRepos`,
`OpenRepo`). `src-tauri` gains `rmcp` (http features) + `axum` as direct deps and constructs
`StreamableHttpService` with a `BonsaiServer` factory. New file `src-tauri/src/mcp.rs`;
`src-tauri/src/lib.rs` gains `pub mod mcp;`, a `.manage(mcp::McpServerState::default())`, four new commands
in `generate_handler!`, and a spawn/shutdown hook in `setup` + `RunEvent::ExitRequested`.

### 3.3 No `bonsai-core` change
`bonsai-core` is untouched. `WorkdirSource`/`SessionRepos` live in `bonsai-mcp` (they reference `AppError`
and `read_repo_info` from core, already deps).

---

## 4. Shared tool-layer factoring — `WorkdirSource` + per-session selection

The P14 `BonsaiServer` holds `workdir: Arc<PathBuf>` (resolved once at startup). The embedded server must
resolve a **per-session selected** repo at **each tool call** (D-2). Because rmcp's `service_factory` builds
one `BonsaiServer` per session, each session naturally owns its own selection state. Generalize the workdir
into a source with two variants — the standalone `Fixed` (unchanged) and the embedded per-session
`Session`:

```rust
// crates/bonsai-mcp/src/server.rs
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use bonsai_core::error::AppError;

/// One open repo as the embedded server sees it (repoId + canonical workdir).
#[derive(Clone)]
pub struct OpenRepo { pub repo_id: String, pub path: PathBuf }

/// Per-SESSION repo state for the embedded server. Each MCP session gets its own
/// instance (built by the service_factory), so `selected` is private to that session.
pub struct SessionRepos {
    /// This session's currently-selected repoId. Seeded from `AppState.active_repo`
    /// at session open (§5); mutated only by `bonsai_select_repo`.
    selected: Mutex<Option<String>>,
    /// Snapshot the app's currently-open tabs (locks `AppState.repos`, clones out).
    /// Cheap (no git2); called at every workdir resolve / list / select.
    list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync>,
}

impl SessionRepos {
    pub fn new(seed: Option<String>, list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync>) -> Self {
        SessionRepos { selected: Mutex::new(seed), list_open }
    }
    /// Snapshot of open tabs (for `bonsai_list_repos`).
    fn open(&self) -> Vec<OpenRepo> { (self.list_open)() }
    /// The session's selected repoId, if any.
    fn selected_id(&self) -> Result<Option<String>, AppError> {
        Ok(self.selected.lock().map_err(pois)?.clone())
    }
    /// Resolve the selected repo -> workdir at git-tool call time.
    fn resolve_workdir(&self) -> Result<PathBuf, AppError> {
        let id = self.selected_id()?
            .ok_or_else(|| AppError::NoRepo)?;              // "no repo selected: call bonsai_select_repo"
        self.open().into_iter().find(|r| r.repo_id == id).map(|r| r.path)
            .ok_or_else(|| AppError::NoRepo)                // selected repo tab was closed since selection
    }
    /// Validate `repo_id` is a currently-open tab, then select it for this session.
    fn select(&self, repo_id: &str) -> Result<(), AppError> {
        if !self.open().iter().any(|r| r.repo_id == repo_id) {
            return Err(AppError::InvalidName(format!("repo '{repo_id}' is not an open tab")));
        }
        *self.selected.lock().map_err(pois)? = Some(repo_id.to_string());
        Ok(())
    }
}

/// Resolves the target repo workdir at each git-tool call.
#[derive(Clone)]
pub enum WorkdirSource {
    /// Standalone stdio server: one fixed, pre-validated canonical workdir.
    /// `list_repos` reports just this repo; `select_repo` is rejected (single-repo).
    Fixed(Arc<PathBuf>),
    /// Embedded server: per-session selection over the app's open tabs.
    Session(Arc<SessionRepos>),
}

impl WorkdirSource {
    /// Workdir for the git tools (locks a mutex + clones a PathBuf — no git2, no `.await`).
    pub fn resolve(&self) -> Result<PathBuf, AppError> {
        match self {
            WorkdirSource::Fixed(p) => Ok((**p).clone()),
            WorkdirSource::Session(s) => s.resolve_workdir(),
        }
    }
}
```
`pois` = a small helper mapping a poisoned lock to `AppError::Other("state lock poisoned")`.

`BonsaiServer` change (field type only; **all P14 `#[tool]` bodies unchanged**):
```rust
#[derive(Clone)]
pub struct BonsaiServer {
    workdir: WorkdirSource,          // was: Arc<PathBuf>
    allow_write: bool,
    tool_router: ToolRouter<BonsaiServer>,
}

impl BonsaiServer {
    /// Standalone constructor (P14 call sites unchanged in behavior).
    pub fn new(workdir: PathBuf, allow_write: bool) -> Self {
        Self::with_source(WorkdirSource::Fixed(Arc::new(workdir)), allow_write)
    }
    /// Embedded per-session constructor (called by the service_factory).
    pub fn with_session(repos: Arc<SessionRepos>, allow_write: bool) -> Self {
        Self::with_source(WorkdirSource::Session(repos), allow_write)
    }
    fn with_source(workdir: WorkdirSource, allow_write: bool) -> Self {
        let mut tool_router = Self::tool_router();
        if allow_write { tool_router.merge(Self::write_router()); }
        Self { workdir, allow_write, tool_router }
    }
}
```

`run_blocking` resolves at call time, then spawns blocking git2 (identical shape otherwise):
```rust
async fn run_blocking<T, F>(&self, f: F) -> Result<T, AppError>
where T: Send + 'static,
      F: FnOnce(&Path) -> Result<T, AppError> + Send + 'static,
{
    let workdir = self.workdir.resolve()?;   // NoRepo/closed error surfaces here → err_result, no panic
    tokio::task::spawn_blocking(move || f(workdir.as_path()))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```
When `resolve()` returns `Err` (no selection / closed tab), each git tool's existing
`Err(e) => err_result(e)` arm turns it into a clean `CallToolResult{is_error:true}` (`kind: "noRepo"`) — the
AI is told to call `bonsai_select_repo` (or re-select). **No panic.**

This is the shared-code story: one field-type change + one line in `run_blocking` + the new
`SessionRepos`/`OpenRepo` types. The P14 32 tool bodies, `ok_json`/`err_result`, param structs, and the
`ServerHandler` impl are reused verbatim by both the stdio bin (`Fixed`) and the embedded server
(`Session`). **P16a introduces the full `WorkdirSource`/`SessionRepos` shape** so P16b can add the two new
tools without reworking the abstraction.

---

## 4b. New repo-selection tools (D-2) — READ tier, always registered

Two tools join the **read** router (`#[tool_router]`), so they are available even when the write-gate is
OFF. They are repo-management, **not** git mutations. Naming/JSON conventions match P14 (snake_case,
`bonsai_` prefix, camelCase fields).

| Tool | Input | Output | Behavior |
|---|---|---|---|
| `bonsai_list_repos` | `{}` | `[OpenRepoSummary]` | Enumerate the repos the user has OPEN in Bonsai. `Fixed`: the single `--repo`. `Session`: snapshot `AppState.repos`, enriching each with a HEAD summary via `read_repo_info` (run in `spawn_blocking`); flags which one this session has `selected`. |
| `bonsai_select_repo` | `{ "repoId": string }` | `OpenRepoSummary` | Set the CALLING SESSION's selected repo. Validates `repoId` against the open set; unknown/closed → `kind: "invalidName"` error. `Fixed`: rejected with a clear "single-repo (standalone) server; repo selection unavailable" error. Returns the now-selected repo's summary. |

Output/param types (in `bonsai-mcp/src/server.rs`):
```rust
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OpenRepoSummary {
    /// Canonical workdir path string = the repoId used by `bonsai_select_repo`.
    repo_id: String,
    /// Canonical workdir path (same value; explicit for readability).
    path: String,
    /// HEAD summary from `read_repo_info` (branch name / detached / unborn); None if unreadable.
    head: Option<bonsai_core::git::repo::HeadInfo>,
    /// True for the repo THIS session currently has selected.
    selected: bool,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SelectRepoArgs {
    /// The repoId (canonical workdir path) of an open Bonsai tab, from `bonsai_list_repos`.
    repo_id: String,
}
```
Tool bodies match on `self.workdir`: `Session(s)` → `s.open()`/`s.select(..)` + `read_repo_info` for the
summary; `Fixed(p)` → single-repo summary / rejection. `bonsai_select_repo` mutates only this session's
`SessionRepos.selected`; it never touches other sessions or the app's focused tab. (If `HeadInfo` is not a
distinct exported type, the summary carries the relevant `RepoInfo` HEAD fields; verify against
`bonsai_core::git::repo` at implement time.)

**Truthfulness note:** `tools/list` advertises **14 read tools** when write is OFF (the P14 12 + these 2)
and **34 tools** when write is ON (14 read + 20 write). The two new tools are in the always-registered read
router, so the write-gate does not change their visibility.

---

## 5. Active-repo seed + `set_active_repo` (reconciled with per-session selection)

Two distinct notions, deliberately kept separate:
- **`AppState.active_repo`** — the app's **focused tab** (live runtime). Drives the GUI and **seeds** a new
  MCP session's initial selection. Set by the frontend on tab switch / open / close.
- **`SessionRepos.selected`** — what the **AI actually acts on** for a given session. Seeded from
  `active_repo` at session open, then freely re-pointed by `bonsai_select_repo` to any OTHER open tab.

So: a new session defaults to whatever tab the user is looking at (or unselected if none), and the AI may
then choose a different open repo without disturbing the user's focused tab or other sessions.

`AppState` gains the seed field:
```rust
// src-tauri/src/state.rs
#[derive(Default)]
pub struct AppState {
    pub repos: Mutex<HashMap<String, RepoEntry>>,
    pub active_repo: Mutex<Option<String>>,   // NEW: focused tab's repoId, or None
}
```

New command (lock-and-clone discipline, like `repo_path`):
```rust
#[tauri::command]
pub async fn set_active_repo(
    state: tauri::State<'_, AppState>,
    repo_id: Option<String>,          // None when the last tab closes / no repo open
) -> Result<(), AppError> {
    *state.active_repo.lock().map_err(|_| AppError::Other("state lock poisoned".into()))? = repo_id;
    Ok(())
}
```

The **service_factory** (in `src-tauri/src/mcp.rs`) builds a fresh per-session `SessionRepos` each time,
seeding it from the current `active_repo` and closing over `AppState.repos` for `list_open`:
```rust
// pseudocode — captures an Arc-shared handle to AppState
let factory = move || -> Result<BonsaiServer, std::io::Error> {
    let seed = state.active_repo.lock().ok().and_then(|g| g.clone());
    let state2 = state.clone();
    let list_open = Box::new(move || -> Vec<OpenRepo> {
        state2.repos.lock().map(|m| m.iter()
            .map(|(id, e)| OpenRepo { repo_id: id.clone(), path: e.path.clone() })
            .collect()).unwrap_or_default()
    });
    let repos = Arc::new(SessionRepos::new(seed, list_open));
    Ok(BonsaiServer::with_session(repos, current_allow_write()))
};
```
Frontend call sites for `set_active_repo`: tab activation, `open_repo` success, `close_repo`, and once on
startup after session restore.

---

## 6. Embedded server — transport & lifecycle

### 6.1 Managed handle
```rust
// src-tauri/src/mcp.rs
pub struct McpRunning {
    pub port: u16,
    pub token: String,                 // bearer (§8); persisted per D-4
    pub allow_write: bool,
    shutdown: tokio::sync::oneshot::Sender<()>,   // fires graceful shutdown
    task: tauri::async_runtime::JoinHandle<()>,   // the axum serve task
}
#[derive(Default)]
pub struct McpServerState { pub inner: std::sync::Mutex<Option<McpRunning>> }  // None = stopped
```
`.manage(McpServerState::default())` in `lib.rs`. `AppState` and `McpServerState` are separate managed
states.

### 6.2 Start (lazy, on user enable — NOT auto-started in `setup`)
Default **OFF** (D-7). The server starts only when the user flips the enable toggle (§9). Starting:
1. Read current `allow_write` from settings.
2. Load the persisted bearer token (or generate + persist one on first ever enable — D-4).
3. Build the per-session `service_factory` (§5): seeds each session from `AppState.active_repo`, closes
   over `AppState.repos`, and reads the current `allow_write`.
4. Bind `127.0.0.1:<port>` per D-4 (persisted-preferred; ephemeral fallback if busy), read back
   `actual_port`.
5. Spawn `axum::serve(..).with_graceful_shutdown(..)` on `tauri::async_runtime` (the app's tokio rt).
6. Store `McpRunning`; emit `mcp-server-changed` (§10).

Runs on the app's existing multi-thread tokio runtime; accept loop + per-session tasks are async and git2
work is inside `spawn_blocking`, so **the UI thread is never blocked**.

### 6.3 Stop
Toggle-off, write-gate change (§9 bounce), or app exit: take `McpRunning` out of the mutex, send on
`shutdown`, `await`/abort `task`. On app exit, hook Tauri `RunEvent::ExitRequested` (or
`WindowEvent::Destroyed`) to release the port. Dropping `McpServerState` is a backstop.

### 6.4 No new watcher wiring
The embedded server mutates the **same workdirs** the app's per-repo `notify` watchers already watch
(`.git/index`, `refs/**`, `HEAD`, worktree files — all pass `is_relevant` in `watcher.rs`). An AI
stage/commit/merge on any open repo fires that repo's debounced `repo-changed`, and the GUI refreshes with
**zero** new plumbing. This holds even when the AI acts on a NON-focused tab: that tab's watcher still
fires, so its content updates and is fresh when the user switches to it. Manual-refresh button +
window-focus rescan remain the mandatory Windows backstops.

---

## 7. Concurrency analysis (AI + human on the same repos)

Model (unchanged): hold a state mutex only long enough to **clone the workdir PathBuf** (or snapshot the
open-repo list), then do all git2 work in `spawn_blocking`, opening the `Repository` fresh inside the
closure. The embedded tools slot into this exactly (§4 `run_blocking`). Consequences:

- **No shared `git2::Repository`** across the AI and command paths — each opens its own `!Send` handle on
  its own blocking thread; nothing crosses `.await` or threads. No new data race.
- **Interleaved index writes** (AI + human on the *same* repo simultaneously) are serialized by libgit2's
  on-disk `index.lock`: the loser gets a lock error → `AppError::Git` (→ `CallToolResult{is_error}` / UI
  toast). Fail-safe (no corruption), same as two `git` CLIs.
- **Multiple AI sessions on different tabs** are fully independent (separate workdirs, separate handles).
- **Stale UI mid-mutation** is resolved by `repo-changed` (§6.4).

**RESOLVED (D-6): no per-repo mutex for P16.** libgit2 index/ref locking + the fresh-open model suffices; a
Bonsai-level mutex would not cover the external `git` CLI and would add deadlock surface. Noted as a future
lever only.

---

## 8. Security model (LOAD-BEARING)

A localhost HTTP port that can **mutate any open repo** must not be drivable by any other local process or
by a web page in the user's browser. Loopback binding alone is insufficient.

### 8.1 Threat model
| Attacker | Capability | Defeated by |
|---|---|---|
| Remote network host | Cannot reach `127.0.0.1` | Bind `127.0.0.1` only (never `0.0.0.0`) |
| Other local process / CLI | Can POST to the port, but does **not** know the token | **Bearer token** (§8.2) |
| Malicious web page (DNS-rebinding / CSRF) | Browser can POST; **cannot read** cross-origin responses (SOP) and **cannot set** a valid `Authorization` it doesn't know | **Token** + **Origin/Host validation** + **no CORS** (§8.3) |
| Local process reading config | Could exfiltrate the persisted token from `settings.json` | Accepted (D-4): same trust level as recent-repos; token in memory + config, never logged |

### 8.2 Bearer token (persisted — D-4)
- 32 random bytes from an OS CSPRNG (`getrandom`/`rand::rngs::OsRng`), base64url (~256 bits), generated
  once and **persisted in `settings.json`** so the user's one-time `claude mcp add` line keeps working
  across app runs (D-4).
- **Required on every request**: `Authorization: Bearer <token>`. The `auth_layer` axum middleware (mounted
  *before* `/mcp`) rejects missing/malformed/mismatched with **HTTP 401**, empty body, **constant-time**
  compare (`subtle::ConstantTimeEq` or a manual `ct_eq`).
- Surfaced in the Settings UI (copyable) plus the full `claude mcp add` line (§10).

### 8.3 Browser-attack hardening (DNS-rebinding / CSRF)
In `auth_layer`:
- **`Origin` — RESOLVED (D-3): reject (403) ANY request that carries an `Origin` header.** Claude Code's
  HTTP transport is a non-browser client and sends no `Origin`; a browser page attacking us *will* send one.
  No browser origin is ever legitimate for this endpoint.
- **`Host`**: reject (403) unless exactly `127.0.0.1:<port>` or `localhost:<port>` — blocks DNS-rebinding.
- **No CORS**: never emit `Access-Control-Allow-Origin`; browsers cannot read any response.
- The token remains the primary defense: even a forged `Host`/absent `Origin` request still needs the secret
  it cannot obtain.

### 8.4 Write-gate + widened-surface tradeoff
When the write-gate is OFF the server registers **only the 14 read tools** (§4b, §9), so even a fully
authenticated client cannot mutate. Defense in depth: auth stops unknown callers; the write-gate bounds what
an *authorized* caller can do.

**D-2 tradeoff — ACCEPTED & bounded.** With repo-selection tools, an authenticated AI can reach **any open
repo tab**, not only the focused one. The hard boundary stands: it can act **only** on repos the user has
OPEN in Bonsai (entries in `AppState.repos`) — never an arbitrary filesystem path. `bonsai_select_repo`
**validates** `repoId` against the live open set and rejects anything else (§4b); the workdir is always a
canonical path already vetted by `open_repo`. There is no tool that opens a repo, takes a path, or escapes
the open set.

### 8.5 Port strategy (D-4)
Persisted-preferred: on first enable, bind an OS-chosen ephemeral port (`127.0.0.1:0`), read it back, and
**persist it in `settings.json`**; on later runs, try the saved port and fall back to a fresh ephemeral if
busy (updating the stored value + status). Stable `claude mcp add` URL across runs, no hard-coded collision.

### 8.6 Security deps
`getrandom`/`rand` (token), `subtle` (constant-time compare) — or hand-rolled `ct_eq`. Verify none conflict
with the pinned toolchain.

---

## 9. Write-gate as a UI toggle

The P14 `--allow-write` CLI flag becomes a per-app setting **"Allow AI to modify this repo"**, default
**OFF** (D-7). Reuses P14's router-merge truthfulness: when OFF the `write_router` is never merged, so
`tools/list` advertises only the **14 read tools** (P14 12 + the 2 D-2 tools); when ON it advertises **34**.

**RESOLVED (D-5): any `allow_write` change BOUNCES the server** (stop + restart), dropping all sessions and
forcing re-negotiation with the new tool set. This is correct for both directions (in particular, turning
OFF immediately revokes write from already-connected sessions). The **token and port stay stable** across a
bounce (kept in `McpServerState` / persisted, D-4), so the user's `claude mcp add` config keeps working; the
client simply reconnects.

`set_mcp_allow_write(true|false)` persists the setting, then bounces the server if running. Settings additions
(`UiSettings`, additive `#[serde(default)]`, same pattern as `ai_enabled`): `mcp_enabled: bool` (false),
`mcp_allow_write: bool` (false), `mcp_consented: bool` (false), `mcp_port: Option<u16>`,
`mcp_token: Option<String>`.

Consent (D-7): reuse the P13/P15 one-time consent pattern — enabling the server (and, separately, first
enabling write) shows a one-time confirmation explaining an external AI client will be able to read (and, if
write is on, modify) **any repo open in Bonsai**. Store `mcp_consented`.

---

## 10. Frontend IPC surface + mock

### 10.1 New Tauri commands (request/response)
| Command | Args | Returns | Purpose |
|---|---|---|---|
| `set_active_repo` | `{ repoId: string \| null }` | `void` | Frontend tells backend the focused tab; seeds new MCP sessions (§5). |
| `get_mcp_status` | `{}` | `McpStatus` | Current server state for the Settings panel. |
| `set_mcp_enabled` | `{ enabled: bool }` | `McpStatus` | Start/stop the embedded server (§6). |
| `set_mcp_allow_write` | `{ allowWrite: bool }` | `McpStatus` | Flip the write-gate; bounces if running (§9). |

All four are `async fn` following the existing `_inner` + lock pattern (settings writes may use
`spawn_blocking` like `set_ui_settings`; no git2).

### 10.2 New event (small push signal)
`mcp-server-changed` — emitted on start/stop/bounce; payload = `McpStatus`. (Existing `repo-changed` already
carries the live-update-on-AI-mutation path, including for non-focused tabs — no new event needed for that.)

### 10.3 TypeScript types (add to `src/ipc/types.ts`)
```ts
export interface McpStatus {
  enabled: boolean;        // server running?
  allowWrite: boolean;     // write tools registered?
  port: number | null;     // bound port when running, else null
  url: string | null;      // e.g. "http://127.0.0.1:8765/mcp"
  token: string | null;    // persisted bearer; null when stopped
  toolCount: number;       // 14 (read-only) or 34 (write enabled)
}
```
There is **no** `claudeAddCommand` field. The frontend builds the ready-to-paste
`claude mcp add` line itself via `src/lib/mcpAddCommand.ts` (`buildClaudeAddCommand`),
because the CLI's `-H, --header` flag is variadic — the server name + URL must come
**before** `--header`, which must be **last**. The helper takes an `McpScope`
(`'user'` | `'local'`) and the running server's `url`/`token`.
Add to `IpcApi` + `tauri.ts`:
```ts
setActiveRepo(repoId: string | null): Promise<void>;
getMcpStatus(): Promise<McpStatus>;
setMcpEnabled(enabled: boolean): Promise<McpStatus>;
setMcpAllowWrite(allowWrite: boolean): Promise<McpStatus>;
onMcpServerChanged(cb: (s: McpStatus) => void): Promise<Unsubscribe>;
```
`tauri.ts` maps to `invoke('set_active_repo', { repoId })` etc. and
`listen<McpStatus>('mcp-server-changed', ...)`.

Rust `McpStatus` (serde `rename_all = "camelCase"`) in `src-tauri/src/mcp.rs` mirrors the TS shape
(no `claude_add_command` field). Registration is a separate command, `register_mcp_with_claude`,
which runs `claude mcp add` for a given scope; the copy-to-clipboard variant of the same line is
produced client-side by `buildClaudeAddCommand` (`src/lib/mcpAddCommand.ts`).

### 10.4 Mock IPC (`src/ipc/mock.ts`) — MANDATORY
Implement all four commands + the event so the harness (`VITE_MOCK_IPC=1`) renders the Settings toggle,
status, URL, token:
- Module state `{ enabled, allowWrite, activeRepo }`.
- `setActiveRepo` stores `activeRepo` → `void`.
- `getMcpStatus`/`setMcpEnabled`/`setMcpAllowWrite` mutate state and return a canned `McpStatus` (fake port
  `8765`, fake token `"mock-token-abc123"`, `toolCount` 14 or 34, plausible `url`).
  `set*` also invokes any registered `onMcpServerChanged` callback.
- No real socket — the harness proves UI wiring; the actual server is exercised by the §12 AI-gate test.

### 10.5 SettingsPanel additions (`src/components/SettingsPanel.tsx`)
A new **"AI access (MCP server)"** section: enable toggle (with consent dialog); "Allow AI to modify this
repo" toggle (disabled unless enabled; warns it bounces the connection); status line (running/stopped + port
+ `toolCount`); read-only URL + token fields with copy buttons; and two registration scopes —
**Globally** (`user`) and **This repository** (`local`, gated on an open repo) — each with a Run
action (Tauri `register_mcp_with_claude`) and a Copy action (client-side `buildClaudeAddCommand`).
Subscribe to `onMcpServerChanged` to stay live. The presentational block lives in
`src/components/SettingsMcpSection.tsx`; `SettingsPanel` remains the container.

---

## 11. Sub-increments (each = one fresh-context senior-dev pass)

**P16a — Shared tool-layer factoring + `bonsai-mcp` lib split.** §3.1, §4. Add `[lib]`; introduce
`OpenRepo`, `SessionRepos`, and the `WorkdirSource { Fixed, Session }` enum **including the full per-session
selection shape** (so P16b needs no rework); change `BonsaiServer.workdir` + `run_blocking`; add
`with_session`; re-point `main.rs` to the lib. The P14 tool bodies are unchanged. The two D-2 tools are NOT
added yet. *Gate:* `cargo build/test -p bonsai-mcp` green; the P14 stdio integration test still passes
verbatim (proves the `Fixed` path is behavior-identical); unit tests on `SessionRepos`
(`resolve_workdir` with none-selected → `NoRepo`; selected-but-closed → `NoRepo`; `select` of an unknown id
→ `InvalidName`; `select` then `resolve_workdir` → the right path).

**P16b — Embedded HTTP server + active-repo + per-session selection + the 2 D-2 tools + token, read-only,
behind the UI toggle.** §4b, §5, §6, §8, §10. Add `active_repo` to `AppState` + `set_active_repo`; add
`bonsai_list_repos` + `bonsai_select_repo` to the read router; `src-tauri/src/mcp.rs` (`McpServerState`,
`McpStatus`, start/stop, the per-session `service_factory` seeding from `active_repo`,
`StreamableHttpService` wiring, `auth_layer` with token + Origin(D-3)/Host checks, persisted port/token per
D-4); `get_mcp_status`/`set_mcp_enabled`; `mcp-server-changed`; register in `lib.rs`; exit-hook shutdown.
Write-gate **forced OFF** (14 read tools only). Frontend: types + tauri.ts + mock.ts + the read-only Settings
section. *Gate:* §12 AI-gate items 1–2, 4–7; browser harness shows the toggle + surfaced URL/token.

**P16c — Write-gate toggle + mutation tools.** §9. `set_mcp_allow_write` + bounce-on-change; register the 20
write tools when on; consent dialog; `mcp_allow_write`/`mcp_enabled`/`mcp_consented`/`mcp_port`/`mcp_token`
settings. *Gate:* §12 AI-gate item 3 (write gating truthful across a toggle — 14 ↔ 34) + the conflict
round-trip.

**P16d — Live-update demo + integration test.** In-process HTTP MCP client test (§12 items 1–7) + document
the `claude mcp add` HTTP flow in `crates/bonsai-mcp/README.md` / a `docs/` note. *Gate:* `cargo test` green;
present for the USER CHECKPOINT.

---

## 12. Acceptance criteria

### 12.1 AI gate (orchestrator-verifiable, no native window)
An integration test (under `src-tauri/tests/`, scratch repos under `D:\Temp`) starts the embedded server on
`127.0.0.1:0`, reads back the port, seeds `AppState.repos` with **two** open scratch repos (A, B) and
`active_repo = A`, then drives it with an HTTP MCP client (rmcp's streamable-http client transport, or a raw
`reqwest` JSON-RPC POST harness):
1. **Token / Origin rejection:** no/wrong `Authorization` → **401**; correct bearer succeeds; a request
   carrying any `Origin` header → **403** (D-3).
2. **Read round-trip + tool count:** authenticated `tools/list` returns exactly the **14 read tools** when
   write is OFF; with the session seeded to A, `bonsai_get_graph` returns a `GraphLayout` whose
   `nodes.len()`/`headIndex` match a direct `compute_graph` on repo A.
3. **Write gating + conflict round-trip:** with write ON, `tools/list` returns the **34-tool** set; run the
   P14 headline conflict flow over HTTP (`bonsai_merge_branch` → `bonsai_get_conflict` →
   `bonsai_resolve_conflict_text` → `bonsai_commit_merge`) and assert the resulting tree oid equals the
   `git`-CLI oracle.
4. **No selection:** with the session seeded from `active_repo = None`, a git tool call returns
   `CallToolResult{is_error:true}` `kind:"noRepo"` — **no panic, no 500**.
5. **`list_repos` + `select_repo` + acting on a NON-active repo:** `bonsai_list_repos` returns both A and B
   with `selected` marking the seed (A); `bonsai_select_repo({repoId: B})` succeeds and returns B's summary;
   a subsequent `bonsai_get_status` reflects **B** (the non-focused tab), proving per-session, call-time
   resolution independent of `active_repo`.
6. **Unknown/closed repoId rejection:** `bonsai_select_repo({repoId: "C:/not/open"})` →
   `is_error` `kind:"invalidName"`; selecting B then removing B from `AppState.repos` and calling a git tool
   → `kind:"noRepo"` (closed-since).
7. **Browser harness** (`pnpm dev` + `VITE_MOCK_IPC=1`): the Settings "AI access" section renders; the
   enable/write toggles flip; the surfaced URL, token, and `claude mcp add` line display and copy.

### 12.2 USER CHECKPOINT (human / real Claude Code)
User enables the server in Settings, copies the `claude mcp add --transport http --header "Authorization:
Bearer <token>" http://127.0.0.1:<port>/mcp` line, registers it with a real Claude Code session, and
confirms: (a) the `bonsai_*` tools appear, incl. `bonsai_list_repos`/`bonsai_select_repo`; (b)
`bonsai_list_repos` enumerates the user's open tabs and the AI can `bonsai_select_repo` one; (c)
`bonsai_get_graph`/`bonsai_get_status` return sane data for the selected repo; (d) with write enabled, an AI
stage+commit (or conflict resolution) makes the Bonsai GUI **live-update** via `repo-changed` — including
when the AI acts on a tab other than the focused one. Orchestrator presents AI-gate evidence and asks the
user to run this — never self-declares.

---

## 13. Risks
1. **rmcp 3.0.1 HTTP API drift** — MEDIUM. Feature verified present; exact `StreamableHttpServerConfig`
   fields / mount call confirmed via `cargo tree` + docs at implement time (§2). Escape hatch: hand-rolled
   axum + JSON-RPC bridge reusing the same `BonsaiServer` tool router — last resort.
2. **axum version alignment** — MEDIUM. Pick the axum major matching rmcp 3.0.1's resolved
   `http`/`tower-service`; verify before P16b.
3. **Security posture** — HIGH if mis-set. Token + Origin(reject-any, D-3) + Host + loopback triad is
   mandatory; never ship a mutating variant without the token. Covered by AI-gate item 1.
4. **Widened surface (D-2)** — MEDIUM, bounded (§8.4). AI reaches any OPEN tab but never an arbitrary path;
   `select_repo` validates against `AppState.repos`. Covered by AI-gate items 5–6.
5. **Write concurrency lock churn** — LOW (§7). Fail-safe `index.lock` errors.
6. **Toolchain deps** (`getrandom`/`subtle`/axum) on the pinned Windows MSVC toolchain — LOW; verify in P16b.
7. **Tokio runtime sharing** — LOW. axum serve on `tauri::async_runtime`; git2 in `spawn_blocking`. Confirm
   Tauri v2's multi-thread default.

---

## 14. Resolved decisions (confirmed by the user)

- **D-1 — RESOLVED:** axum/token/lifecycle wiring lives in `src-tauri/src/mcp.rs`; `bonsai-mcp` stays
  transport-agnostic. *(§3.2)*
- **D-2 — RESOLVED (CHANGED from architect recommendation): EXPOSE repo-selection tools.** The AI enumerates
  and chooses among the user's open tabs via `bonsai_list_repos` / `bonsai_select_repo` (per-session
  selection), rather than implicitly following the focused tab. Reworked throughout: §4 (per-session
  `SessionRepos`), §4b (the two new read tools), §5 (active_repo = seed vs per-session selection = acted-on),
  §8.4 (widened-surface tradeoff, bounded to the open set), §12 (items 5–6). Tool counts → **14 read / 34
  with write**. *(§4, §4b, §5)*
- **D-3 — RESOLVED:** reject (403) any request carrying an `Origin` header. *(§8.3)*
- **D-4 — RESOLVED:** **persist token + port** in `settings.json` for a stable one-time `claude mcp add`.
  *(§8.2, §8.5)*
- **D-5 — RESOLVED:** bounce the server on any `allow_write` change; token/port stay stable. *(§9)*
- **D-6 — RESOLVED:** no per-repo serialization for P16 (rely on libgit2 locking). *(§7)*
- **D-7 — RESOLVED:** server default OFF + write default OFF + one-time consent dialog (P13/P15 pattern).
  *(§9)*

---

## Appendix A — signatures & interface surface

Rust (new / changed):
```rust
// crates/bonsai-mcp/src/server.rs
pub struct OpenRepo { pub repo_id: String, pub path: PathBuf }
pub struct SessionRepos { /* selected: Mutex<Option<String>>, list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync> */ }
impl SessionRepos { pub fn new(seed: Option<String>, list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync>) -> Self; }
pub enum WorkdirSource { Fixed(Arc<PathBuf>), Session(Arc<SessionRepos>) }
impl WorkdirSource { pub fn resolve(&self) -> Result<PathBuf, AppError>; }
impl BonsaiServer {
    pub fn new(workdir: PathBuf, allow_write: bool) -> Self;               // Fixed source (standalone; unchanged)
    pub fn with_session(repos: Arc<SessionRepos>, allow_write: bool) -> Self;  // NEW (embedded)
}
// new read-tier tools: bonsai_list_repos() -> [OpenRepoSummary]; bonsai_select_repo(SelectRepoArgs) -> OpenRepoSummary

// src-tauri/src/state.rs
pub struct AppState { pub repos: Mutex<HashMap<String, RepoEntry>>, pub active_repo: Mutex<Option<String>> }

// src-tauri/src/mcp.rs (NEW)
pub struct McpRunning { pub port: u16, pub token: String, pub allow_write: bool, /* shutdown, task */ }
#[derive(Default)] pub struct McpServerState { pub inner: std::sync::Mutex<Option<McpRunning>> }
#[derive(Clone, serde::Serialize)] #[serde(rename_all="camelCase")]
pub struct McpStatus {
    pub enabled: bool, pub allow_write: bool, pub port: Option<u16>,
    pub url: Option<String>, pub token: Option<String>,
    pub tool_count: u32,   // 14 or 34 (no claude_add_command field; built client-side)
}

// src-tauri/src/commands.rs (NEW #[tauri::command] async fns)
pub async fn set_active_repo(state, repo_id: Option<String>) -> Result<(), AppError>;
pub async fn get_mcp_status(state /* McpServerState */) -> Result<McpStatus, AppError>;
pub async fn set_mcp_enabled(app, state, mcp_state, enabled: bool) -> Result<McpStatus, AppError>;
pub async fn set_mcp_allow_write(app, state, mcp_state, allow_write: bool) -> Result<McpStatus, AppError>;
```

TypeScript (`src/ipc/types.ts`): `McpStatus` (§10.3); `IpcApi` gains `setActiveRepo`, `getMcpStatus`,
`setMcpEnabled`, `setMcpAllowWrite`, `onMcpServerChanged`.

IPC surface summary — **Commands:** `set_active_repo`, `get_mcp_status`, `set_mcp_enabled`,
`set_mcp_allow_write`. **Events:** `mcp-server-changed` (+ existing `repo-changed` carries the live-update).
**Channels:** none new (MCP streaming is internal to the axum/rmcp HTTP endpoint, not a Tauri channel).
**MCP tools:** P14's 14 read (12 + `bonsai_list_repos` + `bonsai_select_repo`) / 34 with write. Settings
(`UiSettings`, additive `#[serde(default)]`): `mcp_enabled` (false), `mcp_allow_write` (false),
`mcp_consented` (false), `mcp_port: Option<u16>`, `mcp_token: Option<String>`.
