//! P3d CLI-oracle rebase tests (contract §9, `rebase_cli.rs`).
//!
//! Twin-repo pattern (identical to merge_cli.rs): two scratch repos are built
//! by the IDENTICAL scripted CLI setup (fixed dates -> identical base oids).
//! Bonsai's rebase fns run on one; the real `git` CLI runs on the other.
//!
//! Locked comparison rule (§9): committer time = now(), so REPLAYED commit oids
//! differ from the twin. We therefore compare per replayed commit: tree oid,
//! author identity (name/email AND author time — preserved), message, and
//! parent topology; plus the final HEAD tree oid — NOT commit oids.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::process::Command;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::conflict::{get_conflict, resolve_conflict, ConflictResolution};
use bonsai_core::git::merge::{merge_branch, MergeOutcome};
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::rebase::{
    rebase_abort, rebase_branch, rebase_continue, rebase_skip, RebaseOutcome,
};
use bonsai_core::git::remote::fetch_all;
use common::{commit_fixed, git, git_raw, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ------------------------------------------------------------ small helpers

/// Runs `git <args>` expecting FAILURE (e.g. a conflicted `git rebase`).
fn git_fail(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        !out.status.success(),
        "expected git {args:?} to fail, but it succeeded: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

fn head_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

fn tree_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD^{tree}"])
}

fn rev_parse(dir: &Path, rev: &str) -> String {
    git(dir, &["rev-parse", rev])
}

/// Number of commits on `rev` that are not reachable from `base` (i.e. the
/// replayed range `base..rev`).
fn count_ahead(dir: &Path, base: &str, rev: &str) -> usize {
    git(dir, &["rev-list", "--count", &format!("{base}..{rev}")])
        .parse()
        .expect("count parse")
}

/// Conflicted path set per the CLI (`git diff --name-only --diff-filter=U`).
fn cli_conflicted(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = git(dir, &["diff", "--name-only", "--diff-filter=U"])
        .lines()
        .map(String::from)
        .collect();
    v.sort();
    v
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn read(dir: &Path, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(name)).expect("read fixture file")
}

fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

fn has_rebase_dir(dir: &Path) -> bool {
    dir.join(".git").join("rebase-merge").exists() || dir.join(".git").join("rebase-apply").exists()
}

fn checkout(dir: &Path, name: &str) {
    git(dir, &["checkout", name]);
}

/// Per-commit descriptor that survives the "committer time differs" rule: it
/// carries ONLY the fields the contract mandates comparing (tree, author
/// identity + author time, message) — never the commit oid or committer.
#[derive(Debug, PartialEq, Eq)]
struct CInfo {
    tree: String,
    author_name: String,
    author_email: String,
    /// author unix timestamp + ISO (tz) — preserved across a rebase.
    author_time: String,
    message: Vec<u8>,
}

fn commit_info(dir: &Path, rev: &str) -> CInfo {
    CInfo {
        tree: git(dir, &["rev-parse", &format!("{rev}^{{tree}}")]),
        author_name: git(dir, &["log", "-1", "--format=%an", rev]),
        author_email: git(dir, &["log", "-1", "--format=%ae", rev]),
        author_time: git(dir, &["log", "-1", "--format=%at %aI", rev]),
        message: git_raw(dir, &["log", "-1", "--format=%B", rev], &[]),
    }
}

/// The top `count` commits reachable from HEAD, newest first.
fn top_infos(dir: &Path, count: usize) -> Vec<CInfo> {
    let arg = format!("--max-count={count}");
    git(dir, &["rev-list", &arg, "HEAD"])
        .lines()
        .map(|r| commit_info(dir, r))
        .collect()
}

/// Twin `git rebase <onto>` with the editor disabled (never blocks on a message
/// prompt). Asserts success.
fn cli_rebase(dir: &Path, onto: &str) {
    common::git_env(dir, &["rebase", onto], &[("GIT_EDITOR", "true")]);
}

/// Twin `git rebase --continue` (editor disabled).
fn cli_rebase_continue(dir: &Path) {
    common::git_env(dir, &["rebase", "--continue"], &[("GIT_EDITOR", "true")]);
}

/// Twin `git rebase --skip` (editor disabled).
fn cli_rebase_skip(dir: &Path) {
    common::git_env(dir, &["rebase", "--skip"], &[("GIT_EDITOR", "true")]);
}

/// Builds twin repos by applying the same script to two fresh scratch repos.
/// Returns (bonsai, twin). Asserts identical base histories.
fn twin_pair(script: fn(&Path)) -> (tempfile::TempDir, tempfile::TempDir) {
    let bonsai = init_repo();
    let twin = init_repo();
    script(bonsai.path());
    script(twin.path());
    assert_eq!(
        head_oid(bonsai.path()),
        head_oid(twin.path()),
        "fixture scripts must produce identical base histories"
    );
    (bonsai, twin)
}

// ------------------------------------------------------------ fixtures

/// topic = 2 commits touching DISJOINT files; main advances a disjoint file.
/// Ends on `main`. -> clean linear rebase, steps == 2.
fn script_clean_linear(d: &Path) {
    write(d, "a.txt", "a base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "t1.txt", "t1\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic one");
    write(d, "t2.txt", "t2\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic two");
    git(d, &["checkout", "main"]);
    write(d, "m.txt", "m\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main advance");
}

/// topic edits the same line of a.txt as main -> a single pick conflicts.
/// Ends on `main`.
fn script_conflict_one(d: &Path) {
    write(d, "a.txt", "line1\nbase\nline3\n");
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
}

/// One topic commit touching THREE files, all conflicting with main -> single
/// pick conflicts on a/b/c (exercises Ours/Theirs/hand-edit in one step).
fn script_conflict_three(d: &Path) {
    write(d, "a.txt", "a\nX\n");
    write(d, "b.txt", "b\nX\n");
    write(d, "c.txt", "c\nX\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a\ntopic\n");
    write(d, "b.txt", "b\ntopic\n");
    write(d, "c.txt", "c\ntopic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic edit");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "a\nmain\n");
    write(d, "b.txt", "b\nmain\n");
    write(d, "c.txt", "c\nmain\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main edit");
}

/// topic = [t1 edits a.txt (conflicts with main), t2 edits other.txt (clean)].
/// Ends on `main` (which also edits a.txt). Rebasing conflicts on t1.
fn script_skip_first(d: &Path) {
    write(d, "a.txt", "line1\nbase\nline3\n");
    write(d, "other.txt", "other base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic a change");
    write(d, "other.txt", "other topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic other change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main a change");
}

// ============================================================ §9.1 clean linear

#[test]
fn clean_linear_rebase_matches_cli_twin() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_clean_linear);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");
    let onto_tip = rev_parse(b, "main");

    let outcome = rebase_branch(b, "main").expect("rebase");
    match &outcome {
        RebaseOutcome::Rebased { branch, head, steps, .. } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, &2, "topic..main range is 2 commits");
            assert_eq!(head, &head_oid(b), "returned head must be HEAD");
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    cli_rebase(t, "main");

    // Final HEAD tree identical.
    assert_eq!(tree_oid(b), tree_oid(t), "final HEAD tree oid must match twin");
    // Each replayed commit: tree + author identity/time + message, in order.
    assert_eq!(top_infos(b, 2), top_infos(t, 2), "replayed commits differ from twin");
    // Linear parent chain rooted at main's (unmoved) tip.
    assert_eq!(rev_parse(b, "HEAD~2"), onto_tip, "chain must root at main tip");
    assert_eq!(count_ahead(b, "main", "HEAD"), 2, "exactly 2 replayed commits");
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(b), "no rebase-merge dir after completion");
}

// ============================================================ §9.2 up-to-date

#[test]
fn rebasing_onto_an_ancestor_is_up_to_date() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["branch", "topic"]); // topic == base, an ancestor once main advances
    write(d, "a.txt", "main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
    let pre = head_oid(d);

    // onto (topic) is an ancestor of HEAD (main) -> nothing to replay.
    assert_eq!(rebase_branch(d, "topic").expect("rebase"), RebaseOutcome::UpToDate);
    assert_eq!(head_oid(d), pre, "HEAD must not move");

    // Rebasing the current branch onto itself also falls out as UpToDate.
    assert_eq!(rebase_branch(d, "main").expect("self"), RebaseOutcome::UpToDate);
    assert_eq!(head_oid(d), pre);
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

// ============================================================ §9.3 fast-forward

#[test]
fn fast_forward_rebase_matches_cli_twin() {
    require_git!();
    // topic is strictly BEHIND main (HEAD ancestor of onto) -> FF, no rewrites.
    let script = |d: &Path| {
        write(d, "a.txt", "base\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "base");
        git(d, &["branch", "topic"]); // topic pinned at base
        write(d, "a.txt", "advance\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "main advance");
    };
    let (bonsai, twin) = twin_pair(script);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");
    let onto_tip = rev_parse(b, "main");
    let onto_tree = git(b, &["rev-parse", "main^{tree}"]);

    let outcome = rebase_branch(b, "main").expect("rebase");
    assert_eq!(
        outcome,
        RebaseOutcome::FastForwarded {
            branch: "topic".to_string(),
            to: onto_tip.clone(),
        }
    );
    // No rewritten commits: FF oids are byte-identical to the CLI twin.
    cli_rebase(t, "main");
    assert_eq!(head_oid(b), onto_tip, "topic fast-forwarded to main tip");
    assert_eq!(head_oid(b), head_oid(t), "FF HEAD must equal twin (no rewrite)");
    assert_eq!(tree_oid(b), onto_tree, "worktree/tree == onto's tree");
    assert_eq!(count_ahead(b, "main", "HEAD"), 0, "no commits ahead of onto");
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(b));
}

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

// ============================================================ §9.8 remote-tracking onto

#[test]
fn rebase_onto_remote_tracking_matches_cli_twin() {
    require_git!();
    let dir = common::scratch_dir();
    let root = dir.path();

    git(root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");
    let bare_s = bare.to_string_lossy().into_owned();

    // Seed publishes: main = base + advance (a.txt); topic diverges from base
    // with a disjoint change (b.txt) -> a clean rebase onto origin/main.
    git(root, &["clone", "-c", "core.autocrlf=false", &bare_s, "seed"]);
    let seed = root.join("seed");
    git(&seed, &["config", "user.name", "Test User"]);
    git(&seed, &["config", "user.email", "test@example.com"]);
    git(&seed, &["checkout", "-B", "main"]);
    write(&seed, "a.txt", "a base\n");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "base");
    git(&seed, &["checkout", "-b", "topic"]);
    write(&seed, "b.txt", "b topic\n");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "topic change");
    git(&seed, &["checkout", "main"]);
    write(&seed, "a.txt", "a main\n");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "main advance");
    git(&seed, &["push", "origin", "main", "topic"]);

    // Bonsai + twin clones (identical state from the same bare).
    let mut clones = Vec::new();
    for name in ["work", "twin"] {
        git(root, &["clone", "-c", "core.autocrlf=false", &bare_s, name]);
        let c = root.join(name);
        git(&c, &["config", "user.name", "Test User"]);
        git(&c, &["config", "user.email", "test@example.com"]);
        // Create the local topic branch tracking origin/topic, and check it out.
        git(&c, &["checkout", "topic"]);
        clones.push(c);
    }
    let (work, twin) = (&clones[0], &clones[1]);
    assert_eq!(head_oid(work), head_oid(twin));

    // Fetch first (the user's job); already current from clone, but exercise it.
    fetch_all(work).expect("fetch_all");
    let onto_tip = rev_parse(work, "refs/remotes/origin/main");

    let outcome = rebase_branch(work, "origin/main").expect("rebase origin/main");
    match &outcome {
        RebaseOutcome::Rebased { branch, head, steps, .. } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, &1);
            assert_eq!(head, &head_oid(work));
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    cli_rebase(twin, "origin/main");

    assert_eq!(tree_oid(work), tree_oid(twin), "final HEAD tree must match twin");
    assert_eq!(top_infos(work, 1), top_infos(twin, 1), "replayed commit differs from twin");
    assert_eq!(rev_parse(work, "HEAD~1"), onto_tip, "commit sits on origin/main tip");
    assert_eq!(count_ahead(work, "refs/remotes/origin/main", "HEAD"), 1);
    assert_eq!(repo_state(work), git2::RepositoryState::Clean);
}

// ============================================================ §9.9 precondition matrix

#[test]
fn precondition_detached_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["branch", "topic"]);
    git(d, &["checkout", "--detach"]);

    match rebase_branch(d, "topic").expect_err("detached") {
        AppError::Git(m) => assert!(m.contains("detached"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

#[test]
fn precondition_unborn_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    match rebase_branch(repo.path(), "main").expect_err("unborn") {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn precondition_dirty_index_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_clean_linear);
    let d = bonsai.path();
    checkout(d, "topic");
    write(d, "t1.txt", "staged edit\n");
    git(d, &["add", "t1.txt"]);

    match rebase_branch(d, "main").expect_err("dirty index") {
        AppError::Git(m) => assert!(m.contains("uncommitted changes"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

#[test]
fn precondition_rebase_during_op_is_rejected() {
    require_git!();
    // Start a merge into a conflict (state != Clean), then attempt a rebase.
    let (bonsai, _twin) = twin_pair(script_conflict_one);
    let d = bonsai.path();
    git(d, &["branch", "other"]); // a second candidate onto
    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected merge Conflicts, got {other:?}"),
    }

    let err = rebase_branch(d, "other").expect_err("rebase during op");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
}

#[test]
fn precondition_unknown_onto_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = rebase_branch(d, "no-such-branch").expect_err("unknown");
    assert!(
        matches!(err, AppError::BranchNotFound(_)),
        "expected BranchNotFound, got {err:?}"
    );
}

#[test]
fn precondition_missing_identity_is_config_missing_before_worktree() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_clean_linear);
    let d = bonsai.path();
    checkout(d, "topic");
    // Blank the repo-local identity (an explicit empty value overrides any
    // global identity for this repo -> resolve_signature reports it missing).
    git(d, &["config", "user.name", ""]);
    git(d, &["config", "user.email", ""]);

    let err = rebase_branch(d, "main").expect_err("no identity");
    assert!(
        matches!(err, AppError::ConfigMissing(_)),
        "expected ConfigMissing, got {err:?}"
    );
    // Surfaces BEFORE the worktree is touched: nothing left behind.
    assert_eq!(repo_state(d), git2::RepositoryState::Clean, "state must stay Clean");
    assert!(!has_rebase_dir(d), "no rebase-merge dir left behind");
}

// ============================================================ §9.10 backend commit guard

#[test]
fn plain_commit_during_paused_rebase_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict_one);
    let d = bonsai.path();
    checkout(d, "topic");
    match rebase_branch(d, "main").expect("rebase") {
        RebaseOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    // Resolve so the only thing blocking a plain commit is the op-state guard.
    resolve_conflict(d, "a.txt", ConflictResolution::Ours).expect("resolve");

    let err = create_commit(d, "sneaky plain commit", None, false).expect_err("gated");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
    // The rebase state is untouched by the refused commit.
    assert!(has_rebase_dir(d), "rebase state must persist after the refused commit");
}

#[test]
fn continue_and_skip_without_a_rebase_are_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    assert!(matches!(
        rebase_continue(d).expect_err("no rebase"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        rebase_skip(d).expect_err("no rebase"),
        AppError::NoOperationInProgress(_)
    ));
}

// ============================================================ §9.11 empty-pick drop

#[test]
fn already_applied_pick_is_dropped_like_cli() {
    require_git!();
    // topic's T1 adds b.txt "feature"; main independently adds an IDENTICAL
    // b.txt "feature" (as if cherry-picked earlier) -> T1 becomes empty on
    // replay and is dropped, exactly like default `git rebase`.
    let script = |d: &Path| {
        write(d, "a.txt", "base\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "base");
        git(d, &["checkout", "-b", "topic"]);
        write(d, "b.txt", "feature\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "add feature"); // T1 (will be empty on replay)
        write(d, "c.txt", "other\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "add other"); // T2 (survives)
        git(d, &["checkout", "main"]);
        write(d, "b.txt", "feature\n"); // identical content already on main
        git(d, &["add", "-A"]);
        commit_fixed(d, "main adds feature too");
    };
    let (bonsai, twin) = twin_pair(script);
    let (b, t) = (bonsai.path(), twin.path());
    checkout(b, "topic");
    checkout(t, "topic");

    match rebase_branch(b, "main").expect("rebase") {
        RebaseOutcome::Rebased { branch, .. } => assert_eq!(branch, "topic"),
        other => panic!("expected Rebased, got {other:?}"),
    }

    cli_rebase(t, "main");

    // The empty pick is DROPPED in both: exactly ONE commit replayed.
    let bonsai_ahead = count_ahead(b, "main", "HEAD");
    let twin_ahead = count_ahead(t, "main", "HEAD");
    assert_eq!(bonsai_ahead, twin_ahead, "replayed-commit count must match twin");
    assert_eq!(bonsai_ahead, 1, "the already-applied pick must be dropped");
    assert_eq!(tree_oid(b), tree_oid(t), "final HEAD tree must match twin");
    assert_eq!(top_infos(b, 1), top_infos(t, 1), "surviving commit differs from twin");
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
}
