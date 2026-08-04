//! Drives the locally-installed `claude` CLI (Claude Code) as a pure text
//! transform on the user's subscription session (no API key). Blocking;
//! all callers invoke under spawn_blocking. (P13)

/// Shared diff-payload renderer for the in-app AI features (P15).
pub mod payload;

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

/// Knobs for one `run_claude` call. `Default` = subscription resolver defaults. (P13)
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// `--model <alias>`; `None` => `DEFAULT_MODEL`. Aliases: sonnet|haiku|opus.
    pub model: Option<String>,
    /// Killed and mapped to `AiFailed("timed out …")` past this deadline.
    pub timeout: Duration,
    /// Appended via `--append-system-prompt`. Sets role + output contract.
    pub system_prompt: Option<String>,
    /// Reserved: `--json-schema <schema>` for structured output. `None` in v1
    /// (§9.1 locks reading `result` prose instead). Wired but unused so a later
    /// feature can opt in without changing the signature.
    pub json_schema: Option<String>,
}

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts { model: None, timeout: DEFAULT_TIMEOUT, system_prompt: None, json_schema: None }
    }
}

/// A successful CLI text transform. `text` is the model's `result` field with a
/// single leading/trailing ``` fence stripped defensively (§3.3). (P13)
#[derive(Debug, Clone)]
pub struct AiResult {
    pub text: String,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
}

/// Cheap health status. NEVER errors — a missing/broken CLI yields
/// `{ installed:false, .. }`, not an `Err`. Wire type mirrored in TS (§7). (P13)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAvailability {
    /// `claude --version` spawned and exited 0.
    pub installed: bool,
    /// v1: reported EQUAL to `installed` (subscription auth is NOT verified in a
    /// cheap probe — a real auth check would cost a billable call). Actual
    /// logged-out state surfaces as `AiFailed` on the first resolve (§9 note).
    pub logged_in: bool,
    /// Parsed from `--version` stdout when installed, else `None`.
    pub version: Option<String>,
    /// Human one-liner for the settings UI ("Claude Code 2.1.220 ready" /
    /// "Claude Code CLI not found on PATH").
    pub detail: String,
}

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
/// side blocks the other. On the deadline we `kill()` then `wait()` (reap) the
/// child.
///
/// The threads are owned (not scoped) so that on timeout we can return WITHOUT
/// joining the readers: a killed `cmd.exe` shim can leave a grandchild (e.g. the
/// stub's `ping`) holding the inherited stdout pipe open, and `read_to_end` would
/// otherwise block well past the deadline. The detached readers exit on their own
/// once the OS finally closes those pipes. To keep the writer `'static`, the
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
                    let _ = child.kill();
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

/// Resolve the binary to spawn: `CLAUDE_BIN_ENV` override (tests) else `claude`
/// (PATH-resolved; picks up the Windows `claude.cmd` shim). (P13)
fn resolve_bin() -> String {
    std::env::var(CLAUDE_BIN_ENV).unwrap_or_else(|_| "claude".to_string())
}

/// Strip a single leading/trailing ``` fence (optionally ```lang) defensively.
/// If the trimmed text opens with a fence line and closes with a fence line, the
/// two fence lines are removed and the inner lines returned; otherwise the text
/// is returned unchanged (§3.3). (P13)
fn strip_fence(text: &str) -> String {
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

    let envelope: Result<ClaudeEnvelope, _> = serde_json::from_str(stdout_str.trim());
    let env = match envelope {
        Ok(env) => env,
        Err(pe) => {
            // 1. Non-zero exit AND unparseable stdout -> surface the stderr tail.
            if !output.success {
                let trimmed = stderr_str.trim();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";

    /// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
    /// process-global and the stub inherits them, so parallel tests would race.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn stub_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude_stub.cmd")
    }

    fn set_mode(mode: &str) {
        std::env::set_var(CLAUDE_BIN_ENV, stub_path());
        std::env::set_var(STUB_MODE_ENV, mode);
    }

    #[test]
    fn run_claude_success_strips_and_parses() {
        let _g = env_lock();
        set_mode("success");
        let res = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
            .expect("success stub should yield Ok");
        assert_eq!(res.text, "MERGED_BODY_OK");
        assert_eq!(res.cost_usd, Some(0.012));
        assert_eq!(res.session_id.as_deref(), Some("sess-abc"));
    }

    #[test]
    fn run_claude_strips_code_fence() {
        let _g = env_lock();
        set_mode("success_fence");
        let res = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
            .expect("fence stub should yield Ok");
        assert_eq!(res.text, "MERGED_FENCED");
    }

    #[test]
    fn run_claude_is_error_maps_to_ai_failed() {
        let _g = env_lock();
        set_mode("error");
        let err = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
            .expect_err("is_error envelope should map to Err");
        match err {
            AppError::AiFailed(m) => assert_eq!(m, "boom"),
            other => panic!("expected AiFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_claude_nonzero_exit_maps_to_ai_failed() {
        let _g = env_lock();
        set_mode("nonzero");
        let err = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
            .expect_err("non-zero exit should map to Err");
        match err {
            AppError::AiFailed(m) => assert!(
                m.contains("something broke"),
                "stderr should surface, got: {m}"
            ),
            other => panic!("expected AiFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_claude_slow_times_out_and_reaps_child() {
        let _g = env_lock();
        set_mode("slow");
        let opts = RunOpts { timeout: Duration::from_secs(1), ..RunOpts::default() };
        let start = Instant::now();
        let err = run_claude(Path::new("."), "prompt", Some("payload"), opts)
            .expect_err("slow stub past the timeout should map to Err");
        let elapsed = start.elapsed();
        match err {
            AppError::AiFailed(m) => {
                assert!(m.contains("timed out"), "expected timeout message, got: {m}");
            }
            other => panic!("expected AiFailed, got {other:?}"),
        }
        // The stub sleeps ~3s; returning well before that proves we killed +
        // reaped the child at the deadline rather than waiting it out.
        assert!(
            elapsed < Duration::from_millis(2500),
            "should return near the 1s deadline, took {elapsed:?}"
        );
    }

    #[test]
    fn run_claude_missing_binary_maps_to_ai_unavailable() {
        let _g = env_lock();
        std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/claude-does-not-exist.exe");
        std::env::remove_var(STUB_MODE_ENV);
        let err = run_claude(Path::new("."), "prompt", None, RunOpts::default())
            .expect_err("missing binary should map to Err");
        assert!(
            matches!(err, AppError::AiUnavailable(_)),
            "expected AiUnavailable, got {err:?}"
        );
    }

    #[test]
    fn run_claude_large_payload_round_trips_without_deadlock() {
        let _g = env_lock();
        set_mode("success");
        // > 128 KiB across many short lines (drain-and-poll proof).
        let payload = "abcdefghij\n".repeat(15_000);
        assert!(payload.len() > 128 * 1024);
        let res = run_claude(Path::new("."), "prompt", Some(&payload), RunOpts::default())
            .expect("large payload should round-trip");
        assert_eq!(res.text, "MERGED_BODY_OK");
    }

    #[test]
    fn check_availability_version_stub_reports_installed() {
        let _g = env_lock();
        set_mode("version");
        let a = check_availability();
        assert!(a.installed);
        assert!(a.logged_in);
        assert_eq!(a.version.as_deref(), Some("2.1.220"));
        assert_eq!(a.detail, "Claude Code 2.1.220 ready");
    }

    #[test]
    fn check_availability_missing_binary_reports_not_installed() {
        let _g = env_lock();
        std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/claude-does-not-exist.exe");
        std::env::remove_var(STUB_MODE_ENV);
        let a = check_availability();
        assert!(!a.installed);
        assert!(!a.logged_in);
        assert_eq!(a.version, None);
        assert_eq!(a.detail, "Claude Code CLI not found on PATH");
    }

    #[test]
    fn strip_fence_only_removes_matching_fences() {
        // Unfenced text is returned verbatim.
        assert_eq!(strip_fence("hello\nworld"), "hello\nworld");
        // Fenced (with lang) -> inner only.
        assert_eq!(strip_fence("```rust\nfn a() {}\n```"), "fn a() {}");
        // Bare fence -> inner only.
        assert_eq!(strip_fence("```\njust text\n```"), "just text");
    }
}
