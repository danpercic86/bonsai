//! External-tool launch (P49): open a filesystem path in the OS **terminal**,
//! **file manager**, or **editor**.
//!
//! Everything here is a *self-contained* `std::process::Command` spawn — no
//! plugin, no `open` crate (P49 D1). Two halves keep it testable on one machine:
//!
//! * **Pure builders** (`spec_from`, [`terminal_ladder`], [`reveal_spec`],
//!   [`editor_ladder`]) produce [`LaunchSpec`]s from an explicit [`TargetOs`]
//!   param — never `cfg!` — so every OS branch runs in unit tests regardless of
//!   the host. They never spawn and read no repo state; their only filesystem
//!   contact is [`safe_cwd`]'s `current_exe()` lookup for the
//!   neutral cwd.
//! * A [`CommandRunner`] ([`SpawnRunner`] in production) turns a `LaunchSpec`
//!   into a real child — detached by default, or waited-on for the macOS
//!   `open` launchers (see [`LaunchSpec::wait_for_exit`]). Tests inject a fake
//!   runner to assert the fallback ladder without launching anything.
//!
//! Safety (P49 D2): a launch is always `program + [args…] + explicit cwd` —
//! nothing is ever handed to a shell.
//!
//! ## Where the program comes from (P112 §0 — the invariant)
//!
//! **Nothing here ever receives a program STRING from settings or the
//! renderer.** A launch program is one of exactly three things:
//!
//! 1. a `&'static str` from `crate::tools::catalog` (the auto ladders below and
//!    every detected tool),
//! 2. an absolute path this crate itself produced from a probe
//!    (`crate::tools::detect`), or
//! 3. the one path a **native dialog the backend opened** returned, stored in
//!    `custom_terminal_path` / `custom_editor_path` — settings fields that
//!    `UiSettingsPatch` has no field able to carry (P112 §5.4).
//!
//! The type that says so is [`crate::tools::PickedTool`]: it is the ONLY input
//! these launchers accept besides `None` (⇒ the auto ladder), and it can only be
//! built by `tools::picked` or by the auto arm of `spec_from`'s callers —
//! **true at the type level only since 2026-09-15** (the third dated correction
//! to a comment of this shape in this file), when its five fields became
//! `pub(crate)`. While they were `pub`, any crate could build the literal and
//! hand it to `terminal_ladder` / `editor_ladder` / `open_in_terminal` /
//! `open_in_editor`, all four of which are `pub` and take
//! `Option<&PickedTool>` — see that type's doc for why `spec_from`'s
//! visibility was never the thing enforcing this. The
//! former `terminalCommand` / `editorCommand` free-text settings, their shape
//! validator (`external_cmd`), the `{path}` template and `PathDelivery` are all
//! **deleted** — so "a renderer-written string names the program" is not
//! rejected at runtime, it is unrepresentable.
//!
//! What this does NOT claim: a browsed `.exe` is still arbitrary code, and a
//! user who selects `make` as their terminal still gets `make` with the repo as
//! its cwd (P112 §5.4, stated rather than hidden). The property bought is
//! provenance, not harmlessness.
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
//! [`safe_cwd`] (the app directory) instead of the repo, removing
//! the Windows DLL-search-order primitive a hostile repo root gave us. The four
//! rungs whose *semantics* are the cwd — `powershell`, `cmd /K`,
//! `x-terminal-emulator` and any browsed terminal program (`Recipe::DirCwd`) —
//! necessarily keep the repo path: "open a terminal here" has no other
//! mechanism.
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
use crate::procutil::safe_cwd;
use crate::tools::catalog::{self, AutoVia, Recipe};
use crate::tools::{PickedTool, ToolKind, ToolSource};
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
/// working directory — [`safe_cwd`] for every rung that passes the
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

/// The ONE spec builder — every launch, picked or auto, goes through it, so the
/// two paths cannot drift (P112 §4).
///
/// `hide_console` is `kind == Editor`: an editor's `.cmd` shim would flash a
/// console window, while a terminal's window IS the feature.
///
/// `(MacOpen, open_arg: None)` is unreachable by construction — `tools::picked`
/// sets `open_arg` for every bundle resolution and the [`AutoVia::MacApp`] arm
/// below carries the catalog `app_name` — so it is a `debug_assert!` plus a spec
/// that simply fails to launch and falls through the ladder, NOT an `expect()`.
/// A launch must never panic on settings-derived state.
///
/// **`pub(crate)` is WIDER than the contract, not narrower** (corrected
/// 2026-09-15): P112 §4 declares `fn spec_from(…)` with no `pub` at all and §1's
/// module table calls it "new **private** `spec_from`". It had to widen, because
/// `tools::settings_ids_tests` imports it (`settings_ids_tests.rs:10`) for the
/// AC6 provenance assertion, and a private fn here is unreachable from a test
/// module under `tools`.
///
/// So the justification is **minimal surface**, not "AC6 says no such
/// constructor exists": AC6 is enforced by [`PickedTool`]'s `pub(crate)` fields
/// (see its doc), which is what actually makes a free-text `LaunchSpec`
/// unconstructible from outside this crate — this function's visibility never
/// did, since the four `pub` launch entry points take a `PickedTool` anyway.
/// `open_in_terminal` / `open_in_editor` are the way in from outside the crate;
/// `terminal_ladder` / `editor_ladder` are `pub` because the contract declares
/// them so and they add no capability those two do not already have.
pub(crate) fn spec_from(picked: &PickedTool, path: &Path) -> LaunchSpec {
    let hide_console = picked.kind == ToolKind::Editor;
    let p = path.display().to_string();
    let safe = safe_cwd();
    match picked.recipe {
        Recipe::MacOpen => {
            debug_assert!(
                picked.open_arg.is_some(),
                "a MacOpen selection always carries the bundle path or the catalog app_name"
            );
            let app = picked.open_arg.as_deref().unwrap_or_default();
            open_spec(&["-a", app, &p], &safe, hide_console)
        }
        Recipe::DirLastArg(fixed) => {
            let args: Vec<&str> = fixed
                .iter()
                .copied()
                .chain(std::iter::once(p.as_str()))
                .collect();
            spec(&picked.program, &args, &safe, hide_console, false)
        }
        Recipe::DirJoinedArg(fixed, prefix) => {
            let joined = format!("{prefix}{p}");
            let args: Vec<&str> = fixed
                .iter()
                .copied()
                .chain(std::iter::once(joined.as_str()))
                .collect();
            spec(&picked.program, &args, &safe, hide_console, false)
        }
        // The directory IS the delivery (LOW-1: these keep the repo as cwd
        // because "open a terminal here" has no other mechanism).
        Recipe::DirCwd(fixed) => spec(&picked.program, fixed, path, hide_console, false),
    }
}

/// The per-OS auto ladder for `kind` — the `""` setting, and the silent fallback
/// for a selection whose tool is gone (the OQ1 ruling).
///
/// **Never probes**: [`launch_first`]'s spawn-fail fall-through is the detector,
/// exactly as the hardcoded ladders were, and the argv is byte-identical to them
/// (AC9).
///
/// `catalog::find_for(kind, id, os)` — **not** `catalog::find` (AMEND-4). `find`
/// resolves host-OS-first, so building the macOS ladder on a Windows host would
/// return the Windows `vscode` row, whose `app_name` is `None`, and the macOS
/// ladder would silently lose its two `open -a` rungs. Any caller that takes
/// `os` as a parameter resolves the catalog by that `os`, never by the host.
///
/// A rung that fails to resolve is SKIPPED rather than panicking: AC8 pins
/// totality as a catalog test, which is where a missing row must fail — not at a
/// user's launch.
fn auto_ladder(kind: ToolKind, os: TargetOs, path: &Path) -> Vec<LaunchSpec> {
    catalog::auto_rungs(kind, os)
        .iter()
        .filter_map(|rung| {
            let e = catalog::find_for(kind, rung.id, os)?;
            let picked = match rung.via {
                // `source: Path` for BOTH arms: the auto path NEVER probes, so
                // `Path` is the "unverified name" bucket. It must NOT be
                // `AppBundle` — `open -a "Visual Studio Code"` is an app-NAME
                // launch with no probed bundle, and a later refactor reading this
                // value would `is_bundle("Visual Studio Code")` and always miss.
                AutoVia::Name => PickedTool {
                    kind,
                    recipe: e.recipe,
                    program: e.program.to_string(),
                    open_arg: None,
                    source: ToolSource::Path,
                },
                AutoVia::MacApp => PickedTool {
                    kind,
                    recipe: Recipe::MacOpen,
                    program: "open".to_string(),
                    open_arg: Some(e.app_name?.to_string()),
                    source: ToolSource::Path,
                },
            };
            Some(spec_from(&picked, path))
        })
        .collect()
}

/// Ordered terminal candidates: the selected tool ⇒ exactly that one spec;
/// `None` ⇒ the per-OS auto ladder. `hide_console` is always `false` — a
/// terminal window MUST be visible.
///
/// LOW-1: the rungs that pass the directory as an ARGUMENT (`wt -d`,
/// `open -a Terminal`, `gnome-terminal --working-directory=`, `konsole
/// --workdir`) launch from [`safe_cwd`]. `powershell`, `cmd /K` and
/// `x-terminal-emulator` take no directory argument at all — their cwd IS where
/// the shell opens — so they keep `path` by necessity, as does a browsed
/// terminal program ([`Recipe::DirCwd`]). Of those, only the Windows two carry a
/// DLL-search risk, and `powershell` is the live Windows 10 default because `wt`
/// is not installed there — see the module docs.
///
/// Safe to be `pub`, unlike the `pub(crate)` it replaces: there is no longer a
/// `program: &str` parameter to hand an unvalidated string to. The only input is
/// a [`PickedTool`], which carries its provenance with it.
pub fn terminal_ladder(os: TargetOs, picked: Option<&PickedTool>, path: &Path) -> Vec<LaunchSpec> {
    match picked {
        Some(p) => vec![spec_from(p, path)],
        None => auto_ladder(ToolKind::Terminal, os, path),
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

/// Ordered editor candidates: the selected tool ⇒ exactly that one spec; `None`
/// ⇒ the per-OS VS Code auto ladder. All `hide_console = true`.
///
/// LOW-1: an editor is always HANDED the folder, never started inside it, so
/// EVERY rung launches from [`safe_cwd`].
///
/// `pub` for the same reason as [`terminal_ladder`].
pub fn editor_ladder(os: TargetOs, picked: Option<&PickedTool>, path: &Path) -> Vec<LaunchSpec> {
    match picked {
        Some(p) => vec![spec_from(p, path)],
        None => auto_ladder(ToolKind::Editor, os, path),
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

/// Launch a terminal at `path` (`None` ⇒ per-OS auto-detect). The caller
/// guarantees `path` exists (the command layer does the fs precheck).
///
/// **Validates nothing, because there is nothing left to validate** (P112 §4):
/// **no code outside `bonsai-core` constructs a [`PickedTool`]** (its fields are
/// `pub(crate)`), and in-crate every production literal is reached either
/// through `tools::picked` — which revalidates the selection's target — or
/// through this module's auto arm, from the static catalog. `PickedTool`'s own
/// doc enumerates those sites; this comment defers to it rather than restating
/// it loosely. The program-string grammar that used to run here is deleted along
/// with the setting that fed it.
pub fn open_in_terminal(
    runner: &dyn CommandRunner,
    os: TargetOs,
    picked: Option<&PickedTool>,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(runner, &terminal_ladder(os, picked, path), "terminal")
}

/// Reveal `path` (a directory) in the OS file manager.
pub fn reveal_in_file_manager(
    runner: &dyn CommandRunner,
    os: TargetOs,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(
        runner,
        std::slice::from_ref(&reveal_spec(os, path)),
        "file manager",
    )
}

/// Open `path` in the selected editor (`None` ⇒ VS Code auto-detect). Validates
/// nothing, for the reason on [`open_in_terminal`].
pub fn open_in_editor(
    runner: &dyn CommandRunner,
    os: TargetOs,
    picked: Option<&PickedTool>,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(runner, &editor_ladder(os, picked, path), "editor")
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

// P112 §4: the picked/browsed launch shapes (AC6's launch half, AC17). Their own
// file so `external_tests.rs` stays the AUTO-ladder record (AC9).
#[cfg(test)]
#[path = "external_picked_tests.rs"]
mod picked_tests;
