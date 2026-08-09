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
//! # Why we pre-check hook-file existence (behaviour-preserving refinement)
//! The contract's normative flow calls `git hook run` unconditionally whenever
//! hooks are enabled, relying on git's "absent hook ⇒ exit 0" no-op. Doing that
//! would make EVERY commit spawn three `git` processes and would make
//! `create_commit` hard-depend on the git binary even for a repo with no hooks.
//! Instead we first resolve whether a hook COULD run and skip the spawn when it
//! provably cannot:
//! - if `core.hooksPath` is set (rare) ⇒ we do NOT second-guess git's
//!   resolution and always delegate to `git hook run`;
//! - otherwise the sole location git consults is `<commondir>/hooks/<name>` —
//!   if that file is absent, git would run nothing, so we skip the spawn.
//!
//! This can NEVER skip a hook git would run (git resolves hooks only from
//! `core.hooksPath` or `<commondir>/hooks`), so the trust invariant is intact,
//! while a no-hook commit stays pure-git2 and git-less-tolerant.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::AppError;
use crate::git::exec::GitExec;

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

/// Effective hook toggle: `!skip && bonsai.runHooks` (default true; a missing
/// key ⇒ true, git's own default). `cfg` is the repo's merged config snapshot.
pub fn hooks_enabled(cfg: &git2::Config, skip: bool) -> bool {
    if skip {
        return false;
    }
    cfg.get_bool("bonsai.runHooks").unwrap_or(true)
}

/// Run one BLOCKING hook via `git hook run <name> [--to-stdin=<file>] [-- <args>]`.
/// Exit 0 or hook-absent ⇒ `Ok(())`. Non-zero ⇒ [`AppError::HookRejected`]
/// (`"<name> hook failed:\n" + stdout + stderr`). A Git < 2.36 unknown
/// `hook` subcommand WITH a hook file present ⇒ [`AppError::Git`] (A-D6). A
/// spawn / I/O failure ⇒ [`AppError::Git`]. NEVER panics.
pub fn run_hook(
    exec: &dyn GitExec,
    workdir: &Path,
    hook: HookName,
    args: &[String],
    stdin: Option<&[u8]>,
) -> Result<(), AppError> {
    if matches!(plan_hook(workdir, hook), HookPlan::Skip) {
        return Ok(()); // no hook file, no core.hooksPath ⇒ nothing to run
    }
    // `git hook run` does not forward its own stdin to the hook; a hook that
    // reads stdin (pre-push) gets it via a temp file passed as --to-stdin. The
    // handle deletes the file on drop, AFTER exec completes.
    let stdin_tmp = match stdin {
        Some(bytes) => Some(write_stdin_tempfile(hook, bytes)?),
        None => None,
    };
    let argv = build_hook_run_args(hook, args, stdin_tmp.as_ref().map(TempStdin::path));
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = exec.exec(&argv_ref, workdir, None, &[])?;
    if out.success {
        return Ok(());
    }
    if is_unknown_subcommand(&out.stderr) {
        // Git < 2.36 has no `hook` subcommand. `plan_hook` already confirmed a
        // hook exists (else Skip), so refuse rather than bypass it silently.
        return Err(AppError::Git(format!(
            "hook execution needs Git ≥ 2.36 (this git cannot run the '{}' hook). \
             Upgrade git, or disable hooks (unset bonsai.runHooks / use Skip hooks).",
            hook.as_str()
        )));
    }
    Err(AppError::HookRejected(format!(
        "{} hook failed:\n{}",
        hook.as_str(),
        combined_output(&out.stdout, &out.stderr)
    )))
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
    if matches!(plan_hook(workdir, hook), HookPlan::Skip) {
        return HookRunInfo { ran: false, success: true, output: String::new() };
    }
    let argv = build_hook_run_args(hook, args, None);
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    match exec.exec(&argv_ref, workdir, None, &[]) {
        Ok(out) if out.success => HookRunInfo {
            ran: true,
            success: true,
            output: combined_output(&out.stdout, &out.stderr),
        },
        Ok(out) if is_unknown_subcommand(&out.stderr) => {
            HookRunInfo { ran: false, success: false, output: String::new() }
        }
        Ok(out) => HookRunInfo {
            ran: true,
            success: false,
            output: combined_output(&out.stdout, &out.stderr),
        },
        Err(_) => HookRunInfo { ran: false, success: false, output: String::new() },
    }
}

/// Pure `git hook run` argv builder (A2):
/// `["hook","run",<name>,("--to-stdin=<path>")?,("--")?,<args>…]`. The `--`
/// separator is emitted ONLY when there are trailing args, so git never sees a
/// dangling `--`.
fn build_hook_run_args(hook: HookName, args: &[String], stdin_path: Option<&Path>) -> Vec<String> {
    let mut out = vec![
        "hook".to_string(),
        "run".to_string(),
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
enum HookPlan {
    Skip,
    Run,
}

fn plan_hook(workdir: &Path, hook: HookName) -> HookPlan {
    let repo = match open_repo(workdir) {
        Ok(r) => r,
        Err(_) => return HookPlan::Run, // cannot introspect ⇒ delegate to git
    };
    let cfg = match repo.config().and_then(|mut c| c.snapshot()) {
        Ok(c) => c,
        Err(_) => return HookPlan::Run,
    };
    // core.hooksPath (rare): don't second-guess git's own resolution.
    if let Ok(p) = cfg.get_string("core.hooksPath") {
        if !p.trim().is_empty() {
            return HookPlan::Run;
        }
    }
    // Default location is unambiguous: `<commondir>/hooks/<name>` (commondir so
    // linked worktrees resolve to the shared hooks dir, matching git).
    let candidate = repo.commondir().join("hooks").join(hook.as_str());
    if candidate.is_file() {
        HookPlan::Run
    } else {
        HookPlan::Skip
    }
}

/// git's message for an unknown subcommand (Git < 2.36 has no `hook`):
/// `git: 'hook' is not a git command.` — stable across git versions.
fn is_unknown_subcommand(stderr: &str) -> bool {
    stderr.contains("is not a git command")
}

/// Combined hook output for the error/info body: stdout then stderr, each
/// trailing-trimmed, joined with a newline; empty parts skipped.
fn combined_output(stdout: &str, stderr: &str) -> String {
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
struct TempStdin {
    path: PathBuf,
}

impl TempStdin {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempStdin {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(0);

fn write_stdin_tempfile(hook: HookName, bytes: &[u8]) -> Result<TempStdin, AppError> {
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
        // No args, no stdin.
        assert_eq!(
            build_hook_run_args(HookName::PreCommit, &[], None),
            vec!["hook", "run", "pre-commit"]
        );
        // Args after `--`, no stdin (commit-msg's message-file arg).
        assert_eq!(
            build_hook_run_args(HookName::CommitMsg, &[".git/COMMIT_EDITMSG".to_string()], None),
            vec!["hook", "run", "commit-msg", "--", ".git/COMMIT_EDITMSG"]
        );
        // --to-stdin present, plus args after `--` (pre-push shape).
        let args = vec!["origin".to_string(), "https://x/y.git".to_string()];
        assert_eq!(
            build_hook_run_args(HookName::PrePush, &args, Some(Path::new("/tmp/refs"))),
            vec![
                "hook",
                "run",
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
            vec!["hook", "run", "pre-push", "--to-stdin=/tmp/refs"]
        );
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
}
