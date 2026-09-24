//! P119 §6 (continued) — tag / worktree / bisect / remote-config / failure /
//! init+clone rows. Same capture harness as `tests_activity_coverage`.

use super::tests_activity_support::*;
use super::tests_support::*;
use super::*;

#[test]
fn tags_worktree_bisect_rows() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let c1 = write_stage_commit(&state, &id, dir.path(), "b.txt", "b\n", "C1").oid;
    let log = subscribe(&state);

    let (r, _) = logged(
        &log,
        "createTag",
        Some("v1"),
        create_tag_inner(&state, &id, "v1".into(), c1.clone(), None, false, None),
    );
    r.expect("tag");
    let (r, _) = logged(
        &log,
        "deleteTag",
        Some("v1"),
        delete_tag_inner(&state, &id, "v1".into()),
    );
    r.expect("untag");

    block_on(create_branch_inner(&state, &id, "wtb".into())).expect("branch");
    take(&log);
    let (r, _) = logged(
        &log,
        "worktreeAdd",
        Some("wt1"),
        add_worktree_inner(&state, &id, "wtb".into(), "wt1".into()),
    );
    r.expect("worktree add");
    let (r, _) = logged(
        &log,
        "worktreeRemove",
        Some("wt1"),
        remove_worktree_inner(&state, &id, "wt1".into()),
    );
    r.expect("worktree remove");

    // Bisect needs testable commits between good (C0) and bad (C3).
    take(&log);
    write_stage_commit(&state, &id, dir.path(), "c.txt", "c\n", "C2");
    let c3 = write_stage_commit(&state, &id, dir.path(), "d.txt", "d\n", "C3").oid;
    take(&log);
    let (r, _) = logged(
        &log,
        "bisectStart",
        Some(short(&c3)),
        start_bisect_inner(&state, &id, c3.clone(), vec![c0.clone()]),
    );
    assert!(
        matches!(r, Ok(BisectOutcome::Testing { .. })),
        "bisect start: {r:?}"
    );
    // C rows name the midpoint HEAD is detached on, read BEFORE the mark.
    let mid = head_oid(dir.path());
    let (r, _) = logged(
        &log,
        "bisectGood",
        Some(short(&mid)),
        bisect_mark_inner(&state, &id, true),
    );
    r.expect("bisect good");
    let (r, _) = logged(&log, "bisectReset", None, bisect_reset_inner(&state, &id));
    r.expect("bisect reset");
}

/// Remote config rows are labelled by the remote NAME; the URL (which can
/// carry `user:token@`) appears in no event at all.
#[test]
fn remote_rows_never_carry_the_url() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let log = subscribe(&state);
    let url = "https://user:s3cret-tok@example.invalid/r.git";

    take(&log);
    block_on(add_remote_inner(&state, &id, "origin".into(), url.into())).expect("add");
    block_on(set_remote_url_inner(
        &state,
        &id,
        "origin".into(),
        url.into(),
    ))
    .expect("set-url");
    block_on(rename_remote_inner(
        &state,
        &id,
        "origin".into(),
        "up".into(),
    ))
    .expect("rename");
    block_on(remove_remote_inner(&state, &id, "up".into())).expect("remove");
    let events = take(&log);
    let started: Vec<(&str, &str)> = events
        .iter()
        .filter(|e| kind(e) == "started")
        .map(|e| {
            (
                e["category"].as_str().unwrap_or(""),
                e["target"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        started,
        [
            ("addRemote", "origin"),
            ("setRemoteUrl", "origin"),
            ("renameRemote", "up"),
            ("removeRemote", "up"),
        ]
    );
    let wire = serde_json::to_string(&events).expect("json");
    assert!(!wire.contains("s3cret"), "no URL/token on the wire: {wire}");
}

/// A failed run ends with the toast's message as a `stderrLine`, then
/// `finished{success:false}`; the op's error is unchanged. An unknown repo id
/// still yields a failed row (`repo_path` runs inside the bracket).
#[test]
fn failure_ends_with_the_reason_line() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let log = subscribe(&state);

    let (r, fin) = logged(
        &log,
        "deleteBranch",
        Some("nope"),
        delete_branch_inner(&state, &id, "nope".into()),
    );
    let err = r.expect_err("missing branch");
    assert!(matches!(err, AppError::BranchNotFound(_)), "{err:?}");
    assert_eq!(fin["success"], false);
    assert!(fin.get("outcome").is_none());

    take(&log);
    let err = block_on(delete_branch_inner(&state, &id, "nope".into())).expect_err("again");
    let events = take(&log);
    let n = events.len();
    assert_eq!(kind(&events[n - 2]), "stderrLine");
    assert_eq!(events[n - 2]["line"], err.message());
    assert_eq!(kind(&events[n - 1]), "finished");

    let (r, _) = logged(
        &log,
        "deleteBranch",
        Some("x"),
        delete_branch_inner(&state, MISSING_ID, "x".into()),
    );
    assert!(matches!(r, Err(AppError::NoRepo)), "{r:?}");
}

/// Init/clone are labelled by the destination folder's leaf; clone reports a
/// Network phase and never puts the source URL on the wire.
#[test]
fn init_and_clone_rows() {
    let state = AppState::default();
    let (src, _id, _c0) = fixture_repo(&state);
    let log = subscribe(&state);
    let base = tempfile::TempDir::new().expect("dir");

    let fresh = base.path().join("fresh-init");
    let (r, _) = logged(
        &log,
        "initRepo",
        Some("fresh-init"),
        init_repo_inner(&state, path_string(&fresh)),
    );
    r.expect("init");

    let url = file_url(src.path());
    let dest = base.path().join("cloned");
    let (r, _) = logged(
        &log,
        "cloneRepo",
        Some("cloned"),
        clone_repo_inner(&state, url.clone(), path_string(&dest), |_| {}),
    );
    r.expect("clone");
    // Re-run the clone into a second dest to inspect the whole stream.
    let dest2 = base.path().join("cloned2");
    block_on(clone_repo_inner(
        &state,
        url.clone(),
        path_string(&dest2),
        |_| {},
    ))
    .expect("clone 2");
    let events = take(&log);
    assert!(
        events
            .iter()
            .any(|e| kind(e) == "phase" && e["phase"]["kind"] == "network"),
        "clone reports a network phase: {events:#?}"
    );
    let wire = serde_json::to_string(&events).expect("json");
    assert!(
        !wire.contains(&url),
        "the source URL never reaches the wire"
    );
}

/// Reverting a commit whose lines were changed again later pauses on
/// conflicts; the H rows (abort) name the HEAD branch.
#[test]
fn revert_conflict_and_abort() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("branch");
    let c1 = write_stage_commit(&state, &id, dir.path(), "a.txt", "one\n", "C1").oid;
    write_stage_commit(&state, &id, dir.path(), "a.txt", "two\n", "C2");
    let log = subscribe(&state);

    let (r, fin) = logged(
        &log,
        "revert",
        Some(short(&c1)),
        revert_commit_inner(&state, &id, c1.clone()),
    );
    assert!(matches!(r, Ok(RevertOutcome::Conflicts { .. })), "{r:?}");
    assert_eq!(fin["success"], true, "paused ≠ failed");
    assert_eq!(fin["outcome"], "conflicts");
    let (r, _) = logged(
        &log,
        "revertAbort",
        Some(main.as_str()),
        revert_abort_inner(&state, &id),
    );
    r.expect("abort");
}

/// SECURITY (P119-2 review): a failed clone's error quotes the URL, and the
/// reason line must not carry its `user:token@` into the dock log. The
/// returned error (the toast text) is left as-is.
#[test]
fn failed_clone_reason_line_never_carries_the_token() {
    let state = AppState::default();
    let log = subscribe(&state);
    let base = tempfile::TempDir::new().expect("dir");
    let url = "https://user:s3cret-tok@127.0.0.1:1/r.git";
    let dest = base.path().join("nope");

    take(&log);
    let r = block_on(clone_repo_inner(
        &state,
        url.into(),
        path_string(&dest),
        |_| {},
    ));
    assert!(r.is_err(), "unreachable remote: {r:?}");
    let events = take(&log);
    let (_, fin) = one_run(&events);
    assert_eq!(fin["success"], false);
    let n = events.len();
    assert_eq!(kind(&events[n - 2]), "stderrLine", "{events:#?}");
    // Whether the transport error quotes the URL is platform-dependent (on
    // Windows/WinHTTP a refused connect is a plain `Git` error with no URL), so
    // the URL-quoting shape is pinned deterministically by the test below.
    let wire = serde_json::to_string(&events).expect("json");
    assert!(!wire.contains("s3cret-tok"), "token leaked: {wire}");
    assert!(!wire.contains("user:"), "userinfo leaked: {wire}");
}

/// SECURITY: the exact message shape `map_remote_err` produces for an auth
/// failure (`authentication failed for '<url>'`) goes through the real command
/// bracket; the reason line keeps the host but drops `user:token@`, while the
/// returned error — the toast text — is unchanged.
#[test]
fn auth_failure_reason_line_is_scrubbed_but_the_error_is_not() {
    let state = AppState::default();
    let log = subscribe(&state);
    let msg = "authentication failed for 'https://user:s3cret-tok@host.invalid/r.git': no usable credentials";
    take(&log);
    let r: Result<(), AppError> = block_on(with_activity_ex(
        state.git_activity_hub(),
        GitActivityCategory::CloneRepo,
        RunSubject::default(),
        no_outcome,
        |_em| async move { Err(AppError::AuthFailed(msg.to_string())) },
    ));
    assert!(
        matches!(&r, Err(AppError::AuthFailed(m)) if m == msg),
        "{r:?}"
    );
    let events = take(&log);
    let n = events.len();
    assert_eq!(kind(&events[n - 2]), "stderrLine", "{events:#?}");
    assert_eq!(
        events[n - 2]["line"],
        "authentication failed for 'https://host.invalid/r.git': no usable credentials"
    );
    let wire = serde_json::to_string(&events).expect("json");
    assert!(!wire.contains("s3cret-tok"), "token leaked: {wire}");
}
