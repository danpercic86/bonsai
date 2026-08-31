//! Process-execution helpers for driving the `claude` CLI, extracted from
//! `ai::mod` unchanged. Spawn/drain/timeout (`run_process`), binary resolution
//! (`resolve_bin`), and the tree-kill (`kill_child_tree`). Re-used by the
//! parent's `run_claude` / `check_availability` via `pub(super)`.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::CLAUDE_BIN_ENV;

/// Result of a drained, timeout-bounded child process. (P13)
pub(super) struct ProcOutput {
    pub(super) timed_out: bool,
    pub(super) success: bool,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
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
pub(super) fn run_process(
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
pub(super) fn resolve_bin() -> std::path::PathBuf {
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
        super::bin_resolve::resolve("claude")
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
pub(super) fn kill_child_tree(child: &mut std::process::Child) {
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
