//! P119 §6 — every repo-changing command family emits exactly one `started`
//! (exact category + `target`/`targetCount`) and one `finished` while the dock
//! is subscribed, and the P119 wire additions (`outcome`, `targetCount`, the
//! failure reason line) land where the contract says. Runtime-free: a real
//! `tauri::ipc::Channel::new` sink captures the JSON the frontend would see.
//!
//! The hook-rejection "no reason line" rule is pinned at the unit level
//! (`activity::tests::failure_reason_map`) — building a rejecting pre-push hook
//! here would add a CLI fixture for a one-line `match`.

use super::tests_activity_support::*;
use super::tests_support::*;
use super::*;

#[test]
fn branch_family_targets_and_categories() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("branch");
    let log = subscribe(&state);

    let (r, _) = logged(
        &log,
        "createBranch",
        Some("topic"),
        create_branch_inner(&state, &id, "topic".into()),
    );
    r.expect("create");
    let (r, _) = logged(
        &log,
        "createBranch",
        Some("here"),
        create_branch_here_inner(&state, &id, "here".into(), c0.clone()),
    );
    r.expect("create here");
    let (r, _) = logged(
        &log,
        "renameBranch",
        Some("renamed"),
        rename_branch_inner(&state, &id, "topic".into(), "renamed".into()),
    );
    r.expect("rename");
    let (r, _) = logged(
        &log,
        "checkoutBranch",
        Some(main.as_str()),
        checkout_branch_inner(&state, &id, main.clone()),
    );
    r.expect("checkout branch");
    let (r, _) = logged(
        &log,
        "checkoutCommit",
        Some(short(&c0)),
        checkout_commit_inner(&state, &id, c0.clone()),
    );
    r.expect("checkout commit");
    block_on(checkout_branch_inner(&state, &id, main.clone())).expect("reattach");
    let (r, _) = logged(
        &log,
        "deleteBranch",
        Some("renamed"),
        delete_branch_inner(&state, &id, "renamed".into()),
    );
    r.expect("delete");
}

/// Multi-item runs carry a COUNT (never a phrase) and no target; one item is a
/// plain target.
#[test]
fn multi_item_runs_carry_a_count() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("branch");
    for b in ["s1", "s2", "s3"] {
        block_on(create_branch_inner(&state, &id, b.into())).expect("branch");
    }
    let log = subscribe(&state);

    let names = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
    block_on(delete_branches_inner(&state, &id, names, Some(main))).expect("batch delete");
    let events = take(&log);
    let (started, _) = one_run(&events);
    assert_eq!(started["category"], "deleteBranches");
    assert_eq!(started["targetCount"], 3);
    assert!(started.get("target").is_none(), "count ⇒ no target");

    std::fs::write(dir.path().join("a.txt"), "dirty\n").expect("write");
    std::fs::write(dir.path().join("b.txt"), "new\n").expect("write");
    let paths = vec!["a.txt".to_string(), "b.txt".to_string()];
    block_on(discard_paths_force_inner(&state, &id, paths)).expect("discard two");
    let events = take(&log);
    let (started, _) = one_run(&events);
    assert_eq!(started["category"], "discard");
    assert_eq!(started["targetCount"], 2);

    std::fs::write(dir.path().join("a.txt"), "dirty again\n").expect("write");
    let (r, _) = logged(
        &log,
        "discard",
        Some("a.txt"),
        discard_paths_inner(&state, &id, vec!["a.txt".into()]),
    );
    r.expect("discard one");
}

/// Merge outcomes: fast-forward, then a conflicted merge that ends
/// `success:true, outcome:"conflicts"`.
#[test]
fn merge_outcomes_fast_forward_and_conflicts() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("branch");
    block_on(create_branch_here_inner(
        &state,
        &id,
        "ff".into(),
        c0.clone(),
    ))
    .expect("ff");
    write_stage_commit(&state, &id, dir.path(), "ff.txt", "ff\n", "ff commit");
    block_on(checkout_branch_inner(&state, &id, main.clone())).expect("main");
    let log = subscribe(&state);

    let (r, fin) = logged(
        &log,
        "merge",
        Some("ff"),
        merge_branch_inner(&state, &id, "ff".into(), None),
    );
    assert!(matches!(r, Ok(MergeOutcome::FastForwarded { .. })), "{r:?}");
    assert_eq!(fin["success"], true);
    assert_eq!(fin["outcome"], "fastForwarded");

    let c1 = head_oid(dir.path());
    take(&log);
    conflicting_sides(&state, &id, dir.path(), &c1);
    take(&log);
    let (r, fin) = logged(
        &log,
        "merge",
        Some("feature"),
        merge_branch_inner(&state, &id, "feature".into(), None),
    );
    assert!(matches!(r, Ok(MergeOutcome::Conflicts { .. })), "{r:?}");
    assert_eq!(fin["success"], true, "paused ≠ failed");
    assert_eq!(fin["outcome"], "conflicts");

    let (r, fin) = logged(
        &log,
        "abortMerge",
        Some(main.as_str()),
        abort_merge_inner(&state, &id),
    );
    r.expect("abort");
    assert!(fin.get("outcome").is_none(), "abort has no outcome");
}

/// A paused rebase ends `conflicts`; the R rows name the branch being rebased
/// even though HEAD is detached mid-rebase.
#[test]
fn rebase_paused_is_conflicts_and_abort_names_the_branch() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let (main, _) = conflicting_sides(&state, &id, dir.path(), &c0);
    block_on(checkout_branch_inner(&state, &id, "feature".into())).expect("feature");
    let log = subscribe(&state);

    let (r, fin) = logged(
        &log,
        "rebase",
        Some(main.as_str()),
        rebase_branch_inner(&state, &id, main.clone()),
    );
    assert!(matches!(r, Ok(RebaseOutcome::Conflicts { .. })), "{r:?}");
    assert_eq!(fin["outcome"], "conflicts");

    let (r, _) = logged(
        &log,
        "rebaseAbort",
        Some("feature"),
        rebase_abort_inner(&state, &id),
    );
    r.expect("abort");
}

#[test]
fn cherry_pick_conflict_and_reset_hard() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let (main, f_tip) = conflicting_sides(&state, &id, dir.path(), &c0);
    let log = subscribe(&state);

    let (r, fin) = logged(
        &log,
        "cherryPick",
        Some(short(&f_tip)),
        cherrypick_commit_inner(&state, &id, f_tip.clone(), None),
    );
    assert!(
        matches!(r, Ok(CherrypickOutcome::Conflicts { .. })),
        "{r:?}"
    );
    assert_eq!(fin["outcome"], "conflicts");
    let (r, _) = logged(
        &log,
        "cherryPickAbort",
        Some(main.as_str()),
        cherrypick_abort_inner(&state, &id),
    );
    r.expect("abort");

    let (r, _) = logged(
        &log,
        "resetHard",
        Some(short(&c0)),
        reset_branch_command_inner(&state, &id, c0.clone(), ResetMode::Hard),
    );
    r.expect("reset hard");
    let (r, _) = logged(
        &log,
        "resetSoft",
        Some(short(&c0)),
        reset_branch_command_inner(&state, &id, c0.clone(), ResetMode::Soft),
    );
    r.expect("reset soft");
}

/// Stash create names the HEAD branch; a conflicting pop ends `conflicts`.
#[test]
fn stash_create_and_conflicting_pop() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("branch");
    let log = subscribe(&state);

    std::fs::write(dir.path().join("a.txt"), "stashed\n").expect("write");
    let (r, _) = logged(
        &log,
        "stashCreate",
        Some(main.as_str()),
        create_stash_inner(&state, &id, None, StashScope::AllWithUntracked),
    );
    assert!(r.expect("stash").created);
    take(&log);
    write_stage_commit(&state, &id, dir.path(), "a.txt", "committed\n", "clash");
    take(&log);
    let (r, fin) = logged(
        &log,
        "stashPop",
        Some("stash@{0}"),
        pop_stash_inner(&state, &id, 0, false, None),
    );
    assert!(
        matches!(r, Ok(ApplyStashOutcome::Conflicts { .. })),
        "{r:?}"
    );
    assert_eq!(fin["outcome"], "conflicts");
}
