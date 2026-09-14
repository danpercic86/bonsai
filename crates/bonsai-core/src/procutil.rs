//! Small shared process-spawn helpers (no spawn logic itself — pure path
//! resolution). Extracted from `external.rs` (P49) so the AI CLI driver
//! (`crate::ai`) can reuse the same PATHEXT-aware resolution (audit §2.7).

use std::path::PathBuf;

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

#[cfg(test)]
#[path = "procutil_tests.rs"]
mod resolve_tests;
