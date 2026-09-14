//! Probing: the only part of `tools` that touches the machine.
//!
//! Mirrors [`crate::gitbin::GitEnv`] — same injection seam, same "no run" rule:
//! [`ToolEnv`] deliberately exposes **no way to execute a candidate**, so
//! detection can read the filesystem and the registry but can never launch
//! anything. A hit is always an existence-checked absolute path (or a
//! `BuiltIn` name), so a wrong candidate *fails to detect*; it cannot
//! mis-detect.
//!
//! Every rung degrades to `None` on any failure — a missing variable, an absent
//! key, unparseable registry output, a failed stat, an exhausted budget.
//! Degradation only ever means "not offered in the picker".

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::external::TargetOs;
use crate::gitbin::{win_join, GitEnv, HostGitEnv};

use super::catalog::{Recipe, Rung, ToolEntry, APP_PATHS_SUBKEY};
use super::{Resolution, ToolKind, ToolSource};

/// Wall-clock budget for **all** registry work in ONE tool scan.
///
/// Deliberately its own constant rather than [`crate::winenv`]'s `REG_BUDGET`:
/// that one is 1.5 s shared across at most three spawns of PATH rehydration,
/// which runs *before the window exists*. A scan issues one `reg.exe` spawn per
/// `AppPaths` rung (three today, and the table is meant to grow), so borrowing
/// winenv's budget would make the two features starve each other — the scan
/// running out of time, or PATH rehydration losing the time it needs to make
/// the app usable at all.
///
/// **Measured, because the original "a few tens of milliseconds" estimate was
/// wrong** (this host, 2026-09-14): a full scan is ~0.45 s warm and ~2.1 s on a
/// COLD filesystem cache, dominated by the PATH walk (55 `PATH` directories x
/// 11 `PATHEXT` entries ~ 4 400 stats) and NOT by the registry at all. The
/// clock starts at [`HostToolEnv::new`], before any of that, so a cold scan can
/// spend this whole budget before the first `AppPaths` rung even runs — and
/// then every remaining one yields `None`.
///
/// That is safe (the tool is simply not offered, and every `AppPaths` entry in
/// today's catalog also carries a `WinFolder` rung, so a default install is
/// still found — that premise is no longer just this comment: it is pinned by
/// `catalog_tests::every_app_paths_row_also_has_a_well_known_folder_rung`), but
/// it does mean an install in a NON-default folder — the case
/// only `AppPaths` can find — can be missed on the first scan after a cold
/// boot; the picker's Rescan then finds it. Starting the deadline at the first
/// registry call, or budgeting registry time only, is the fix and is its own
/// change.
///
/// Enforcement is a **pre-spawn** cut-off (it bounds how many spawns a scan may
/// start, not how long one may hang). That is deliberate: `reg.exe` is invoked
/// through [`HostGitEnv`], whose spawn is unbounded, and a scan runs inside
/// `spawn_blocking` — so a wedged `reg.exe` costs one blocking thread, never the
/// window. winenv needs its bounded wait precisely because it has no window yet.
const SCAN_REG_BUDGET: Duration = Duration::from_millis(1_500);

/// Every environment interaction a probe performs, injected so all three OS
/// ladders run in unit tests on a single machine (`FakeToolEnv`).
///
/// There is no `run`/`spawn` method, and there must never be one: "is this tool
/// installed?" is answered by *looking*, never by executing a candidate.
pub trait ToolEnv {
    /// A process environment variable.
    fn var(&self, key: &str) -> Option<String>;
    /// An existing regular file.
    fn is_file(&self, p: &Path) -> bool;
    /// A directory AND a real macOS bundle: `p.is_dir()` **and**
    /// `p/Contents/Info.plist` is a file. The `Info.plist` requirement is what
    /// distinguishes a bundle from a directory merely named `*.app`.
    fn is_bundle(&self, p: &Path) -> bool;
    /// At least one execute bit on unix; on Windows, true for any file (there
    /// is no exec bit — the extension is the gate, enforced where it matters,
    /// in `validate_custom_program`).
    fn is_executable(&self, p: &Path) -> bool;
    /// `PATH` (+ `PATHEXT` on Windows) lookup of a bare program name.
    fn resolve_on_path(&self, program: &str) -> Option<PathBuf>;
    /// ONE registry string value. `value == ""` means the key's **default**
    /// value (`reg query <key> /ve`).
    ///
    /// `None` on a spawn error, a non-zero exit, unparseable output, or an
    /// exhausted [`SCAN_REG_BUDGET`] — but **not** on every failure. An
    /// existing key whose default value is UNSET makes `reg query ... /ve` exit
    /// 0 and print `(Default) REG_SZ (value not set)`, which the parser returns
    /// as a non-path `Some` (verified against `HKCU\Environment` on this host,
    /// 2026-09-14). Callers must therefore SHAPE-CHECK the result; the only
    /// caller here is [`executable_hit`], whose [`locally_absolute`] refuses it
    /// (`(` is not a drive letter). Do NOT "fix" this by matching the literal —
    /// `reg.exe` localizes it.
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
    /// The user's home directory, for `$HOME`-relative bundle rungs.
    fn home(&self) -> Option<PathBuf>;
}

/// Production implementation: real `std::env`, real filesystem, real `reg.exe`.
///
/// One instance per scan — it carries that scan's [`SCAN_REG_BUDGET`] deadline.
pub struct HostToolEnv {
    deadline: Instant,
}

impl HostToolEnv {
    /// Start this scan's registry budget clock.
    pub fn new() -> HostToolEnv {
        HostToolEnv {
            deadline: Instant::now() + SCAN_REG_BUDGET,
        }
    }

    /// Test seam: an env whose registry budget is already spent, so the
    /// pre-spawn cut-off is assertable without waiting 1.5 s.
    #[cfg(test)]
    pub(crate) fn with_deadline(deadline: Instant) -> HostToolEnv {
        HostToolEnv { deadline }
    }
}

impl Default for HostToolEnv {
    fn default() -> Self {
        HostToolEnv::new()
    }
}

impl ToolEnv for HostToolEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn is_file(&self, p: &Path) -> bool {
        p.is_file()
    }

    fn is_bundle(&self, p: &Path) -> bool {
        p.is_dir() && p.join("Contents").join("Info.plist").is_file()
    }

    #[cfg(windows)]
    fn is_executable(&self, p: &Path) -> bool {
        p.is_file()
    }

    #[cfg(not(windows))]
    fn is_executable(&self, p: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }

    /// Delegated to [`HostGitEnv`], which already does the PATHEXT-aware
    /// Windows lookup and the explicit PATH walk elsewhere (an empty PATH
    /// component would otherwise resolve against the process cwd).
    fn resolve_on_path(&self, program: &str) -> Option<PathBuf> {
        HostGitEnv.resolve_on_path(program)
    }

    /// Delegated to [`HostGitEnv`] (absolute `reg.exe`, `CREATE_NO_WINDOW`,
    /// defensive parsing), gated by this scan's own budget.
    ///
    /// NOT delegated to [`crate::winenv::HostWinEnv`] on purpose — see
    /// [`SCAN_REG_BUDGET`].
    fn registry_string(&self, key: &str, value: &str) -> Option<String> {
        if self.deadline.saturating_duration_since(Instant::now()).is_zero() {
            return None;
        }
        HostGitEnv.registry_string(key, value)
    }

    fn home(&self) -> Option<PathBuf> {
        // `HOME` on unix (the only OS with `$HOME`-relative bundle rungs);
        // `USERPROFILE` keeps the Windows branch honest rather than `None`.
        std::env::var("HOME")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
    }
}

/// `"C:\X\y.exe"` ⇒ `C:\X\y.exe`. App Paths default values are often quoted.
fn trim_quotes(value: &str) -> &str {
    value.trim().trim_matches('"').trim()
}

/// DETECTION's "a local absolute path on `os`" test — deliberately **stricter
/// than the browse path's**, and kept here rather than shared so it can stay
/// that way.
///
/// [`super::custom::is_absolute_for`] IS shared: the drive-relative refusal
/// (`\Windows\Code.exe` and `/Windows/Code.exe` both resolve against the
/// process cwd on Win32) is what both callers want, and it must be `os`-aware
/// because `Path::is_absolute` is host-relative.
///
/// The UNC arm is **not** shared. AMEND-6 (user ruling #26,
/// `docs/contracts/P112-external-tool-detection.md`) allows UNC via Browse and
/// keeps refusing it here: a scan has ONE 1500 ms budget
/// ([`SCAN_REG_BUDGET`]) for every rung on the machine, and a picker that comes
/// up empty on a slow VPN is worse than one that omits a share install. A
/// single shared predicate would make that divergence unrepresentable — so the
/// decision lives at this call site. Today both sides still refuse UNC; the
/// split is what lets the browse side change alone.
///
/// **Which rungs this actually spares network I/O on:** `WinFolder` and
/// `AppPaths` only — there the check precedes the stat. (`UnixFile` stats
/// first, via `is_executable` in [`probe_entry`], so it is post-hoc too; it is
/// moot there, since those candidates are static catalog literals and can
/// never be UNC.) For **`OnPath` the refusal is purely post-hoc**:
/// `crate::procutil::resolve_in` calls `is_file()` on every candidate and
/// `HostToolEnv::resolve_on_path` delegates straight to it, so one UNC `PATH`
/// entry has already cost ~12 network stats before this is reached. Moving the
/// guard into `resolve_in` would prevent that I/O, but it changes app-wide
/// program resolution and is its own change (filed separately). Do not justify
/// this refusal on I/O-avoidance grounds without that caveat.
fn locally_absolute(os: TargetOs, value: &str) -> bool {
    !super::custom::is_unc(value) && super::custom::is_absolute_for(os, value)
}

/// An executable resolution: a local absolute path for detection
/// ([`locally_absolute`]), an existing file, and on Windows a *launchable* one.
///
/// **The Windows extension rule is a correctness heuristic, NOT a security
/// boundary.** It catches exactly one shape: an extension-LESS PATH hit.
/// Measured on this host (2026-09-14), VS Code installs a `#!/usr/bin/env sh`
/// shim at `...\Microsoft VS Code\bin\code` beside `bin\code.cmd`, and
/// spawning the shim fails with `os error 193` ("%1 is not a valid Win32
/// application") because it carries no PE header — a tool that would be listed
/// and then fail to launch.
///
/// It is NOT true that Windows cannot execute an extension-less file:
/// `CreateProcess` ignores the extension and validates the image header, so a
/// PE named without one runs fine (`.cmd`/`.bat` are the shell's special
/// cases). `Path::extension()` also yields `Some("")` for a trailing-dot name
/// (`code.`), which Windows resolves by stripping the dot — so the guard is
/// trivially bypassable and nothing may lean on it for safety.
///
/// Since [`crate::procutil::resolve_program`] now prefers `PATHEXT` matches
/// over the bare name (the follow-up that fixed the shipped "Open in editor"
/// failure), `resolve_on_path("code")` returns `bin\code.cmd` and this rung
/// **hits** — it no longer misses and falls through to the App Paths rung's
/// `Code.exe`. That is intended: the auto ladder launches the same `.cmd`, and
/// `std`'s spawn applies batch-specific argv escaping (the mitigated
/// CVE-2024-24576 class).
///
/// `TargetOs::Windows` is the predicate, never the host OS: host-gating would
/// zero AC2's unix ladders, whose `/usr/bin/...` candidates are all
/// extension-less.
fn executable_hit(
    env: &dyn ToolEnv,
    cand: PathBuf,
    source: ToolSource,
    os: TargetOs,
) -> Option<Resolution> {
    if os == TargetOs::Windows && cand.extension().is_none() {
        return None;
    }
    let value = cand.to_string_lossy();
    // DETECTION's predicate, not the browse path's: per AMEND-6 (ruling #26)
    // the two diverge on UNC on purpose — the disagreement IS the ruling.
    if !locally_absolute(os, &value) {
        return None;
    }
    env.is_file(&cand).then(|| Resolution {
        program: value.into_owned(),
        bundle: None,
        source,
    })
}

/// Run `entry`'s ladder against `env`; the first hit wins, rungs in order.
pub(crate) fn probe_entry(env: &dyn ToolEnv, entry: &ToolEntry) -> Option<Resolution> {
    entry.rungs.iter().find_map(|rung| match rung {
        // Present by definition: touches nothing. An `is_file("cmd")` test
        // would be false and would break picking `cmd` / `powershell`.
        Rung::BuiltIn => Some(Resolution {
            program: entry.program.to_string(),
            bundle: None,
            source: ToolSource::BuiltIn,
        }),
        Rung::OnPath => {
            let cand = env.resolve_on_path(entry.program)?;
            executable_hit(env, cand, ToolSource::Path, entry.os)
        }
        // HKCU first: a per-user install is the one the user actually chose.
        Rung::AppPaths { exe } => ["HKCU", "HKLM"].into_iter().find_map(|root| {
            let key = format!("{root}\\{APP_PATHS_SUBKEY}\\{exe}");
            let raw = env.registry_string(&key, "")?;
            let cand = PathBuf::from(trim_quotes(&raw));
            executable_hit(env, cand, ToolSource::Registry, entry.os)
        }),
        Rung::WinFolder { var, suffix } => {
            let base = env.var(var)?;
            if base.is_empty() {
                return None;
            }
            executable_hit(env, win_join(&base, suffix), ToolSource::WellKnown, entry.os)
        }
        Rung::Bundle { path, home } => {
            let cand = if *home {
                // A string join, not `Path::join`: on a Windows host the latter
                // produces `\`-mixed output, so the probed bundle path — which
                // becomes an argv token and the picker subtitle — would not be
                // byte-identical to what a Mac produces.
                let base = env.home()?;
                PathBuf::from(format!(
                    "{}/{}",
                    base.to_string_lossy().trim_end_matches(['/', '\\']),
                    path
                ))
            } else {
                PathBuf::from(path)
            };
            env.is_bundle(&cand).then(|| Resolution {
                program: "open".to_string(),
                bundle: Some(cand.to_string_lossy().into_owned()),
                source: ToolSource::AppBundle,
            })
        }
        Rung::UnixFile { path } => {
            let cand = PathBuf::from(path);
            if !env.is_executable(&cand) {
                return None;
            }
            executable_hit(env, cand, ToolSource::WellKnown, entry.os)
        }
    })
}

/// Probe every catalog entry for `os`, in table order.
///
/// `os` is an explicit parameter (never `cfg!`) so all three ladders execute on
/// one machine against a fake env — the house pattern from
/// [`crate::gitbin::resolve_ladder_for`].
pub fn scan_for(env: &dyn ToolEnv, os: TargetOs) -> Vec<(&'static ToolEntry, Resolution)> {
    super::catalog::CATALOG
        .iter()
        .filter(|e| e.os == os)
        .filter_map(|e| probe_entry(env, e).map(|r| (e, r)))
        .collect()
}

/// The one fs recheck [`super::picked`] does before a launch, by source.
///
/// A launch never re-runs the ladder — a `Registry` rung spawns a process — so
/// this is the whole liveness check: one stat, or nothing at all for `BuiltIn`.
pub(crate) fn still_present(env: &dyn ToolEnv, res: &Resolution) -> bool {
    // Loud rather than silent: a `Custom` resolution is revalidated by
    // `validate_custom_program` instead, so routing one here is a caller bug
    // that would otherwise surface as an unexplained "the tool is gone".
    debug_assert!(
        res.source != ToolSource::Custom,
        "a Custom resolution is revalidated by validate_custom_program, never by still_present"
    );
    match res.source {
        // Present by definition; an fs test here would silently break `cmd`.
        ToolSource::BuiltIn => true,
        ToolSource::Path | ToolSource::Registry | ToolSource::WellKnown => {
            env.is_file(Path::new(&res.program))
        }
        ToolSource::AppBundle => res
            .bundle
            .as_deref()
            .is_some_and(|b| env.is_bundle(Path::new(b))),
        // Revalidated by `validate_custom_program` instead (a changed shape
        // must be refused, not launched), so never routed here.
        ToolSource::Custom => false,
    }
}

/// `(entry, resolution)` ⇒ the launch-ready selection, normalising bundles.
///
/// Any bundle resolution becomes `open -a <bundle> <dir>`, so the launch-side
/// spec builder needs no bundle branch and the picked and auto paths cannot
/// drift.
pub(crate) fn resolution_to_picked(
    kind: ToolKind,
    entry: &'static ToolEntry,
    res: &Resolution,
) -> super::PickedTool {
    match &res.bundle {
        Some(bundle) => super::PickedTool {
            kind,
            recipe: Recipe::MacOpen,
            program: "open".to_string(),
            open_arg: Some(bundle.clone()),
            source: res.source,
        },
        None => super::PickedTool {
            kind,
            recipe: entry.recipe,
            program: res.program.clone(),
            open_arg: None,
            source: res.source,
        },
    }
}
