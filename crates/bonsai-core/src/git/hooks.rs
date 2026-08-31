//! Git hook execution (P59a).
//!
//! Bonsai mutates the repo through git2 (libgit2), which does NOT fire git
//! hooks. This module runs the relevant hooks around commit / amend / merge
//! (and, in P59a-2, push) through the git binary's own `git hook run <name>`
//! (Git ≥ 2.36) — reusing the shared [`GitExec`] seam so hook DISCOVERY, the
//! executable-bit check, and (the real trap) the **Windows shell-script path**
//! are all git's job, never re-derived here (A-D1).
//!
//! # Trust invariant
//! A BLOCKING hook that exits non-zero maps to [`AppError::HookRejected`]
//! carrying the hook's own output — **never a silent success**. If a hook file
//! exists but `git hook run` is unavailable (Git < 2.36, surfaced as an
//! unknown-subcommand error) we surface a one-time [`AppError::Git`] rather than
//! committing unverified (A-D6).
//!
//! # Absent hooks (F-A4-1)
//! Bare `git hook run <name>` EXITS 1 for an absent hook ("cannot find a
//! hook named …") — it is NOT a no-op. Before the F-A4-1 fix that exit was
//! misread as a hook rejection, so a Husky-style repo (`core.hooksPath`
//! set) missing any one of pre-commit/commit-msg/pre-push had EVERY
//! commit/amend/merge/push blocked. Two layers fix it:
//! - the argv always carries `--ignore-missing` (same Git ≥ 2.36 floor as
//!   the `hook run` subcommand itself), so an absent — or, on unix,
//!   present-but-non-executable — hook is a clean exit-0 no-op, exactly as
//!   `git commit` treats it;
//! - the existence pre-check below skips the spawn entirely when the hook
//!   provably cannot run.
//!
//! # Why we pre-check hook-file existence (behaviour-preserving refinement)
//! Calling `git hook run` unconditionally whenever hooks are enabled would
//! make EVERY commit spawn three `git` processes and would make
//! `create_commit` hard-depend on the git binary even for a repo with no
//! hooks. Instead we first resolve whether a hook COULD run and skip the
//! spawn when it provably cannot:
//! - if `core.hooksPath` is set, we resolve it the way git does (tilde
//!   expansion via the config layer; a relative path is relative to the
//!   worktree root, where git chdirs before running hooks — githooks(5))
//!   and skip when `<hooksPath>/<name>` is absent; if the value cannot be
//!   read/expanded we delegate to git rather than guess;
//! - otherwise the sole location git consults is `<commondir>/hooks/<name>` —
//!   if that file is absent, git would run nothing, so we skip the spawn.
//!
//! A wrong "run" decision is harmless (`--ignore-missing` makes git no-op);
//! a "skip" is only taken when git's own resolution rules find no file, so
//! the trust invariant — never skip a hook git would run — is intact, while
//! a no-hook commit stays pure-git2 and git-less-tolerant.
//!
//! # Timeouts (deliberate git parity)
//! Hooks run with NO timeout — a hung hook hangs the operation, exactly as
//! it hangs `git commit` itself. Bonsai adds no watchdog; the user's
//! recourse is the same as with git (kill the hook / fix the script).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::AppError;
use crate::git::exec::GitExec;
// P87: the streaming hook variants live in `hooks_stream` (file-size split);
// re-exported so `crate::git::hooks::run_hook_streaming` keeps resolving for the
// commit/merge/remote cores.
pub use crate::git::hooks_stream::{run_hook_nonblocking_streaming, run_hook_streaming};

/// The hooks Bonsai runs around its git2 mutations. `PrePush` is wired in
/// P59a-2; the three commit hooks are P59a-core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName {
    PreCommit,
    CommitMsg,
    PostCommit,
    PrePush,
}

impl HookName {
    /// Canonical git hook filename (also the `git hook run` sub-argument).
    pub fn as_str(&self) -> &'static str {
        match self {
            HookName::PreCommit => "pre-commit",
            HookName::CommitMsg => "commit-msg",
            HookName::PostCommit => "post-commit",
            HookName::PrePush => "pre-push",
        }
    }
}

/// Outcome of a NON-blocking hook (post-commit): whether it actually ran, its
/// exit success, and captured combined output (for optional info surfacing). A
/// non-zero exit is NEVER an error here — git ignores post-commit's status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunInfo {
    pub ran: bool,
    pub success: bool,
    pub output: String,
}

impl HookRunInfo {
    /// The user-facing warning for a hook that failed to run or exited
    /// non-zero; `None` on success (including the "no hook installed" no-op).
    /// Audit #2 §3.3: a spawn failure used to map to `ran:false, output:""` —
    /// indistinguishable from an absent hook — so the failure was invisible.
    pub fn warning(&self, hook: HookName) -> Option<String> {
        if self.success {
            return None;
        }
        if self.ran {
            let body = if self.output.is_empty() {
                "(no output)"
            } else {
                self.output.as_str()
            };
            Some(format!("{} hook failed:\n{body}", hook.as_str()))
        } else {
            // `output` already carries the full "failed to run …" message.
            Some(self.output.clone())
        }
    }
}

/// Effective hook toggle: `!skip && bonsai.runHooks` (default true; a missing
/// key ⇒ true, git's own default). `cfg` is the repo's merged config snapshot.
pub fn hooks_enabled(cfg: &git2::Config, skip: bool) -> bool {
    if skip {
        return false;
    }
    cfg.get_bool("bonsai.runHooks").unwrap_or(true)
}

/// Run one BLOCKING hook via
/// `git hook run --ignore-missing <name> [--to-stdin=<file>] [-- <args>]`.
///
/// Exit 0 ⇒ `Ok(())`. An ABSENT hook ⇒ `Ok(())` — via the pre-check skip
/// when resolvable, else via `--ignore-missing` (bare `git hook run` would
/// exit 1 for it; F-A4-1). A non-zero exit from the HOOK ⇒
/// [`AppError::HookRejected`] (`"<name> hook failed:\n" + stdout + stderr`).
/// A git-infrastructure failure (git's own `fatal:`/`error:` before the hook
/// ran, F-A4-5) or a Git < 2.36 unknown `hook` subcommand WITH a hook file
/// present ⇒ [`AppError::Git`] (A-D6). A spawn / I/O failure ⇒
/// [`AppError::Git`]. NEVER panics.
///
/// No timeout (git parity — see the module doc). `args` are passed as `$1…`
/// to the hook; callers build them from paths via lossy UTF-8 conversion, so
/// a theoretical non-UTF-8 repo path would reach the hook mangled (the hook
/// itself still runs; the repo paths Bonsai manages are UTF-8-checked at
/// open time).
pub fn run_hook(
    exec: &dyn GitExec,
    workdir: &Path,
    hook: HookName,
    args: &[String],
    stdin: Option<&[u8]>,
) -> Result<(), AppError> {
    run_hook_streaming(exec, workdir, hook, args, stdin, None)
}

/// Run a NON-blocking hook (post-commit): capture output, IGNORE a non-zero exit
/// (git semantics). NEVER errors on a hook failure — the caller has already
/// committed; a failing post-commit only informs. A Git < 2.36 unknown
/// subcommand or a spawn failure is reported as `ran: false`.
pub fn run_hook_nonblocking(
    exec: &dyn GitExec,
    workdir: &Path,
    hook: HookName,
    args: &[String],
) -> HookRunInfo {
    run_hook_nonblocking_streaming(exec, workdir, hook, args, None)
}

/// Pure `git hook run` argv builder (A2):
/// `["hook","run","--ignore-missing",<name>,("--to-stdin=<path>")?,("--")?,<args>…]`.
/// `--ignore-missing` is ALWAYS present (F-A4-1): without it an absent hook
/// exits 1 and would block the operation. The `--` separator is emitted ONLY
/// when there are trailing args, so git never sees a dangling `--`.
pub(crate) fn build_hook_run_args(hook: HookName, args: &[String], stdin_path: Option<&Path>) -> Vec<String> {
    let mut out = vec![
        "hook".to_string(),
        "run".to_string(),
        "--ignore-missing".to_string(),
        hook.as_str().to_string(),
    ];
    if let Some(p) = stdin_path {
        out.push(format!("--to-stdin={}", p.display()));
    }
    if !args.is_empty() {
        out.push("--".to_string());
        out.extend(args.iter().cloned());
    }
    out
}

/// Whether `git hook run` should be spawned for `hook`. See the module-level
/// "Why we pre-check" note for the trust argument.
pub(crate) enum HookPlan {
    Skip,
    Run,
}

/// The on-disk path git would consult for `hook` — `core.hooksPath` if set &
/// non-empty (`get_path` applies the config layer's tilde expansion; a relative
/// value is relative to the worktree root, where git chdirs before running hooks
/// — githooks(5) — which is exactly `workdir` for every caller), else the
/// unambiguous default `<commondir>/hooks/<name>` (commondir so linked worktrees
/// resolve to the shared hooks dir, matching git). `None` iff introspection
/// fails (repo won't open / config unreadable). Does NOT check existence — the
/// single discovery path shared by [`plan_hook`] and [`repo_has_runnable_hooks`]
/// (A-D1: discovery is git's job, single-sourced).
fn hook_file_path(workdir: &Path, hook: HookName) -> Option<PathBuf> {
    let repo = open_repo(workdir).ok()?;
    let cfg = repo.config().and_then(|mut c| c.snapshot()).ok()?;
    if let Ok(p) = cfg.get_path("core.hooksPath") {
        if !p.as_os_str().is_empty() {
            let base = if p.is_absolute() { p } else { workdir.join(p) };
            return Some(base.join(hook.as_str()));
        }
    }
    Some(repo.commondir().join("hooks").join(hook.as_str()))
}

pub(crate) fn plan_hook(workdir: &Path, hook: HookName) -> HookPlan {
    // Skip only when git's own resolution finds no file; a wrong "run" (incl.
    // the introspection-failure `None` ⇒ delegate to git) is harmless — the
    // argv carries `--ignore-missing`.
    hook_file_path(workdir, hook).map_or(HookPlan::Run, |p| {
        if p.is_file() {
            HookPlan::Run
        } else {
            HookPlan::Skip
        }
    })
}

/// The hooks a repo could fire during a Bonsai commit / amend / merge-commit /
/// push, checked by [`repo_has_runnable_hooks`].
const DISCLOSABLE_HOOKS: [HookName; 4] = [
    HookName::PreCommit,
    HookName::CommitMsg,
    HookName::PostCommit,
    HookName::PrePush,
];

/// True iff the repo has ≥1 hook Bonsai would actually run — for the one-time
/// per-repo disclosure, NOT the execution path. PRECISE (unlike [`plan_hook`],
/// which over-runs harmlessly under `--ignore-missing`): a hook counts only when
/// it is present AND, on unix, executable (`mode & 0o111 != 0`); on windows,
/// present is enough (git is shebang-driven, no exec bit) — so we disclose only
/// for repos git itself would actually run a hook in. Introspection failure ⇒
/// `false` (nothing to disclose we can prove). Blocking (git2 + fs) → callers
/// wrap in `spawn_blocking`. NEVER panics.
pub fn repo_has_runnable_hooks(workdir: &Path) -> bool {
    DISCLOSABLE_HOOKS
        .iter()
        .any(|&hook| hook_file_path(workdir, hook).is_some_and(|p| is_runnable_hook_file(&p)))
}

/// unix: a hook counts only if it is a file with any execute bit set — exactly
/// what git checks before running a `.git/hooks` script.
#[cfg(unix)]
fn is_runnable_hook_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(m) => m.is_file() && (m.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

/// windows: no exec bit — git runs a present hook via its shebang, so presence
/// as a file is enough.
#[cfg(not(unix))]
fn is_runnable_hook_file(path: &Path) -> bool {
    path.is_file()
}

/// git's message for an unknown subcommand (Git < 2.36 has no `hook`):
/// `git: 'hook' is not a git command.` — stable across git versions.
pub(crate) fn is_unknown_subcommand(stderr: &str) -> bool {
    stderr.contains("is not a git command")
}

/// Whether a non-zero `git hook run` exit is GIT's own failure rather than a
/// hook rejection (F-A4-5). Deliberately NARROW — a hook's stderr may itself
/// contain `error:`/`fatal:` lines (e.g. it runs git internally), so only the
/// specific messages `builtin/hook.c` / run-command emit BEFORE the hook runs
/// are matched, and only at the START of stderr: "cannot find a hook named"
/// (near-unreachable now that `--ignore-missing` is passed — kept as a
/// belt-and-braces classifier), a spawn failure, and not-a-repository.
pub(crate) fn is_git_infra_failure(stderr: &str) -> bool {
    let first = stderr.lines().next().unwrap_or("");
    let msg = first
        .strip_prefix("error: ")
        .or_else(|| first.strip_prefix("fatal: "))
        .unwrap_or("");
    msg.starts_with("cannot find a hook named")
        || msg.starts_with("cannot run ")
        || msg.starts_with("cannot spawn ")
        || msg.starts_with("not a git repository")
}

/// Combined hook output for the error/info body: stdout then stderr, each
/// trailing-trimmed, joined with a newline; empty parts skipped.
pub(crate) fn combined_output(stdout: &str, stderr: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let o = stdout.trim_end();
    let e = stderr.trim_end();
    if !o.is_empty() {
        parts.push(o);
    }
    if !e.is_empty() {
        parts.push(e);
    }
    parts.join("\n")
}

/// Open the repo at `workdir` with `NO_SEARCH` (same recipe as the rest of git/).
fn open_repo(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// A temp file holding hook stdin bytes; deletes itself on drop. Dependency-free
/// (tempfile is a dev-only dep) so it can live in non-test code — used by
/// pre-push (P59a-2); the commit hooks pass no stdin.
pub(crate) struct TempStdin {
    path: PathBuf,
}

impl TempStdin {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempStdin {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_stdin_tempfile(hook: HookName, bytes: &[u8]) -> Result<TempStdin, AppError> {
    use std::io::Write;
    let mut path = std::env::temp_dir();
    path.push(format!(
        "bonsai-{}-stdin-{}-{}.txt",
        hook.as_str(),
        std::process::id(),
        NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = std::fs::File::create(&path)
        .map_err(|e| AppError::Io(format!("failed to create hook stdin temp file: {e}")))?;
    f.write_all(bytes)
        .map_err(|e| AppError::Io(format!("failed to write hook stdin: {e}")))?;
    Ok(TempStdin { path })
}

#[cfg(test)]
mod tests;
