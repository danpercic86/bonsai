//! Small shared process-spawn helpers (no spawn logic itself — pure path
//! resolution plus the launch-neutral working directory). Extracted from
//! `external.rs` (P49) so the AI CLI driver (`crate::ai`) can reuse the same
//! PATHEXT-aware resolution (audit §2.7).
//!
//! [`safe_cwd`] moved here from `external_cmd.rs` in P112 sub-increment 3
//! **verbatim**: §7 deletes that module (its program-string grammar is the
//! capability P112 removes), and this was the one function in it that the
//! launchers and [`crate::external_url`] still need. Moving it lets the module
//! be deleted outright instead of surviving as a one-function stub.

use std::path::{Path, PathBuf};

/// Resolve a program name to something `Command` can spawn.
///
/// On Windows `Command::new("code")` searches `PATH` for `code`/`code.exe` only
/// — it does NOT find the `code.cmd` shim (npm installs, e.g. `claude.cmd`). So
/// resolve a bare name against `PATH` here: per PATH directory, try each
/// `PATHEXT` extension (`.COM`, `.EXE`, `.BAT`, `.CMD`, …) **first**, then the
/// bare name as a fallback. The first hit wins and an unresolvable name is an
/// `Err` (callers pick the fallback: `external.rs` walks its ladder,
/// `ai::resolve_bin` falls back to the bare name so its "not found" error path
/// still fires naturally). A name that already contains a path separator is
/// used verbatim.
///
/// **Why `PATHEXT` comes before the bare name** (P112 follow-up, measured
/// 2026-09-14). VS Code installs an extension-LESS POSIX shim at
/// `…\Microsoft VS Code\bin\code`, beside `bin\code.cmd`. Bare-first returned
/// the shim, and spawning it fails with `os error 193` ("%1 is not a valid
/// Win32 application") — it is a 2073-byte `#!/usr/bin/env sh` script with no
/// PE header — so `external.rs`' Windows `editor_ladder`, which opens with a
/// bare `code` rung, failed outright on a standard VS Code install. Bare-first
/// was believed load-bearing for npm's `claude.cmd`; it is not — the `PATHEXT`
/// loop finds that on its own. The bare name stays as a LAST resort, for the
/// rare extension-less PE (which `CreateProcess` does run: it validates the
/// image header, not the name).
///
/// Two `PATH`-hygiene guards every caller inherits, mirroring
/// [`crate::gitbin::HostGitEnv::resolve_on_path`]'s non-Windows branch: an
/// **empty** `PATH` component (`C:\bin;;…`, or the very common trailing `;` —
/// `std::env::split_paths` yields those, verified on this host) is skipped, and
/// the candidate must be **absolute**. Without them `dir.join(program)` is the
/// bare RELATIVE name resolved against the process cwd, so a stray file named
/// `git`/`code` inside the open repository could be spawned as the program.
/// A relative `PATH` entry (`.`) therefore never resolves: deliberate.
#[cfg(windows)]
pub fn resolve_program(program: &str) -> Result<PathBuf, String> {
    if program.contains('/') || program.contains('\\') {
        return Ok(PathBuf::from(program));
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    resolve_in(program, &path_var, &pathext)
        .ok_or_else(|| format!("`{program}` was not found on PATH"))
}

/// The search half of [`resolve_program`], with `PATH` and `PATHEXT` as
/// parameters so the extension ordering and both hygiene guards are testable
/// without mutating process-global environment state.
#[cfg(windows)]
fn resolve_in(program: &str, path_var: &std::ffi::OsStr, pathext: &str) -> Option<PathBuf> {
    let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();
    std::env::split_paths(path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .find_map(|dir| {
            // PATHEXT first, bare name last, WITHIN one directory — so PATH
            // order stays the primary precedence and only the extension
            // preference is new.
            exts.iter()
                .map(|ext| dir.join(format!("{program}{ext}")))
                .chain(std::iter::once(dir.join(program)))
                .find(|cand| cand.is_absolute() && cand.is_file())
        })
}

/// Non-Windows: hand the name to `Command` unchanged and let the OS do the
/// normal `PATH` search (`spawn()` yields `NotFound` → `Err` when it is absent).
#[cfg(not(windows))]
pub fn resolve_program(program: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(program))
}

/// The launch-neutral working directory for every rung that already carries the
/// target path as an argv token (audit LOW-1).
///
/// **Why the app directory and not a system directory:** under
/// `SafeDllSearchMode` the current directory is searched after `System32` but
/// BEFORE `PATH`, so a hostile repo shipping a `.dll` at its root gets a
/// DLL-planting primitive against every child we spawn from it. The app's own
/// directory removes that primitive without inventing a per-OS system-path table
/// (`C:\Windows\System32` / `/usr` / `/private/var`), and an attacker who can
/// write next to `bonsai.exe` already owns the box. It is also the one directory
/// that is guaranteed to exist for a running process.
///
/// Observable behaviour is unchanged because every caller of this passes the
/// directory explicitly as an argument. The rungs whose semantics ARE the cwd
/// (`powershell`, `cmd /K`, `x-terminal-emulator`, and any browsed terminal
/// program, which launches through `Recipe::DirCwd`) keep the repo path — see
/// `external::terminal_ladder`. Of those, only the WINDOWS ones carry the
/// DLL-search risk, and `powershell` is the live DEFAULT there rather than an
/// edge case: `wt` ships with Windows 11 but not with stock Windows 10, so rung
/// 2 is what a Win10 user gets.
///
/// Falls back to [`std::env::temp_dir`] when `current_exe()` is unavailable —
/// deliberately NOT `"."`, which is the process cwd and, under `pnpm tauri dev`,
/// IS the repo root, i.e. the exact primitive this function removes. The temp
/// directory is never a repository, always exists, and keeps this from turning a
/// working launch into a failure.
pub fn safe_cwd() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
#[path = "procutil_tests.rs"]
mod resolve_tests;

#[cfg(test)]
#[path = "procutil_cwd_tests.rs"]
mod cwd_tests;
