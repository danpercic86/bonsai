//! macOS/Linux `claude` CLI discovery ladder (spec 001, `docs/specs/001-macos-claude-cli-path/`).
//!
//! When Bonsai is launched by double-clicking the app, or via Spotlight/the
//! Dock, the process inherits launchd's (or the Linux display manager's)
//! minimal default `PATH` — not the user's shell-configured one. A `claude`
//! CLI installed via a method that only lands it in a shell-only location
//! (`~/.local/bin`, `/opt/homebrew/bin` on Apple Silicon, an nvm/volta shim,
//! …) is invisible to a bare `Command::new("claude")` in that case, even
//! though it works fine from a terminal. This module fixes that by widening
//! the search: the process's own `PATH` first (so nothing changes for users
//! where discovery already works), then the user's **login shell**'s `PATH`
//! (the standard technique other GUI apps use for this exact class of
//! problem), then a short list of well-known install directories.
//!
//! # Lazy, not at startup
//!
//! Unlike [`crate::winenv`]'s sibling Windows fix, this does NOT run at app
//! launch. A `reg.exe` read is cheap; spawning a full login shell is not — a
//! heavy `.zshrc` (nvm, oh-my-zsh, …) can take hundreds of ms to over a
//! second, which would violate spec 001's "no noticeable startup delay"
//! goal. So [`resolve`] is only ever reached from `ai::resolve_bin()`,
//! already called from `spawn_blocking` contexts, and only on the failure
//! path where the cheap inherited-`PATH` lookup already missed.
//!
//! # Never mutates global process state
//!
//! Unlike `winenv`, nothing here calls `std::env::set_var` (or any other
//! environment mutation) — this module only *reads* `PATH`/`SHELL` and
//! returns a resolved [`PathBuf`] to the caller; the process's own `PATH` is
//! left untouched.
//!
//! # Search order — precedence when a user has multiple `claude` installs
//!
//! 1. The process's own inherited `PATH` — trust what already resolves
//!    things normally first (identical outcome to a bare `Command::new`, so
//!    behavior is unchanged for anyone discovery already works for).
//! 2. The user's login shell's `PATH`, probed once and cached for the
//!    process's lifetime — trust the shell's own idea of `PATH` order next.
//! 3. A short list of well-known install directories, in case the shell
//!    probe itself failed (broken `$SHELL`, no shell available, etc.).
//! 4. Give up: return the bare program name, unresolved, so the existing
//!    spawn `NotFound` → `AppError::AiUnavailable` path fires naturally —
//!    mirroring [`crate::procutil::resolve_program`]'s documented fallback
//!    convention.

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Wall-clock budget for probing the login shell's `PATH`. Generous for a
/// slow rc file (nvm, oh-my-zsh, …), bounded so a hung or misconfigured shell
/// can't stall an AI feature indefinitely (spec 001 edge case).
const SHELL_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// How often the probe polls the child for exit while waiting on
/// [`SHELL_PROBE_TIMEOUT`]. Mirrors `ai::run_process`'s poll interval.
const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Resolve `program` (a bare name, e.g. `"claude"`) against the ladder
/// described in the module doc comment. Never panics, never blocks longer
/// than [`SHELL_PROBE_TIMEOUT`], never mutates process state.
pub(super) fn resolve(program: &str) -> PathBuf {
    if let Some(found) = find_in(&current_path_dirs(), program) {
        return found;
    }
    if let Some(found) = find_in(login_shell_path_dirs(), program) {
        return found;
    }
    if let Some(found) = find_in(&fallback_dirs(), program) {
        return found;
    }
    PathBuf::from(program)
}

/// The process's own inherited `PATH`, split into directories. Unset/empty
/// yields an empty list (never a hard failure).
fn current_path_dirs() -> Vec<PathBuf> {
    env::var_os("PATH").map(|p| env::split_paths(&p).collect()).unwrap_or_default()
}

/// The user's login-shell `PATH`, probed at most once per process (AC4) and
/// cached for the rest of the process's lifetime.
fn login_shell_path_dirs() -> &'static [PathBuf] {
    static CACHE: OnceLock<Vec<PathBuf>> = OnceLock::new();
    CACHE.get_or_init(|| probe_login_shell_path().unwrap_or_default())
}

/// Spawn `$SHELL -ilc "echo $PATH"` — the standard technique other GUI apps
/// use to recover a login-shell environment — and parse its stdout.
///
/// - `$SHELL` is read from the environment, falling back to `/bin/zsh` if
///   unset (spec 001 edge case: an unusual/misconfigured `$SHELL`).
/// - `-i` (interactive) + `l` (login) so both login-file
///   (`.zprofile`/`.zlogin`) and interactive-file (`.zshrc`/`.bashrc`) `PATH`
///   edits are picked up, since either is a plausible home for a user's
///   addition.
/// - stdin/stderr are discarded; only stdout is captured. Its output is tiny
///   (one `PATH` line, maybe a startup banner), so — unlike
///   `ai::run_process`'s deadline loop, which this otherwise mirrors — no
///   concurrent reader thread is needed: it's read once, after the child has
///   exited.
/// - Bounded by [`SHELL_PROBE_TIMEOUT`]: on timeout the child is killed and
///   reaped rather than left to leak, and the probe returns `None` so a
///   hung/broken shell degrades to the fallback tiers instead of stalling an
///   AI feature.
/// - Takes the LAST non-empty stdout line, defensively, in case a startup
///   banner or plugin `echo` printed before the real `PATH` line.
///
/// Any failure (spawn error, non-zero exit, timeout, no usable output) →
/// `None`, so [`resolve`] falls through to [`fallback_dirs`].
fn probe_login_shell_path() -> Option<Vec<PathBuf>> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut child = Command::new(&shell)
        .arg("-ilc")
        .arg("echo $PATH")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + SHELL_PROBE_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(PROBE_POLL_INTERVAL);
            }
            Err(_) => return None,
        }
    };
    if !status.success() {
        return None;
    }

    let mut stdout = child.stdout.take()?;
    let mut buf = String::new();
    stdout.read_to_string(&mut buf).ok()?;

    let last_line = buf.lines().rev().find(|line| !line.trim().is_empty())?;
    let dirs: Vec<PathBuf> = env::split_paths(last_line.trim()).collect();
    if dirs.is_empty() {
        None
    } else {
        Some(dirs)
    }
}

/// Last-resort well-known install locations, tried when the login-shell
/// probe itself fails. Best-effort, not exhaustive — the shell probe is the
/// primary mechanism.
///
/// - `~/.local/bin` — this bug's own repro case; also the Claude Code
///   standalone installer's default.
/// - `~/.claude/local` — an older installer layout.
/// - `/opt/homebrew/bin` — Apple Silicon Homebrew; not on launchd's default
///   `PATH` either.
/// - `/usr/local/bin` — Intel Homebrew / most manual installs.
fn fallback_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".claude/local"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

/// `true` iff `path` is a regular file with at least one executable bit set.
/// Not just `is_file()`, so a same-named non-executable file never wins.
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// Search `dirs` in order for an executable file named `program`. `None` if
/// none of `dirs` contains one.
fn find_in(dirs: &[PathBuf], program: &str) -> Option<PathBuf> {
    dirs.iter().map(|dir| dir.join(program)).find(|candidate| is_executable_file(candidate))
}

#[cfg(test)]
#[path = "bin_resolve_tests.rs"]
mod tests;
