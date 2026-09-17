//! P59a-2 pre-push hook oracle for force-push-with-lease — split out of
//! `force_push_cli.rs` to keep each file under the ~500-line limit. Declared as
//! a child module of `force_push_cli`, so it reuses that file's `require_git!`
//! macro and its bare-origin + clone fixture helpers.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

use super::{init_origin_and_clone, origin_main, rewrite_head};
use crate::common;
use bonsai_core::error::AppError;
use bonsai_core::git::exec::SpawnGitExec;
use bonsai_core::git::remote::{force_push_with_lease, PushResult};

// ---------------------------------- §A6 pre-push hook oracle (P59a-2)

/// A failing `pre-push` hook ABORTS the force-push with `HookRejected`, and the
/// remote ref stays UNCHANGED. The hook echoes the stdin ref line — which for a
/// force-push carries the LEASE baseline as the remote-oid — proving the stdin
/// synthesis. Requires Git ≥ 2.36 (`git hook run`).
#[test]
fn pre_push_hook_blocks_force_push() {
    require_git!();
    if !common::git_version_at_least(2, 36) {
        eprintln!("skipping: git < 2.36 (no `git hook run`)");
        return;
    }
    let f = init_origin_and_clone();
    let x = origin_main(&f);

    common::write_pre_push_hook(
        &f.work,
        "read line\necho \"pre-push saw: $line\" >&2\nexit 1\n",
    );
    let z = rewrite_head(&f.work, "z");
    assert_ne!(z, x);

    let err =
        force_push_with_lease(&f.work, &SpawnGitExec, false).expect_err("pre-push must block");
    match err {
        AppError::HookRejected(m) => {
            assert!(m.contains("pre-push hook failed:"), "prefix: {m}");
            assert!(m.contains("refs/heads/main"), "stdin ref surfaced: {m}");
            // The remote-oid field is the lease baseline X (not 40 zeros).
            assert!(
                m.contains(&x),
                "stdin remote-oid must be the lease baseline: {m}"
            );
        }
        other => panic!("expected HookRejected, got {other:?}"),
    }
    // Oracle: the force-push never happened — origin unchanged.
    assert_eq!(
        origin_main(&f),
        x,
        "origin main must be unchanged after a blocked pre-push"
    );
}

/// P59a-2: `skip_hooks = true` (≡ --no-verify) bypasses a failing pre-push — the
/// force-push proceeds and origin moves to the rewritten tip.
#[test]
fn pre_push_hook_skipped_allows_force_push() {
    require_git!();
    if !common::git_version_at_least(2, 36) {
        eprintln!("skipping: git < 2.36");
        return;
    }
    let f = init_origin_and_clone();
    common::write_pre_push_hook(&f.work, "exit 1\n");
    let z = rewrite_head(&f.work, "z");

    let res = force_push_with_lease(&f.work, &SpawnGitExec, true)
        .expect("skip_hooks bypasses the failing pre-push");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");
    assert_eq!(
        origin_main(&f),
        z,
        "origin main must move when the hook is skipped"
    );
}

/// A PASSING pre-push (exit 0) allows the force-push through git's atomic lease.
#[test]
fn pre_push_hook_pass_allows_force_push() {
    require_git!();
    if !common::git_version_at_least(2, 36) {
        eprintln!("skipping: git < 2.36");
        return;
    }
    let f = init_origin_and_clone();
    common::write_pre_push_hook(&f.work, "exit 0\n");
    let z = rewrite_head(&f.work, "z");

    let res =
        force_push_with_lease(&f.work, &SpawnGitExec, false).expect("passing pre-push allows push");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");
    assert_eq!(origin_main(&f), z);
}
