//! Drives the locally-installed `claude` CLI (Claude Code) as a pure text
//! transform on the user's subscription session (no API key). Blocking;
//! all callers invoke under spawn_blocking. (P13)

/// Shared diff-payload renderer for the in-app AI features (P15).
pub mod payload;

// P68 §A: the streaming sibling of `run_claude` — pure line interpretation
// (`stream`), process lifecycle (`session`) and the cancel/reply map
// (`registry`), split so the NDJSON mapping is testable without a child (D12).
pub mod registry;
/// macOS/Linux `claude` CLI discovery ladder (spec 001): the process's own
/// `PATH`, then the user's login-shell `PATH`, then a short list of
/// well-known install directories. Windows keeps using
/// [`crate::procutil::resolve_program`] instead (see `resolve_bin` below) —
/// this module doesn't exist on that platform.
#[cfg(not(windows))]
mod bin_resolve;
/// Private: [`RunControl`] is the module's whole public surface (re-exported
/// below), and `run_claude_streaming` is the only way to drive a session.
mod session;
/// The LOCKED streaming argv (§3.4) + its pure assertions, split out of `session`
/// so the flag set is tested without a child process.
mod session_argv;
/// The reader/writer threads and the mpsc funnel they report on — no policy.
mod session_pipes;
pub mod stream;

/// Public data types (`ToolPolicy`, `RunLimits`, `RunOpts`, `AiResult`,
/// `AiAvailability`), split out of this file; re-exported so their paths
/// (`crate::ai::RunOpts`, etc.) are unchanged.
mod types;
pub use types::{AiAvailability, AiResult, RunLimits, RunOpts, ToolPolicy};

pub use registry::{AiRunHandle, AiRunRegistry};
pub use session::RunControl;
pub use stream::{
    classify_line, sentinel_question, AiRunEvent, AiRunEventKind, LineOutcome, StreamLogItem,
    MAX_EVENT_TEXT, SENTINEL,
};

#[cfg(test)]
mod testutil;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod session_io_tests;

use crate::error::AppError;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Default resolution model. `sonnet` = strong code-merge quality at ~1/5 the
/// cost/latency of `opus`; far better than `haiku` for conflict reasoning
/// (§9.2). Configurable per call via `RunOpts.model`. (P13)
pub const DEFAULT_MODEL: &str = "sonnet";
/// Wall-clock cap for one resolution call (§9.4). (P13)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);
/// Short cap for the `--version` availability probe. (P13)
pub const AVAILABILITY_TIMEOUT: Duration = Duration::from_secs(10);
/// Test/override hook: when set, this binary path is spawned instead of PATH
/// `claude` (points at the stub script in tests). (P13)
pub const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";

/// Default idle-output watchdog (P68 §A). A streaming run is killed only after
/// this long with NO output from the child; `Duration::ZERO` disables it. There
/// is deliberately NO wall-clock deadline for streaming runs — the user cancels
/// instead. `DEFAULT_TIMEOUT` still governs every `run_claude` caller (D6).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// How long the session loop blocks on `recv_timeout` before it polls the cancel
/// flag / the watchdog / the reply channel. Bounds cancel latency (P68 §A).
pub const RECV_TICK: Duration = Duration::from_millis(250);
/// Grace period between dropping stdin and force-killing the child on a COMPLETED
/// run (the child normally exits on stdin EOF). (P68 §A)
pub const EXIT_GRACE: Duration = Duration::from_secs(2);
/// Max `result` lines (turns) per streaming run before a still-questioning model
/// is failed (P68 §B rule 3).
pub const DEFAULT_MAX_TURNS: u32 = 6;

/// How many streaming AI runs may be live AT ONCE (P68 OQ1, confirmed = 3: one CLI
/// process per run, subscription rate limits, and >3 live logs is unreadable in one
/// dock).
///
/// Lives HERE, and is enforced by the command layer, because the cap belongs on the
/// far side of the trust boundary: streaming has **no wall-clock deadline by
/// design** (D3/D7) and **no spend cap by default** (`ai_max_budget_usd = 0.0`), so
/// the only reaper is the idle watchdog. A frontend-only cap means a `useAiRuns`
/// regression, a second window, a retried IPC call or a double-fired dock action
/// fans out unbounded `claude` process trees against a metered subscription. The
/// frontend MIRRORS this number (`src/settings/ranges.ts`) instead of owning it, so
/// the two cannot drift.
///
/// The guard REJECTS; it never queues. A queued run with no visible state is worse
/// than an error the user can act on.
pub const AI_MAX_CONCURRENT_RUNS: usize = 3;

/// The JSON envelope emitted by `claude --output-format json`. Lenient: fields
/// are already snake_case and unknown fields are ignored by serde default. (P13)
#[derive(serde::Deserialize)]
struct ClaudeEnvelope {
    result: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
}

/// Result of a drained, timeout-bounded child process. (P13)
struct ProcOutput {
    timed_out: bool,
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Spawn `cmd`, pipe `stdin_payload` in, drain stdout/stderr concurrently, and
/// wait up to `timeout` (std-only drain-and-poll — §3.2). A writer + two reader
/// threads run concurrently, so there is NO pipe-buffer deadlock for large
/// payloads: the child can write stdout freely while we feed stdin, and neither
/// side blocks the other. On the deadline we kill the child's whole process
/// TREE ([`kill_child_tree`] — a bare `kill()` would orphan the node process
/// behind a `.cmd` shim, audit §2.7) then `wait()` (reap) the child.
///
/// The threads are owned (not scoped) so that on timeout we can return WITHOUT
/// joining the readers: the tree kill is best-effort, and a surviving grandchild
/// (e.g. the stub's `ping`) can hold the inherited stdout pipe open, so
/// `read_to_end` could otherwise block well past the deadline. The detached
/// readers exit on their own once the OS finally closes those pipes. To keep the writer `'static`, the
/// payload is copied into an owned `String`.
///
/// Only spawn failure yields `Err` (an `io::Error`); everything else is reported
/// in `ProcOutput` so callers decide the mapping. (P13)
fn run_process(
    mut cmd: Command,
    timeout: Duration,
    stdin_payload: Option<&str>,
) -> std::io::Result<ProcOutput> {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Owned copy so the writer thread needs no borrow of `stdin_payload`.
    let payload_owned = stdin_payload.map(|s| s.to_string());
    let writer = thread::spawn(move || {
        if let Some(mut si) = stdin {
            if let Some(p) = payload_owned {
                let _ = si.write_all(p.as_bytes());
            }
            // `si` dropped here -> EOF on the child's stdin. A child that exits
            // early closes the read end -> BrokenPipe, which we ignore.
        }
    });
    let out_h = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut so) = stdout {
            let _ = so.read_to_end(&mut buf);
        }
        buf
    });
    let err_h = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut se) = stderr {
            let _ = se.read_to_end(&mut buf);
        }
        buf
    });

    let mut timed_out = false;
    let mut success = false;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                success = status.success();
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_child_tree(&mut child);
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }

    if timed_out {
        // Detach the reader/writer threads (they drop at return) rather than
        // joining — see the doc comment. Content is irrelevant on the timeout
        // path (the caller maps it to `AiFailed("timed out …")`).
        return Ok(ProcOutput {
            timed_out: true,
            success: false,
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }

    // Normal exit: the child closed its pipe ends, so the readers reach EOF.
    let stdout_buf = out_h.join().unwrap_or_default();
    let stderr_buf = err_h.join().unwrap_or_default();
    let _ = writer.join();
    Ok(ProcOutput { timed_out: false, success, stdout: stdout_buf, stderr: stderr_buf })
}

/// Resolve the binary to spawn: `CLAUDE_BIN_ENV` override (tests) wins,
/// verbatim (AC6, spec 001). Otherwise the ladder is platform-specific:
///
/// - **Windows**: resolve `claude` against `PATH` with PATHEXT awareness via
///   [`crate::procutil::resolve_program`] — a bare `Command::new("claude")`
///   does NOT find the npm `claude.cmd` shim (CreateProcess only appends
///   `.exe`), so an npm-only install would fail every AI feature (audit
///   §2.7). Unchanged by spec 001 (AC5).
/// - **macOS/Linux**: [`bin_resolve::resolve`] — the process's own inherited
///   `PATH` first (AC2: identical outcome to today when discovery already
///   works, e.g. a terminal launch), then the user's **login shell**'s
///   `PATH` (spec 001's actual fix: a GUI launch — double-click, Spotlight,
///   Dock — inherits a minimal launchd/display-manager `PATH` that omits
///   anything only added by `.zshrc`/`.zprofile`/`.bashrc`), then a short
///   list of well-known install directories. The login-shell probe is cached
///   for the process's lifetime, so it runs at most once (AC4), and is
///   bounded by a short timeout so a hung/broken shell can't stall an AI
///   feature (spec 001 edge case).
///
/// Either ladder falls back to the bare `claude` name, unresolved, so the
/// spawn's `NotFound` → `AiUnavailable` error path still fires naturally when
/// nothing is found anywhere (AC3). (P13; spec 001)
fn resolve_bin() -> std::path::PathBuf {
    if let Ok(overridden) = std::env::var(CLAUDE_BIN_ENV) {
        return std::path::PathBuf::from(overridden);
    }
    #[cfg(windows)]
    {
        crate::procutil::resolve_program("claude")
            .unwrap_or_else(|_| std::path::PathBuf::from("claude"))
    }
    #[cfg(not(windows))]
    {
        bin_resolve::resolve("claude")
    }
}

/// Kill `child` AND its descendants (audit §2.7). On Windows the resolved
/// binary is usually the npm `claude.cmd` shim: `child.kill()` terminates only
/// the cmd.exe wrapper and orphans the node process behind it, which keeps
/// running (and holding the inherited pipes) past the deadline — so kill the
/// whole tree via `taskkill /T /F` (best-effort, hidden console; mirrors
/// `external.rs`' console suppression), with `child.kill()` as the backstop.
/// Non-Windows children are spawned directly (no shim), so a plain `kill()`
/// suffices.
fn kill_child_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
}

/// Strip a single leading/trailing ``` fence (optionally ```lang) defensively.
/// If the trimmed text opens with a fence line and closes with a fence line, the
/// two fence lines are removed and the inner lines returned; otherwise the text
/// is returned unchanged (§3.3). (P13)
///
/// `pub(crate)` since P68 §6.2: each per-file block of a BULK reply gets the same
/// defensive de-fencing that [`parse_result_envelope`] applies to a whole reply,
/// from this one implementation.
pub(crate) fn strip_fence(text: &str) -> String {
    let trimmed = text.trim();
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() >= 2 {
        let first = lines[0].trim_end();
        let last = lines[lines.len() - 1].trim();
        let first_is_open = first.starts_with("```");
        let last_is_close = last == "```";
        if first_is_open && last_is_close {
            return lines[1..lines.len() - 1].join("\n");
        }
    }
    text.to_string()
}

/// Parse a version string from `--version` stdout: the first whitespace-split
/// token of the trimmed output, or `None` if empty. (P13)
fn parse_version(out: &str) -> Option<String> {
    let t = out.trim();
    if t.is_empty() {
        return None;
    }
    t.split_whitespace().next().map(|s| s.to_string())
}

/// Blocking. Spawns claude as a headless text transform, pipes `stdin_payload`
/// to its stdin, waits up to `opts.timeout`, parses the JSON envelope, returns
/// the model text. `cwd` is the child's working dir (the repo workdir).
///
/// Argv (LOCKED, verified on CLI v2.1.220):
///   claude -p <prompt> --output-format json --safe-mode --tools ""
///          --no-session-persistence --model <model>
///          [--append-system-prompt <system_prompt>]
///
/// See the contract §2/§3 for the full rationale (`--safe-mode` not `--bare`,
/// `--tools ""` to forbid all tools). (P13)
pub fn run_claude(
    cwd: &Path,
    prompt: &str,
    stdin_payload: Option<&str>,
    opts: RunOpts,
) -> Result<AiResult, AppError> {
    let bin = resolve_bin();
    let model = opts.model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string());

    // BatBadBut-class caveat: on Windows `bin` typically resolves to the npm
    // `claude.cmd` shim, and argv text reaching a `.cmd` is re-expanded by
    // cmd.exe (`%VAR%`, metacharacters). INVARIANT (keep truthful): every argv
    // element below is a Bonsai-controlled constant (prompts, flags), a vetted
    // model alias, or a single-line system prompt constant — ALL repo-derived
    // and user data flows exclusively through `stdin_payload`, never argv.
    let mut cmd = Command::new(&bin);
    cmd.current_dir(cwd)
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("json")
        .arg("--safe-mode")
        .arg("--tools")
        .arg("")
        .arg("--no-session-persistence")
        .arg("--model")
        .arg(&model);
    if let Some(sp) = &opts.system_prompt {
        cmd.arg("--append-system-prompt").arg(sp);
    }
    // `opts.json_schema` is reserved for a future `--json-schema` opt-in (§9.1);
    // intentionally unused in v1.

    let output = match run_process(cmd, opts.timeout, stdin_payload) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::AiUnavailable(format!(
                "Claude Code CLI not found: {e}"
            )));
        }
        Err(e) => return Err(AppError::AiUnavailable(e.to_string())),
    };

    if output.timed_out {
        return Err(AppError::AiFailed(format!(
            "Claude timed out after {}s",
            opts.timeout.as_secs()
        )));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    parse_result_envelope(stdout_str.trim(), output.success, &stderr_str)
}

/// Interpret ONE Claude JSON result envelope. EXTRACTED VERBATIM from
/// `run_claude` (P68 §A) so the streaming `result` line and the one-shot
/// `--output-format json` output — byte-compatible envelopes (spike §1.3) — share
/// one copy of this logic. Branches, unchanged:
/// (1) unparseable + non-zero exit -> stderr tail capped at 500 chars;
/// (2) unparseable + zero exit -> "could not parse Claude output";
/// (3) `is_error` -> result|subtype; (4) empty/blank result -> "no output";
/// (5) success -> `strip_fence`.
pub(crate) fn parse_result_envelope(
    stdout: &str,
    success: bool,
    stderr: &str,
) -> Result<AiResult, AppError> {
    let envelope: Result<ClaudeEnvelope, _> = serde_json::from_str(stdout);
    let env = match envelope {
        Ok(env) => env,
        Err(pe) => {
            // 1. Non-zero exit AND unparseable stdout -> surface the stderr tail.
            if !success {
                let trimmed = stderr.trim();
                let msg = if trimmed.is_empty() {
                    "Claude exited with a non-zero status".to_string()
                } else {
                    trimmed.chars().take(500).collect::<String>()
                };
                return Err(AppError::AiFailed(msg));
            }
            // 2. Parse failure on a zero exit.
            return Err(AppError::AiFailed(format!(
                "could not parse Claude output: {pe}"
            )));
        }
    };

    // 3. Explicit error envelope.
    if env.is_error {
        let msg = env
            .result
            .or(env.subtype)
            .unwrap_or_else(|| "Claude reported an error".to_string());
        return Err(AppError::AiFailed(msg));
    }

    // 4. Empty / absent result.
    let result_text = match env.result {
        Some(r) if !r.trim().is_empty() => r,
        _ => return Err(AppError::AiFailed("Claude returned no output".to_string())),
    };

    // 5. Success.
    Ok(AiResult {
        text: strip_fence(&result_text),
        cost_usd: env.total_cost_usd,
        session_id: env.session_id,
    })
}

/// Blocking. The single streaming entry point (P68 §A); callers invoke it under
/// `spawn_blocking`. `opts.model` and `opts.system_prompt` are honoured;
/// **`opts.timeout` is IGNORED** (streaming is governed by `limits`) — documented
/// rather than removed so a caller can pass a `RunOpts` it already has.
///
/// Emits every event through `on_event` (seq starts at 0 with `Started`, carrying
/// the `runId` — D8) and returns the LAST turn's parsed result.
///
/// `prompt` is the positional prompt in one-shot mode and is prepended to the
/// stdin user message in interactive mode (D13 — never argv there).
///
/// Errors: `AiUnavailable` (spawn/NotFound) | `AiFailed` (protocol, watchdog,
/// hard cap, turn budget, unparseable/`is_error` result) | `AiCancelled`. On
/// EVERY error path the events already emitted stand (D2).
///
/// `ctl` is BORROWED (not consumed) since P68b: a bulk resolve is several
/// sequential child processes under ONE run id (§6.3), and they must share the
/// same cancel flag, the same `awaiting` flag and the same reply channel. The
/// registry mints exactly one [`RunControl`] per run, whatever the batch count.
pub fn run_claude_streaming(
    cwd: &Path,
    prompt: &str,
    payload: &str,
    opts: RunOpts,
    limits: RunLimits,
    ctl: &RunControl,
    on_event: &(dyn Fn(AiRunEvent) + Send + Sync),
) -> Result<AiResult, AppError> {
    session::run(cwd, prompt, payload, opts, limits, ctl, on_event)
}

/// Kill a process TREE by pid — the app-exit path (D7), where no `Child` handle
/// survives. Windows: `taskkill /T /F /PID` with a hidden console (the npm
/// `claude.cmd` shim orphans a node grandchild otherwise); elsewhere a
/// best-effort `kill -9`. Never panics, never blocks longer than the spawn; pid 0
/// means "never spawned" and is ignored.
pub(crate) fn kill_pid_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Wall-clock cap for the one-shot `claude mcp add` registration call (P16).
pub const REGISTER_TIMEOUT: Duration = Duration::from_secs(30);

/// Blocking. Registers Bonsai's embedded MCP server with the local `claude` CLI
/// by spawning `claude mcp add ...` as an ARGUMENT LIST (no shell — so the
/// variadic `--header` cannot swallow the URL as it does in a hand-typed line).
/// `cwd` is the child's working dir, which determines where a `local`-scoped
/// registration is written. Respects `CLAUDE_BIN_ENV`.
///
/// Argv (name + URL BEFORE the variadic `--header`, which is LAST):
///   claude mcp add --transport http --scope <scope> bonsai <url>
///          --header "Authorization: Bearer <token>"
///
/// Errors: spawn `NotFound` -> `AiUnavailable`; non-zero exit / timeout ->
/// `AiFailed(<stderr tail>)` (mirrors `run_claude`). (P16)
pub fn register_with_claude(
    url: &str,
    token: &str,
    scope: &str,
    cwd: &Path,
) -> Result<(), AppError> {
    let bin = resolve_bin();
    let mut cmd = Command::new(&bin);
    cmd.current_dir(cwd)
        .arg("mcp")
        .arg("add")
        .arg("--transport")
        .arg("http")
        .arg("--scope")
        .arg(scope)
        .arg("bonsai")
        .arg(url)
        .arg("--header")
        .arg(format!("Authorization: Bearer {token}"));

    let output = match run_process(cmd, REGISTER_TIMEOUT, None) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::AiUnavailable(format!(
                "Claude Code CLI not found: {e}"
            )));
        }
        Err(e) => return Err(AppError::AiUnavailable(e.to_string())),
    };

    if output.timed_out {
        return Err(AppError::AiFailed(format!(
            "claude mcp add timed out after {}s",
            REGISTER_TIMEOUT.as_secs()
        )));
    }

    if !output.success {
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        let trimmed = stderr_str.trim();
        let msg = if trimmed.is_empty() {
            "claude mcp add exited with a non-zero status".to_string()
        } else {
            trimmed.chars().take(500).collect::<String>()
        };
        return Err(AppError::AiFailed(msg));
    }

    Ok(())
}

/// Blocking, never errors. Spawns `<bin> --version` (`AVAILABILITY_TIMEOUT`);
/// returns a populated `AiAvailability`. Respects `CLAUDE_BIN_ENV`. (P13)
pub fn check_availability() -> AiAvailability {
    let bin = resolve_bin();
    let mut cmd = Command::new(&bin);
    cmd.arg("--version");

    let not_found = AiAvailability {
        installed: false,
        logged_in: false,
        version: None,
        detail: "Claude Code CLI not found on PATH".to_string(),
    };

    match run_process(cmd, AVAILABILITY_TIMEOUT, None) {
        Ok(o) if o.success && !o.timed_out => {
            let out = String::from_utf8_lossy(&o.stdout);
            let version = parse_version(&out);
            let detail = match &version {
                Some(v) => format!("Claude Code {v} ready"),
                None => "Claude Code ready".to_string(),
            };
            AiAvailability { installed: true, logged_in: true, version, detail }
        }
        _ => not_found,
    }
}

