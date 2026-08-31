//! Shared harness for the embedded-MCP HTTP integration tests: git fixtures,
//! a hand-rolled streamable-HTTP MCP client, and the `spawn`/`probe_status`
//! drivers over the runtime-free [`spawn_server`] core. Split out of the test
//! module for size.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HOST, ORIGIN};
use reqwest::StatusCode;
use serde_json::{json, Value};

use super::*;

// -------- read/write tool catalogs (mirror crates/bonsai-mcp/tests) --------

pub(super) const READ_TOOLS: &[&str] = &[
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

pub(super) const WRITE_TOOLS: &[&str] = &[
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

pub(super) const TEST_TOKEN: &str = "s3cr3t-bearer-token-for-p16d-tests";
pub(super) const MCP_ACCEPT: &str = "application/json, text/event-stream";
pub(super) const READ_TIMEOUT: Duration = Duration::from_secs(30);

// ------------------------------------------------------------ git fixtures

pub(super) fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

/// Scratch dir under `D:\Data\Temp\bonsai-scratch` on Windows (MEMORY rule —
/// never C:, never the system temp). On macOS/Linux there is no such
/// constraint, so scratch dirs fall back to
/// `std::env::temp_dir()/bonsai-scratch`.
#[cfg(windows)]
pub(super) fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from("D:\\Data\\Temp\\bonsai-scratch")
}

#[cfg(not(windows))]
pub(super) fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

pub(super) fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-http-mcp-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

pub(super) fn git(dir: &Path, args: &[&str]) -> String {
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
pub(super) fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(super) fn init_repo() -> tempfile::TempDir {
    let dir = scratch_dir();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.name", "Test User"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "core.autocrlf", "false"]);
    dir
}

pub(super) fn write_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// A linear repo with `n` commits on `main`.
pub(super) fn build_linear(dir: &Path, n: usize) {
    for i in 0..n {
        write_file(dir, "f.txt", &format!("line {i}\n"));
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", &format!("commit {i}")]);
    }
}

// The `bothModified` two-branch conflict on `a.txt` (identical byte content to
// crates/bonsai-mcp/tests/mcp_stdio.rs so twin repos resolve to the same tree).
pub(super) const BASE_A: &str = "line1\nbase\nline3\n";
pub(super) const OURS_A: &str = "line1\nmain\nline3\n";
pub(super) const THEIRS_A: &str = "line1\nfeature\nline3\n";
pub(super) const MERGED_A: &str = "line1\nmain\nfeature\nline3\n";

pub(super) fn build_conflict_fixture(dir: &Path) {
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
pub(super) fn open_repo(dir: &Path) -> OpenRepo {
    OpenRepo {
        repo_id: dir.to_string_lossy().into_owned(),
        path: dir.to_path_buf(),
    }
}

pub(super) fn graph_node_count(dir: &Path) -> usize {
    bonsai_core::graph::compute_graph(dir)
        .expect("compute_graph oracle")
        .nodes
        .len()
}

pub(super) fn arc_list(repos: Vec<OpenRepo>) -> Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync> {
    Arc::new(move || repos.clone())
}

pub(super) fn seed_some(id: String) -> Arc<dyn Fn() -> Option<String> + Send + Sync> {
    Arc::new(move || Some(id.clone()))
}

pub(super) fn seed_none() -> Arc<dyn Fn() -> Option<String> + Send + Sync> {
    Arc::new(|| None)
}

// ------------------------------------------------------ SSE / JSON-RPC glue

/// Extract every JSON `data:` payload from a `text/event-stream` body. Non-JSON
/// SSE lines (priming/retry events, keep-alive comments) are skipped.
pub(super) fn parse_sse(body: &str) -> Vec<Value> {
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

pub(super) fn no_proxy_client() -> reqwest::Client {
    // reqwest resolves to `rustls-no-provider` workspace-wide (tauri and
    // bonsai-forge both select it), so a crypto provider must be installed
    // before any Client is built. `ring` keeps the TLS stack pure-Rust; the
    // result is ignored because installation is process-global and only the
    // first caller wins.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("build reqwest client")
}

/// A hand-rolled streamable-HTTP MCP client over `reqwest`. Owns one session
/// (established in [`connect`]).
pub(super) struct HttpMcp {
    client: reqwest::Client,
    url: String,
    session: String,
    next_id: i64,
}

impl HttpMcp {
    /// `initialize` + capture `Mcp-Session-Id` + `notifications/initialized`.
    pub(super) async fn connect(port: u16, token: &str) -> Self {
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

    pub(super) async fn notify(&self, method: &str, params: Value) {
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
    pub(super) async fn rpc(&mut self, method: &str, params: Value) -> Value {
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

    pub(super) async fn list_tool_names(&mut self) -> Vec<String> {
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

    pub(super) async fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.rpc("tools/call", json!({ "name": name, "arguments": arguments }))
            .await
    }
}

/// Assert a `tools/call` succeeded (no JSON-RPC error, `isError` not true) →
/// its `structuredContent`.
pub(super) fn ok_structured(resp: &Value) -> Value {
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
pub(super) fn err_structured(resp: &Value) -> Value {
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
pub(super) async fn spawn(
    allow_write: bool,
    preferred_port: Option<u16>,
    list_open: Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync>,
    seed: Arc<dyn Fn() -> Option<String> + Send + Sync>,
) -> McpRunning {
    spawn_server(
        |_actual_port| TEST_TOKEN.to_string(),
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
pub(super) async fn probe_status(
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
