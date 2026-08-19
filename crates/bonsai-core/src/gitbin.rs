//! Git executable resolution (P70 D1).
//!
//! Sibling of [`crate::procutil`] — that module is the generic, git-agnostic
//! "resolve a program name against PATH/PATHEXT" helper; THIS module is the
//! git-specific concern: a candidate ladder (override → PATH → Git-for-Windows
//! registry → well-known install folders → bare-name fallback), a
//! process-lifetime refreshable cache, and the one [`Command`] factory every
//! production `git` shell-out goes through.
//!
//! **Why it exists.** The MSI updater relaunches `bonsai.exe` as a child of
//! `msiexec.exe`, so the app inherits msiexec's environment block. A per-user
//! Git install (`%LOCALAPPDATA%\Programs\Git\cmd\git.exe`) lives only in the
//! **User** PATH, so `Command::new("git")` cannot resolve and every shell-out
//! fails — historically as a misleading "no cached credentials" auth toast.
//! The ladder recovers the real path; [`spawn_error`] makes the remaining
//! failure honest.
//!
//! **Never executes a candidate** (D4): rungs are validated with `is_file()`
//! only, because resolution sits on the hot path of every search / graph /
//! signing call and a Windows spawn costs 20–80 ms. Execution-based validation
//! belongs in the (off-hot-path) preflight.
//!
//! **Hermetic by construction**: every environment interaction goes through the
//! injected [`GitEnv`] seam and the OS branch is an explicit [`TargetOs`]
//! parameter (P49 house pattern), so the whole ladder is unit-tested on any
//! host with ZERO `std::env` mutation.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

use crate::error::AppError;
use crate::external::TargetOs;

/// Explicit override; mirrors the `CLAUDE_BIN_ENV` idiom in `ai/mod.rs`. Used
/// verbatim (no PATHEXT expansion, no existence check) and doubles as the
/// hermetic test seam for out-of-process integration tests.
pub const GIT_BIN_ENV: &str = "BONSAI_GIT_BIN";

/// Windows console-suppression flag (mirrors `ai/mod.rs`, `external.rs`).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Which rung of the ladder produced the path. `Fallback` means the ladder was
/// exhausted and we handed `Command` the bare name `git` so its own `NotFound`
/// error path still fires naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitBinSource {
    Override,
    Path,
    Registry,
    WellKnown,
    Fallback,
}

/// A resolved git executable + the rung that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBin {
    pub path: PathBuf,
    pub source: GitBinSource,
}

impl GitBin {
    /// `false` iff the ladder was exhausted (`source == Fallback`).
    pub fn found(&self) -> bool {
        self.source != GitBinSource::Fallback
    }

    /// Directory to prepend to a child's PATH (see [`git_command`]). `None` for
    /// `Path`/`Fallback` sources — those already resolved through the ambient
    /// PATH, so there is nothing to repair.
    pub fn bin_dir(&self) -> Option<&Path> {
        match self.source {
            GitBinSource::Path | GitBinSource::Fallback => None,
            _ => self.path.parent().filter(|p| !p.as_os_str().is_empty()),
        }
    }
}

// ---- injection seam -----------------------------------------------------------

/// Every environment interaction the ladder performs, injected so
/// [`resolve_ladder_for`] is a pure function under test. Precedent: `GitRunner`
/// (search.rs) / `GitExec` (exec.rs) / `CommandRunner` (external.rs).
///
/// Note there is deliberately NO "run this program" method: the ladder
/// structurally CANNOT execute a candidate (D4).
pub trait GitEnv {
    fn var(&self, key: &str) -> Option<String>;
    fn is_file(&self, p: &Path) -> bool;
    /// PATH + PATHEXT resolution; production delegates to
    /// [`crate::procutil::resolve_program`].
    fn resolve_on_path(&self, program: &str) -> Option<PathBuf>;
    /// Read ONE registry string value. `key` is a full path
    /// (`"HKCU\\SOFTWARE\\GitForWindows"`), `value` a value name
    /// (`"InstallPath"`). `None` on ANY failure. No-op on non-Windows.
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
}

/// Production implementation: real `std::env`, real filesystem, real `reg.exe`.
pub struct HostGitEnv;

impl GitEnv for HostGitEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn is_file(&self, p: &Path) -> bool {
        p.is_file()
    }

    /// Windows: reuse the PATHEXT-aware [`crate::procutil::resolve_program`]
    /// (it already returns a concrete, existence-checked file).
    #[cfg(windows)]
    fn resolve_on_path(&self, program: &str) -> Option<PathBuf> {
        crate::procutil::resolve_program(program)
            .ok()
            .filter(|p| p.is_file())
    }

    /// Non-Windows: `procutil::resolve_program` deliberately hands the bare name
    /// back for the OS to search, which cannot answer "is git on PATH?" — so do
    /// the PATH walk here. Without it the ladder could never distinguish a
    /// PATH hit from a total miss on macOS/Linux, and a git installed somewhere
    /// non-standard but ON PATH (nix, snap, asdf, Homebrew on Intel) would be
    /// mis-reported as missing.
    #[cfg(not(windows))]
    fn resolve_on_path(&self, program: &str) -> Option<PathBuf> {
        if program.contains('/') {
            // Same gate as the PATH walk below: an already-qualified program is
            // only a hit if it is a file we could actually execute. (Unreachable
            // today — callers pass a bare name — but the two branches must not
            // disagree about what "resolved" means.)
            let p = PathBuf::from(program);
            return (p.is_file() && is_executable(&p)).then_some(p);
        }
        let path_var = std::env::var_os("PATH")?;
        std::env::split_paths(&path_var)
            // An EMPTY PATH component (`/usr/bin:` — very common) would make
            // `dir.join("git")` the RELATIVE path `git`, resolved against the
            // app's cwd: a stray file named `git` inside the open repository
            // would then be treated as the git binary and spawned with
            // `.current_dir(repo)`. Skip empties, and require an absolute
            // candidate so no relative path can ever reach `Command`.
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| dir.join(program))
            .find(|cand| cand.is_absolute() && cand.is_file() && is_executable(cand))
    }

    #[cfg(windows)]
    fn registry_string(&self, key: &str, value: &str) -> Option<String> {
        // ABSOLUTE path to reg.exe on purpose: the entire premise of this module
        // is that PATH may be unusable, so a bare `reg` could resolve to nothing
        // (or, worse, to something else on a poisoned PATH).
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        let reg_exe = PathBuf::from(system_root).join("System32").join("reg.exe");
        if !reg_exe.is_file() {
            return None;
        }
        let mut cmd = Command::new(&reg_exe);
        cmd.args(["query", key, "/v", value])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        // ANY failure (spawn error, non-zero exit, unparseable output) => None.
        let out = cmd.output().ok()?;
        if !out.status.success() {
            return None;
        }
        parse_reg_query(&String::from_utf8_lossy(&out.stdout), value)
    }

    #[cfg(not(windows))]
    fn registry_string(&self, _key: &str, _value: &str) -> Option<String> {
        None
    }
}

/// `true` when `p` carries at least one execute bit. A non-executable file
/// named `git` earlier on PATH would otherwise be a false hit whose spawn fails
/// with `PermissionDenied` — which, since `git_missing()` would then be `false`,
/// surfaces as an opaque error instead of the honest "git not found" one.
#[cfg(not(windows))]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

/// Parse `reg.exe query <key> /v <value>` stdout for `value`'s string data.
/// Pure + defensive: a localized, truncated, or garbage block yields `None`
/// rather than a panic. A value NAME that is a prefix of another (`Install` vs
/// `InstallPath`) never cross-matches — the first whitespace token must be
/// exactly `value`.
fn parse_reg_query(stdout: &str, value: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        let mut tokens = trimmed.split_whitespace();
        if tokens.next() != Some(value) {
            continue;
        }
        for ty in ["REG_EXPAND_SZ", "REG_SZ"] {
            if let Some(idx) = trimmed.find(ty) {
                let data = trimmed[idx + ty.len()..].trim();
                if !data.is_empty() {
                    // A REG_EXPAND_SZ value is used as-is: Git for Windows
                    // writes a literal, already-expanded path.
                    return Some(data.to_string());
                }
            }
        }
    }
    None
}

// ---- the ladder ---------------------------------------------------------------

/// Git-for-Windows registry keys, in probe order. HKCU FIRST: a per-user
/// install is the one the user actually chose and is the exact failing case
/// this milestone fixes; a machine-wide install is the fallback. The
/// `WOW6432Node` variant covers a 32-bit install seen through the 64-bit
/// registry view (`HKCU\Software\...` is NOT subject to redirection, so it
/// needs no variant).
const WIN_REGISTRY_KEYS: [&str; 3] = [
    r"HKCU\SOFTWARE\GitForWindows",
    r"HKLM\SOFTWARE\GitForWindows",
    r"HKLM\SOFTWARE\WOW6432Node\GitForWindows",
];

/// `(env var, suffix)` well-known Windows install locations, in probe order.
const WIN_WELL_KNOWN: [(&str, &str); 4] = [
    ("LOCALAPPDATA", r"Programs\Git\cmd\git.exe"),
    ("ProgramFiles", r"Git\cmd\git.exe"),
    ("ProgramW6432", r"Git\cmd\git.exe"),
    ("ProgramFiles(x86)", r"Git\cmd\git.exe"),
];

/// Well-known Unix install locations, in probe order.
const UNIX_WELL_KNOWN: [&str; 3] = ["/usr/bin/git", "/usr/local/bin/git", "/opt/homebrew/bin/git"];

/// Join a Windows base directory and a backslash-relative suffix into ONE path,
/// independently of the HOST separator, so the Windows ladder behaves
/// identically under a Linux/macOS unit-test run.
fn win_join(base: &str, suffix: &str) -> PathBuf {
    let base = base.trim_end_matches(['\\', '/']);
    PathBuf::from(format!("{base}\\{suffix}"))
}

/// Run the candidate ladder against `env` for the HOST OS. Never panics, never
/// returns `Err`, never executes anything.
pub fn resolve_ladder(env: &dyn GitEnv) -> GitBin {
    resolve_ladder_for(env, TargetOs::host())
}

/// The ladder with an EXPLICIT OS branch (P49 pattern) so both platform
/// ladders execute in unit tests on a single machine.
///
/// Rungs are tried strictly in order and a later rung is reached ONLY when the
/// earlier candidate is absent — except rung 1, which short-circuits
/// unconditionally, and rung 2, which relies on `resolve_on_path`'s own
/// existence checks.
pub fn resolve_ladder_for(env: &dyn GitEnv, os: TargetOs) -> GitBin {
    // 1. explicit override — verbatim, NO validation (test seam + user escape
    //    hatch; a bad value must surface as an honest launch failure, not be
    //    silently swallowed by the next rung).
    if let Some(v) = env.var(GIT_BIN_ENV) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return GitBin {
                path: PathBuf::from(trimmed),
                source: GitBinSource::Override,
            };
        }
    }

    // 2. process PATH (fast path; PATHEXT-aware on Windows).
    if let Some(p) = env.resolve_on_path("git") {
        return GitBin {
            path: p,
            source: GitBinSource::Path,
        };
    }

    match os {
        TargetOs::Windows => {
            // 3. Git for Windows canonical registry key.
            for key in WIN_REGISTRY_KEYS {
                if let Some(install) = env.registry_string(key, "InstallPath") {
                    let cand = win_join(&install, r"cmd\git.exe");
                    if env.is_file(&cand) {
                        return GitBin {
                            path: cand,
                            source: GitBinSource::Registry,
                        };
                    }
                }
            }
            // 4. well-known install folders.
            for (var, suffix) in WIN_WELL_KNOWN {
                if let Some(base) = env.var(var) {
                    let cand = win_join(&base, suffix);
                    if env.is_file(&cand) {
                        return GitBin {
                            path: cand,
                            source: GitBinSource::WellKnown,
                        };
                    }
                }
            }
        }
        TargetOs::MacOs | TargetOs::Linux => {
            for cand in UNIX_WELL_KNOWN {
                let cand = PathBuf::from(cand);
                if env.is_file(&cand) {
                    return GitBin {
                        path: cand,
                        source: GitBinSource::WellKnown,
                    };
                }
            }
        }
    }

    // 5. bare name — lets the existing NotFound spawn error fire naturally.
    GitBin {
        path: PathBuf::from("git"),
        source: GitBinSource::Fallback,
    }
}

// ---- process-lifetime cache ---------------------------------------------------

/// Resolved once, then reused. A refreshable `RwLock` rather than a `OnceLock`
/// (D3) so "install Git, press Re-check" works without restarting the app; a
/// read costs one uncontended read lock + one `PathBuf` clone, negligible
/// against the process spawn that always follows.
static GIT_BIN: RwLock<Option<GitBin>> = RwLock::new(None);

/// Read the cache, recovering from poison (a panicking writer must not wedge
/// every future git spawn — the value is a plain struct, so poison is benign).
fn cached() -> Option<GitBin> {
    GIT_BIN
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .cloned()
}

/// Cached resolution. The first call runs the ladder; later calls read the
/// cache. Cheap enough to call per spawn. Never panics.
pub fn git_bin() -> GitBin {
    if let Some(bin) = cached() {
        return bin;
    }
    refresh_git_bin()
}

/// Re-run the ladder and replace the cache. Used by the preflight so a git
/// installed WHILE Bonsai runs is picked up without a restart. A benign race
/// (two threads resolving at once) simply resolves twice and stores the same
/// answer.
pub fn refresh_git_bin() -> GitBin {
    let resolved = resolve_ladder(&HostGitEnv);
    let mut w = GIT_BIN.write().unwrap_or_else(|p| p.into_inner());
    *w = Some(resolved.clone());
    resolved
}

/// Drop the cached resolution (unit tests only).
#[cfg(test)]
pub fn reset_git_bin_cache() {
    *GIT_BIN.write().unwrap_or_else(|p| p.into_inner()) = None;
}

/// `true` when the ladder was exhausted (`source == Fallback`) — i.e. no git
/// executable could be located at all. Cheap (cached).
pub fn git_missing() -> bool {
    !git_bin().found()
}

// ---- the spawn factory --------------------------------------------------------

/// THE production `git` [`Command`] factory. Sets the resolved program,
/// `CREATE_NO_WINDOW` on Windows, and — when the binary came from a NON-PATH
/// rung — prepends its directory to the CHILD's `PATH` so a hook script or
/// credential helper that itself calls `git` still works even though the
/// inherited PATH is broken.
///
/// Sets NO other env: call sites keep their own never-prompt hardening
/// (`GIT_TERMINAL_PROMPT=0`, askpass removal, `-c core.askpass=`), which their
/// existing tests assert.
pub fn git_command() -> Command {
    let bin = git_bin();
    let mut cmd = Command::new(&bin.path);
    if let Some(dir) = bin.bin_dir() {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        if let Some(joined) = child_path(dir, &existing) {
            cmd.env("PATH", joined);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Build the child's `PATH` value: `dir` prepended to `existing`, preserving
/// order and every existing entry. `None` when the join fails (an entry
/// containing the platform separator), in which case the caller leaves the
/// inherited `PATH` alone rather than shipping a corrupted one.
///
/// Extracted as a pure function purely so the prepend order and the
/// preservation of the inherited value can be asserted hermetically — the
/// end-to-end assertion through [`git_command`] is vacuous on any host where
/// git resolves from PATH (i.e. every dev box and CI runner), because
/// `bin_dir()` is `None` for the `Path` rung.
fn child_path(dir: &Path, existing: &std::ffi::OsStr) -> Option<std::ffi::OsString> {
    let mut dirs = vec![dir.to_path_buf()];
    // Empty components are dropped: on Unix an empty PATH entry means "the
    // current directory", which we must not hand to a child that we then run
    // with `.current_dir(repo)` (Windows' `split_paths` already filters them).
    dirs.extend(std::env::split_paths(existing).filter(|p| !p.as_os_str().is_empty()));
    std::env::join_paths(dirs).ok()
}

// ---- honest diagnostics -------------------------------------------------------

/// Classify a spawn/IO error from a `git` child. A `NotFound` (or a resolution
/// that already fell back to the bare name) is an honest
/// [`AppError::GitNotFound`] carrying [`git_not_found_message`]; anything else
/// stays an ordinary [`AppError::Git`] naming the subcommand.
pub fn spawn_error(subcmd: &str, e: &std::io::Error) -> AppError {
    if e.kind() == std::io::ErrorKind::NotFound || git_missing() {
        return AppError::GitNotFound(git_not_found_message());
    }
    AppError::Git(format!("failed to run `git {subcmd}`: {e}"))
}

/// Platform-branched user-facing copy for "no runnable git". Deliberately names
/// the rungs that were checked, denies the authentication reading, and gives
/// three concrete fixes.
#[cfg(windows)]
pub fn git_not_found_message() -> String {
    "Git is not available. Bonsai could not find a runnable `git` executable — it checked \
     BONSAI_GIT_BIN, PATH, the Git for Windows registry key, and the standard install \
     folders. This is NOT an authentication failure: your saved credentials were never \
     consulted, because Bonsai could not start the credential helper. This affects HTTPS \
     remotes (which resolve credentials through Git's credential helper) plus commit search \
     and signing; SSH remotes using an ssh-agent are unaffected. Fix: quit Bonsai and \
     relaunch it from the Start menu (an in-app update can \
     leave the app running with an incomplete PATH), or install Git for Windows, or set \
     BONSAI_GIT_BIN to the full path of git.exe and restart."
        .to_string()
}

/// See the Windows variant.
#[cfg(not(windows))]
pub fn git_not_found_message() -> String {
    "Git is not available. Bonsai could not find a runnable `git` executable — it checked \
     BONSAI_GIT_BIN, PATH, and the standard install locations (/usr/bin, /usr/local/bin, \
     /opt/homebrew/bin). This is NOT an authentication failure: your saved credentials were \
     never consulted, because Bonsai could not start the credential helper. This affects \
     HTTPS remotes (which resolve credentials through Git's credential helper) plus commit \
     search and signing; SSH remotes using an ssh-agent are unaffected. Fix: install Git, or \
     set BONSAI_GIT_BIN to the full path of the git binary and restart Bonsai."
        .to_string()
}

#[path = "gitbin_preflight.rs"]
mod preflight;
pub use preflight::{check_availability, GitAvailability};

#[cfg(test)]
#[path = "gitbin_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gitbin_diag_tests.rs"]
mod diag_tests;
