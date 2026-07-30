//! P14d — scripted-stdio integration tests for the `bonsai-mcp` MCP server
//! (contract §8.1 P14b/c items 1–5).
//!
//! Each test spawns the *built* server binary (`env!("CARGO_BIN_EXE_bonsai-mcp")`)
//! and drives it over stdio with a tiny in-test newline-delimited JSON-RPC
//! client: it writes framed `initialize` + `notifications/initialized`, then
//! `tools/list` / `tools/call`, and matches responses by `id`. stdout reads use
//! a timeout so a hung server fails fast instead of blocking forever; the child
//! is killed on drop so no server process leaks.
//!
//! HEADLINE (case 3) is a CLI-oracle equality check: a two-branch `bothModified`
//! conflict resolved through the MCP tools (`merge_branch` → `get_conflict` →
//! `resolve_conflict_text` → `commit_merge`) must produce the SAME git tree oid
//! as the identical conflict resolved by hand via the `git` CLI. Tree equality
//! (not commit equality) is the invariant — commit oids differ by author/time.
//!
//! Scratch discipline (MEMORY: C: is critically full): every scratch repo lives
//! under `D:\Temp\bonsai-scratch`, and the spawned server's `TMP`/`TEMP` point at
//! `D:\Temp`. Tests skip (pass with a note) when `git` is not on PATH.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

/// The 12 read tools registered unconditionally (contract §7.1).
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
];

/// The 20 mutation tools registered only under `--allow-write` (contract §7.3).
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

/// Read timeout for any single line off the server's stdout.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

macro_rules! require_git {
    () => {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ----------------------------------------------------------------- git fixtures

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

/// Scratch dir under `D:\Temp\bonsai-scratch` (MEMORY rule — never C:, never the
/// system temp). Mirrors `bonsai-core`'s `tests/common::scratch_dir()`.
fn scratch_dir() -> tempfile::TempDir {
    let root = Path::new("D:\\Temp\\bonsai-scratch");
    std::fs::create_dir_all(root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-mcp-")
        .tempdir_in(root)
        .expect("scratch dir")
}

/// Runs `git <args>` in `dir`, asserting success; returns trimmed stdout.
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

/// Runs `git <args>` in `dir` and reports only whether it succeeded (used where a
/// conflicted `git merge` is EXPECTED to exit non-zero).
fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `git init -b main` + deterministic local identity/config in a fresh scratch.
fn init_repo() -> tempfile::TempDir {
    let dir = scratch_dir();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.name", "Test User"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "core.autocrlf", "false"]);
    dir
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// The `bothModified` two-branch conflict on `a.txt`. Leaves `main` checked out
/// with `feature` ready to merge (yields a conflict). Byte content is fixed so
/// twin repos resolve to identical trees.
const BASE_A: &str = "line1\nbase\nline3\n";
const OURS_A: &str = "line1\nmain\nline3\n"; // stage 2 (main = HEAD/OURS)
const THEIRS_A: &str = "line1\nfeature\nline3\n"; // stage 3 (feature = THEIRS)
const MERGED_A: &str = "line1\nmain\nfeature\nline3\n"; // hand-authored resolution

fn build_conflict_fixture(dir: &Path) {
    write(dir, "a.txt", BASE_A);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "base"]);
    git(dir, &["checkout", "-b", "feature"]);
    write(dir, "a.txt", THEIRS_A);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "feature change"]);
    git(dir, &["checkout", "main"]);
    write(dir, "a.txt", OURS_A);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "main change"]);
}

/// A small linear repo with three commits on `main` (for graph fidelity).
fn build_linear_fixture(dir: &Path) {
    for (i, body) in ["first\n", "first\nsecond\n", "first\nsecond\nthird\n"]
        .iter()
        .enumerate()
    {
        write(dir, "f.txt", body);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", &format!("commit {i}")]);
    }
}

// ------------------------------------------------------- tiny JSON-RPC client

/// A newline-delimited JSON-RPC-over-stdio client wrapping a spawned server. The
/// child is killed on drop.
struct McpClient {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    next_id: i64,
}

impl McpClient {
    /// Spawn `bonsai-mcp --repo <repo> [--allow-write]`, complete the MCP
    /// handshake (`initialize` + `notifications/initialized`), and return a
    /// ready client.
    fn connect(repo: &Path, allow_write: bool) -> Self {
        let exe = env!("CARGO_BIN_EXE_bonsai-mcp");
        let mut cmd = Command::new(exe);
        cmd.arg("--repo").arg(repo);
        if allow_write {
            cmd.arg("--allow-write");
        }
        // MEMORY: keep the spawned process's temp on D: as well.
        cmd.env("TMP", "D:\\Temp").env("TEMP", "D:\\Temp");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn bonsai-mcp: {e}"));
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");

        // Reader thread: newline-framed lines onto a channel so reads can time out.
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break; // client dropped
                        }
                    }
                    Err(_) => break, // stdout closed (server exited)
                }
            }
        });

        let mut client = McpClient {
            child,
            stdin,
            rx,
            next_id: 1,
        };
        client.initialize();
        client
    }

    fn initialize(&mut self) {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "bonsai-mcp-test", "version": "0.0.0" }
        });
        let resp = self.request("initialize", params);
        assert!(
            resp.get("result").is_some(),
            "initialize failed: {resp}"
        );
        // Notification: no id, no response expected.
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    fn write_message(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).expect("serialize request");
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .expect("write to server stdin");
    }

    /// Read the next JSON message off stdout, or panic on timeout / EOF.
    fn next_message(&self) -> Value {
        let line = self.rx.recv_timeout(READ_TIMEOUT).unwrap_or_else(|e| {
            panic!("timed out / disconnected reading server stdout: {e}")
        });
        serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("server emitted non-JSON line ({e}): {line}"))
    }

    /// Send a request and return the response whose `id` matches (skipping any
    /// interleaved notifications).
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }));
        loop {
            let msg = self.next_message();
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
            // Otherwise a server-initiated notification — ignore and keep reading.
        }
    }

    /// `tools/list` → the sorted set of advertised tool names.
    fn list_tool_names(&mut self) -> Vec<String> {
        let resp = self.request("tools/list", json!({}));
        let tools = resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("tools/list has no result.tools array: {resp}"));
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

    /// `tools/call` → the full JSON-RPC response message.
    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// --------------------------------------------------- tool-call result helpers

/// Assert a `tools/call` succeeded (no JSON-RPC error, `isError` not true) and
/// return its `structuredContent`.
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

/// Extract the `{ result }` object of a domain-error call (`isError == true`),
/// returning its structured `{ kind, message }` content.
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

// ============================================================ case 1: gating

#[test]
fn tools_list_gating_read_only_vs_allow_write() {
    require_git!();
    let repo = init_repo();
    build_linear_fixture(repo.path());

    // Without --allow-write: exactly the 12 read tools, no mutation tools.
    let mut ro = McpClient::connect(repo.path(), false);
    let ro_names = ro.list_tool_names();
    assert_eq!(
        ro_names.len(),
        READ_TOOLS.len(),
        "read-only server must advertise exactly {} tools, got {}: {ro_names:?}",
        READ_TOOLS.len(),
        ro_names.len()
    );
    for t in READ_TOOLS {
        assert!(ro_names.contains(&t.to_string()), "missing read tool {t}");
    }
    for t in WRITE_TOOLS {
        assert!(
            !ro_names.contains(&t.to_string()),
            "read-only server must NOT advertise mutation tool {t}"
        );
    }
    drop(ro);

    // With --allow-write: 12 read + 20 mutation = 32 tools.
    let mut rw = McpClient::connect(repo.path(), true);
    let rw_names = rw.list_tool_names();
    assert_eq!(
        rw_names.len(),
        READ_TOOLS.len() + WRITE_TOOLS.len(),
        "allow-write server must advertise 32 tools, got {}: {rw_names:?}",
        rw_names.len()
    );
    for t in READ_TOOLS.iter().chain(WRITE_TOOLS.iter()) {
        assert!(rw_names.contains(&t.to_string()), "missing tool {t}");
    }
}

// ============================================================ case 2: graph fidelity

#[test]
fn get_graph_matches_in_process_compute_graph() {
    require_git!();
    let repo = init_repo();
    build_linear_fixture(repo.path());

    // Oracle: call the core layout engine in-process on the same repo.
    let oracle = bonsai_core::graph::compute_graph(repo.path()).expect("compute_graph");

    let mut client = McpClient::connect(repo.path(), false);
    let resp = client.call_tool("bonsai_get_graph", json!({}));
    let layout = ok_structured(&resp);

    let nodes = layout
        .get("nodes")
        .and_then(Value::as_array)
        .expect("layout.nodes array");
    assert_eq!(
        nodes.len(),
        oracle.nodes.len(),
        "MCP get_graph nodes.len() must equal compute_graph's"
    );

    // headIndex: Option<u32>. Absent/null on the MCP side means None.
    let head_index = layout.get("headIndex").and_then(Value::as_u64).map(|v| v as u32);
    assert_eq!(
        head_index, oracle.head_index,
        "MCP get_graph headIndex must equal compute_graph's"
    );
}

// ================================================ case 3: HEADLINE conflict oracle

#[test]
fn conflict_round_trip_tree_oid_matches_cli_oracle() {
    require_git!();

    // ---- MCP side: drive the whole conflict lifecycle through the server. ----
    let mcp_repo = init_repo();
    build_conflict_fixture(mcp_repo.path());

    let mut client = McpClient::connect(mcp_repo.path(), true);

    // merge_branch(feature) -> Conflicts { paths: ["a.txt"], .. }
    let merge = ok_structured(&client.call_tool("bonsai_merge_branch", json!({ "name": "feature" })));
    assert_eq!(
        merge.get("kind").and_then(Value::as_str),
        Some("conflicts"),
        "expected a conflicts MergeOutcome, got: {merge}"
    );
    let paths: Vec<String> = merge
        .get("paths")
        .and_then(Value::as_array)
        .expect("conflicts.paths")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        paths.contains(&"a.txt".to_string()),
        "conflict paths must include a.txt, got: {paths:?}"
    );

    // get_conflict(a.txt) -> kind bothModified; ours/theirs = the branch versions.
    let conflict = ok_structured(&client.call_tool("bonsai_get_conflict", json!({ "path": "a.txt" })));
    assert_eq!(
        conflict.get("kind").and_then(Value::as_str),
        Some("bothModified"),
        "conflict kind must be bothModified, got: {conflict}"
    );
    assert_eq!(
        conflict.get("ours").and_then(Value::as_str),
        Some(OURS_A),
        "conflict.ours must equal main's version of a.txt"
    );
    assert_eq!(
        conflict.get("theirs").and_then(Value::as_str),
        Some(THEIRS_A),
        "conflict.theirs must equal feature's version of a.txt"
    );

    // resolve_conflict_text(a.txt, MERGED_A) -> success (null).
    let resolve = client.call_tool(
        "bonsai_resolve_conflict_text",
        json!({ "path": "a.txt", "content": MERGED_A }),
    );
    ok_structured(&resolve); // asserts no error / isError

    // commit_merge("resolve") -> CommitResult { oid, .. }
    let commit = ok_structured(&client.call_tool("bonsai_commit_merge", json!({ "message": "resolve" })));
    assert!(
        commit.get("oid").and_then(Value::as_str).is_some(),
        "commit_merge must return a CommitResult with an oid, got: {commit}"
    );
    drop(client);

    let mcp_tree = git(mcp_repo.path(), &["rev-parse", "HEAD^{tree}"]);

    // ---- Oracle side: identical history resolved by the git CLI. ----
    let cli_repo = init_repo();
    build_conflict_fixture(cli_repo.path());
    // Sanity: twin base histories are identical up to HEAD.
    assert_eq!(
        git(mcp_repo.path(), &["rev-parse", "HEAD~0"]).len(),
        40,
        "HEAD resolves"
    );
    // `git merge feature` is EXPECTED to fail with conflicts.
    assert!(
        !git_ok(cli_repo.path(), &["merge", "feature"]),
        "git merge feature should conflict"
    );
    write(cli_repo.path(), "a.txt", MERGED_A);
    git(cli_repo.path(), &["add", "a.txt"]);
    git(cli_repo.path(), &["commit", "-m", "resolve"]);
    let cli_tree = git(cli_repo.path(), &["rev-parse", "HEAD^{tree}"]);

    assert_eq!(
        mcp_tree, cli_tree,
        "MCP-resolved tree oid ({mcp_tree}) must equal the git-CLI tree oid ({cli_tree})"
    );
}

// ============================================================ case 4: error discriminant

#[test]
fn commit_empty_message_returns_structured_empty_message_error() {
    require_git!();
    let repo = init_repo();
    build_linear_fixture(repo.path());

    let mut client = McpClient::connect(repo.path(), true);
    let resp = client.call_tool("bonsai_commit", json!({ "message": "" }));
    let err = err_structured(&resp);
    assert_eq!(
        err.get("kind").and_then(Value::as_str),
        Some("emptyMessage"),
        "empty commit message must surface kind=emptyMessage, got: {err}"
    );
    assert!(
        err.get("message").and_then(Value::as_str).is_some(),
        "error must carry a human message, got: {err}"
    );
}

// ============================================================ case 5: write gate (protocol)

#[test]
fn mutation_tool_on_read_only_server_is_protocol_error_and_no_side_effect() {
    require_git!();
    let repo = init_repo();
    build_linear_fixture(repo.path());

    let mut client = McpClient::connect(repo.path(), false);
    let resp = client.call_tool("bonsai_create_branch", json!({ "name": "x" }));

    // An unregistered tool is a JSON-RPC error (rmcp: -32602 "tool not found"),
    // NOT a tool-domain result.
    let error = resp
        .get("error")
        .unwrap_or_else(|| panic!("expected a JSON-RPC error, got: {resp}"));
    assert_eq!(
        error.get("code").and_then(Value::as_i64),
        Some(-32602),
        "expected INVALID_PARAMS (-32602), got: {error}"
    );
    let emsg = error.get("message").and_then(Value::as_str).unwrap_or("");
    assert!(
        emsg.contains("tool not found"),
        "expected 'tool not found', got: {emsg}"
    );
    drop(client);

    // Side-effect check: the branch must NOT exist.
    let branches = git(repo.path(), &["branch", "--list", "x"]);
    assert!(
        branches.is_empty(),
        "read-only server must not have created branch 'x'; git branch --list x = {branches:?}"
    );
}
