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
mod tests {
    use super::*;
    use crate::git::commit::create_commit;
    use crate::git::exec::SpawnGitExec;
    use crate::git::stage::stage_paths;
    use std::process::Command;

    // ---- pure builders / toggle (git-free) --------------------------------

    #[test]
    fn build_hook_run_args_shapes() {
        // No args, no stdin. `--ignore-missing` is ALWAYS present (F-A4-1).
        assert_eq!(
            build_hook_run_args(HookName::PreCommit, &[], None),
            vec!["hook", "run", "--ignore-missing", "pre-commit"]
        );
        // Args after `--`, no stdin (commit-msg's message-file arg).
        assert_eq!(
            build_hook_run_args(HookName::CommitMsg, &[".git/COMMIT_EDITMSG".to_string()], None),
            vec!["hook", "run", "--ignore-missing", "commit-msg", "--", ".git/COMMIT_EDITMSG"]
        );
        // --to-stdin present, plus args after `--` (pre-push shape).
        let args = vec!["origin".to_string(), "https://x/y.git".to_string()];
        assert_eq!(
            build_hook_run_args(HookName::PrePush, &args, Some(Path::new("/tmp/refs"))),
            vec![
                "hook",
                "run",
                "--ignore-missing",
                "pre-push",
                "--to-stdin=/tmp/refs",
                "--",
                "origin",
                "https://x/y.git"
            ]
        );
        // --to-stdin present, NO trailing args ⇒ no dangling `--`.
        assert_eq!(
            build_hook_run_args(HookName::PrePush, &[], Some(Path::new("/tmp/refs"))),
            vec!["hook", "run", "--ignore-missing", "pre-push", "--to-stdin=/tmp/refs"]
        );
    }

    /// Audit #2 §3.3: `warning` distinguishes every failure shape from the
    /// silent "no hook installed" no-op.
    #[test]
    fn hook_run_info_warning_shapes() {
        // Success (ran) and the absent-hook no-op ⇒ no warning.
        let ok = HookRunInfo { ran: true, success: true, output: "out".to_string() };
        let absent = HookRunInfo { ran: false, success: true, output: String::new() };
        assert_eq!(ok.warning(HookName::PostCommit), None);
        assert_eq!(absent.warning(HookName::PostCommit), None);
        // Ran but exited non-zero ⇒ named failure with the hook's own output.
        let failed = HookRunInfo { ran: true, success: false, output: "boom".to_string() };
        assert_eq!(
            failed.warning(HookName::PostCommit).as_deref(),
            Some("post-commit hook failed:\nboom")
        );
        // Ran, non-zero, silent ⇒ still visibly a failure.
        let silent = HookRunInfo { ran: true, success: false, output: String::new() };
        assert_eq!(
            silent.warning(HookName::PostCommit).as_deref(),
            Some("post-commit hook failed:\n(no output)")
        );
        // Spawn failure ⇒ the carried "failed to run …" message passes through.
        let spawn = HookRunInfo {
            ran: false,
            success: false,
            output: "failed to run the post-commit hook: exec failed".to_string(),
        };
        assert_eq!(
            spawn.warning(HookName::PostCommit).as_deref(),
            Some("failed to run the post-commit hook: exec failed")
        );
    }

    #[test]
    fn git_infra_failure_classifier_is_narrow() {
        // git's own pre-hook failures ⇒ infra.
        assert!(is_git_infra_failure("error: cannot find a hook named pre-commit"));
        assert!(is_git_infra_failure("fatal: cannot run .husky/pre-commit: No such file"));
        assert!(is_git_infra_failure("error: cannot spawn .git/hooks/pre-commit: exec failed"));
        assert!(is_git_infra_failure("fatal: not a git repository (or any of the parent directories)"));
        // A hook's OWN output — even git-flavored — stays a rejection.
        assert!(!is_git_infra_failure("lint failed: bad code"));
        assert!(!is_git_infra_failure("error: your commit message is bad"));
        assert!(!is_git_infra_failure("fatal: pre-commit checks failed"));
        assert!(!is_git_infra_failure("hook output\nerror: cannot find a hook named x"));
        assert!(!is_git_infra_failure(""));
    }

    #[test]
    fn hook_name_as_str() {
        assert_eq!(HookName::PreCommit.as_str(), "pre-commit");
        assert_eq!(HookName::CommitMsg.as_str(), "commit-msg");
        assert_eq!(HookName::PostCommit.as_str(), "post-commit");
        assert_eq!(HookName::PrePush.as_str(), "pre-push");
    }

    #[test]
    fn hooks_enabled_truth_table() {
        // skip=true always disables, regardless of the key.
        let (_d0, on) = cfg_with(&[("bonsai.runHooks", "true")]);
        let (_d1, off) = cfg_with(&[("bonsai.runHooks", "false")]);
        let (_d2, unset) = cfg_with(&[]);
        assert!(!hooks_enabled(&on, true));
        assert!(!hooks_enabled(&off, true));
        assert!(!hooks_enabled(&unset, true));
        // skip=false follows the key; unset ⇒ true (git default).
        assert!(hooks_enabled(&on, false));
        assert!(!hooks_enabled(&off, false));
        assert!(hooks_enabled(&unset, false));
    }

    fn cfg_with(entries: &[(&str, &str)]) -> (tempfile::TempDir, git2::Config) {
        let dir = crate::testutil::scratch_dir();
        let file = dir.path().join("gitconfig");
        std::fs::write(&file, "").expect("create config");
        let mut cfg = git2::Config::open(&file).expect("open config");
        for (k, v) in entries {
            cfg.set_str(k, v).expect("set");
        }
        (dir, cfg)
    }

    // ---- oracle (real hooks; git ≥ 2.36 only) -----------------------------

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    /// True when the git on PATH is ≥ `(major, minor)` — `git hook run` needs 2.36.
    fn git_version_at_least(major: u32, minor: u32) -> bool {
        let out = match Command::new("git").arg("--version").output() {
            Ok(o) if o.status.success() => o,
            _ => return false,
        };
        let s = String::from_utf8_lossy(&out.stdout);
        let ver = s.split_whitespace().nth(2).unwrap_or("");
        let mut it = ver.split('.');
        let maj: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let min: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        maj > major || (maj == major && min >= minor)
    }

    /// Skip guard for the oracle tests. Prints a note and returns false when the
    /// git on PATH is missing or older than 2.36.
    fn oracle_ready() -> bool {
        if !have_git() {
            eprintln!("skipping hooks oracle: `git` not found");
            return false;
        }
        if !git_version_at_least(2, 36) {
            eprintln!("skipping hooks oracle: git < 2.36 (no `git hook run`)");
            return false;
        }
        true
    }

    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Hook Tester").expect("name");
        cfg.set_str("user.email", "hooks@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    fn hooks_dir(repo: &git2::Repository) -> PathBuf {
        repo.commondir().join("hooks")
    }

    /// Write an executable `#!/bin/sh` hook with LF endings (git's bundled sh
    /// parses the shebang on Windows too — the point of A-D1).
    fn write_hook(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).expect("mkdir hooks");
        let path = dir.join(name);
        std::fs::write(&path, body.replace("\r\n", "\n")).expect("write hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod");
        }
    }

    fn head_message(repo: &git2::Repository) -> String {
        repo.head()
            .expect("head")
            .peel_to_commit()
            .expect("commit")
            .message()
            .unwrap_or("")
            .to_string()
    }

    /// pre-commit `exit 1` BLOCKS the commit with its output; HEAD unchanged.
    #[test]
    fn pre_commit_fail_blocks_with_output() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "pre-commit",
            "#!/bin/sh\necho \"lint failed: bad code\" >&2\nexit 1\n",
        );
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        let err = create_commit(dir.path(), "subject", None, false).expect_err("must block");
        match err {
            AppError::HookRejected(m) => {
                assert!(m.contains("pre-commit hook failed:"), "prefix: {m}");
                assert!(m.contains("lint failed: bad code"), "hook output: {m}");
            }
            other => panic!("expected HookRejected, got {other:?}"),
        }
        assert!(repo.head().is_err(), "no commit landed (HEAD still unborn)");
    }

    /// pre-commit `exit 0` ALLOWS the commit.
    #[test]
    fn pre_commit_pass_allows() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(&hooks_dir(&repo), "pre-commit", "#!/bin/sh\nexit 0\n");
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        create_commit(dir.path(), "ok subject", None, false).expect("commit");
        assert_eq!(head_message(&repo), "ok subject\n");
    }

    /// commit-msg that appends a trailer REWRITES the committed message.
    #[test]
    fn commit_msg_rewrites_message() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "commit-msg",
            "#!/bin/sh\nprintf '\\nSigned-off-by: Hook <hook@example.com>\\n' >> \"$1\"\nexit 0\n",
        );
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        create_commit(dir.path(), "subject", None, false).expect("commit");
        let msg = head_message(&repo);
        assert!(msg.starts_with("subject"), "keeps subject: {msg}");
        assert!(
            msg.contains("Signed-off-by: Hook <hook@example.com>"),
            "commit-msg trailer must be in the committed message: {msg}"
        );
    }

    /// commit-msg `exit 1` BLOCKS; no commit lands.
    #[test]
    fn commit_msg_fail_blocks() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "commit-msg",
            "#!/bin/sh\necho \"bad message\" >&2\nexit 1\n",
        );
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        let err = create_commit(dir.path(), "subject", None, false).expect_err("must block");
        assert!(matches!(err, AppError::HookRejected(m) if m.contains("commit-msg hook failed:")));
        assert!(repo.head().is_err(), "no commit landed");
    }

    /// post-commit is NON-blocking: an `exit 1` post-commit still lets the commit
    /// land, and `run_hook_nonblocking` captures `success: false`.
    #[test]
    fn post_commit_non_blocking() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "post-commit",
            "#!/bin/sh\necho \"post ran\"\nexit 1\n",
        );
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        // The commit must succeed despite the failing post-commit.
        create_commit(dir.path(), "subject", None, false).expect("commit lands");
        assert_eq!(head_message(&repo), "subject\n");

        // Direct capture: ran, but not successful.
        let info = run_hook_nonblocking(&SpawnGitExec, dir.path(), HookName::PostCommit, &[]);
        assert!(info.ran, "post-commit ran");
        assert!(!info.success, "post-commit reported failure");
        assert!(info.output.contains("post ran"), "captured output: {}", info.output);
    }

    /// `core.hooksPath` pointing at a sibling dir is honoured (proves discovery
    /// is git's, not a hardcoded `.git/hooks`).
    #[test]
    fn core_hooks_path_is_honored() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        let alt = dir.path().join("myhooks");
        write_hook(&alt, "pre-commit", "#!/bin/sh\necho \"alt hook\" >&2\nexit 1\n");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("core.hooksPath", alt.to_str().expect("utf8"))
                .expect("set hooksPath");
        }
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        let err = create_commit(dir.path(), "subject", None, false).expect_err("blocked by alt");
        assert!(matches!(err, AppError::HookRejected(m) if m.contains("alt hook")));
        assert!(repo.head().is_err(), "no commit landed");
    }

    /// Opt-out: a failing pre-commit is NOT run when `bonsai.runHooks=false`,
    /// nor when `skip_hooks=true` — the commit succeeds either way.
    #[test]
    fn opt_out_skips_hooks() {
        if !oracle_ready() {
            return;
        }
        // (a) bonsai.runHooks=false
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_bool("bonsai.runHooks", false).expect("disable");
        }
        write_hook(&hooks_dir(&repo), "pre-commit", "#!/bin/sh\nexit 1\n");
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        create_commit(dir.path(), "cfg opt-out", None, false).expect("commit despite hook");
        assert_eq!(head_message(&repo), "cfg opt-out\n");

        // (b) skip_hooks=true (bonsai.runHooks left at default true)
        let dir2 = crate::testutil::scratch_dir();
        let repo2 = init_repo(dir2.path());
        write_hook(&hooks_dir(&repo2), "pre-commit", "#!/bin/sh\nexit 1\n");
        std::fs::write(dir2.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir2.path(), &["a.txt".to_string()]).expect("stage");
        create_commit(dir2.path(), "skip opt-out", None, true).expect("commit despite hook");
        assert_eq!(head_message(&repo2), "skip opt-out\n");
    }

    /// A pre-commit that writes + `git add`s a file ⇒ the reloaded index
    /// (`index.read(true)`) includes it in the committed tree.
    #[test]
    fn pre_commit_restage_is_picked_up() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "pre-commit",
            "#!/bin/sh\necho generated > generated.txt\ngit add generated.txt\nexit 0\n",
        );
        std::fs::write(dir.path().join("a.txt"), "x\n").expect("write");
        stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");

        create_commit(dir.path(), "with generated", None, false).expect("commit");
        let tree = repo
            .head()
            .expect("head")
            .peel_to_commit()
            .expect("commit")
            .tree()
            .expect("tree");
        assert!(
            tree.get_name("generated.txt").is_some(),
            "hook-staged file must be in the committed tree"
        );
        assert!(tree.get_name("a.txt").is_some(), "originally-staged file present");
    }

    /// With hooks enabled but NO hook file present, `run_hook` must NOT spawn git
    /// (pure-git2 no-hook path). A panicking `GitExec` proves it.
    #[test]
    fn no_hook_file_does_not_spawn() {
        struct PanicExec;
        impl GitExec for PanicExec {
            fn exec(
                &self,
                _a: &[&str],
                _c: &Path,
                _s: Option<&[u8]>,
                _e: &[(&str, &str)],
            ) -> Result<crate::git::exec::GitOutput, AppError> {
                panic!("run_hook must not spawn git when no hook file exists");
            }
        }
        let dir = crate::testutil::scratch_dir();
        init_repo(dir.path());
        run_hook(&PanicExec, dir.path(), HookName::PreCommit, &[], None).expect("no-op ok");
        let info = run_hook_nonblocking(&PanicExec, dir.path(), HookName::PostCommit, &[]);
        assert!(!info.ran && info.success, "post-commit no-op, not spawned");
    }

    // ---- detection: repo_has_runnable_hooks (git-binary-free) --------------

    /// A fresh repo with no hooks installed ⇒ nothing to disclose.
    #[test]
    fn repo_has_runnable_hooks_none_false() {
        let dir = crate::testutil::scratch_dir();
        init_repo(dir.path());
        assert!(!repo_has_runnable_hooks(dir.path()));
    }

    /// An executable pre-commit ⇒ detected (present + exec bit on unix).
    #[test]
    fn repo_has_runnable_hooks_exec_pre_commit_true() {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(&hooks_dir(&repo), "pre-commit", "#!/bin/sh\nexit 0\n");
        assert!(repo_has_runnable_hooks(dir.path()));
    }

    /// unix: a present-but-NON-executable hook is NOT runnable — git skips it, so
    /// we must not disclose for it. (No exec bit on Windows, so unix-only.)
    #[cfg(unix)]
    #[test]
    fn repo_has_runnable_hooks_non_exec_false() {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        let hooks = hooks_dir(&repo);
        std::fs::create_dir_all(&hooks).expect("mkdir hooks");
        // Plain write ⇒ 0o644 (no execute bit).
        std::fs::write(hooks.join("pre-commit"), "#!/bin/sh\nexit 0\n").expect("write hook");
        assert!(!repo_has_runnable_hooks(dir.path()));
    }

    /// `core.hooksPath` is honored — a runnable hook under the configured dir is
    /// detected (proves discovery is git's, not a hardcoded `.git/hooks`).
    #[test]
    fn repo_has_runnable_hooks_honors_core_hooks_path() {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        let alt = dir.path().join("myhooks");
        write_hook(&alt, "pre-commit", "#!/bin/sh\nexit 0\n");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("core.hooksPath", alt.to_str().expect("utf8"))
                .expect("set hooksPath");
        }
        assert!(repo_has_runnable_hooks(dir.path()));
    }

    /// Detection covers ALL disclosable hooks, not just the commit ones: a repo
    /// with only an executable pre-push is still disclosed.
    #[test]
    fn repo_has_runnable_hooks_pre_push_only_true() {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(&hooks_dir(&repo), "pre-push", "#!/bin/sh\nexit 0\n");
        assert!(repo_has_runnable_hooks(dir.path()));
    }

}
