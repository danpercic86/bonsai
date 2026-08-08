//! Small shared process-spawn helpers (no spawn logic itself — pure path
//! resolution). Extracted from `external.rs` (P49) so the AI CLI driver
//! (`crate::ai`) can reuse the same PATHEXT-aware resolution (audit §2.7).

use std::path::PathBuf;

/// Resolve a program name to something `Command` can spawn.
///
/// On Windows `Command::new("code")` searches `PATH` for `code`/`code.exe` only
/// — it does NOT find the `code.cmd` shim (npm installs, e.g. `claude.cmd`). So
/// resolve a bare name against `PATH` trying it as-is then with each `PATHEXT`
/// extension (`.EXE`, `.CMD`, `.BAT`, …); the first hit wins and an
/// unresolvable name is an `Err` (callers pick the fallback: `external.rs`
/// walks its ladder, `ai::resolve_bin` falls back to the bare name so its
/// "not found" error path still fires naturally). A name that already contains
/// a path separator is used verbatim.
#[cfg(windows)]
pub fn resolve_program(program: &str) -> Result<PathBuf, String> {
    if program.contains('/') || program.contains('\\') {
        return Ok(PathBuf::from(program));
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();
    for dir in std::env::split_paths(&path_var) {
        let bare = dir.join(program);
        if bare.is_file() {
            return Ok(bare);
        }
        for ext in &exts {
            let candidate = dir.join(format!("{program}{ext}"));
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(format!("`{program}` was not found on PATH"))
}

/// Non-Windows: hand the name to `Command` unchanged and let the OS do the
/// normal `PATH` search (`spawn()` yields `NotFound` → `Err` when it is absent).
#[cfg(not(windows))]
pub fn resolve_program(program: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(program))
}
