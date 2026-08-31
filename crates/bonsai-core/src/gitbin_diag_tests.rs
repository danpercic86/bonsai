//! Cache / spawn-factory / diagnostics tests for [`super`] (P70 §6.1 items
//! 12, 21–23 plus the copy guards). Split from `gitbin_tests.rs` (which covers
//! the ladder itself) to keep both files under the 500-line limit.

use std::path::{Path, PathBuf};

use super::*;


// The cache returns a stable answer and `refresh_git_bin` re-runs the ladder.
// (Uses the REAL host env — no mutation, only reads.)
//
// NOTE: `reset_git_bin_cache()` mutates process-global state, so in principle a
// parallel test could observe the cleared cache. It is benign: the very next
// `git_bin()` re-runs the ladder against the same (unmutated) host env and
// stores the same answer, so no observer can see a DIFFERENT value — only a
// slower one.
#[test]
fn cache_is_stable_and_refreshable() {
    let first = git_bin();
    assert_eq!(git_bin(), first, "cached read is stable");
    assert_eq!(refresh_git_bin(), first, "refresh re-resolves to the same answer");
    reset_git_bin_cache();
    assert_eq!(git_bin(), first, "re-resolve after a cache clear");
}

// The factory always targets the resolved binary; never mutates process env.
#[test]
fn git_command_targets_the_resolved_binary() {
    let bin = git_bin();
    let cmd = git_command();
    assert_eq!(Path::new(cmd.get_program()), bin.path.as_path());
    // A PATH-resolved (normal dev machine) binary needs no child-PATH repair;
    // a non-PATH rung prepends exactly one entry. NOTE this assertion is
    // host-dependent and vacuous wherever git resolves from PATH — the real
    // content assertion lives in `child_path_*` below.
    let path_env: Vec<_> = cmd.get_envs().filter(|(k, _)| *k == "PATH").collect();
    assert_eq!(path_env.len(), usize::from(bin.bin_dir().is_some()));
}

// 12. The child-PATH augmentation, asserted hermetically: `bin_dir()` is
//     prepended, the inherited PATH is preserved in order, nothing is dropped.
#[test]
fn child_path_prepends_bin_dir_and_preserves_existing() {
    let existing = std::env::join_paths([PathBuf::from("/aaa"), PathBuf::from("/bbb")])
        .expect("join fixture PATH");
    let joined = child_path(Path::new("/opt/git/cmd"), &existing).expect("child_path");
    let parts: Vec<PathBuf> = std::env::split_paths(&joined).collect();
    assert_eq!(
        parts,
        vec![
            PathBuf::from("/opt/git/cmd"),
            PathBuf::from("/aaa"),
            PathBuf::from("/bbb"),
        ],
        "bin_dir first, inherited entries after, in order"
    );
}

// An EMPTY inherited PATH yields just the prepended directory (no stray empty
// component, which on Unix would mean "the current directory").
#[test]
fn child_path_handles_an_empty_inherited_path() {
    let joined =
        child_path(Path::new("/opt/git/cmd"), std::ffi::OsStr::new("")).expect("child_path");
    let parts: Vec<PathBuf> = std::env::split_paths(&joined).collect();
    assert_eq!(parts, vec![PathBuf::from("/opt/git/cmd")]);
}

// 17. spawn_error: NotFound -> GitNotFound (honest); any other io error with a
//     resolvable git stays an ordinary Git error naming the subcommand.
#[test]
fn spawn_error_maps_not_found_to_git_not_found() {
    let err = spawn_error("log", &std::io::Error::from(std::io::ErrorKind::NotFound));
    match err {
        AppError::GitNotFound(m) => {
            assert!(m.contains("NOT an authentication failure"), "{m}");
            assert!(m.contains("BONSAI_GIT_BIN"), "{m}");
        }
        other => panic!("expected GitNotFound, got {other:?}"),
    }

    let other = spawn_error("log", &std::io::Error::from(std::io::ErrorKind::Interrupted));
    if git_missing() {
        // No git on this machine: everything is honestly GitNotFound.
        assert!(matches!(other, AppError::GitNotFound(_)));
    } else {
        match other {
            AppError::Git(m) => assert!(m.contains("failed to run `git log`"), "{m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }
}

// 16. Wire shape: the new variant serializes under its own kind so the frontend
//     can route it to ONE banner instead of N toasts.
#[test]
fn git_not_found_serializes_with_its_own_kind() {
    let json = serde_json::to_string(&AppError::GitNotFound("x".to_string())).expect("serialize");
    assert_eq!(json, r#"{"kind":"gitNotFound","message":"x"}"#);
}

// The copy must never claim an auth problem and must deny it explicitly.
#[test]
fn git_not_found_message_is_honest() {
    let m = git_not_found_message();
    assert!(m.starts_with("Git is not available."), "{m}");
    assert!(m.contains("NOT an authentication failure"), "{m}");
    assert!(!m.contains("cached credentials for this remote"), "{m}");
    assert!(m.contains("BONSAI_GIT_BIN"), "{m}");
    assert!(m.contains("PATH"), "{m}");
    // The scope sentence is the whole point of the §3.1 rewrite: without it an
    // SSH-agent user reads this as "my remotes are broken", which is false.
    assert!(
        m.contains("SSH remotes using an ssh-agent are unaffected"),
        "{m}"
    );
    assert!(
        m.contains("because Bonsai could not start the credential helper"),
        "{m}"
    );
    assert!(
        m.contains("This affects HTTPS remotes"),
        "{m}"
    );
}
