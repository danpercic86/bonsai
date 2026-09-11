//! External-tool launch (P49): open a filesystem path in the OS **terminal**,
//! **file manager**, or **editor**.
//!
//! Everything here is a *self-contained* `std::process::Command` spawn — no
//! plugin, no `open` crate (P49 D1). Two halves keep it testable on one machine:
//!
//! * **Pure builders** (`program_spec`, `terminal_ladder`, [`reveal_spec`],
//!   `editor_ladder` — the first and last two are `pub(crate)`, see below)
//!   produce [`LaunchSpec`]s from an explicit [`TargetOs`]
//!   param — never `cfg!` — so every OS branch runs in unit tests regardless of
//!   the host. They never spawn and read no repo state; their only filesystem
//!   contact is [`crate::external_cmd::safe_cwd`]'s `current_exe()` lookup for the
//!   neutral cwd.
//! * A [`CommandRunner`] ([`SpawnRunner`] in production) turns a `LaunchSpec`
//!   into a real child — detached by default, or waited-on for the macOS
//!   `open` launchers (see [`LaunchSpec::wait_for_exit`]). Tests inject a fake
//!   runner to assert the fallback ladder without launching anything.
//!
//! Safety (P49 D2): a launch is always `program + [args…] + explicit cwd` —
//! nothing is ever handed to a shell.
//!
//! ## The user-configured program (audit 2026-09-03 MEDIUM-2, NARROWED 2026-09-11)
//!
//! A configured `terminalCommand` / `editorCommand` is now a **program, not a
//! command line**: [`crate::external_cmd::validate_command_setting`] admits only a bare
//! program name or an absolute path to an existing file, so it can carry neither
//! arguments nor shell syntax — `set_ui_settings` is an unprivileged webview
//! command, so a parked `powershell -c …` used to be one click from execution.
//!
//! It does **not** make *renderer compromise ≠ arbitrary local execution* true,
//! and this module doc said so wrongly for one day: an absolute path to any
//! existing runnable file still launches — with this app's privileges, and with
//! its console suppressed if it is a console-subsystem image — and `node <dir>`
//! or `make` with the directory as cwd still execute repo-authored code. The
//! three surviving routes are enumerated in the `external_cmd` module docs; the
//! capability itself is slated for REMOVAL as its own milestone (ruled
//! 2026-09-11). The `{path}` placeholder is GONE with the tokenizer (it needed a
//! second argv token); the launcher delivers the directory itself, see
//! [`PathDelivery`].
//!
//! WHERE THE PATH COMES FROM (corrected 2026-09-03 — the old note here falsely
//! called it "never attacker-controlled", which was load-bearing): the target is
//! also `sub.absPath`, a repo-authored `.gitmodules` path. Containment is upheld
//! one layer UP, at the producer —
//! `git::submodule_abs_path::contained_abs_path` rejects any rooted/UNC/
//! traversing path (→ `absPath: null`), so a path reaching a `LaunchSpec` here
//! is always workdir-contained. Residual, accepted:
//!  * **Windows `.cmd`/`.bat` shims** (e.g. VS Code's `code.cmd`): when the
//!    resolved program is a batch shim, Windows runs it via `cmd.exe`, which
//!    performs `%VAR%` environment-variable expansion on the argv it receives.
//!    A path literally containing `%FOO%` would be expanded by that shim. We do
//!    NOT quote/escape `%` because there is no robust cross-shim escaping; the
//!    post-CVE (2024-24576) Rust argv-quoting still applies to the raw argument.
//!  * **Windows Terminal (`wt`) `;`**: `wt` treats `;` in ITS OWN argument
//!    parsing as a sub-command delimiter (independent of any shell), so a `;` in
//!    a contained path could start a second `wt` pane. A `wt`-specific arg
//!    convention, not shell injection — and no longer reachable through the
//!    configured program, which can no longer contain `;` at all.
//!
//! ## The child's working directory (audit LOW-1, NARROWED 2026-09-11)
//!
//! Every rung that already passes the target as an argv token launches from
//! [`crate::external_cmd::safe_cwd`] (the app directory) instead of the repo, removing
//! the Windows DLL-search-order primitive a hostile repo root gave us. The four
//! rungs whose *semantics* are the cwd — `powershell`, `cmd /K`,
//! `x-terminal-emulator` and a configured terminal program — necessarily keep
//! the repo path: "open a terminal here" has no other mechanism.
//!
//! Two corrections to how that residual was first written (2026-09-11):
//! * only the **Windows** rungs carry the DLL-search risk. Neither the Linux
//!   dynamic linker nor macOS dyld searches the current directory by default, so
//!   listing `x-terminal-emulator` as part of the residual inflated it.
//! * `powershell` is **the Windows 10 default**, not an edge case: `wt` ships
//!   with Windows 11 but is absent from stock Windows 10, so rung 1 is missing
//!   there and rung 2 is what launches. Keeping its cwd is still the right
//!   trade — the alternative, `powershell -Command "Set-Location '<path>'"`,
//!   would turn a repo-authored path containing a quote into PowerShell
//!   injection, which is strictly worse than a DLL-search primitive.

use crate::error::AppError;
use crate::external_cmd::{safe_cwd, validate_command_setting};
use std::path::{Path, PathBuf};

/// Which OS to build argv for. [`host`](TargetOs::host) picks the running target
/// in production; tests pass each variant explicitly so all branches execute on
/// a single machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    MacOs,
    Linux,
}

impl TargetOs {
    /// The host OS. Anything that is not Windows or macOS is treated as Linux
    /// (the "generic X11/Wayland desktop" ladder).
    pub fn host() -> TargetOs {
        #[cfg(target_os = "windows")]
        {
            TargetOs::Windows
        }
        #[cfg(target_os = "macos")]
        {
            TargetOs::MacOs
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            TargetOs::Linux
        }
    }
}

/// A fully-resolved child launch. Pure output of the builders; a
/// [`CommandRunner`] turns it into a real spawn. NEVER a shell command line —
/// `program` + separate `args` + explicit `cwd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    /// INVARIANT: `cwd` MUST be an existing DIRECTORY (every runner sets it as
    /// the child's `current_dir`). The command layer rejects a non-directory
    /// target EXPLICITLY (audit 2026-09-03 MEDIUM-1), not via the accidental
    /// `NotADirectory` spawn failure a file used to hit.
    pub cwd: PathBuf,
    /// Windows only: suppress the transient console window a `.cmd` shim (VS
    /// Code's `code.cmd`) or `explorer` would flash. MUST be `false` for
    /// terminals — we WANT that window. Ignored on macOS/Linux.
    pub hide_console: bool,
    /// Wait for the child to EXIT and treat a non-zero status as a failure so
    /// the ladder advances (default `false` = detached spawn, exit ignored).
    ///
    /// Set ONLY for the macOS `/usr/bin/open` launchers. `open` hands the
    /// request to LaunchServices and returns immediately, so waiting costs
    /// milliseconds and does NOT block on the launched app's lifetime — but it
    /// is the only way to learn that `open -a "Visual Studio Code"` printed
    /// "Unable to find application", which it reports via its EXIT CODE, not by
    /// failing to spawn. Without this the first `open` rung always "succeeds"
    /// and the fallback ladder never runs (silent no-op, finding F-MAC-1).
    ///
    /// MUST stay `false` everywhere else: Windows `explorer` routinely exits
    /// non-zero after successfully handing off, and a real editor/terminal
    /// would keep us waiting for as long as the user keeps it open.
    pub wait_for_exit: bool,
}

/// Injected so argv-building + ladder logic are testable without launching apps.
/// `Ok(())` = launched — spawned and left detached, or (when
/// [`LaunchSpec::wait_for_exit`]) exited zero. `Err(msg)` = this candidate
/// failed, so [`launch_first`] tries the next ladder entry (or surfaces the
/// error if it was the last).
pub trait CommandRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String>;
}

/// Production runner: builds a `std::process::Command`, sets program/args/cwd,
/// applies `CREATE_NO_WINDOW` iff `spec.hide_console` (Windows), then either
/// `spawn()`s without waiting (default — the child is detached) or, iff
/// `spec.wait_for_exit`, `status()`s and reports a non-zero exit as an error.
pub struct SpawnRunner;

impl CommandRunner for SpawnRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
        let program = resolve_program(&spec.program)?;
        let mut cmd = std::process::Command::new(program);
        cmd.args(&spec.args).current_dir(&spec.cwd);
        #[cfg(windows)]
        if spec.hide_console {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if spec.wait_for_exit {
            // macOS `open` only: it returns as soon as LaunchServices has taken
            // the request, so this does NOT wait on the launched app. Its exit
            // code is the ONLY signal that the app was not found, so a non-zero
            // status must be an `Err` for `launch_first` to try the next rung.
            let status = cmd.status().map_err(|e| e.to_string())?;
            return if status.success() {
                Ok(())
            } else {
                Err(match status.code() {
                    Some(code) => format!("exited with status {code}"),
                    None => "terminated by signal".to_string(),
                })
            };
        }
        // Spawn and drop the handle: we never wait — the launched app is
        // detached and outlives us. Only a spawn failure is a failure (an
        // `explorer` nonzero *exit* is irrelevant because we don't wait).
        cmd.spawn().map(|_child| ()).map_err(|e| e.to_string())
    }
}

/// PATHEXT-aware program resolution — promoted to [`crate::procutil`] so the
/// AI CLI driver shares it (audit §2.7); the semantics for the ladder are
/// unchanged (unresolvable name → `Err` → next ladder entry).
use crate::procutil::resolve_program;

// ---- pure builders (no fs, no spawn) ------------------------------------------

/// Small constructor keeping the ladder tables terse. `cwd` is the child's
/// working directory — [`crate::external_cmd::safe_cwd`] for every rung that passes the
/// target as an argument, the target itself only where the directory IS the
/// feature (audit LOW-1). `wait_for_exit` is the macOS-`open` flag documented on
/// [`LaunchSpec::wait_for_exit`]; every other entry passes `false`.
///
/// `pub(crate)` so the sibling `external_url` ladder shares one constructor.
pub(crate) fn spec(
    program: &str,
    args: &[&str],
    cwd: &Path,
    hide_console: bool,
    wait_for_exit: bool,
) -> LaunchSpec {
    LaunchSpec {
        program: program.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        cwd: cwd.to_path_buf(),
        hide_console,
        wait_for_exit,
    }
}

/// A macOS `/usr/bin/open` ladder entry: same as [`spec`] but always
/// `wait_for_exit = true`, so `open`'s "Unable to find application" exit code
/// makes the ladder fall through instead of silently "succeeding".
pub(crate) fn open_spec(args: &[&str], cwd: &Path, hide_console: bool) -> LaunchSpec {
    spec("open", args, cwd, hide_console, true)
}

/// How the target directory reaches a user-configured program, now that the
/// setting is a program name and can carry no `{path}` placeholder
/// (audit MEDIUM-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathDelivery {
    /// Appended as ONE argv token, launched from [`crate::external_cmd::safe_cwd`].
    /// The editor case: `code`, `subl` and `notepad++.exe` all open the folder
    /// they are HANDED — they ignore their cwd.
    Argument,
    /// Passed as the child's `cwd`, with NO arguments. The terminal case: a
    /// shell opens where it is started, and `powershell <dir>` would try to RUN
    /// the directory as a script.
    WorkingDir,
}

/// Build the single [`LaunchSpec`] for a configured program. `None` for an
/// empty/whitespace-only setting (⇒ the caller's auto-detect ladder).
///
/// The caller MUST have validated `program` first
/// ([`crate::external_cmd::validate_command_setting`]) — [`open_in_terminal`] and
/// [`open_in_editor`] do, before any ladder is built. No spawn and no tokenizing
/// (a validated setting is exactly one token); the only filesystem contact is
/// [`crate::external_cmd::safe_cwd`]'s `current_exe()` lookup.
///
/// **Deliberately `pub(crate)`**, for the reason [`crate::external_url`]'s `url_ladder`
/// is private: handed an UNVALIDATED string this builds a spec that launches it,
/// so the only thing between it and an arbitrary program is that the caller
/// validated first. A doc comment is not a sufficient guard for a primitive of
/// that shape — [`open_in_terminal`] / [`open_in_editor`], which validate
/// unconditionally, are the way in.
pub(crate) fn program_spec(
    program: &str,
    path: &Path,
    hide_console: bool,
    delivery: PathDelivery,
) -> Option<LaunchSpec> {
    let program = program.trim();
    if program.is_empty() {
        return None;
    }
    let (args, cwd) = match delivery {
        PathDelivery::Argument => (vec![path.display().to_string()], safe_cwd()),
        PathDelivery::WorkingDir => (Vec::new(), path.to_path_buf()),
    };
    Some(LaunchSpec {
        program: program.to_string(),
        args,
        cwd,
        hide_console,
        // A user-configured program is arbitrary (`subl`, `nvim`, a wrapper
        // script) and may run for the whole editing session — NEVER wait on it,
        // even when it is literally `open`.
        wait_for_exit: false,
    })
}

/// Ordered terminal candidates. A configured program ⇒ exactly that one spec;
/// an empty setting ⇒ the per-OS auto ladder. All `hide_console = false` — a
/// terminal window MUST be visible.
///
/// LOW-1: the rungs that pass the directory as an ARGUMENT (`wt -d`,
/// `open -a Terminal`, `gnome-terminal --working-directory=`, `konsole
/// --workdir`) launch from [`safe_cwd`]. `powershell`, `cmd /K` and
/// `x-terminal-emulator` take no directory argument at all — their cwd IS where
/// the shell opens — so they keep `path` by necessity, as does a configured
/// program ([`PathDelivery::WorkingDir`]). Of those, only the Windows two carry
/// a DLL-search risk, and `powershell` is the live Windows 10 default because
/// `wt` is not installed there — see the module docs.
///
/// `pub(crate)` for the same reason as [`program_spec`]: `program` must already
/// be validated.
pub(crate) fn terminal_ladder(os: TargetOs, program: &str, path: &Path) -> Vec<LaunchSpec> {
    if let Some(parsed) = program_spec(program, path, false, PathDelivery::WorkingDir) {
        return vec![parsed];
    }
    let p = path.display().to_string();
    let safe = safe_cwd();
    match os {
        TargetOs::Windows => vec![
            spec("wt", &["-d", &p], &safe, false, false),
            spec("powershell", &[], path, false, false),
            spec("cmd", &["/K"], path, false, false),
        ],
        // Terminal.app ships with macOS so this rung effectively never fails,
        // but `open` still gets the wait flag: it is the uniform rule for every
        // `open` launcher, and it upgrades a hypothetical failure from a silent
        // no-op to a real error instead of leaving it invisible.
        TargetOs::MacOs => vec![open_spec(&["-a", "Terminal", &p], &safe, false)],
        TargetOs::Linux => vec![
            spec("gnome-terminal", &[&format!("--working-directory={p}")], &safe, false, false),
            spec("konsole", &["--workdir", &p], &safe, false, false),
            spec("x-terminal-emulator", &[], path, false, false),
        ],
    }
}

/// The single reveal-in-file-manager spec (not configurable). Opens the
/// directory itself in the OS file manager (`hide_console = true`).
///
/// LOW-1: every rung takes the directory as an argument, so all three launch
/// from [`safe_cwd`].
pub fn reveal_spec(os: TargetOs, path: &Path) -> LaunchSpec {
    let p = path.display().to_string();
    let safe = safe_cwd();
    match os {
        // Windows `explorer` MUST stay detached: it habitually exits non-zero
        // after a successful hand-off, so waiting on it would report a bogus
        // failure.
        TargetOs::Windows => spec("explorer", &[&p], &safe, true, false),
        TargetOs::MacOs => open_spec(&[&p], &safe, true),
        TargetOs::Linux => spec("xdg-open", &[&p], &safe, true, false),
    }
}

/// Ordered editor candidates. A configured program ⇒ exactly that one spec; an
/// empty setting ⇒ the per-OS VS Code auto ladder. All `hide_console = true`.
///
/// LOW-1: an editor is always HANDED the folder ([`PathDelivery::Argument`] for
/// a configured program), never started inside it, so EVERY rung — auto and
/// configured — launches from [`safe_cwd`].
///
/// `pub(crate)` for the same reason as [`program_spec`]: `program` must already
/// be validated.
pub(crate) fn editor_ladder(os: TargetOs, program: &str, path: &Path) -> Vec<LaunchSpec> {
    if let Some(parsed) = program_spec(program, path, true, PathDelivery::Argument) {
        return vec![parsed];
    }
    let p = path.display().to_string();
    let safe = safe_cwd();
    match os {
        TargetOs::Windows | TargetOs::Linux => vec![
            spec("code", &[&p], &safe, true, false),
            spec("code-insiders", &[&p], &safe, true, false),
        ],
        // The two `open -a` rungs MUST wait: `open` always spawns fine and
        // signals "Unable to find application" only through its exit code, so
        // without the flag rung #1 would always win and a Mac without VS Code
        // would get a silent no-op instead of falling through to `code`.
        TargetOs::MacOs => vec![
            open_spec(&["-a", "Visual Studio Code", &p], &safe, true),
            open_spec(&["-a", "Visual Studio Code - Insiders", &p], &safe, true),
            spec("code", &[&p], &safe, true, false),
        ],
    }
}

// ---- thin orchestration -------------------------------------------------------

/// Try each spec in order; the first `Ok` wins. If all fail, return
/// [`AppError::ExternalToolFailed`] naming the last candidate + its error.
/// `what` is a human label ("terminal" | "file manager" | "editor").
pub fn launch_first(
    runner: &dyn CommandRunner,
    ladder: &[LaunchSpec],
    what: &str,
) -> Result<(), AppError> {
    let mut last_err: Option<(String, String)> = None;
    for spec in ladder {
        match runner.run(spec) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some((spec.program.clone(), e)),
        }
    }
    Err(AppError::ExternalToolFailed(match last_err {
        Some((prog, e)) => format!("could not launch {what} ({prog}): {e}"),
        None => format!("no {what} command is configured"),
    }))
}

/// Launch a terminal at `path` (empty `program` ⇒ per-OS auto-detect). The
/// caller guarantees `path` exists (the command layer does the fs precheck).
///
/// MEDIUM-2: the configured program is validated BEFORE the ladder is built, so
/// a refused setting never reaches a process — the same ordering
/// [`crate::external_url::open_url`] uses for URLs.
pub fn open_in_terminal(
    runner: &dyn CommandRunner,
    os: TargetOs,
    program: &str,
    path: &Path,
) -> Result<(), AppError> {
    validate_command_setting(program, "Terminal command")?;
    launch_first(runner, &terminal_ladder(os, program, path), "terminal")
}

/// Reveal `path` (a directory) in the OS file manager.
pub fn reveal_in_file_manager(
    runner: &dyn CommandRunner,
    os: TargetOs,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(runner, std::slice::from_ref(&reveal_spec(os, path)), "file manager")
}

/// Open `path` in the configured editor (empty `program` ⇒ VS Code
/// auto-detect). Validates the configured program first — see
/// [`open_in_terminal`].
pub fn open_in_editor(
    runner: &dyn CommandRunner,
    os: TargetOs,
    program: &str,
    path: &Path,
) -> Result<(), AppError> {
    validate_command_setting(program, "Editor command")?;
    launch_first(runner, &editor_ladder(os, program, path), "editor")
}

#[cfg(test)]
#[path = "external_fake.rs"]
pub(crate) mod fake;

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;

// The injection-safety cases moved in-crate when the ladder builders became
// `pub(crate)` (2026-09-11) — see the file header.
#[cfg(test)]
#[path = "external_spawn_tests.rs"]
mod spawn_tests;
