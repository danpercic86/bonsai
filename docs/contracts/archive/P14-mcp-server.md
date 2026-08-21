# P14 — `bonsai-core` crate + standalone `bonsai-mcp` MCP server

Status: contract (design only). Author: architect. Depends on all prior contracts (types are
reused verbatim from the existing `git/*`, `graph.rs`, `error.rs`).

## 0. Goal & scope

Extract the pure Git/graph logic into a reusable `bonsai-core` library crate, then build a
standalone **stdio MCP server** (`bonsai-mcp`) that exposes only Bonsai's *differentiated* surface
to AI assistants (Claude Code is the reference consumer). This is Tier 2 of the AI-integration
analysis: NOT a 1:1 mirror of `git` (redundant with the `git` CLI Claude Code already drives).
The value is:

1. `get_graph` → precomputed `GraphLayout` (lane/edge topology) — the crown jewel.
2. Structured typed diffs (`FileDiff`/`Hunk`/`DiffLine`, `CommitDiff`, `CompareDiff`).
3. The conflict trio: `list_conflicts` → `get_conflict` (structured ours/theirs/base +
   `ConflictKind`) → `resolve_conflict_text(path, content)`. Strongest AI use case.
4. Safety-railed mutations (FF-only, autostash, blocked unmerged-delete) — a *safer* git.

Non-goals: no new Git features, no changes to the Tauri app's behavior, no network/credential
tooling in v1 (see §7.2), no re-exposing the P13 in-app AI subprocess layer (§7.2).

Hard rule for the implementer: **no application logic is rewritten.** P14a is a pure *move* +
`pub` visibility changes + import re-pointing. `bonsai-mcp` is a thin adapter that calls existing
`bonsai_core` functions verbatim.

---

## 1. Workspace layout

Convert the single `src-tauri` crate into a 3-member Cargo workspace. Keep `src-tauri` where it is
(minimizes Tauri disruption); add two crates under a new `crates/` dir.

```
D:\Repos\Playground\bonsai\
  Cargo.toml                 # NEW: workspace root (virtual manifest, no [package])
  rust-toolchain.toml        # unchanged (already at repo root)
  package.json               # unchanged
  src/                       # React frontend, unchanged
  crates/
    bonsai-core/
      Cargo.toml             # NEW: package `bonsai-core`, lib name `bonsai_core`
      src/lib.rs             # NEW: re-exports the moved modules
      src/error.rs           # MOVED from src-tauri/src/error.rs
      src/graph.rs           # MOVED
      src/fixture.rs         # MOVED
      src/testutil.rs        # MOVED
      src/git/               # MOVED (whole dir, all 14 files incl. ai_resolve.rs)
      src/ai/                # MOVED (mod.rs — see §2.3 decision + flag)
      benches/graph_layout.rs  # MOVED from src-tauri/benches/
      tests/                 # MOVED: all 18 integration tests + tests/common/mod.rs
    bonsai-mcp/
      Cargo.toml             # NEW: package `bonsai-mcp`, bin `bonsai-mcp`
      src/main.rs            # NEW
      src/server.rs          # NEW (tool router)
      README.md              # NEW (claude mcp add instructions, §11d)
  src-tauri/
    Cargo.toml               # EDITED: drops moved deps, adds `bonsai-core` path dep
    src/lib.rs               # EDITED: module decls removed, `use bonsai_core::...`
    src/commands.rs          # EDITED: import paths only
    src/state.rs             # unchanged logic (watcher dep stays local)
    src/settings.rs          # unchanged
    src/watcher.rs           # unchanged
    tauri.conf.json          # unchanged
    build.rs / capabilities  # unchanged
```

### 1.1 Root workspace `Cargo.toml` (NEW)

```toml
[workspace]
members = ["src-tauri", "crates/bonsai-core", "crates/bonsai-mcp"]
resolver = "2"

[workspace.dependencies]
git2 = "0.20"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tempfile = "3"
```

Members reference these via `git2 = { workspace = true }` etc. (optional but recommended for
version unification). `criterion = "0.5"` stays a `bonsai-core` dev-dep (only the bench uses it).

### 1.2 `crates/bonsai-core/Cargo.toml` (NEW)

```toml
[package]
name = "bonsai-core"
version = "0.1.0"
edition = "2021"

[lib]
name = "bonsai_core"

[dependencies]
git2 = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
criterion = "0.5"

[[bench]]
name = "graph_layout"
harness = false
```

No `notify`, no `tauri`, no `tauri-plugin-dialog` — those stay app-only.

### 1.3 `crates/bonsai-core/src/lib.rs` (NEW)

```rust
pub mod ai;        // §2.3 — flagged
pub mod error;
#[doc(hidden)]
pub mod fixture;   // pub (was already exported) so the bench + gate test reach it
pub mod git;
pub mod graph;
#[doc(hidden)]
pub mod testutil;  // was #[cfg(test)]; make pub #[doc(hidden)] so tests/ + benches reach it
```

`testutil` visibility change (from `#[cfg(test)] pub mod` to `#[doc(hidden)] pub mod`): required
because integration tests in `tests/` and the perf bench live in separate compilation units and
cannot see a `#[cfg(test)]` lib module. This is a pure visibility widening, no logic change.
`relax_odb_hash_verification()` stays in `git/mod.rs` and is re-exported as
`bonsai_core::git::relax_odb_hash_verification`.

### 1.4 `src-tauri/Cargo.toml` (EDITED)

Remove `git2`, `serde_json`, `thiserror` as direct deps only if unused elsewhere — **keep `serde`**
(commands.rs defines its own `#[derive(Serialize)]` payload types) and **keep `serde_json`** if
still referenced. Add:

```toml
bonsai-core = { path = "../crates/bonsai-core" }
```

Keep `tauri`, `tauri-plugin-dialog`, `notify`. The `[[bench]]` block and `criterion` dev-dep
**move out** of `src-tauri` into `bonsai-core`. `tempfile` dev-dep stays in `src-tauri` only if any
`src-tauri/tests/*` remain there (none do after §2.4 — so it can be dropped from src-tauri).

### 1.5 Tauri workspace build — verified expectations & gotchas

Tauri v2 fully supports Cargo workspaces. Confirmed-safe:
- `tauri.conf.json` paths (`frontendDist: "../dist"`, `beforeBuildCommand`) are relative to the
  `src-tauri` dir and are unaffected by adding a workspace root.
- `tauri-build`/`build.rs` and `capabilities/` read `tauri.conf.json` from the `src-tauri` dir;
  unaffected.
- The crate's `[lib] crate-type = ["staticlib","cdylib","rlib"]` and `bonsai_lib` name stay as-is.

**Gotchas the implementer MUST verify (AI gate P14a):**
1. The build target dir moves from `src-tauri/target/` to the workspace-root `target/`. Check
   `.gitignore` still ignores `target/` (root-level `target/` — add if only `src-tauri/target/` was
   listed). Check no tooling hardcodes `src-tauri/target`.
2. `pnpm tauri dev` and `pnpm tauri build` must still work. Tauri CLI auto-detects the workspace;
   no `tauri.conf.json` change is expected. If the CLI cannot find the binary, set
   `build.runner` or verify the CLI resolves the `bonsai` package by name — do NOT change the
   package name.
3. `cargo build`/`cargo test` are now **workspace-wide** by default; running from repo root builds
   all three crates. Use `-p bonsai` / `-p bonsai-core` / `-p bonsai-mcp` to scope.
4. MEMORY rule still applies: never run `cargo test` and `cargo clippy` concurrently
   (shared `target/` race); run sequentially. Scratch/temp under `D:\Temp`.

---

## 2. What moves into `bonsai-core` vs stays in the Tauri app

### 2.1 Moves to `bonsai-core` (pure — verified: depends only on `crate::error` + each other)

| Item | Current path | New path |
|---|---|---|
| `AppError` | `src-tauri/src/error.rs` | `crates/bonsai-core/src/error.rs` |
| Graph engine + all graph types | `src-tauri/src/graph.rs` | `crates/bonsai-core/src/graph.rs` |
| All git wrappers + their serde types | `src-tauri/src/git/*` (14 files) | `crates/bonsai-core/src/git/*` |
| Fixture generator | `src-tauri/src/fixture.rs` | `crates/bonsai-core/src/fixture.rs` |
| Test util | `src-tauri/src/testutil.rs` | `crates/bonsai-core/src/testutil.rs` |
| Graph bench | `src-tauri/benches/graph_layout.rs` | `crates/bonsai-core/benches/graph_layout.rs` |
| All integration tests | `src-tauri/tests/*` (18 files + `common/`) | `crates/bonsai-core/tests/*` |

**Verification performed:** every `git/*.rs` and `graph.rs` imports only `crate::error::AppError`
and sibling `crate::git::*` items — **zero `tauri::` imports**. The serde types the MCP server
needs are all defined *inside* these modules (confirmed): `graph::{GraphLayout, GraphNode,
GraphEdge, RefLabel, RefKind}`, `status::{StatusSnapshot, StatusEntry, FileStatus}`,
`diff::{FileDiff, Hunk, DiffLine, LineKind, FileDiffHeader, CommitDiff, CommitDetails, CompareDiff,
CompareEndpoint}`, `branches::{BranchesSnapshot, BranchInfo, RemoteBranchInfo,
CreateBranchHereResult}`, `repo::{RepoInfo, HeadInfo}`, `conflict::{ConflictEntry, ConflictFile,
ConflictKind, ConflictResolution}`, `merge::MergeOutcome`, `rebase::RebaseOutcome`,
`opstate::RepoOpState`, `stash::{StashEntry, ApplyStashOutcome, CreateStashResult}`,
`remote::{FetchResult, RemoteFetchResult, PullResult, PushResult}`, `commit::CommitResult`. These
are already `pub` and already derive `serde::Serialize` (inputs like `ConflictResolution`,
`UiSettingsPatch` derive `Deserialize`). **No new derives are needed on core types** — the MCP
server serializes outputs with `serde_json` and accepts inputs via its own param structs (§6).

### 2.2 Stays in the Tauri app (`bonsai`/`bonsai_lib`)

`commands.rs` (all `#[tauri::command]` + `*_inner` helpers), `state.rs` (`AppState`, `RepoEntry` —
holds `WatcherHandle`), `settings.rs`, `watcher.rs` (`notify`), and `lib.rs::run()`. These keep
Tauri/app-shell coupling. Their only edit is import re-pointing: `crate::git::…` →
`bonsai_core::git::…`, `crate::graph::…` → `bonsai_core::graph::…`, `crate::error::AppError` →
`bonsai_core::error::AppError`, `crate::ai::…` → `bonsai_core::ai::…`,
`crate::git::ai_resolve::…` → `bonsai_core::git::ai_resolve::…`.

### 2.3 DECISION + FLAG — the `ai` module & `git/ai_resolve.rs`

The prompt suggested keeping `ai` in the Tauri app. Investigation contradicts the premise that
this is clean: `git/ai_resolve.rs` **imports `crate::ai`** and is part of the `git` module tree
that must move. `ai/mod.rs` itself is **pure** (imports only `crate::error::AppError` + `std::*`;
it shells out to the `claude` CLI via `std::process::Command`). Options:

- **Option A (RECOMMENDED): move `ai` + `git/ai_resolve.rs` into `bonsai-core`.** Keeps the `git`
  module tree and all its tests (`ai_resolve_cli.rs` uses `bonsai_lib::ai::{run_claude, RunOpts,
  DEFAULT_MODEL}`) intact as one unit. `ai` is pure, so this drags nothing Tauri into core. The
  MCP server simply does **not** expose any AI tool (§7.2). Minimal churn, cleanest boundary.
- **Option B: keep `ai` in the app; relocate `git/ai_resolve.rs` out of `git/` into
  `src-tauri/src/ai_resolve.rs`.** Honors the prompt literally but splits the git tree, forces
  `ai_resolve_cli.rs` to stay in `src-tauri/tests` and depend on *both* crates, and re-points
  `commands.rs` from `crate::git::ai_resolve` to `crate::ai_resolve`. More churn, weaker cohesion.

**Recommendation: Option A.** Rationale: `ai` is pure and is a genuine dependency of core git
logic; the app-vs-core split is about *Tauri coupling*, and `ai` has none. **Flag for
orchestrator:** this deviates from the prompt's "keep `ai` in the app" suggestion; confirm before
P14a if Option B is preferred for policy reasons.

### 2.4 Tests & bench that move (all reference `bonsai_lib::{git,graph,error,fixture}` today)

All 18 files in `src-tauri/tests/` plus `tests/common/mod.rs`, and `benches/graph_layout.rs`, move
to `bonsai-core` and change `bonsai_lib::` → `bonsai_core::`. `ai_resolve_cli.rs` additionally
changes `bonsai_lib::ai` → `bonsai_core::ai` (Option A). No test *logic* changes — this is a
rename-and-move. After the move, `cargo test -p bonsai-core` runs the full existing CLI-oracle
suite; `cargo test --workspace` must be green (existing tests are the P14a regression gate).

---

## 3. MCP SDK & transport

**Use the official Rust SDK: `rmcp`** (`modelcontextprotocol/rust-sdk`). It is the idiomatic
choice: `#[tool_router]`/`#[tool]`/`#[tool_handler]` macros generate JSON Schemas from
`schemars`-derived param structs and handle stdio JSON-RPC framing.

```toml
# crates/bonsai-mcp/Cargo.toml
[package]
name = "bonsai-mcp"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "bonsai-mcp"
path = "src/main.rs"

[dependencies]
bonsai-core = { path = "../bonsai-core" }
rmcp = { version = "<PIN AT IMPLEMENT TIME>", features = ["server", "transport-io", "macros"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "io-std"] }
serde = { workspace = true }
serde_json = { workspace = true }
schemars = "<match rmcp's re-export>"   # prefer `rmcp::schemars` to avoid version skew
anyhow = "1"
```

**RISK — rmcp version churn (flag).** crates.io currently resolves `rmcp` to a fast-moving `0.1x`
series (0.16.x observed 2026-07) while some docs show a `3.x` line; the macro surface
(`Parameters<T>`, `#[tool_router(server_handler)]`, `CallToolResult`) has shifted across minor
versions. **Senior-dev MUST run `cargo add rmcp --features server,transport-io,macros` at
implement time, pin the exact resolved version in the contract's follow-up note, and adapt to the
installed macro API** — do not hardcode the snippet below verbatim if the macros differ. Prefer the
`rmcp`-re-exported `schemars` (`use rmcp::schemars;`) so the derive version always matches the SDK.
If (and only if) `rmcp` fails to build on the pinned Windows toolchain, fall back to a hand-rolled
JSON-RPC-over-stdio loop reading line-delimited MCP messages (`initialize`, `tools/list`,
`tools/call`) — but this is the escape hatch, not the plan.

Transport: **stdio** (`rmcp::transport::stdio()`), so the server registers with
`claude mcp add bonsai -- <path-to-bonsai-mcp.exe> --repo <path>` (§11d).

Async runtime: `#[tokio::main]`. git2 is blocking → §5.

Canonical server shape (adapt to pinned API):

```rust
// src/main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = ServerConfig::from_args();      // parses --repo <path> [--allow-write]
    let server = BonsaiServer::new(cfg)?;     // opens+validates repo once (§4)
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

---

## 4. Repo-open model for the server

**Decision: single repo, opened from a required `--repo <path>` startup argument.** The server
holds one immutable `BonsaiServer` value:

```rust
#[derive(Clone)]
struct BonsaiServer {
    /// Canonical workdir path (from bonsai_core::git::repo::read_repo_info().path).
    workdir: std::sync::Arc<std::path::PathBuf>,
    /// Mutation tools are inert unless true (§7.3). Default false.
    allow_write: bool,
    tool_router: rmcp::handler::server::router::tool::ToolRouter<BonsaiServer>, // per pinned API
}
```

Startup validation (fail fast, exit non-zero with a clear stderr message BEFORE serving):
call `read_repo_info(--repo)`; require `is_repo == true && bare == false`. Store `info.path`
(canonical) as `workdir`. Rationale for single-repo: matches how Claude Code is launched per
project (one workspace = one repo); keeps every tool argument-light (no `repo_path` on each call);
mirrors the app's `repoId == canonical workdir path` model without needing `AppState`'s multi-repo
map or watcher.

**No `Repository` caching.** Every `bonsai_core` function already opens the repo itself from the
workdir path (`open_workdir_repo(workdir)` / `Repository::open_ext(.., NO_SEARCH, ..)`), so the
server holds only the `PathBuf`. git2 opens are cheap; the heavy cost is the walk inside
`compute_graph`, which is inherent and not helped by caching a handle. (`git2::Repository` is
`!Send` and must not be held across `.await` or shared between worker threads anyway — see §5.)

**Rejected alternative:** a per-tool `repo_path` argument. More flexible (one server, many repos)
but heavier for the AI and invites path-injection mistakes. **Flag for orchestrator:** if a
multi-repo server is later desired, add an *optional* `repo_path` override param to each tool that
defaults to the startup `--repo`; the tool bodies already take a `&Path`, so this is additive.

---

## 5. Threading (git2 is blocking)

rmcp drives tools on the tokio async runtime; every `bonsai_core` call is blocking git2. Each tool
body wraps its core call in `tokio::task::spawn_blocking`, cloning the `Arc<PathBuf>` into the
closure so nothing `!Send` crosses `.await`:

```rust
async fn run_blocking<T, F>(&self, f: F) -> Result<T, bonsai_core::error::AppError>
where
    T: Send + 'static,
    F: FnOnce(&std::path::Path) -> Result<T, bonsai_core::error::AppError> + Send + 'static,
{
    let workdir = self.workdir.clone();
    tokio::task::spawn_blocking(move || f(workdir.as_path()))
        .await
        .map_err(|e| bonsai_core::error::AppError::Other(format!("task join error: {e}")))?
}
```

The closure opens its own `Repository` inside `bonsai_core`; the handle never escapes the blocking
thread. This mirrors the Tauri app's `spawn_blocking` discipline exactly.

---

## 6. Error mapping

`AppError` already serializes as `{ "kind": "...", "message": "..." }` (see `error.rs`). The MCP
server surfaces two error tiers:

- **Protocol errors** (JSON-RPC `error`, i.e. `rmcp::ErrorData`/`McpError`): only for
  server-fatal/param-level failures — malformed tool arguments (rmcp raises these automatically
  from `Parameters<T>` deserialization), or a mutation tool invoked while `allow_write == false`
  (return `ErrorData::invalid_request` with a clear message).
- **Tool-domain errors** (`CallToolResult` with `is_error = true`): every `AppError` returned by a
  `bonsai_core` call. Preserve the discriminant so the AI can branch on it — put the
  `{kind, message}` JSON in structured content AND a human `"<kind>: <message>"` string in text.

Two helpers in `server.rs`:

```rust
/// Success: structured JSON content (serde of the core type) + a compact text echo.
fn ok_json<T: serde::Serialize>(v: &T) -> rmcp::model::CallToolResult; // is_error = false

/// Domain error: preserves AppError's {kind,message}; is_error = true.
fn err_result(e: bonsai_core::error::AppError) -> rmcp::model::CallToolResult;
```

`err_result` obtains `{kind,message}` via `serde_json::to_value(&e)` (AppError's custom Serialize).
Every `AppError::kind` string listed in `error.rs` (`git`, `io`, `noRepo`, `nothingToCommit`,
`checkoutConflict`, `unmergedBranch`, `noUpstream`, `pushRejected`, `operationInProgress`,
`unresolvedConflicts`, `invalidName`, …) reaches the AI unchanged. This lets an assistant, e.g.,
see `kind == "checkoutConflict"` and stash before retrying.

---

## 7. The curated tool set

Naming: snake_case, prefixed `bonsai_` to avoid collisions with any built-in git tools in the
consumer. All outputs are the exact `bonsai_core` serde types from §2.1 (camelCase JSON, as the
frontend already receives). "oid" = full 40-char hex; "path" = repo-relative, forward slashes.

### 7.1 Read tools (always registered; safe)

| Tool | Input (JSON) | Core call | Output type | Why over `git` CLI |
|---|---|---|---|---|
| `bonsai_get_graph` | `{}` | `graph::compute_graph(wd)` | `GraphLayout` | Precomputed lane/edge topology + HEAD index + ref pills; **impossible** from CLI without reimplementing Bonsai's layout math. |
| `bonsai_get_status` | `{}` | `status::read_status(wd)` | `StatusSnapshot` | Structured staged/unstaged/untracked/**conflicted** split lists with rename detection; no porcelain parsing. |
| `bonsai_list_branches` | `{}` | `branches::list_refs(wd)` | `BranchesSnapshot` | One call yields locals+remotes+tags+HEAD with per-branch upstream + ahead/behind + tips. |
| `bonsai_get_commit_diff` | `{ "oid": string }` | `diff::commit_diff(wd, oid)` | `CommitDiff` | Commit details + per-file headers (adds/dels/status) vs first parent, structured. |
| `bonsai_get_commit_file_diff` | `{ "oid": string, "path": string, "origPath"?: string }` | `diff::commit_file_diff(wd, oid, path, origPath)` | `FileDiff` | Typed hunks/lines with old/new line numbers; no `@@` parsing. |
| `bonsai_get_workdir_file_diff` | `{ "path": string, "origPath"?: string, "staged": bool }` | `diff::workdir_file_diff(wd, path, origPath, staged)` | `FileDiff` | Structured working-dir diff (`staged=false`: index↔workdir; `staged=true`: HEAD↔index). |
| `bonsai_compare_with_head` | `{ "oid": string }` | `diff::compare_head_diff(wd, oid)` | `CompareDiff` | Tree-vs-tree HEAD→oid file headers, structured. |
| `bonsai_compare_with_head_file_diff` | `{ "oid": string, "path": string, "origPath"?: string }` | `diff::compare_head_file_diff(wd, oid, path, origPath)` | `FileDiff` | Per-file hunks of the HEAD→oid comparison. |
| `bonsai_get_op_state` | `{}` | `opstate::read_op_state(wd)` | `RepoOpState` | Tells the AI if a merge/rebase/cherry-pick/revert is mid-flight and the step counters — drives the resolution loop. |
| `bonsai_list_conflicts` | `{}` | `conflict::list_conflicts(wd)` | `Vec<ConflictEntry>` | Structured conflict inventory with `ConflictKind` per path. |
| `bonsai_get_conflict` | `{ "path": string }` | `conflict::get_conflict(wd, path)` | `ConflictFile` | **The crown conflict tool:** separated `ours`/`theirs` blob text + marker text + kind + binary/tooLarge/missing flags — exactly what an AI needs to author a resolution. |
| `bonsai_list_stashes` | `{}` | `stash::list_stashes(wd)` | `Vec<StashEntry>` | Structured stash stack (index/message/oid/base/ts). |

### 7.2 Explicitly NOT exposed (and why)

- `open_repo` / `close_repo` — server holds one repo from `--repo` (§4); no runtime open.
- `get_recent_repos` / `remove_recent_repo` / `get_ui_settings` / `set_ui_settings` /
  `get_session` / `set_session` — pure UI persistence, no value to an AI, and they depend on the
  Tauri `AppHandle`/settings file that don't exist in the server.
- `fetch` / `pull` / `push` — **out of v1.** They perform network I/O and trigger credential
  resolution (Windows Credential Manager / SSH agent); side-effectful, non-differentiated (the
  `git` CLI does these and Claude Code can invoke it directly), and awkward to sandbox. *Note:*
  `pull` (FF-only) is genuinely safer than raw `git pull`; if the orchestrator wants it later, add
  it as a gated mutation returning `PullResult` — **flag** deferred, not designed here.
- `checkout_remote` / `delete_remote_tracking` — low AI value; deferred with the remote family.
- `check_ai_availability` / `ai_resolve_conflict` (P13) — **deliberately excluded.** The MCP
  consumer *is* an AI; having a tool that shells out to the `claude` CLI to resolve conflicts would
  be recursive and can loop. The AI should call `bonsai_get_conflict` and author the resolution
  itself, then `bonsai_resolve_conflict_text`. (This is also why moving `ai` into core in §2.3 is
  harmless — it is simply never surfaced.)

### 7.3 Mutation tools (registered only when `--allow-write`; default OFF)

**Recommendation: read-only by default.** The server starts safe; `--allow-write` opts in. When
off, mutation tools are **not registered** so `tools/list` advertises only the read set (the AI
sees exactly what it can do). Implement via router composition: a base `tool_router` for §7.1 and a
second `write_router` merged in only when `allow_write` (rmcp `ToolRouter` supports merge/`+`;
adapt to pinned API). If the pinned rmcp lacks router merge, fall back to registering all tools and
having each mutation body early-return an `ErrorData::invalid_request("write tools disabled; start
bonsai-mcp with --allow-write")` when `!allow_write`.

| Tool | Input (JSON) | Core call | Output | Safety rail / why |
|---|---|---|---|---|
| `bonsai_stage` | `{ "paths": [string] }` | `stage::stage_paths(wd, &paths)` | `null` | Atomic batch stage; worktree untouched. |
| `bonsai_unstage` | `{ "paths": [string] }` | `stage::unstage_paths(wd, &paths)` | `null` | Never touches worktree. |
| `bonsai_commit` | `{ "message": string }` | `commit::create_commit(wd, msg)` | `CommitResult` | Errors clearly on empty message / missing git identity / nothing-to-commit (`kind`). |
| `bonsai_resolve_conflict_text` | `{ "path": string, "content": string }` | `conflict::resolve_conflict_text(wd, path, &content)` | `null` | **Primary AI resolution path:** writes AI-authored merged text to the worktree file and stages it; `validate_rel_path` guards traversal. |
| `bonsai_resolve_conflict` | `{ "path": string, "resolution": "ours"\|"theirs"\|"markResolved" }` | `conflict::resolve_conflict(wd, path, ConflictResolution)` | `null` | Take-ours / take-theirs / mark-resolved shortcut per the P3c matrix. `resolution` deserializes into `ConflictResolution` (camelCase). |
| `bonsai_merge_branch` | `{ "name": string }` | `merge::merge_branch(wd, name)` | `MergeOutcome` | FF/clean-merge/conflicts distinguished in the typed outcome; autostash handled; never force. |
| `bonsai_commit_merge` | `{ "message": string }` | `merge::commit_merge(wd, msg)` | `CommitResult` | Finalizes a paused merge; refuses on `unresolvedConflicts`. |
| `bonsai_abort_merge` | `{}` | `merge::abort_merge(wd)` | `null` | Worktree-destructive → allowed only under `--allow-write`; the AI must confirm intent in its own turn. |
| `bonsai_rebase_branch` | `{ "onto": string }` | `rebase::rebase_branch(wd, onto)` | `RebaseOutcome` | Typed FF/rebased/conflicts with step counters; needed to make conflict resolution end-to-end. |
| `bonsai_rebase_continue` | `{}` | `rebase::rebase_continue(wd)` | `RebaseOutcome` | Resume after resolving. |
| `bonsai_rebase_skip` | `{}` | `rebase::rebase_skip(wd)` | `RebaseOutcome` | Skip current step. |
| `bonsai_rebase_abort` | `{}` | `rebase::rebase_abort(wd)` | `null` | Worktree-destructive; gated. |
| `bonsai_create_branch` | `{ "name": string }` | `branches::create_branch(wd, name)` | `null` | At HEAD, no checkout; `invalidName`/`branchExists` mapped. |
| `bonsai_create_branch_here` | `{ "name": string, "oid": string }` | `branches::create_branch_here(wd, name, oid)` | `CreateBranchHereResult` | Branch at a commit with autostash across checkout. |
| `bonsai_checkout_branch` | `{ "name": string }` | `branches::checkout_branch(wd, name)` | `null` | Safe checkout — **never force**; `checkoutConflict` surfaces instead of clobbering. |
| `bonsai_delete_branch` | `{ "name": string }` | `branches::delete_branch(wd, name)` | `null` | Blocks unmerged deletion (`unmergedBranch`); no force-delete — a *safer* git. |
| `bonsai_create_stash` | `{ "message"?: string, "includeUntracked": bool }` | `stash::create_stash(wd, message, includeUntracked)` | `CreateStashResult` | Lets the AI park work before a risky op. `created=false` = nothing to stash (not an error). |
| `bonsai_apply_stash` | `{ "index": number }` | `stash::apply_stash(wd, index)` | `ApplyStashOutcome` | Apply without dropping; conflicts reported as typed paths. |
| `bonsai_pop_stash` | `{ "index": number }` | `stash::pop_stash(wd, index)` | `ApplyStashOutcome` | Apply+drop on clean success only. |
| `bonsai_drop_stash` | `{ "index": number }` | `stash::drop_stash(wd, index)` | `null` | Permanent; gated by `--allow-write`. |

Optional trims for a leaner v1 (orchestrator's call — **flag**): the rebase family and stash
apply/pop/drop can be deferred if P14c scope is too large; the *minimum* end-to-end conflict story
is `merge_branch → get_conflict → resolve_conflict_text/resolve_conflict → commit_merge`
(+ `abort_merge` escape). Keep at least those.

### 7.4 Input param structs (in `bonsai-mcp`, derive `Deserialize + schemars::JsonSchema`)

Defined in `server.rs`, one per tool that takes args; wrapped by `rmcp`'s `Parameters<T>`. Field
docs become the JSON-Schema descriptions the AI reads — write them. Example:

```rust
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct GetConflictArgs {
    /// Repo-relative path (forward slashes) of a currently-conflicted file.
    path: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResolveConflictTextArgs {
    /// Repo-relative path of the conflicted file to resolve.
    path: String,
    /// Full final file content (no conflict markers). Written to the worktree and staged.
    content: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResolveConflictArgs {
    path: String,
    /// One of: "ours" | "theirs" | "markResolved".
    resolution: bonsai_core::git::conflict::ConflictResolution,
}
```

`ConflictResolution` already derives `Deserialize` with `rename_all = "camelCase"`; add
`#[cfg_attr(feature = ..., derive(JsonSchema))]`? — **no**: to avoid adding `schemars` to core,
the MCP arg struct may instead take `resolution: String` and map it to `ConflictResolution` via
`serde_json::from_value(json!(resolution))` (or a small local match). Recommend the local match to
keep core dependency-free. (Same technique wherever a core `Deserialize` enum is a tool input.)

---

## 8. Acceptance criteria

### 8.1 AI gate (orchestrator-verifiable, no native window)

P14a (extraction):
- `cargo build --workspace` and `cargo test --workspace` are **green** with all pre-existing
  `bonsai-core` tests (the 18 moved integration suites + inline unit tests) passing unchanged.
- `cargo clippy --workspace` clean (run sequentially vs test — MEMORY rule).
- `pnpm build` (frontend) unaffected; `cargo build -p bonsai` still produces the Tauri lib.
- Browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`) still renders (no IPC contract changed — P14
  touches no `#[tauri::command]` signatures, only their import paths). Confirms the mock layer is
  untouched and the frontend is unaffected.

P14b/c (server):
- `cargo build -p bonsai-mcp` builds the stdio binary on the pinned Windows toolchain.
- A **scripted stdio MCP session** (a test harness that spawns `bonsai-mcp --repo <scratch>
  --allow-write`, writes framed `initialize`, `tools/list`, `tools/call` JSON-RPC to stdin, reads
  stdout) against a scratch repo built with `bonsai_core::fixture`/`testutil` (temp under
  `D:\Temp`), asserting:
  1. `tools/list` returns exactly the read set when started **without** `--allow-write`, and read
     ∪ write set **with** it.
  2. `bonsai_get_graph` returns a `GraphLayout` whose `nodes.len()`/`headIndex` match a direct
     `compute_graph` call on the same repo.
  3. **Conflict round-trip (the headline gate):** seed a scratch repo with a known two-branch
     text conflict; `bonsai_merge_branch` → `MergeOutcome::Conflicts`; `bonsai_get_conflict`
     returns `ours`/`theirs` matching the two branch versions and the correct `ConflictKind`
     (`bothModified`); `bonsai_resolve_conflict_text` with a hand-authored merge, then
     `bonsai_commit_merge`; assert the resulting tree oid **equals** the tree produced by resolving
     the same conflict via the `git` CLI (`git merge` + hand-write same content + `git commit`) —
     i.e. a CLI-oracle equality check, consistent with the existing `conflict_cli.rs`/`merge_cli.rs`
     style.
  4. An `AppError` case (e.g. `bonsai_commit` with empty message) returns a `CallToolResult` with
     `is_error = true` and structured `{ "kind": "emptyMessage", ... }`.
  5. A mutation tool called without `--allow-write` yields a protocol error / not-listed (per §7.3
     strategy chosen).

### 8.2 USER CHECKPOINT (human / real Claude Code)

- User runs `claude mcp add bonsai -- <abs path>\bonsai-mcp.exe --repo <their repo> --allow-write`
  (or the read-only form), starts a Claude Code session, and confirms:
  1. The `bonsai_*` tools appear and are callable.
  2. `bonsai_get_graph` / `bonsai_get_status` return sane data for a real repo.
  3. A real conflict is resolvable end-to-end via `bonsai_get_conflict` +
     `bonsai_resolve_conflict_text` inside the session.
- Orchestrator MUST present the AI-gate evidence and explicitly ask the user to run this — never
  self-declare it passed.

---

## 9. Sub-increments (each = one fresh-context senior-dev pass)

**P14a — Workspace + `bonsai-core` extraction.** Create root workspace `Cargo.toml`; create
`crates/bonsai-core` (Cargo.toml + lib.rs); move `error.rs`, `graph.rs`, `git/`, `ai/`,
`fixture.rs`, `testutil.rs`, `benches/`, `tests/` per §1–§2; widen `testutil` visibility; re-point
all `bonsai_lib::`→`bonsai_core::` in moved tests and `crate::`→`bonsai_core::` in `src-tauri`
(`lib.rs`, `commands.rs`, `state.rs` if needed); edit `src-tauri/Cargo.toml` deps; fix root
`.gitignore` for `target/`. **Gate:** §8.1-P14a fully green; `pnpm tauri dev`/`build` still run
(USER-adjacent smoke — orchestrator runs the build, user confirms the window if needed). No new
functionality.

**P14b — `bonsai-mcp` skeleton + read-only tools.** New crate; `ServerConfig`/`--repo` parsing +
startup validation (§4); `BonsaiServer` + `run_blocking` (§5); rmcp stdio wiring; the §7.1 read
tools; `ok_json`/`err_result` (§6). **Gate:** builds; scripted session `tools/list` + `get_graph`
+ `get_status` assertions (§8.1 items 1–2).

**P14c — Mutation tools + `--allow-write` gate + conflict flow.** The §7.3 write tools with router
composition/gating; the full conflict/merge/rebase lifecycle; input param structs (§7.4).
**Gate:** §8.1 items 3–5 (conflict round-trip CLI-oracle, error mapping, write gate).

**P14d — Tests + Claude Code wiring.** Formalize the scripted-stdio harness as a
`crates/bonsai-mcp/tests/` integration test (spawns the built binary; temp under `D:\Temp`); write
`crates/bonsai-mcp/README.md` with the exact `claude mcp add` command, `--repo`/`--allow-write`
semantics, and the tool catalog. **Gate:** `cargo test -p bonsai-mcp` green; then present for the
§8.2 USER CHECKPOINT.

---

## 10. Risks & open questions (for the orchestrator)

1. **rmcp version churn** (§3) — HIGH. Pin at implement time; macro API (`Parameters`,
   `tool_router`, `CallToolResult`, router merge) may differ from the snippets here. Senior-dev
   adapts and records the pinned version. Use `rmcp::schemars` re-export to avoid schemars skew.
2. **Tauri workspace build** (§1.5) — MEDIUM. Verify `pnpm tauri dev`/`build` after target-dir
   relocation and `.gitignore` update. No `tauri.conf.json` change expected; if the CLI can't find
   the `bonsai` package in the workspace, investigate before forcing config changes.
3. **`ai`/`ai_resolve` placement** (§2.3) — decision flagged. Recommend Option A (move to core, not
   exposed); confirm if Option B is required.
4. **git2 `!Send`** (§5) — LOW, handled: handles never cross `.await`; every core fn opens its own
   repo inside the blocking closure.
5. **Read-only default & router gating** (§7.3) — confirm the `--allow-write` default-off policy
   and the not-listed-vs-inert strategy for disabled mutations.
6. **`fetch`/`pull`/`push` exclusion** (§7.2) — confirm out-of-v1; note FF-only `pull` is a
   reasonable safe add later.
7. **Scratch/temp discipline** — all P14b–d tests spawn processes and build repos under
   `D:\Temp\bonsai-scratch` (MEMORY: C: is full); the harness must set `TMP`/`TEMP` to `D:\Temp`
   for the spawned `bonsai-mcp` process too.
8. **Windows `--repo` path** — canonicalize via `read_repo_info` (already returns a canonical
   path); accept both `\` and `/`; quote in `claude mcp add` docs.

---

## 11. Appendix — exact `bonsai_core` call signatures the server wraps

(Verbatim from the current code; all return `Result<T, bonsai_core::error::AppError>`.)

```
graph::compute_graph(&Path) -> GraphLayout
status::read_status(&Path) -> StatusSnapshot
repo::read_repo_info(&Path) -> RepoInfo               // startup validation only
branches::list_refs(&Path) -> BranchesSnapshot
branches::create_branch(&Path, &str)
branches::create_branch_here(&Path, &str, &str) -> CreateBranchHereResult
branches::checkout_branch(&Path, &str)
branches::delete_branch(&Path, &str)
diff::commit_diff(&Path, &str) -> CommitDiff
diff::commit_file_diff(&Path, &str, &str, Option<&str>) -> FileDiff
diff::workdir_file_diff(&Path, &str, Option<&str>, bool) -> FileDiff
diff::compare_head_diff(&Path, &str) -> CompareDiff
diff::compare_head_file_diff(&Path, &str, &str, Option<&str>) -> FileDiff
conflict::list_conflicts(&Path) -> Vec<ConflictEntry>
conflict::get_conflict(&Path, &str) -> ConflictFile
conflict::resolve_conflict(&Path, &str, ConflictResolution)
conflict::resolve_conflict_text(&Path, &str, &str)
merge::merge_branch(&Path, &str) -> MergeOutcome
merge::commit_merge(&Path, &str) -> CommitResult
merge::abort_merge(&Path)
rebase::rebase_branch(&Path, &str) -> RebaseOutcome
rebase::rebase_continue(&Path) -> RebaseOutcome
rebase::rebase_skip(&Path) -> RebaseOutcome
rebase::rebase_abort(&Path)
opstate::read_op_state(&Path) -> RepoOpState
stage::stage_paths(&Path, &[String])
stage::unstage_paths(&Path, &[String])
commit::create_commit(&Path, &str) -> CommitResult
stash::list_stashes(&Path) -> Vec<StashEntry>
stash::create_stash(&Path, Option<&str>, bool) -> CreateStashResult
stash::apply_stash(&Path, usize) -> ApplyStashOutcome
stash::pop_stash(&Path, usize) -> ApplyStashOutcome
stash::drop_stash(&Path, usize)
```
