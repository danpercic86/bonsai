//! Windows `PATH` rehydration backstop (P71 R2).
//!
//! When the app is launched by an installer rather than by the shell, it
//! inherits the *installer's* environment block instead of the user's. That is
//! exactly what the Windows MSI updater did: the WiX `LaunchApplication` custom
//! action runs inside `msiexec.exe`, so `bonsai.exe` came up with msiexec's
//! environment and every directory that lives only in `HKCU\Environment\Path`
//! — a per-user Git install, `%APPDATA%\npm`, a per-user editor — vanished.
//!
//! This module reads the two registry `Path` values that a normal launch would
//! have been built from and **appends any entry missing from the process
//! `PATH`**, in-process only.
//!
//! # This is NOT "the fix"
//!
//! **The fix is P71 R1**: the Windows update channel now ships the NSIS
//! artifact only, whose relaunch (`nsis_tauri_utils::RunAsUser` →
//! `CreateProcessWithTokenW` with `lpEnvironment = NULL`) builds the
//! environment from the user's profile. R2 exists for one specific reason:
//! **R1 does nothing for clients already installed via the MSI** — including
//! the user who reported the bug. Their app keeps launching with a foreign
//! environment after every future update until they reinstall. R2 repairs those
//! clients **in place**, on the next launch, with no reinstall.
//!
//! R2 is a **`PATH` patch, not an environment repair.** It deliberately does
//! **not** restore (contract §5.2):
//!
//! - `USERPROFILE` / `HOME` / `XDG_CONFIG_HOME` → a wrong global git config
//!   (identity, `credential.helper`, `safe.directory`, signing config) stays
//!   wrong.
//! - `SSH_AUTH_SOCK`, `GIT_SSH`, `GIT_SSH_COMMAND` → agent-backed SSH auth
//!   stays broken.
//! - `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` / `NO_PROXY` → forge/PR calls
//!   behind a corporate proxy stay broken.
//! - `TEMP` / `TMP` → scratch files may still land in a system temp dir.
//! - `BONSAI_GIT_BIN`, `BONSAI_GIT_TIMEOUT`, `BONSAI_REQUIRE_GIT_STRICT` →
//!   user overrides set in `HKCU\Environment` stay invisible.
//!
//! `HKCU\Volatile Environment` is read (see below) **purely as input to `%VAR%`
//! expansion**; those variables are NOT exported into the process. Nothing but
//! `PATH` is ever written, so `%LOCALAPPDATA%` stays wrong for everything
//! except the `PATH` entries R2 rebuilds — R2 has not quietly grown into an
//! environment repair.
//!
//! It therefore does not make R1 optional and must never be described as the
//! fix. See `docs/contracts/P71-updater-relaunch-env.md` §5.
//!
//! # Missing entries are APPENDED, never prepended (contract §5.5 — do not re-flip)
//!
//! Recovered entries go **after** the inherited `PATH`, so **nothing on the
//! inherited `PATH` is ever shadowed**. Prepending was specified first and
//! reversed: it bought nothing, because only entries that are **absent** are
//! ever added — a directory missing entirely cannot lose a race it is not in —
//! while it did introduce a real precedence hazard. Windows composes a normal
//! environment as *system* `Path` first with the *user* `Path` appended after,
//! so prepending the recovered block wholesale would have put recovered
//! **user** entries ahead of **system** ones, the inverse of a real launch. In
//! the msiexec case the inherited `PATH` is essentially the machine `PATH`, so
//! `%LOCALAPPDATA%\Microsoft\WindowsApps` (user-writable, part of the default
//! user `Path`) would have preceded `C:\Windows\System32` for every child
//! process — including the bare-name `cmd`/`explorer` in [`crate::external`]
//! and every P59 hook resolving `node`/`npx`/`python`. System-sourced entries
//! still precede user-sourced ones *within* the appended block, and the
//! inherited portion is emitted verbatim.
//!
//! # The profile block, not the inherited environment
//!
//! `HKCU\Environment\Path` is a `REG_EXPAND_SZ` full of `%USERPROFILE%`,
//! `%LOCALAPPDATA%` and `%APPDATA%` references. Expanding those against the
//! *process* environment — the very environment this module distrusts — is how
//! the rescue silently fails: under an MSI custom action descending from a
//! SYSTEM-context installer they resolve below
//! `C:\Windows\system32\config\systemprofile\…`, so the entries R2 exists to
//! recover (`%APPDATA%\npm` for `claude.cmd`, `%LOCALAPPDATA%\Programs\…` for
//! a per-user Git) would be rehydrated pointing at the wrong directory while
//! `applied: true` was reported for a rescue that did nothing. [`PROFILE_VARS`]
//! are therefore resolved from [`VOLATILE_ENV_KEY`] through the same `reg.exe`
//! seam, falling back to the process environment only when that read fails.
//! Machine-scope names (`SystemRoot`, `ProgramFiles`, …) are identical for
//! every process on the box and take the `env.var()` path.
//!
//! # Shape
//!
//! Mirrors [`crate::gitbin`]'s P70 pattern: `reg.exe` by ABSOLUTE path (the
//! whole premise is that `PATH` may be unusable), `CREATE_NO_WINDOW`, defensive
//! parsing, every failure silently skipped, and a [`WinEnv`] injection seam so
//! that **every** case — including the `applied: true` branch — is unit-tested
//! on any host OS with **zero `std::env` mutation**. The pure text half lives
//! in [`crate::winenv_merge`]. No new crate dependency; no IPC, no event, no
//! UI.
//!
//! # Deliberately NOT called from `bonsai-mcp`
//!
//! [`rehydrate_path_once`] has exactly one call site: the first statement of
//! `bonsai::run()`. `crates/bonsai-mcp` does **not** rehydrate and must not
//! start — it is shell-launched (so it already inherits a correct environment),
//! it is multi-threaded well before any equivalent hook point, and
//! `std::env::set_var` is only sound while a process is single-threaded.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub use crate::winenv_merge::{expand_segment, is_absolute_windows_path, merge_path};
use crate::winenv_merge::is_applicable;
#[cfg(windows)]
use crate::winenv_merge::{parse_reg_query, parse_reg_values};

/// Windows console-suppression flag (mirrors `gitbin.rs`, `external.rs`).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Registry sources, read in this order so **system** entries land ahead of
/// **user** entries within the appended block — mirroring the order Windows
/// composes a real environment in.
pub(crate) const SYSTEM_PATH_KEY: &str =
    r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
pub(crate) const USER_PATH_KEY: &str = r"HKCU\Environment";
/// The value name under both keys. Compared case-insensitively when parsing
/// `reg.exe` output — the stored casing differs between hives and Windows
/// builds (`Path` vs `PATH`).
pub(crate) const PATH_VALUE: &str = "Path";

/// The live per-user profile block, written by the session manager at
/// interactive logon. Under a foreign (installer) environment this is the only
/// trustworthy source for the names below.
pub const VOLATILE_ENV_KEY: &str = r"HKCU\Volatile Environment";

/// Variables resolved from [`VOLATILE_ENV_KEY`] rather than from the inherited
/// process environment (contract §5.3.1). Matched case-insensitively.
///
/// `HOMEDRIVE` accompanies `HOMEPATH` because `HOMEPATH` alone is
/// drive-relative, so the pair must come from the same source to expand to an
/// absolute path at all.
pub const PROFILE_VARS: [&str; 7] = [
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERNAME",
    "OneDrive",
];

/// Total wall-clock budget for **all** `reg.exe` work in one rehydration.
///
/// This runs before the Tauri builder, so a wedged `reg.exe` or a hung hive
/// would otherwise mean no window is ever created — worse than the bug being
/// fixed. The budget is shared across every spawn (at most three: the two
/// `Path` values plus one whole-block read of the profile), not granted per
/// spawn. Measured cost on a healthy machine is a few tens of milliseconds in
/// total; 1.5 s is headroom before the app gives up and launches with whatever
/// `PATH` it has.
const REG_BUDGET: Duration = Duration::from_millis(1_500);

// ---- injection seam -----------------------------------------------------------

/// Every environment interaction the backstop performs, injected so that both
/// the planning half and the apply half are testable. Mirrors
/// [`crate::gitbin::GitEnv`].
///
/// Note there is deliberately no registry *write* method: R2 never writes the
/// registry, never touches the parent/user/machine environment, never persists
/// anything, and never broadcasts `WM_SETTINGCHANGE` (contract §5.4
/// constraint 2). The single mutation in the whole module is [`Self::set_path`],
/// which exists as a seam so the `applied: true` branch is assertable on every
/// host OS without touching `std::env`.
pub trait WinEnv {
    /// `reg query <key> /v <value>` → the raw value string, or `None` on any
    /// failure (missing key, non-zero exit, unparseable output, exhausted
    /// budget, non-Windows).
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
    /// Read a process environment variable.
    fn var(&self, key: &str) -> Option<String>;
    /// Replace **this process's** `PATH`. Returns `true` iff the write actually
    /// happened (`false` on non-Windows, where the whole backstop is inert).
    fn set_path(&self, value: &str) -> bool;
}

/// Production implementation: real `std::env`, real `reg.exe`.
pub struct HostWinEnv {
    /// Absolute cut-off shared by every `reg.exe` spawn (see [`REG_BUDGET`]).
    #[cfg_attr(not(windows), allow(dead_code))]
    deadline: Instant,
    /// [`VOLATILE_ENV_KEY`] read ONCE as a whole block and answered from
    /// memory: `PATH` typically references three or four [`PROFILE_VARS`]
    /// several times over, and a `reg.exe` spawn per reference would multiply
    /// the pre-first-paint cost for no new information. Keys are uppercased.
    #[cfg_attr(not(windows), allow(dead_code))]
    volatile: OnceCell<BTreeMap<String, String>>,
}

impl HostWinEnv {
    /// Start the [`REG_BUDGET`] clock. One instance per rehydration attempt.
    pub fn new() -> Self {
        HostWinEnv {
            deadline: Instant::now() + REG_BUDGET,
            volatile: OnceCell::new(),
        }
    }

    /// The profile block, read at most once per instance. An unreadable block
    /// yields an empty map, so every name falls back to the process
    /// environment.
    #[cfg(windows)]
    fn volatile_block(&self) -> &BTreeMap<String, String> {
        self.volatile.get_or_init(|| {
            let mut map = BTreeMap::new();
            if let Some(stdout) = self.run_reg(&["query", VOLATILE_ENV_KEY]) {
                for (name, data) in parse_reg_values(&stdout) {
                    map.insert(name.to_uppercase(), data);
                }
            }
            map
        })
    }

    /// Run `reg.exe <args>` with a bounded wait, returning its stdout.
    ///
    /// `None` on a spawn failure, a non-zero exit, or an exhausted budget. The
    /// child is killed on timeout; the helper thread only drains a pipe and
    /// never reads or writes the environment, so it cannot race the `set_var`
    /// that follows.
    #[cfg(windows)]
    fn run_reg(&self, args: &[&str]) -> Option<String> {
        use std::io::Read;
        use std::os::windows::process::CommandExt;
        use std::path::PathBuf;
        use std::process::{Command, Stdio};
        use std::sync::mpsc;

        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        // ABSOLUTE path on purpose: a bare `reg` would be resolved through the
        // very PATH this module exists to repair (or, worse, through a poisoned
        // one).
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        let reg_exe = PathBuf::from(system_root).join("System32").join("reg.exe");
        if !reg_exe.is_file() {
            return None;
        }
        let mut cmd = Command::new(&reg_exe);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd.spawn().ok()?;
        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.try_wait();
            return None;
        };
        // Drain the pipe on a helper thread: reading inline could block past
        // the deadline, and polling `try_wait` without draining would deadlock
        // on a value larger than the pipe buffer.
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::Builder::new()
            .name("bonsai-winenv-reg".to_string())
            .spawn(move || {
                let mut buf = Vec::new();
                let ok = stdout.read_to_end(&mut buf).is_ok();
                let _ = tx.send(ok.then_some(buf));
            });
        if reader.is_err() {
            let _ = child.kill();
            let _ = child.try_wait();
            return None;
        }
        let bytes = match rx.recv_timeout(remaining) {
            Ok(Some(bytes)) => bytes,
            // Read error, or the deadline elapsed.
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.try_wait();
                return None;
            }
        };
        // EOF on stdout means the child is finished or gone; still poll rather
        // than block, so no path here can outlive the budget.
        if !wait_bounded(&mut child, self.deadline)?.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

impl Default for HostWinEnv {
    fn default() -> Self {
        HostWinEnv::new()
    }
}

/// Poll for the child's exit until `deadline`, then kill it. Never blocks
/// unboundedly. `None` means "no usable exit status" (killed, or `try_wait`
/// itself failed).
#[cfg(windows)]
fn wait_bounded(
    child: &mut std::process::Child,
    deadline: Instant,
) -> Option<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.try_wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl WinEnv for HostWinEnv {
    #[cfg(windows)]
    fn registry_string(&self, key: &str, value: &str) -> Option<String> {
        // The profile block is answered from the one-shot cache; every other
        // key is a targeted `/v` query.
        if key.eq_ignore_ascii_case(VOLATILE_ENV_KEY) {
            return self.volatile_block().get(&value.to_uppercase()).cloned();
        }
        let stdout = self.run_reg(&["query", key, "/v", value])?;
        parse_reg_query(&stdout, value)
    }

    #[cfg(not(windows))]
    fn registry_string(&self, _key: &str, _value: &str) -> Option<String> {
        None
    }

    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    /// The one mutation in the module. Windows only: on any other OS the whole
    /// backstop is inert, and reporting `false` keeps [`PathRehydration`]
    /// honest instead of claiming a write that never happened.
    #[cfg(windows)]
    fn set_path(&self, value: &str) -> bool {
        std::env::set_var("PATH", value);
        true
    }

    #[cfg(not(windows))]
    fn set_path(&self, _value: &str) -> bool {
        false
    }
}

// ---- result type --------------------------------------------------------------

/// Outcome of a rehydration attempt. Diagnostic only — never crosses IPC.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathRehydration {
    /// `true` iff the process `PATH` was actually replaced.
    pub applied: bool,
    /// Directories present in the registry `PATH` but absent from the process
    /// `PATH`, **fully expanded**, in the order they were appended (system
    /// -sourced first). Segments dropped by the expansion or absolute-path
    /// guards never appear here. The concrete evidence of a foreign
    /// environment; empty when `applied` is false.
    pub added: Vec<String>,
}

// ---- decide -------------------------------------------------------------------

/// Read both registry `PATH`s through `env` and merge them against the process
/// `PATH` (expansion happens per segment inside [`merge_path`]).
///
/// **Pure** — computes only, mutates nothing — which is what lets every
/// merge/expansion case be unit-tested hermetically on any host, and what makes
/// it safe to call from a multi-threaded test binary.
///
/// `None` when nothing is missing, when nothing usable could be read, or when
/// the process `PATH` itself is unreadable.
pub fn plan_rehydration(env: &dyn WinEnv) -> Option<(String, Vec<String>)> {
    // `std::env::var` returns Err for BOTH "unset" and "set but not valid
    // Unicode". Defaulting either to "" would make the merge believe the
    // process PATH is EMPTY and hand back the registry entries alone —
    // discarding an inherited PATH we merely could not decode, which is worse
    // than the bug being fixed. A genuinely unset PATH is not worth rescuing.
    let process = env.var("PATH")?;

    let system = env.registry_string(SYSTEM_PATH_KEY, PATH_VALUE);
    let user = env.registry_string(USER_PATH_KEY, PATH_VALUE);
    if system.is_none() && user.is_none() {
        return None;
    }
    merge_path(
        system.as_deref().unwrap_or_default(),
        user.as_deref().unwrap_or_default(),
        &process,
        env,
    )
}

// ---- apply --------------------------------------------------------------------

/// Decide (via [`plan_rehydration`]) and apply, through the [`WinEnv`] seam.
///
/// Silent no-op returning [`PathRehydration::default()`] on non-Windows, on any
/// registry read failure, on malformed values, when the merged value would be
/// unusable, and when nothing is missing — never an error, never a panic,
/// nothing logged at error level.
///
/// # Safety-adjacent precondition
///
/// The production [`WinEnv::set_path`] calls `std::env::set_var`, which is only
/// sound while the process is still single-threaded. Call it from the first
/// statement of `run()`, before any thread, the async runtime, or the Tauri
/// builder exists — and before `gitbin`'s process-lifetime cache is populated,
/// so the P70 ladder sees the repaired `PATH`.
pub fn rehydrate_path(env: &dyn WinEnv) -> PathRehydration {
    let Some((merged, added)) = plan_rehydration(env) else {
        return PathRehydration::default();
    };
    // `set_var` PANICS on a NUL byte and on an over-long value; this runs
    // before the first paint, so a panic here would mean the app never opens.
    if !is_applicable(&merged) {
        return PathRehydration::default();
    }
    if !env.set_path(&merged) {
        return PathRehydration::default();
    }
    PathRehydration {
        applied: true,
        added,
    }
}

/// Production entry point. MUST be the FIRST statement of `bonsai::run()` (see
/// [`rehydrate_path`]). Idempotent: the work happens once per process and the
/// recorded outcome is returned on every later call.
pub fn rehydrate_path_once() -> PathRehydration {
    static ONCE: OnceLock<PathRehydration> = OnceLock::new();
    ONCE.get_or_init(|| rehydrate_path(&HostWinEnv::new()))
        .clone()
}

#[cfg(test)]
#[path = "winenv_fake.rs"]
pub(crate) mod fake;

#[cfg(test)]
#[path = "winenv_tests.rs"]
mod tests;
