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
/// The scan's own cost is a few tens of milliseconds on a healthy machine; 1.5 s
/// is headroom before it gives up. On exhaustion every remaining `AppPaths`
/// rung yields `None`, which is safe: the tool is simply not offered.
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
    /// value (`reg query <key> /ve`). `None` on ANY failure, including an
    /// exhausted [`SCAN_REG_BUDGET`].
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

/// A LOCAL absolute path in the sense of the **target** OS, not the host.
///
/// `Path::is_absolute` is host-relative: `/usr/bin/konsole` is not absolute on
/// Windows, so using it would make the Linux and macOS ladders untestable from
/// a Windows box (AC2) while silently accepting relative candidates there.
///
/// Two shapes are refused on purpose, both of which `Path::is_absolute` would
/// get wrong for our purpose:
/// * a UNC / double-slash root (`\\server\share`, `//host/share`) — the house
///   rule (`external_cmd::is_unc`, `git::submodule_abs_path`): a remote share
///   is not a local tool, and stat-ing one inside a budgeted scan goes to the
///   network;
/// * a single leading `\` (`\Windows\x.exe`), which is DRIVE-relative on
///   Windows, not absolute, and so would resolve against the process cwd.
fn looks_absolute(p: &Path) -> bool {
    let s = p.to_string_lossy();
    let mut cs = s.chars();
    match (cs.next(), cs.next(), cs.next()) {
        (Some('/'), Some('/'), _) | (Some('\\'), Some('\\'), _) => false,
        (Some('/'), _, _) => true,
        (Some(c), Some(':'), Some('/' | '\\')) => c.is_ascii_alphabetic(),
        _ => false,
    }
}

/// `"C:\X\y.exe"` ⇒ `C:\X\y.exe`. App Paths default values are often quoted.
fn trim_quotes(value: &str) -> &str {
    value.trim().trim_matches('"').trim()
}

/// An executable resolution: an absolute, existing — and on Windows,
/// *launchable* — file.
///
/// **The Windows extension rule, and why it is here.** Verified on this host
/// (2026-09-14): `resolve_on_path("code")` returns
/// `…\Microsoft VS Code\bin\code`, VS Code's extension-LESS POSIX shim, because
/// [`crate::procutil::resolve_program`] tries the bare name before each
/// `PATHEXT` extension (it must, for npm's `claude.cmd`). Windows cannot
/// execute an extension-less file, so offering that path in the picker would
/// mean a tool that is listed and then fails to launch. Requiring an extension
/// makes the rung MISS and the ladder fall through to the App Paths /
/// well-known rungs, which name the real `Code.exe`. Every Windows candidate
/// this crate builds itself already carries `.exe`, so the rule only ever
/// rejects a PATH hit that could not have launched.
fn executable_hit(
    env: &dyn ToolEnv,
    cand: PathBuf,
    source: ToolSource,
    os: TargetOs,
) -> Option<Resolution> {
    if os == TargetOs::Windows && cand.extension().is_none() {
        return None;
    }
    (looks_absolute(&cand) && env.is_file(&cand)).then(|| Resolution {
        program: cand.to_string_lossy().into_owned(),
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
