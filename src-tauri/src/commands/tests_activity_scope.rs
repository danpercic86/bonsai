//! P123 R9-R12: the brackets route a run to its own repo's subscribers only;
//! an unscoped run (clone/init) broadcasts; unsubscribe is a no-op on unknown ids.

use std::sync::{Arc, Mutex};

use super::tests_activity_support::*;
use super::tests_support::*;
use super::*;
use crate::state::AppState;

/// Two open fixture repos (A, B) in one state.
fn two_repos(state: &AppState) -> (tempfile::TempDir, String, tempfile::TempDir, String) {
    let (dir_a, a, _) = fixture_repo(state);
    let (dir_b, b, _) = fixture_repo(state);
    assert_ne!(a, b);
    (dir_a, a, dir_b, b)
}

/// R9: a `logged_blocking` op and a `with_activity` op (commit) on A log in A's
/// channel only, with `started.repoId == A`; B's channel stays empty.
#[test]
fn scoped_run_reaches_only_its_repo() {
    let state = AppState::default();
    let (dir_a, a, _dir_b, b) = two_repos(&state);
    let head = head_oid(dir_a.path());
    let (log_a, _) = subscribe_for(&state, &a);
    let (log_b, _) = subscribe_for(&state, &b);

    let (r, _) = logged(
        &log_a,
        "createBranch",
        Some("topic"),
        create_branch_here_inner(&state, &a, "topic".into(), head),
    );
    r.expect("branch on A");
    assert!(take(&log_b).is_empty(), "B saw A's logged_blocking run");

    std::fs::write(dir_a.path().join("n.txt"), "n\n").expect("write");
    block_on(stage_inner(&state, &a, vec!["n.txt".into()])).expect("stage");
    take(&log_a);
    block_on(commit_inner(&state, &a, "C1".into(), None, None)).expect("commit");
    let events = take(&log_a);
    let (started, _) = one_run(&events);
    assert_eq!(started["category"], "commit");
    assert_eq!(started["repoId"], a.as_str(), "{events:#?}");
    for ev in events.iter().filter(|e| kind(e) != "started") {
        assert!(ev.get("repoId").is_none(), "only started carries repoId");
    }
    assert!(take(&log_b).is_empty(), "B saw A's commit run");
}

/// R10: only B is subscribed ⇒ an op on A takes the buffered passthrough
/// (`run` sees `None`) and B's channel stays empty.
#[test]
fn op_on_unsubscribed_repo_is_passthrough() {
    let state = AppState::default();
    let (_dir_a, a, _dir_b, b) = two_repos(&state);
    let (log_b, _) = subscribe_for(&state, &b);
    let saw: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let saw2 = Arc::clone(&saw);
    let out: Result<u32, AppError> = block_on(super::activity::with_activity(
        state.git_activity_for(&a),
        GitActivityCategory::Push,
        None,
        move |em| async move {
            *saw2.lock().expect("lock") = Some(em.is_none());
            Ok(1)
        },
    ));
    assert_eq!(out.ok(), Some(1));
    assert_eq!(*saw.lock().expect("lock"), Some(true), "passthrough");
    assert!(take(&log_b).is_empty(), "B saw A's run");
    assert!(!state.git_activity_for(&a).is_active());
    assert!(state.git_activity_for(&b).is_active());
}

/// R11 (D3/Q1 option a): `init_repo_inner` with A and B subscribed logs one run
/// in BOTH channels, and its `started` has no `repoId`.
#[test]
fn unscoped_init_broadcasts_without_repo_id() {
    let state = AppState::default();
    let (_dir_a, a, _dir_b, b) = two_repos(&state);
    let (log_a, _) = subscribe_for(&state, &a);
    let (log_b, _) = subscribe_for(&state, &b);
    let base = tempfile::tempdir().expect("tempdir");
    let fresh = base.path().join("fresh-init");
    block_on(init_repo_inner(&state, path_string(&fresh))).expect("init");
    for log in [&log_a, &log_b] {
        let events = take(log);
        let (started, _) = one_run(&events);
        assert_eq!(started["category"], "initRepo");
        assert!(started.get("repoId").is_none(), "{events:#?}");
    }
}

/// R12: unsubscribing an unknown / already-dropped id is a no-op (no panic),
/// and a dropped sub receives nothing further.
#[test]
fn unsubscribe_unknown_id_is_noop() {
    let state = AppState::default();
    let (_dir_a, a, _dir_b, _b) = two_repos(&state);
    let (log_a, id) = subscribe_for(&state, &a);
    assert!(!state.git_activity.unsubscribe("gas-does-not-exist"));
    assert_eq!(state.git_activity.subscriber_count(), 1);
    assert!(state.git_activity.unsubscribe(&id));
    assert!(!state.git_activity.unsubscribe(&id), "idempotent");
    assert_eq!(state.git_activity.subscriber_count(), 0);
    let head = {
        let dir = state.repos.lock().expect("repos")[&a].path.clone();
        head_oid(&dir)
    };
    block_on(create_branch_here_inner(&state, &a, "t2".into(), head)).expect("branch");
    assert!(take(&log_a).is_empty(), "dropped sub received a run");
}
