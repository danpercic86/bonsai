//! Shared test harness for the `bonsai-mcp` stdio integration suites
//! (`mcp_stdio_2.rs` adversarial frames, `mcp_stdio_3.rs` write-gate matrix +
//! drift). Kept out of the P14d `mcp_stdio.rs` file (which is self-contained) so
//! those headline tests stay untouched.
//!
//! Provides: platform scratch dirs (Windows → `D:\Data\Temp\bonsai-scratch`, never
//! C:), a `git` CLI fixture builder, the canonical read/write tool catalogs, and
//! a newline-delimited JSON-RPC-over-stdio client that can ALSO emit raw /
//! malformed bytes and observe whether the server survives.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The 14 read tools registered unconditionally (contract §7.1 + P16 §4b).
pub const READ_TOOLS: &[&str] = &[
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

/// The 20 mutation tools registered only under `--allow-write` (contract §7.3).
pub const WRITE_TOOLS: &[&str] = &[
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
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

// ----------------------------------------------------------------- git fixtures

/// Whether the `git` CLI is on PATH (honours `BONSAI_REQUIRE_GIT_STRICT=1`).
pub fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

/// `true` (with a printed note) when the suite should skip because `git` is
/// absent. Use as `if common::skip_if_no_git() { return; }`.
pub fn skip_if_no_git() -> bool {
    if have_git() {
        false
    } else {
        eprintln!("skipping: `git` CLI not found on PATH");
        true
    }
}

#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    Path::new("D:\\Data\\Temp\\bonsai-scratch").to_path_buf()
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// Scratch dir under the platform scratch root (MEMORY: on Windows never C:,
/// never the system temp).
pub fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-mcp-adv-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Runs `git <args>` in `dir`, asserting success; returns trimmed stdout.
pub fn git(dir: &Path, args: &[&str]) -> String {
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

/// `git init -b main` + deterministic local identity in a fresh scratch.
pub fn init_repo() -> tempfile::TempDir {
    let dir = scratch_dir();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.name", "Test User"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "core.autocrlf", "false"]);
    dir
}

pub fn write_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// A linear repo with `n` commits on `main`.
pub fn build_linear(dir: &Path, n: usize) {
    for i in 0..n {
        write_file(dir, "f.txt", &format!("line {i}\n"));
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", &format!("commit {i}")]);
    }
}

/// Working-tree porcelain (short status) — the side-effect witness. Empty on a
/// clean tree.
pub fn porcelain(dir: &Path) -> String {
    git(dir, &["status", "--porcelain"])
}

/// Every ref oid the repo knows (branches/tags/HEAD), as a stable sorted blob —
/// a stronger "nothing moved" witness than porcelain alone.
pub fn ref_snapshot(dir: &Path) -> String {
    let mut lines: Vec<String> = git(dir, &["show-ref"])
        .lines()
        .map(str::to_string)
        .collect();
    lines.sort();
    lines.join("\n")
}

// ------------------------------------------------------- tiny JSON-RPC client

/// A newline-delimited JSON-RPC-over-stdio client wrapping a spawned server.
/// Beyond the well-formed `request`/`call_tool` helpers it can emit RAW bytes
/// (`write_raw`/`write_line`) and observe survival, and drive the process to a
/// clean EOF exit. The child is killed on drop.
pub struct McpClient {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<String>,
    next_id: i64,
}

impl McpClient {
    /// Spawn `bonsai-mcp --repo <repo> [--allow-write]` WITHOUT performing the
    /// handshake (for raw / EOF probes that must control the very first bytes).
    pub fn spawn(repo: &Path, allow_write: bool) -> Self {
        let exe = env!("CARGO_BIN_EXE_bonsai-mcp");
        let mut cmd = Command::new(exe);
        cmd.arg("--repo").arg(repo);
        if allow_write {
            cmd.arg("--allow-write");
        }
        #[cfg(windows)]
        cmd.env("TMP", "D:\\Data\\Temp").env("TEMP", "D:\\Data\\Temp");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn bonsai-mcp: {e}"));
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        McpClient {
            child,
            stdin: Some(stdin),
            rx,
            next_id: 1,
        }
    }

    /// Spawn + complete the MCP handshake (`initialize` +
    /// `notifications/initialized`); the returned client is ready for tool calls.
    pub fn connect(repo: &Path, allow_write: bool) -> Self {
        let mut c = Self::spawn(repo, allow_write);
        c.initialize();
        c
    }

    fn initialize(&mut self) {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "bonsai-mcp-adv-test", "version": "0.0.0" }
        });
        let resp = self.request("initialize", params);
        assert!(resp.get("result").is_some(), "initialize failed: {resp}");
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    fn stdin_mut(&mut self) -> &mut ChildStdin {
        self.stdin.as_mut().expect("stdin still open")
    }

    fn write_message(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).expect("serialize request");
        let s = self.stdin_mut();
        s.write_all(line.as_bytes())
            .and_then(|_| s.write_all(b"\n"))
            .and_then(|_| s.flush())
            .expect("write to server stdin");
    }

    /// Write a raw line (the harness appends the `\n` frame terminator).
    pub fn write_line(&mut self, line: &str) {
        let s = self.stdin_mut();
        s.write_all(line.as_bytes())
            .and_then(|_| s.write_all(b"\n"))
            .and_then(|_| s.flush())
            .expect("write raw line");
    }

    /// Write raw bytes verbatim (NO framing added) — for CRLF frames, partial
    /// lines, or deliberately malformed byte sequences.
    pub fn write_raw(&mut self, bytes: &[u8]) {
        let s = self.stdin_mut();
        s.write_all(bytes).and_then(|_| s.flush()).expect("write raw bytes");
    }

    /// Drop the child's stdin, signalling EOF to the server.
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Read the next JSON message off stdout, or panic on timeout / EOF.
    fn next_message(&self) -> Value {
        let line = self
            .rx
            .recv_timeout(READ_TIMEOUT)
            .unwrap_or_else(|e| panic!("timed out / disconnected reading server stdout: {e}"));
        serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("server emitted non-JSON line ({e}): {line}"))
    }

    /// Send a request; return the response whose `id` matches (skipping
    /// notifications AND any error responses to previously-sent garbage frames).
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }));
        self.await_id(id)
    }

    /// Read until a response with `id` arrives (skipping everything else).
    pub fn await_id(&self, id: i64) -> Value {
        loop {
            let msg = self.next_message();
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
        }
    }

    /// Allocate the next request id (for callers that frame the request by hand
    /// via [`write_line`](Self::write_line) / [`write_raw`](Self::write_raw)).
    pub fn take_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// `tools/list` → the sorted set of advertised tool names.
    pub fn list_tool_names(&mut self) -> Vec<String> {
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
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": arguments }))
    }

    /// Prove the server is still alive: a fresh `tools/list` must return a
    /// non-empty tool set.
    pub fn assert_alive(&mut self) {
        let names = self.list_tool_names();
        assert!(
            !names.is_empty(),
            "server must still answer tools/list after adversarial input"
        );
    }

    /// Wait up to `dur` for the process to exit; returns whether it did.
    pub fn wait_for_exit(&mut self, dur: Duration) -> bool {
        let deadline = Instant::now() + dur;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(_) => return false,
            }
        }
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
pub fn ok_structured(resp: &Value) -> Value {
    assert!(resp.get("error").is_none(), "unexpected JSON-RPC error: {resp}");
    let result = resp
        .get("result")
        .unwrap_or_else(|| panic!("call had no result: {resp}"));
    let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);
    assert!(!is_error, "tool reported isError=true: {result}");
    result
        .get("structuredContent")
        .cloned()
        .unwrap_or_else(|| panic!("no structuredContent: {result}"))
}

/// Extract the `{ result }` of a domain-error call (`isError == true`), returning
/// its structured `{ kind, message }` content.
pub fn err_structured(resp: &Value) -> Value {
    assert!(
        resp.get("error").is_none(),
        "expected a tool-domain error (isError), got a JSON-RPC error: {resp}"
    );
    let result = resp
        .get("result")
        .unwrap_or_else(|| panic!("call had no result: {resp}"));
    let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);
    assert!(is_error, "expected isError=true, got: {result}");
    result
        .get("structuredContent")
        .cloned()
        .unwrap_or_else(|| panic!("error had no structuredContent: {result}"))
}

/// A response is a well-formed JSON-RPC reply (has a `result` XOR an `error`),
/// regardless of which — the "server answered rather than died" witness.
pub fn is_well_formed_reply(resp: &Value) -> bool {
    resp.get("result").is_some() ^ resp.get("error").is_some()
}
