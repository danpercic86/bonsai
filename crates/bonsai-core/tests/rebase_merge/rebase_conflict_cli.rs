//! P3d CLI-oracle rebase tests — the conflict flow: pause, continue, skip, and
//! abort (contract §9.4–§9.7).
//!
//! Split out of `rebase_cli.rs`; the twin-repo scaffolding, helpers, and
//! fixtures live in `rebase_support.rs`. Same locked comparison rule (§9):
//! committer time = now(), so REPLAYED commit oids differ from the twin — we
//! compare tree oid, author identity, message, and parent topology only.

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::conflict::{get_conflict, resolve_conflict, ConflictResolution};
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::rebase::{
    rebase_abort, rebase_branch, rebase_continue, rebase_skip, RebaseOutcome,
};
use crate::common;
use crate::common::{commit_fixed, git};
use crate::rebase_support::{
    checkout, cli_conflicted, cli_rebase_continue, cli_rebase_skip, count_ahead, git_fail,
    has_rebase_dir, head_oid, read, repo_state, require_git, rev_parse, script_conflict_one,
    script_conflict_three, script_skip_first, top_infos, tree_oid, twin_pair, write,
};

// ============================================================ §9.4 conflict -> paused

#[test]
fn conflicting_rebase_pauses_with_matching_state() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_conflict_one);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");
    let onto_tip = rev_parse(b, "main");

    let (paths, cur, total) = match rebase_branch(b, "main").expect("rebase") {
        RebaseOutcome::Conflicts { paths, current_step, total_steps } => {
            (paths, current_step, total_steps)
        }
        other => panic!("expected Conflicts, got {other:?}"),
    };
    assert_eq!(paths, vec!["a.txt".to_string()]);
    assert_eq!(total, 1, "single-commit topic -> total_steps 1");
    assert_eq!(cur, 1, "paused at the only step");

    // Twin conflicts on the same set.
    checkout(t, "topic");
    git_fail(t, &["rebase", "main"]);
    assert_eq!(paths, cli_conflicted(t), "conflicted path sets differ from twin");

    // State is a rebase state.
    assert!(
        matches!(
            repo_state(b),
            git2::RepositoryState::RebaseMerge
                | git2::RepositoryState::Rebase
                | git2::RepositoryState::RebaseInteractive
        ),
        "expected a rebase state, got {:?}",
        repo_state(b)
    );

    // read_op_state mirrors the paused engine's counters (§2 assertion).
    match read_op_state(b).expect("op state") {
        RepoOpState::Rebase { head_name, onto, current_step, total_steps } => {
            assert_eq!(head_name, Some("topic".to_string()));
            assert_eq!(onto, Some(onto_tip), "onto must be the main tip oid");
            assert_eq!(current_step, cur, "op-state current_step must match outcome");
            assert_eq!(total_steps, total, "op-state total_steps must match outcome");
        }
        other => panic!("expected Rebase op state, got {other:?}"),
    }

    // Worktree carries conflict markers; get_conflict is non-empty.
    let cf = get_conflict(b, "a.txt").expect("get_conflict");
    assert!(!cf.binary && !cf.too_large && !cf.missing, "expected a text marker view");
    assert!(cf.text.contains("<<<<<<<"), "missing <<<<<<< marker: {}", cf.text);
    assert!(cf.text.contains("======="), "missing ======= marker");
    assert!(cf.text.contains(">>>>>>>"), "missing >>>>>>> marker");
}

// ============================================================ §9.5 continue

#[test]
fn continue_after_resolving_matches_cli_twin() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_conflict_three);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");

    match rebase_branch(b, "main").expect("rebase") {
        RebaseOutcome::Conflicts { paths, .. } => {
            assert_eq!(paths, vec!["a.txt".to_string(), "b.txt".to_string(), "c.txt".to_string()]);
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }

    // Continuing with conflicts still present is rejected.
    let err = rebase_continue(b).expect_err("unresolved");
    assert!(
        matches!(err, AppError::UnresolvedConflicts(_)),
        "expected UnresolvedConflicts, got {err:?}"
    );

    // Resolve across cells: a=Ours, b=Theirs, c=hand-edit + MarkResolved.
    resolve_conflict(b, "a.txt", ConflictResolution::Ours).expect("resolve a");
    resolve_conflict(b, "b.txt", ConflictResolution::Theirs).expect("resolve b");
    write(b, "c.txt", "c\nmerged\n");
    resolve_conflict(b, "c.txt", ConflictResolution::MarkResolved).expect("resolve c");

    let outcome = rebase_continue(b).expect("continue");
    match &outcome {
        RebaseOutcome::Rebased { branch, head, steps, .. } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, &1);
            assert_eq!(head, &head_oid(b));
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    // Twin: identical resolutions via the CLI, then --continue.
    git_fail(t, &["rebase", "main"]);
    git(t, &["checkout", "--ours", "--", "a.txt"]);
    git(t, &["add", "a.txt"]);
    git(t, &["checkout", "--theirs", "--", "b.txt"]);
    git(t, &["add", "b.txt"]);
    write(t, "c.txt", "c\nmerged\n");
    git(t, &["add", "c.txt"]);
    cli_rebase_continue(t);

    assert_eq!(tree_oid(b), tree_oid(t), "final HEAD tree must match twin");
    assert_eq!(top_infos(b, 1), top_infos(t, 1), "replayed commit differs from twin");
    assert_eq!(count_ahead(b, "main", "HEAD"), 1);
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(b));
}

// ============================================================ §9.6 skip

/// §9.6 skip semantics, validated on a rebase where a LATER op conflicts (the
/// first op replays cleanly and is committed BEFORE the skip). `rebase_skip`
/// drops the offending commit and completes; the result matches the CLI twin's
/// `git rebase --skip` byte-for-byte (final tree, surviving commit).
///
/// The contract's §9.6 exact wording (skip the FIRST conflicting commit) is
/// covered by `skip_first_conflicting_op_works` below — a historical
/// skip-on-first-op state corruption was fixed in 8219ebd, so both paths are
/// exercised against the CLI oracle.
#[test]
fn skip_later_conflicting_commit_matches_cli_twin() {
    require_git!();
    // topic = [t1 clean disjoint change, t2 conflicts with main]. Rebasing
    // replays t1 cleanly, then conflicts on t2 (step 2/2).
    let script = |d: &Path| {
        write(d, "a.txt", "line1\nbase\nline3\n");
        write(d, "other.txt", "other base\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "base");
        git(d, &["checkout", "-b", "topic"]);
        write(d, "other.txt", "other topic\n"); // t1: clean
        git(d, &["add", "-A"]);
        commit_fixed(d, "topic other change");
        write(d, "a.txt", "line1\ntopic\nline3\n"); // t2: conflicts
        git(d, &["add", "-A"]);
        commit_fixed(d, "topic a change");
        git(d, &["checkout", "main"]);
        write(d, "a.txt", "line1\nmain\nline3\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "main a change");
    };
    let (bonsai, twin) = twin_pair(script);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");
    let onto_tip = rev_parse(b, "main");

    // Second pick (topic a change) conflicts.
    match rebase_branch(b, "main").expect("rebase") {
        RebaseOutcome::Conflicts { paths, current_step, .. } => {
            assert_eq!(paths, vec!["a.txt".to_string()]);
            assert_eq!(current_step, 2, "conflict is on the SECOND replayed commit");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }

    // Skip drops the offender; the already-replayed t1 survives.
    match rebase_skip(b).expect("skip") {
        RebaseOutcome::Rebased { head, .. } => {
            assert_eq!(head, &head_oid(b) as &str);
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    // Twin: --skip drops the offender.
    git_fail(t, &["rebase", "main"]);
    cli_rebase_skip(t);

    assert_eq!(tree_oid(b), tree_oid(t), "final HEAD tree must match twin");
    // Skipped commit absent from both: only the clean t1 replayed onto main.
    assert_eq!(count_ahead(b, "main", "HEAD"), 1, "skipped commit must be absent");
    assert_eq!(count_ahead(t, "main", "HEAD"), 1);
    assert_eq!(rev_parse(b, "HEAD~1"), onto_tip, "surviving commit sits on main tip");
    assert_eq!(top_infos(b, 1), top_infos(t, 1), "surviving commit differs from twin");
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(b));
}

/// §9.6 exact requirement: skip the FIRST conflicting commit (no commit
/// replayed yet). REGRESSION test for a bug fixed in 8219ebd: the original
/// `repo.reset(HEAD, Hard)` step deleted the on-disk `rebase-merge` state
/// (msgnum et al.) on Windows/libgit2, so the follow-up `rebase.next()` failed;
/// the fix reverts the conflicted paths only (paths-only reset), leaving the
/// sequencer state intact. Bonsai now matches `git rebase --skip` exactly —
/// final tree, surviving commit, and the `branch` field in the outcome.
#[test]
fn skip_first_conflicting_op_works() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_skip_first);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");
    let onto_tip = rev_parse(b, "main");

    match rebase_branch(b, "main").expect("rebase") {
        RebaseOutcome::Conflicts { paths, current_step, .. } => {
            assert_eq!(paths, vec!["a.txt".to_string()]);
            assert_eq!(current_step, 1, "conflict is on the FIRST replayed commit");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }

    match rebase_skip(b).expect("skip") {
        RebaseOutcome::Rebased { branch, head, .. } => {
            assert_eq!(branch, "topic");
            assert_eq!(head, &head_oid(b) as &str);
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    git_fail(t, &["rebase", "main"]);
    cli_rebase_skip(t);

    assert_eq!(tree_oid(b), tree_oid(t), "final HEAD tree must match twin");
    assert_eq!(count_ahead(b, "main", "HEAD"), 1, "skipped commit must be absent");
    assert_eq!(count_ahead(t, "main", "HEAD"), 1);
    assert_eq!(rev_parse(b, "HEAD~1"), onto_tip, "surviving commit sits on main tip");
    assert_eq!(top_infos(b, 1), top_infos(t, 1), "surviving commit differs from twin");
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(b));
}

// ============================================================ §9.7 abort

/// DIVERGENCE FROM CONTRACT §3.1.5 / §9.7 (reported to the orchestrator):
/// the contract (copied from merge) claims a rebase may START with unstaged
/// worktree changes and that an unrelated unstaged edit survives an abort.
/// That is FALSE for rebase — both libgit2 (`repo.rebase()`) and the `git`
/// CLI refuse to start a rebase while the worktree has ANY unstaged change,
/// so there is no in-progress rebase whose abort could preserve the edit.
/// Bonsai's behavior MATCHES the CLI. This test pins the actual, correct
/// contract: (1) a dirty START is rejected and leaves everything untouched;
/// (2) abort from a clean start restores HEAD/index/worktree byte-identically.
#[test]
fn dirty_start_is_rejected_like_the_cli_then_abort_restores_byte_identically() {
    require_git!();
    // Conflict on a.txt; unrelated.txt is committed at base and never touched
    // by the rebase.
    let script = |d: &Path| {
        write(d, "a.txt", "line1\nbase\nline3\n");
        write(d, "unrelated.txt", "orig\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "base");
        git(d, &["checkout", "-b", "topic"]);
        write(d, "a.txt", "line1\ntopic\nline3\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "topic change");
        git(d, &["checkout", "main"]);
        write(d, "a.txt", "line1\nmain\nline3\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "main change");
    };

    // -- Part 1: a dirty worktree refuses to START (matches `git rebase`). ----
    let (bonsai, twin) = twin_pair(script);
    let (d, t) = (bonsai.path(), twin.path());
    checkout(d, "topic");
    checkout(t, "topic");

    let unrelated = "edited but not staged\n";
    write(d, "unrelated.txt", unrelated);
    let pre_head = head_oid(d);

    let err = rebase_branch(d, "main").expect_err("dirty worktree refuses to start");
    assert!(
        matches!(err, AppError::Git(_) | AppError::CheckoutConflict(_)),
        "expected a Git/CheckoutConflict rejection, got {err:?}"
    );
    // Nothing mutated; the unstaged edit is untouched (no rebase ever ran).
    assert_eq!(repo_state(d), git2::RepositoryState::Clean, "state must stay Clean");
    assert!(!has_rebase_dir(d), "no rebase state may be left behind");
    assert_eq!(head_oid(d), pre_head, "HEAD must not move");
    assert_eq!(
        std::fs::read_to_string(d.join("unrelated.txt")).expect("read unrelated"),
        unrelated,
        "the unstaged edit must survive the refused start"
    );
    // Twin (`git rebase`) refuses identically.
    write(t, "unrelated.txt", unrelated);
    git_fail(t, &["rebase", "main"]);

    // -- Part 2: abort from a CLEAN start restores byte-identically. ---------
    let (bonsai2, _twin2) = twin_pair(script);
    let d2 = bonsai2.path();
    checkout(d2, "topic");
    let pre_a = read(d2, "a.txt");
    let pre_unrelated = read(d2, "unrelated.txt");
    let pre_head2 = head_oid(d2);

    match rebase_branch(d2, "main").expect("rebase") {
        RebaseOutcome::Conflicts { paths, .. } => assert_eq!(paths, vec!["a.txt".to_string()]),
        other => panic!("expected Conflicts, got {other:?}"),
    }

    rebase_abort(d2).expect("abort");

    assert_eq!(head_oid(d2), pre_head2, "branch oid must return to the pre-rebase tip");
    assert_eq!(repo_state(d2), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(d2), "no rebase-merge dir after abort");
    assert_eq!(git(d2, &["write-tree"]), tree_oid(d2), "index tree must equal HEAD tree");
    assert!(git(d2, &["ls-files", "-u"]).is_empty(), "no conflict stages may remain");
    assert_eq!(read(d2, "a.txt"), pre_a, "conflicted file restored to pre-rebase bytes");
    assert_eq!(read(d2, "unrelated.txt"), pre_unrelated, "untouched file byte-identical");

    // Abort with no rebase in progress -> NoOperationInProgress.
    let err = rebase_abort(d2).expect_err("no rebase");
    assert!(
        matches!(err, AppError::NoOperationInProgress(_)),
        "expected NoOperationInProgress, got {err:?}"
    );
}
