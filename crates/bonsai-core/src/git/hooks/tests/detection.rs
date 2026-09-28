//! Detection: `repo_has_runnable_hooks` (git-binary-free).

use super::*;

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
