//! Pure builders / toggle (git-free).

use super::*;

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
        build_hook_run_args(
            HookName::CommitMsg,
            &[".git/COMMIT_EDITMSG".to_string()],
            None
        ),
        vec![
            "hook",
            "run",
            "--ignore-missing",
            "commit-msg",
            "--",
            ".git/COMMIT_EDITMSG"
        ]
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
        vec![
            "hook",
            "run",
            "--ignore-missing",
            "pre-push",
            "--to-stdin=/tmp/refs"
        ]
    );
}

/// Audit #2 §3.3: `warning` distinguishes every failure shape from the
/// silent "no hook installed" no-op.
#[test]
fn hook_run_info_warning_shapes() {
    // Success (ran) and the absent-hook no-op ⇒ no warning.
    let ok = HookRunInfo {
        ran: true,
        success: true,
        output: "out".to_string(),
    };
    let absent = HookRunInfo {
        ran: false,
        success: true,
        output: String::new(),
    };
    assert_eq!(ok.warning(HookName::PostCommit), None);
    assert_eq!(absent.warning(HookName::PostCommit), None);
    // Ran but exited non-zero ⇒ named failure with the hook's own output.
    let failed = HookRunInfo {
        ran: true,
        success: false,
        output: "boom".to_string(),
    };
    assert_eq!(
        failed.warning(HookName::PostCommit).as_deref(),
        Some("post-commit hook failed:\nboom")
    );
    // Ran, non-zero, silent ⇒ still visibly a failure.
    let silent = HookRunInfo {
        ran: true,
        success: false,
        output: String::new(),
    };
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
    assert!(is_git_infra_failure(
        "error: cannot find a hook named pre-commit"
    ));
    assert!(is_git_infra_failure(
        "fatal: cannot run .husky/pre-commit: No such file"
    ));
    assert!(is_git_infra_failure(
        "error: cannot spawn .git/hooks/pre-commit: exec failed"
    ));
    assert!(is_git_infra_failure(
        "fatal: not a git repository (or any of the parent directories)"
    ));
    // A hook's OWN output — even git-flavored — stays a rejection.
    assert!(!is_git_infra_failure("lint failed: bad code"));
    assert!(!is_git_infra_failure("error: your commit message is bad"));
    assert!(!is_git_infra_failure("fatal: pre-commit checks failed"));
    assert!(!is_git_infra_failure(
        "hook output\nerror: cannot find a hook named x"
    ));
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
