//! Where [`merge_branch_gated`]'s hook gate is consulted, and what a refusal
//! leaves behind (review 2026-09-11).
//!
//! The gate exists so a caller that may not run undisclosed repository code can
//! REFUSE the clean auto-merge's `commit-msg` hook instead of running or
//! silently skipping it. Two claims are load-bearing and asserted here rather
//! than through `bonsai-mcp`'s stdio harness (cheaper, and this is where the
//! decision lives):
//!
//! 1. the gate is consulted ONLY on a path that could really run the hook — not
//!    on `UpToDate`, not on a fast-forward;
//! 2. a refusal changes NOTHING — no commit, no MERGE_HEAD, no autostash left
//!    on the stack, worktree untouched.
//!
//! Its own file (file-size discipline): `tests.rs` and `autostash_tests.rs` are
//! already large, and the p8 fixtures are reused rather than re-invented.

use super::p8_helpers::*;
use super::*;
use crate::error::AppError;
use std::cell::RefCell;

thread_local! {
    /// The hook-name lists the gate was handed, in call order. Thread-local
    /// because `merge_branch_gated` is synchronous — the gate runs on the
    /// calling thread — so parallel test threads cannot see each other's calls.
    static GATE_CALLS: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
}

/// Record the call and REFUSE. Used to prove both that the gate was reached and
/// that its `Err` aborts the merge.
fn refusing_gate(hooks: &[&str]) -> Result<(), AppError> {
    GATE_CALLS.with(|c| {
        c.borrow_mut()
            .push(hooks.iter().map(|h| h.to_string()).collect())
    });
    Err(AppError::HooksNotPermitted("refused by test".to_string()))
}

/// Record the call and ALLOW. Proves an `Ok` gate behaves exactly like
/// [`MergeHookGate::Run`].
fn allowing_gate(hooks: &[&str]) -> Result<(), AppError> {
    GATE_CALLS.with(|c| {
        c.borrow_mut()
            .push(hooks.iter().map(|h| h.to_string()).collect())
    });
    Ok(())
}

fn reset_calls() {
    GATE_CALLS.with(|c| c.borrow_mut().clear());
}

fn calls() -> Vec<Vec<String>> {
    GATE_CALLS.with(|c| c.borrow().clone())
}

/// Write an executable `commit-msg` hook that would touch a witness file, so a
/// test can prove the hook never ran.
///
/// Install it AFTER the fixture history exists: `p8_commit` goes through
/// `create_commit`, which runs hooks, so a hook written first would fire during
/// setup and forge the witness.
fn write_commit_msg_hook(repo: &git2::Repository) {
    let hooks = repo.commondir().join("hooks");
    std::fs::create_dir_all(&hooks).expect("mkdir hooks");
    let path = hooks.join("commit-msg");
    std::fs::write(
        &path,
        "#!/bin/sh\necho ran > \"$(git rev-parse --show-toplevel)/hook-ran.txt\"\nexit 0\n",
    )
    .expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

/// base <- main, base <- topic: divergent, so merging topic is a NORMAL merge
/// that would auto-commit. Returns the pre-merge HEAD oid of the current branch.
fn diverged(dir: &std::path::Path, repo: &git2::Repository) -> git2::Oid {
    p8_commit(dir, "base", &[("a.txt", "base\n")]);
    let base = repo.find_commit(p8_head_oid(repo)).expect("base");
    p8_commit_on_ref(
        repo,
        "refs/heads/topic",
        &base,
        &[("topic.txt", "topic\n")],
        "topic side",
    );
    p8_commit(dir, "main side", &[("main.txt", "main\n")]);
    p8_head_oid(repo)
}

/// A fast-forward creates no commit, so it fires no hook — the gate must not
/// even be consulted. A refusing gate therefore cannot break a FF.
#[test]
fn gate_is_not_consulted_on_a_fast_forward() {
    reset_calls();
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());
    p8_commit(dir.path(), "base", &[("a.txt", "base\n")]);
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    let topic = p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("feature.txt", "feature\n")],
        "topic advance",
    );
    write_commit_msg_hook(&repo);

    let outcome = merge_branch_gated(dir.path(), "topic", MergeHookGate::Gate(refusing_gate))
        .expect("a fast-forward must never be refused over commit hooks");
    assert!(matches!(outcome, MergeOutcome::FastForwarded { .. }));
    assert_eq!(p8_head_oid(&repo), topic, "HEAD must reach the topic tip");
    assert!(calls().is_empty(), "the gate must not be consulted on a FF");
    assert!(!dir.path().join("hook-ran.txt").exists(), "no hook may run");
}

/// An up-to-date merge is a no-op: no commit, no hook, no gate call.
#[test]
fn gate_is_not_consulted_when_up_to_date() {
    reset_calls();
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());
    p8_commit(dir.path(), "base", &[("a.txt", "base\n")]);
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    repo.reference("refs/heads/topic", base.id(), true, "topic at base")
        .expect("create topic");
    p8_commit(dir.path(), "advance main", &[("b.txt", "b\n")]);
    write_commit_msg_hook(&repo);

    let outcome = merge_branch_gated(dir.path(), "topic", MergeHookGate::Gate(refusing_gate))
        .expect("up-to-date must never be refused");
    assert_eq!(outcome, MergeOutcome::UpToDate);
    assert!(calls().is_empty(), "the gate must not be consulted");
}

/// A normal merge consults the gate exactly ONCE, and an `Ok` proceeds to the
/// merge just like `MergeHookGate::Run`. No hook is installed here, so the list
/// it is handed is empty (the "it sees `commit-msg`" half is asserted by
/// `gate_refusal_on_a_normal_merge_changes_nothing`).
#[test]
fn gate_on_a_normal_merge_is_consulted_once_and_ok_proceeds() {
    reset_calls();
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());
    diverged(dir.path(), &repo);

    let outcome = merge_branch_gated(dir.path(), "topic", MergeHookGate::Gate(allowing_gate))
        .expect("an allowing gate must not change the outcome");
    assert!(matches!(outcome, MergeOutcome::Merged { .. }));
    assert_eq!(
        calls(),
        vec![Vec::<String>::new()],
        "consulted exactly once, with no hook installed"
    );
}

/// A repo with NO `commit-msg` hook hands the gate an empty list, so a gate
/// that only refuses non-empty lists (the real MCP one) lets the merge through.
/// Asserted with the real predicate rather than a stub: the probe must be
/// merge-scoped, so a `pre-commit`-only repo still merges.
#[test]
fn gate_sees_nothing_for_a_pre_commit_only_repo() {
    reset_calls();
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());
    diverged(dir.path(), &repo);
    // Installed AFTER the fixture commits (which would have fired it) and made
    // to FAIL: if the auto-merge commit ever started firing `pre-commit`, this
    // merge would come back `HookRejected` instead of `Merged`.
    let hooks = repo.commondir().join("hooks");
    std::fs::create_dir_all(&hooks).expect("mkdir hooks");
    let path = hooks.join("pre-commit");
    std::fs::write(&path, "#!/bin/sh\nexit 1\n").expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }

    // Same shape as the MCP gate: refuse iff at least one hook would run.
    fn refuse_if_any(hooks: &[&str]) -> Result<(), AppError> {
        GATE_CALLS.with(|c| {
            c.borrow_mut()
                .push(hooks.iter().map(|h| h.to_string()).collect())
        });
        if hooks.is_empty() {
            Ok(())
        } else {
            Err(AppError::HooksNotPermitted(hooks.join(", ")))
        }
    }

    let outcome = merge_branch_gated(dir.path(), "topic", MergeHookGate::Gate(refuse_if_any))
        .expect("a pre-commit-only repo runs no merge hook, so it must not be refused");
    assert!(matches!(outcome, MergeOutcome::Merged { .. }));
    assert_eq!(
        calls(),
        vec![Vec::<String>::new()],
        "the merge-scoped probe must not report pre-commit"
    );
}

/// THE refusal test: a dirty worktree + a divergent branch, refused. Nothing
/// may have changed — no commit, state Clean (no MERGE_HEAD), no autostash on
/// the stack, the dirty edit still in the worktree, and no hook executed.
#[test]
fn gate_refusal_on_a_normal_merge_changes_nothing() {
    reset_calls();
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());
    let head_before = diverged(dir.path(), &repo);
    write_commit_msg_hook(&repo);
    // Dirty tracked file: this is what an autostash would have swept away.
    std::fs::write(dir.path().join("a.txt"), "locally edited\n").expect("dirty");

    let err = merge_branch_gated(dir.path(), "topic", MergeHookGate::Gate(refusing_gate))
        .expect_err("the gate's Err must abort the merge");
    match err {
        AppError::HooksNotPermitted(m) => assert_eq!(m, "refused by test"),
        other => panic!("expected the gate's own error, got {other:?}"),
    }
    assert_eq!(
        calls(),
        vec![vec!["commit-msg".to_string()]],
        "the gate must be told which hook would run"
    );

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "a refusal must not park the repo mid-merge"
    );
    assert_eq!(p8_head_oid(&repo), head_before, "HEAD must not move");
    assert!(
        !repo.path().join("MERGE_HEAD").exists(),
        "no MERGE_HEAD may be written"
    );
    assert_eq!(
        p8_stash_count(dir.path()),
        0,
        "a refusal must not leave an autostash behind"
    );
    assert_eq!(
        p8_read(dir.path(), "a.txt"),
        "locally edited\n",
        "the dirty edit must still be in the worktree"
    );
    assert!(
        !dir.path().join("topic.txt").exists(),
        "nothing from the incoming branch may be checked out"
    );
    assert!(
        !dir.path().join("hook-ran.txt").exists(),
        "the hook must not have executed"
    );
}

/// `MergeHookGate::Run` / `Skip` are exactly the old `skip_hooks` bool, so the
/// un-gated entry point keeps its behaviour: no probe, no gate, no refusal.
#[test]
fn run_and_skip_are_the_old_skip_hooks_bool() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());
    diverged(dir.path(), &repo);
    assert!(matches!(
        merge_branch_gated(dir.path(), "topic", MergeHookGate::Skip).expect("skip merges"),
        MergeOutcome::Merged { .. }
    ));

    let dir2 = crate::testutil::scratch_dir();
    let repo2 = p8_init(dir2.path());
    diverged(dir2.path(), &repo2);
    assert!(matches!(
        merge_branch(dir2.path(), "topic", false).expect("plain merge_branch still works"),
        MergeOutcome::Merged { .. }
    ));
}
