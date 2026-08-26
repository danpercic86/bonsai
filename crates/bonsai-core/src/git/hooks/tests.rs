//! hooks unit tests. Extracted verbatim from the former inline `mod tests`
//! (file-size discipline).

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

