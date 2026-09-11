//! Shape validation for the two user-configurable launch programs
//! (`terminalCommand` / `editorCommand`), plus the launch-neutral working
//! directory every path-carrying rung uses.
//!
//! **Partially** closes audit 2026-09-03 **MEDIUM-2** (unvalidated program
//! strings) and **LOW-1** (children inherit the repo directory as cwd): see
//! "Residual" below for the three launch routes that survive. Full closure is
//! the removal milestone the user scheduled on 2026-09-11.
//!
//! # Why this exists
//!
//! `set_ui_settings` is an unprivileged webview command, so *anything* achieving
//! script execution in the renderer could park a program string in
//! `settings.json` and have it executed on the next "Open in terminal". The
//! property one WANTS is **renderer compromise ≠ arbitrary local execution**;
//! before this module a parked `powershell -c …` made it spectacularly false.
//! It is still **not true** — see "Residual" — but the gap is now "start an
//! existing program, or any existing file by absolute path" instead of "run an
//! arbitrary command line".
//!
//! # STOPGAP — the capability itself is slated for removal
//!
//! The user ruled (2026-09-11) that user-supplied launch commands go on the
//! roadmap for **removal**, with shape validation as the interim mitigation.
//! This module is therefore deliberately narrow: it does not try to become a
//! general "safe command line" parser, because the end state is no
//! user-supplied program at all (a picked-from-a-list editor instead). Do not
//! grow it into an argument grammar — that is the thing being removed.
//!
//! # The accepted shapes (exactly two)
//!
//! 1. **Empty after trim** — means "use the per-OS auto-detect ladder". Both
//!    settings ship empty, so this is the overwhelmingly common case.
//! 2. **A bare program name**, `[A-Za-z0-9._+-]` only (`code`, `wt`,
//!    `code-insiders`, `notepad++.exe`). Resolved through `PATH` by
//!    [`crate::procutil::resolve_program`]. The allow-list rejects every shell
//!    metacharacter, every separator and all whitespace *by construction* — it
//!    is not a deny-list of the bytes someone has thought of so far.
//! 3. **An absolute path to an existing file** — the portable-editor case
//!    (`D:\Tools\npp\notepad++.exe`, `/Applications/…/bin/code`). Space and
//!    `()` ARE allowed here, because `C:\Program Files (x86)\…` is a real
//!    install path; everything a shell would treat as syntax is still rejected,
//!    and the path must resolve to an existing file.
//!
//! Everything else is refused: embedded arguments (the `powershell -c …`
//! upgrade to arbitrary execution), shell metacharacters, quotes, control
//! characters, relative paths with separators (`./code`, `tools\code.exe`) and
//! UNC paths (`\\server\share\x.exe` — absolute *and* remotely supplied, so it
//! stays out even though `is_file()` would happily fetch it over SMB; the same
//! house rule `git::submodule_abs_path` applies to repo paths).
//!
//! Because arguments are no longer accepted, the target directory is delivered
//! by the launcher, not by the user's string — see `external::PathDelivery`.
//!
//! # Residual — what shape validation does NOT close
//!
//! Corrected 2026-09-11 after the security audit OF THIS INCREMENT (the first
//! version of this paragraph claimed "not arbitrary execution" and "at most one
//! argument, which must be an existing directory"; all of that was false).
//! Validation NARROWS the primitive, it does not close it. A renderer compromise
//! still reaches three launches, none of which needs an argument the validator
//! could refuse:
//!
//! 1. **Any existing FILE, by absolute path.** The absolute branch gates on
//!    `is_file()` and nothing else — not executability, not location, not trust.
//!    A hostile repo ships `payload.exe` inside the working tree, the renderer
//!    points `editorCommand` at that absolute path, and it validates. What that
//!    buys, stated precisely:
//!    * the file must be something `CreateProcess` will run — a PE image or a
//!      `.cmd`/`.bat` — and its path must contain no `is_shell_syntax`
//!      character, or validation refuses it;
//!    * when it does run, it runs **with Bonsai's own privileges**, planted by
//!      the repo rather than installed by the user;
//!    * the editor path passes `hide_console = true` ⇒ `CREATE_NO_WINDOW`, which
//!      suppresses the console of a **console-subsystem** image. That is not
//!      invisible execution in general: a GUI payload still shows its own
//!      windows.
//!
//!    On Windows the file CHECKED and the file SPAWNED can also differ: std's
//!    `resolve_exe` appends `.exe` to a separator-carrying path with no
//!    extension, so `D:\x\foo` is `is_file()`-checked while `D:\x\foo.exe` is
//!    what runs.
//! 2. **`node` + the directory as an ARGUMENT** (`external::PathDelivery::Argument`,
//!    the editor path): `node <dir>` executes that directory's `package.json`
//!    `main` — or its `index.js`. A bare name, so the allow-list admits it, and
//!    on a developer machine with a JS project open it is likelier than the
//!    `python <dir>` → `__main__.py` case.
//! 3. **A build tool + the directory as CWD** (`external::PathDelivery::WorkingDir`,
//!    the terminal path): `make`, `nmake`, `just` or `msbuild` with cwd = the
//!    target directory and **zero arguments** runs that directory's build file.
//!
//! The directory is not bounded to the opened repo either:
//! `commands::external::launch_inner` accepts ANY existing directory the
//! renderer names — `is_dir()` is its only check (pre-existing P49 design).
//!
//! So the honest sentence is: a renderer compromise can start a program the user
//! already has — or any existing file, by absolute path — against any directory
//! the renderer names. Not arbitrary argv; not "not arbitrary execution" either.
//! Closing it needs the capability GONE (pick an editor from a detected list),
//! which the user scheduled on 2026-09-11 as its own removal milestone.

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Generous for any real install path; a program string longer than this is not
/// a program string.
const MAX_LEN: usize = 512;

/// Characters that may appear in a **bare program name**. An allow-list, so a
/// metacharacter nobody enumerated is still rejected.
fn is_bare_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-')
}

/// Characters refused inside an **absolute path**. Space and `()` are absent on
/// purpose (`C:\Program Files (x86)\Notepad++\notepad++.exe`); everything a
/// shell — or `cmd.exe`'s `%VAR%` / `^` escaping — would treat as syntax is
/// present. Control characters are handled separately so newlines and NULs are
/// covered without listing them.
fn is_shell_syntax(c: char) -> bool {
    matches!(
        c,
        '&' | '|' | ';' | '<' | '>' | '^' | '$' | '`' | '{' | '}' | '\'' | '"' | '%' | '*' | '?'
    )
}

/// Characters refused in BOTH branches: C0/C1 controls (`\n`, `\r`, `\t`,
/// `\u{7f}`) plus the bidi overrides and isolates — the same set
/// `ai::stream::strip_control_chars` uses, because the reason is the same. A
/// newline cannot appear in a program name, and a bidi override exists only to
/// make a string read as something other than what launches.
///
/// Note `validate_command_setting` trims first, so ordinary surrounding
/// whitespace (including a trailing newline from a paste) is forgiven rather
/// than refused — `external::program_spec` runs the SAME trim, so both sides see
/// a byte-identical string. That is as far as the claim goes: what the OS
/// finally executes can still differ from what was checked, because Rust std's
/// Windows `resolve_exe` appends `.exe` to a separator-carrying path that has no
/// extension (residual route 1 below).
fn is_disallowed_char(c: char) -> bool {
    let bidi =
        matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    c.is_control() || bidi
}

/// A UNC / double-slash root (`\\server\share`, `//server/share`). Absolute on
/// Windows and fetched over the network by `is_file()`, so it is refused before
/// the filesystem is ever touched.
fn is_unc(value: &str) -> bool {
    let mut cs = value.chars();
    matches!((cs.next(), cs.next()), (Some('\\'), Some('\\')) | (Some('/'), Some('/')))
}

/// One refusal, in the CATEGORY-ONLY style `external::validate_web_url`
/// documents: it names the SETTING and the rule, and never echoes the value.
///
/// The value is renderer- or config-supplied: it can be arbitrarily long, carry
/// bidi overrides, or imitate a system message, and this string is rendered in a
/// toast. `label` is the settings-panel label ("Terminal command" / "Editor
/// command") so the user knows which field to fix.
fn refuse(label: &str) -> AppError {
    AppError::ExternalToolFailed(format!(
        "the {label} setting must be a plain program name (like `code`) or the absolute path to an \
         existing program — clear it to auto-detect"
    ))
}

/// Validate one configured launch program. `Ok(())` for an empty/whitespace-only
/// value (⇒ auto-detect) and for the two accepted shapes; otherwise a
/// category-only [`AppError::ExternalToolFailed`] naming `label`.
///
/// Touches the filesystem ONLY for the absolute branch (`is_file()`), so a bare
/// name costs no IO. Called before any ladder is built, so a refused value never
/// reaches a process — the same ordering `external::open_url` uses.
pub fn validate_command_setting(value: &str, label: &str) -> Result<(), AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.len() > MAX_LEN || trimmed.chars().any(is_disallowed_char) || is_unc(trimmed) {
        return Err(refuse(label));
    }
    let path = Path::new(trimmed);
    // The absolute branch FIRST: an install path legitimately contains spaces
    // and separators, which the bare-name allow-list forbids. Note
    // `is_absolute()` is per-host — a `/Applications/…` string on Windows is
    // not absolute there, falls through to the bare-name branch, and is refused
    // for containing separators. That is correct: it could not launch anyway.
    if path.is_absolute() {
        if trimmed.chars().any(is_shell_syntax) {
            return Err(refuse(label));
        }
        // Must be an existing FILE, not a directory and not a missing path.
        // `is_file()` is also false when the stat itself fails, which is the
        // safe direction here.
        if !path.is_file() {
            return Err(refuse(label));
        }
        return Ok(());
    }
    if trimmed.chars().all(is_bare_name_char) {
        return Ok(());
    }
    Err(refuse(label))
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
/// (`powershell`, `cmd /K`, `x-terminal-emulator`, and a configured terminal
/// program) keep the repo path — see `external::terminal_ladder`. Of those, only
/// the WINDOWS ones carry the DLL-search risk, and `powershell` is the live
/// DEFAULT there rather than an edge case: `wt` ships with Windows 11 but not
/// with stock Windows 10, so rung 2 is what a Win10 user gets.
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
#[path = "external_cmd_tests.rs"]
mod tests;
