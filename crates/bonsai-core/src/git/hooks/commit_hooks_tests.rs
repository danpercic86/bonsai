//! Tests for [`commit_hooks_that_would_run`] — the commit-scoped, toggle-aware
//! hook probe added for audit 2026-09-11 LOW (the MCP server refusing an
//! agent commit whose hooks the user was never shown).
//!
//! Its own file rather than appended to the already-489-line `hooks/tests.rs`
//! (file-size discipline). Git-binary-free: nothing here runs a hook, it only
//! asks which ones WOULD run, so these pass on a host without `git` on PATH.

use super::super::hooks::{commit_hooks_that_would_run, merge_commit_hooks_that_would_run};
use std::path::{Path, PathBuf};

/// A scratch repo with a git identity (mirrors `hooks::tests::init_repo`; kept
/// local so this file does not depend on that module's private helpers).
fn init_repo(dir: &Path) -> git2::Repository {
    git2::Repository::init(dir).expect("init")
}

fn hooks_dir(repo: &git2::Repository) -> PathBuf {
    repo.commondir().join("hooks")
}

/// Write an executable `#!/bin/sh` hook with LF endings.
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

/// No hooks installed ⇒ empty (committing executes no repository code).
#[test]
fn commit_hooks_none_is_empty() {
    let dir = crate::testutil::scratch_dir();
    init_repo(dir.path());
    assert!(commit_hooks_that_would_run(dir.path()).is_empty());
}

/// Each of the three commit hooks is reported by its canonical git name, and
/// several together come back in `COMMIT_HOOKS` order.
#[test]
fn commit_hooks_reports_each_commit_hook_by_name() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    let hooks = hooks_dir(&repo);
    write_hook(&hooks, "commit-msg", "#!/bin/sh\nexit 0\n");
    assert_eq!(commit_hooks_that_would_run(dir.path()), vec!["commit-msg"]);

    write_hook(&hooks, "pre-commit", "#!/bin/sh\nexit 0\n");
    write_hook(&hooks, "post-commit", "#!/bin/sh\nexit 0\n");
    assert_eq!(
        commit_hooks_that_would_run(dir.path()),
        vec!["pre-commit", "commit-msg", "post-commit"],
        "all three commit hooks, in COMMIT_HOOKS order"
    );
}

/// `pre-push` is NOT a commit hook: a repo with only a pre-push hook reports
/// nothing, even though `repo_has_runnable_hooks` (the disclosure probe) is
/// true for it. Refusing a COMMIT over a hook no commit fires would be wrong.
#[test]
fn commit_hooks_excludes_pre_push() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    write_hook(&hooks_dir(&repo), "pre-push", "#!/bin/sh\nexit 0\n");
    assert!(
        commit_hooks_that_would_run(dir.path()).is_empty(),
        "pre-push never runs on a commit"
    );
    assert!(
        super::super::hooks::repo_has_runnable_hooks(dir.path()),
        "…while the broader disclosure probe still sees it"
    );
}

/// `bonsai.runHooks=false` ⇒ empty even with a runnable hook present: the repo
/// opted out, so nothing would execute and there is nothing to refuse over.
#[test]
fn commit_hooks_honors_run_hooks_opt_out() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    write_hook(&hooks_dir(&repo), "pre-commit", "#!/bin/sh\nexit 0\n");
    assert_eq!(commit_hooks_that_would_run(dir.path()), vec!["pre-commit"]);

    repo.config()
        .expect("config")
        .set_bool("bonsai.runHooks", false)
        .expect("disable hooks");
    assert!(
        commit_hooks_that_would_run(dir.path()).is_empty(),
        "bonsai.runHooks=false ⇒ nothing would run"
    );
}

/// `core.hooksPath` is honored (discovery is git's, shared with `plan_hook`).
#[test]
fn commit_hooks_honors_core_hooks_path() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    let alt = dir.path().join("myhooks");
    write_hook(&alt, "pre-commit", "#!/bin/sh\nexit 0\n");
    repo.config()
        .expect("config")
        .set_str("core.hooksPath", alt.to_str().expect("utf8"))
        .expect("set hooksPath");
    assert_eq!(commit_hooks_that_would_run(dir.path()), vec!["pre-commit"]);
}

/// unix: a present-but-non-executable hook is skipped by git, so it is not
/// reported (same precision as `repo_has_runnable_hooks`).
#[cfg(unix)]
#[test]
fn commit_hooks_ignores_non_executable() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    let hooks = hooks_dir(&repo);
    std::fs::create_dir_all(&hooks).expect("mkdir hooks");
    // Plain write ⇒ 0o644 (no execute bit).
    std::fs::write(hooks.join("pre-commit"), "#!/bin/sh\nexit 0\n").expect("write hook");
    assert!(commit_hooks_that_would_run(dir.path()).is_empty());
}

/// A path that is not a repository at all ⇒ empty, never a panic.
#[test]
fn commit_hooks_non_repo_is_empty() {
    let dir = crate::testutil::scratch_dir();
    assert!(commit_hooks_that_would_run(dir.path()).is_empty());
}

// ------------------------------- the merge-scoped probe (review 2026-09-11)

/// The clean auto-merge commit fires `commit-msg` ALONE, so the merge-scoped
/// probe must report that hook and ignore `pre-commit` / `post-commit` /
/// `pre-push` even when all three are installed. Naming a hook the operation
/// would not run is precisely the over-refusal this probe exists to avoid.
#[test]
fn merge_commit_hooks_reports_commit_msg_only() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    let hooks = hooks_dir(&repo);
    for name in ["pre-commit", "post-commit", "pre-push"] {
        write_hook(&hooks, name, "#!/bin/sh\nexit 0\n");
    }
    assert!(
        merge_commit_hooks_that_would_run(dir.path()).is_empty(),
        "no commit-msg hook means a clean auto-merge runs nothing"
    );
    assert_eq!(
        commit_hooks_that_would_run(dir.path()),
        vec!["pre-commit", "post-commit"],
        "while the commit probe does see those two"
    );

    write_hook(&hooks, "commit-msg", "#!/bin/sh\nexit 0\n");
    assert_eq!(
        merge_commit_hooks_that_would_run(dir.path()),
        vec!["commit-msg"]
    );
}

/// The merge probe shares the toggle/discovery engine, so
/// `bonsai.runHooks=false` empties it too.
#[test]
fn merge_commit_hooks_honors_run_hooks_opt_out() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    write_hook(&hooks_dir(&repo), "commit-msg", "#!/bin/sh\nexit 0\n");
    assert_eq!(
        merge_commit_hooks_that_would_run(dir.path()),
        vec!["commit-msg"]
    );
    repo.config()
        .expect("config")
        .set_bool("bonsai.runHooks", false)
        .expect("disable hooks");
    assert!(merge_commit_hooks_that_would_run(dir.path()).is_empty());
}
