//! M3 missing-config integration test (contract §6.3, command path).
//!
//! Cargo runs each integration binary in its OWN process, so the process-
//! global libgit2 mutation below cannot race any other test.
//!
//! DEVIATION from contract §6.3 (which prescribed GIT_CONFIG_GLOBAL /
//! GIT_CONFIG_SYSTEM / GIT_CONFIG_NOSYSTEM env vars): libgit2 does NOT
//! consult those env vars on our open path (`Repository::open_ext` without
//! FROM_ENV) — verified empirically: the developer machine's real identity
//! leaked through and the commit succeeded. The reliable in-process
//! equivalent is `git2::opts::set_search_path`, which redirects libgit2's
//! global/xdg/system/programdata config discovery to an empty scratch dir.
//!
//! Keep this file to a SINGLE #[test]: tests within one binary run on
//! parallel threads, and the process-global setup must not race repo opens.

mod common;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::stage::stage_paths;

#[test]
fn commit_without_identity_fails_then_local_identity_fixes_it() {
    // Redirect every non-local config level to an EMPTY scratch dir BEFORE
    // any repo open, so no real user/system gitconfig is discoverable.
    let iso = common::scratch_dir();
    for level in [
        git2::ConfigLevel::Global,
        git2::ConfigLevel::XDG,
        git2::ConfigLevel::System,
        git2::ConfigLevel::ProgramData,
    ] {
        // Safety: single-threaded at this point, own process, before any
        // other libgit2 use (set_search_path mutates process-global state).
        unsafe {
            git2::opts::set_search_path(level, iso.path()).expect("set search path");
        }
    }

    // Init via git2 (not the CLI — no identity written anywhere).
    let dir = common::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");

    std::fs::write(dir.path().join("first.txt"), "first\n").expect("write first.txt");
    stage_paths(dir.path(), &["first.txt".to_string()]).expect("stage_paths");

    // No identity anywhere -> ConfigMissing naming BOTH keys.
    match create_commit(dir.path(), "first commit").expect_err("identity is not configured") {
        AppError::ConfigMissing(m) => {
            assert!(m.contains("user.name"), "must name user.name, got: {m}");
            assert!(m.contains("user.email"), "must name user.email, got: {m}");
            assert!(m.contains("git config --global"), "must hint the fix, got: {m}");
        }
        other => panic!("expected ConfigMissing, got: {other:?}"),
    }
    // No commit was created: HEAD is still unborn.
    assert!(repo.head().is_err(), "HEAD must still be unborn");

    // Repo-local identity (set via git2, so the env above stays authoritative
    // for the other levels) makes the same commit succeed.
    {
        let mut cfg = repo.config().expect("open repo config");
        cfg.set_str("user.name", "Local User").expect("set user.name");
        cfg.set_str("user.email", "local@example.com").expect("set user.email");
    }
    let res = create_commit(dir.path(), "first commit").expect("commit with local identity");
    assert_eq!(res.summary, "first commit");
    assert!(res.branch.is_some(), "first commit creates the branch");

    let head = repo.head().expect("HEAD born");
    let commit = head.peel_to_commit().expect("HEAD commit");
    assert_eq!(commit.id().to_string(), res.oid);
    assert_eq!(commit.author().name(), Some("Local User"));
    assert_eq!(commit.author().email(), Some("local@example.com"));
}
