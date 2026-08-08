//! P3c CLI-oracle merge tests (contract §9, `merge_cli.rs`).
//!
//! Twin-repo pattern: two scratch repos are built by the IDENTICAL scripted
//! CLI setup (fixed dates -> identical base oids). Bonsai's merge fns run on
//! one; the real `git` CLI runs on the other; results are compared
//! byte-exactly (tree oids, parents, messages, conflicted sets).
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::process::Command;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::conflict::{resolve_conflict, ConflictResolution};
use bonsai_core::git::merge::{abort_merge, commit_merge, merge_branch, MergeOutcome};
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use common::{commit_fixed, git, git_raw, init_repo, FIXED_DATE};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ------------------------------------------------------------ small helpers

/// Runs `git <args>` expecting FAILURE (e.g. a conflicted `git merge`).
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

/// Parent oids of HEAD, in order.
fn parents(dir: &Path) -> Vec<String> {
    git(dir, &["log", "-1", "--format=%P"])
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// Raw commit-message body of HEAD (byte-exact).
fn message(dir: &Path) -> Vec<u8> {
    git_raw(dir, &["log", "-1", "--format=%B"], &[])
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

fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

/// Number of entries on the stash stack (P8 autostash assertions). Uses the
/// git CLI since these tests already `require_git!`; git2's stash_save2 writes
/// the standard refs/stash + reflog, so `git stash list` sees it.
fn stash_count(dir: &Path) -> usize {
    git(dir, &["stash", "list"])
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Twin `git merge --no-edit <name>` with fixed committer/author dates.
fn cli_merge(dir: &Path, name: &str) {
    common::git_env(
        dir,
        &["merge", "--no-edit", name],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

// ------------------------------------------------------------ fixtures

/// Diverged branches touching DISJOINT files -> clean true merge.
fn script_clean_diverged(d: &Path) {
    write(d, "a.txt", "a base\n");
    write(d, "b.txt", "b base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "b.txt", "b topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "a main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
}

/// Same line edited on both sides -> guaranteed conflict on a.txt.
fn script_conflict(d: &Path) {
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

/// TWO guaranteed-conflict files (a.txt, b.txt).
fn script_conflict_two_files(d: &Path) {
    write(d, "a.txt", "a base\n");
    write(d, "b.txt", "b base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a topic\n");
    write(d, "b.txt", "b topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "a main\n");
    write(d, "b.txt", "b main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
}

/// Builds twin repos by applying the same script to two fresh scratch repos.
/// Returns (bonsai, twin).
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

// ============================================================ §9.1 clean merge

#[test]
fn clean_merge_matches_cli_twin() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_clean_diverged);
    let pre_head = head_oid(bonsai.path());

    let outcome = merge_branch(bonsai.path(), "topic").expect("merge");
    let oid = match outcome {
        MergeOutcome::Merged { oid, .. } => oid,
        other => panic!("expected Merged, got {other:?}"),
    };
    assert_eq!(oid, head_oid(bonsai.path()), "returned oid must be HEAD");

    cli_merge(twin.path(), "topic");

    // Tree byte-identical, parents identical (HEAD first), message identical.
    assert_eq!(tree_oid(bonsai.path()), tree_oid(twin.path()));
    let p = parents(bonsai.path());
    assert_eq!(p, parents(twin.path()));
    assert_eq!(p[0], pre_head, "first parent must be pre-merge HEAD");
    assert_eq!(
        message(bonsai.path()),
        message(twin.path()),
        "merge message differs from `git merge` (expected `Merge branch 'topic'`)"
    );

    // State Clean, MERGE_HEAD gone.
    assert_eq!(repo_state(bonsai.path()), git2::RepositoryState::Clean);
    assert!(
        !bonsai.path().join(".git").join("MERGE_HEAD").exists(),
        "MERGE_HEAD must be removed after auto-commit"
    );
}

// ============================================================ §9.2 fast-forward

#[test]
fn fast_forward_matches_cli_twin() {
    require_git!();
    let script = |d: &Path| {
        write(d, "a.txt", "base\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "base");
        git(d, &["checkout", "-b", "topic"]);
        write(d, "a.txt", "topic\n");
        git(d, &["add", "-A"]);
        commit_fixed(d, "topic change");
        git(d, &["checkout", "main"]);
    };
    let (bonsai, twin) = twin_pair(script);

    let outcome = merge_branch(bonsai.path(), "topic").expect("merge");
    cli_merge(twin.path(), "topic"); // fast-forwards

    let twin_head = head_oid(twin.path());
    assert_eq!(
        outcome,
        MergeOutcome::FastForwarded {
            branch: "main".to_string(),
            to: twin_head.clone(),
            stashed: false,
        }
    );
    assert_eq!(head_oid(bonsai.path()), twin_head);
    // No new commit: HEAD is the topic tip with a single parent.
    assert_eq!(parents(bonsai.path()).len(), 1);
    assert_eq!(repo_state(bonsai.path()), git2::RepositoryState::Clean);
}

// ============================================================ §9.3 up-to-date

#[test]
fn merging_an_ancestor_is_up_to_date() {
    require_git!();
    let bonsai = init_repo();
    let d = bonsai.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["branch", "topic"]); // topic == base, an ancestor after main advances
    write(d, "a.txt", "main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
    let pre = head_oid(d);

    assert_eq!(merge_branch(d, "topic").expect("merge"), MergeOutcome::UpToDate);
    assert_eq!(head_oid(d), pre, "HEAD must not move");

    // Merging the current branch by name also falls out as UpToDate.
    assert_eq!(merge_branch(d, "main").expect("merge self"), MergeOutcome::UpToDate);
    assert_eq!(head_oid(d), pre);
}

// ============================================================ §9.4 remote-tracking merge

#[test]
fn remote_tracking_merge_matches_cli_twin() {
    require_git!();
    let dir = common::scratch_dir();
    let root = dir.path();

    git(root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");
    let bare_s = bare.to_string_lossy().into_owned();

    // Seed publishes: main = base + "main change"; topic diverges from base.
    // core.autocrlf is set AT CLONE TIME so the checkout itself is LF-clean
    // (a post-clone `git config` flip would make checked-out files look
    // locally modified under a CRLF-converting global config).
    git(root, &["clone", "-c", "core.autocrlf=false", &bare_s, "seed"]);
    let seed = root.join("seed");
    git(&seed, &["config", "user.name", "Test User"]);
    git(&seed, &["config", "user.email", "test@example.com"]);
    git(&seed, &["checkout", "-B", "main"]);
    write(&seed, "a.txt", "a base\n");
    write(&seed, "b.txt", "b base\n");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "base");
    git(&seed, &["checkout", "-b", "topic"]);
    write(&seed, "b.txt", "b topic\n");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "topic change");
    git(&seed, &["checkout", "main"]);
    write(&seed, "a.txt", "a main\n");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "main change");
    git(&seed, &["push", "origin", "main", "topic"]);

    // Bonsai + twin clones (identical state from the same bare).
    let mut clones = Vec::new();
    for name in ["work", "twin"] {
        git(root, &["clone", "-c", "core.autocrlf=false", &bare_s, name]);
        let c = root.join(name);
        git(&c, &["config", "user.name", "Test User"]);
        git(&c, &["config", "user.email", "test@example.com"]);
        clones.push(c);
    }
    let (work, twin) = (&clones[0], &clones[1]);
    assert_eq!(head_oid(work), head_oid(twin));
    let pre_head = head_oid(work);

    let outcome = merge_branch(work, "origin/topic").expect("merge origin/topic");
    let oid = match outcome {
        MergeOutcome::Merged { oid, .. } => oid,
        other => panic!("expected Merged, got {other:?}"),
    };
    assert_eq!(oid, head_oid(work));

    cli_merge(twin, "origin/topic");

    assert_eq!(tree_oid(work), tree_oid(twin));
    let p = parents(work);
    assert_eq!(p, parents(twin));
    assert_eq!(p[0], pre_head);
    assert_eq!(
        message(work),
        message(twin),
        "expected `Merge remote-tracking branch 'origin/topic'` to match the CLI"
    );
    assert_eq!(
        String::from_utf8_lossy(&message(work)).trim(),
        "Merge remote-tracking branch 'origin/topic'"
    );
}

// ============================================================ §9.5 guaranteed conflict

#[test]
fn conflicted_merge_matches_cli_conflicted_set() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_conflict);

    let outcome = merge_branch(bonsai.path(), "topic").expect("merge");
    let paths = match outcome {
        MergeOutcome::Conflicts { paths, .. } => paths,
        other => panic!("expected Conflicts, got {other:?}"),
    };

    git_fail(twin.path(), &["merge", "topic"]);
    assert_eq!(paths, cli_conflicted(twin.path()), "conflicted path sets differ");

    assert_eq!(repo_state(bonsai.path()), git2::RepositoryState::Merge);

    let merge_msg = std::fs::read_to_string(bonsai.path().join(".git").join("MERGE_MSG"))
        .expect("read MERGE_MSG");
    assert_eq!(
        merge_msg, "Merge branch 'topic'\n\nConflicts:\n\ta.txt\n",
        "MERGE_MSG must carry the sorted Conflicts block"
    );

    match read_op_state(bonsai.path()).expect("op state") {
        RepoOpState::Merge { incoming, message } => {
            assert_eq!(incoming, "topic");
            assert!(message.starts_with("Merge branch 'topic'"), "got: {message}");
            assert!(message.contains("Conflicts:\n\ta.txt"), "got: {message}");
        }
        other => panic!("expected Merge op state, got {other:?}"),
    }
}

// ============================================================ §9.6 preconditions

#[test]
fn detached_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["branch", "topic"]);
    git(d, &["checkout", "--detach"]);

    let err = merge_branch(d, "topic").expect_err("detached");
    match err {
        AppError::Git(m) => assert!(m.contains("detached"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn unborn_head_is_rejected() {
    require_git!();
    let repo = init_repo();
    let err = merge_branch(repo.path(), "topic").expect_err("unborn");
    match err {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

/// P8 §2.1 removed the pre-P8 dirty-INDEX refusal: a STAGED change to a file
/// the merge does not touch is now AUTOSTASHED, the (non-FF, clean) merge
/// proceeds and auto-commits, then the stash is re-applied. Matrix row #3
/// (dirty + clean normal merge + clean pop) -> `Merged { stashed: true }`.
/// Per OPEN Q#1 (no REINSTATE_INDEX) the change comes back UNSTAGED.
#[test]
fn staged_change_is_autostashed_and_merge_proceeds() {
    require_git!();
    // script_clean_diverged: topic edits b.txt, main edits a.txt -> non-FF
    // clean merge on disjoint files. The staged edit below is to a.txt, which
    // the merge leaves at main's version, so the pop applies cleanly.
    let (bonsai, _twin) = twin_pair(script_clean_diverged);
    let d = bonsai.path();
    write(d, "a.txt", "staged edit\n");
    git(d, &["add", "a.txt"]);

    let outcome = merge_branch(d, "topic").expect("merge");
    let oid = match &outcome {
        MergeOutcome::Merged { oid, stashed } => {
            assert!(*stashed, "staged change must be autostashed -> stashed:true");
            oid.clone()
        }
        other => panic!("expected Merged{{stashed:true}}, got {other:?}"),
    };

    // A real 2-parent merge commit landed.
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(oid, head_oid(d), "returned oid must be HEAD");
    assert_eq!(parents(d).len(), 2, "normal merge -> 2-parent commit");
    // Disjoint clean merge kept both sides.
    assert_eq!(std::fs::read_to_string(d.join("b.txt")).expect("b"), "b topic\n");

    // The staged change's CONTENT survives...
    assert_eq!(
        std::fs::read_to_string(d.join("a.txt")).expect("a"),
        "staged edit\n",
        "the autostashed change content must be restored"
    );
    // ...and returns UNSTAGED (OPEN Q#1: no REINSTATE_INDEX). Nothing staged;
    // a.txt shows only as a worktree modification.
    assert!(
        git(d, &["diff", "--cached", "--name-only"]).trim().is_empty(),
        "OPEN Q#1: the restored change must NOT be re-staged"
    );
    assert_eq!(
        git(d, &["diff", "--name-only"]).trim(),
        "a.txt",
        "the restored change must be an UNSTAGED worktree modification"
    );
    assert_eq!(stash_count(d), 0, "clean pop -> stash applied and dropped");
}

#[test]
fn merge_during_merge_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict);
    let d = bonsai.path();
    git(d, &["branch", "other"]); // second candidate branch
    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }

    let err = merge_branch(d, "other").expect_err("nested merge");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
}

#[test]
fn unknown_branch_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = merge_branch(d, "no-such-branch").expect_err("unknown");
    assert!(
        matches!(err, AppError::BranchNotFound(_)),
        "expected BranchNotFound, got {err:?}"
    );
}

/// P8: an UNSTAGED edit to a merge-touched file is no longer a pre-flight
/// CheckoutConflict. The edit is autostashed, the (non-FF, clean) merge runs
/// and auto-commits, then re-applying the stash onto the merged tree conflicts
/// on that same file. Matrix row #4 -> `StashPopConflicts { head, paths }`:
/// state Clean (a conflicted stash-apply is not a merge op), worktree has
/// markers, the stash is RETAINED. Pinned against real `git merge --autostash`.
#[test]
fn unstaged_edit_to_merge_touched_file_autostashes_then_pop_conflicts() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_clean_diverged);
    let d = bonsai.path();
    // topic changes b.txt; make an UNSTAGED local edit to the SAME file.
    let local = "b local unstaged\n";
    write(d, "b.txt", local);

    let (head, paths) = match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::StashPopConflicts { head, paths } => (head, paths),
        other => panic!("expected StashPopConflicts, got {other:?}"),
    };
    assert_eq!(paths, vec!["b.txt".to_string()], "b.txt conflicted on the pop");
    assert_eq!(head, head_oid(d), "head = the new merge-commit oid");

    // A conflicted stash-apply is NOT a merge op: state stays Clean.
    assert_eq!(repo_state(d), git2::RepositoryState::Clean, "state must be Clean");
    assert!(!d.join(".git").join("MERGE_HEAD").exists(), "no MERGE_HEAD");
    assert_eq!(parents(d).len(), 2, "the merge itself committed (2 parents)");
    // a.txt is main's side (merge untouched); b.txt has conflict markers.
    assert_eq!(std::fs::read_to_string(d.join("a.txt")).expect("a"), "a main\n");
    let b = std::fs::read_to_string(d.join("b.txt")).expect("b");
    assert!(
        b.contains("<<<<<<<") && b.contains(">>>>>>>"),
        "b.txt must carry conflict markers, got:\n{b}"
    );
    assert_eq!(stash_count(d), 1, "conflicting pop RETAINS the stash");

    // Oracle: real `git merge --autostash topic` on the twin with the same
    // unstaged edit. The COMMITTED merge tree is stable regardless of the
    // post-commit pop, so compare HEAD trees; also confirm git likewise keeps
    // a stash and leaves markers. (Commit oids differ: timestamps differ.)
    write(twin.path(), "b.txt", local);
    let _ = Command::new("git")
        .args(["merge", "--autostash", "--no-edit", "topic"])
        .current_dir(twin.path())
        .output()
        .expect("run git merge --autostash");
    assert_eq!(
        tree_oid(d),
        tree_oid(twin.path()),
        "our committed merge tree must equal `git merge --autostash`'s"
    );
    assert_eq!(
        stash_count(twin.path()),
        1,
        "real git also RETAINS the autostash on a conflicting re-apply"
    );
    let twin_b = std::fs::read_to_string(twin.path().join("b.txt")).expect("twin b");
    assert!(
        twin_b.contains("<<<<<<<") && twin_b.contains(">>>>>>>"),
        "real git also leaves conflict markers in b.txt"
    );
}

// ============================================================ §9.7 commit_merge

#[test]
fn commit_merge_after_resolving_matches_cli_twin() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_conflict_two_files);
    let pre_head = head_oid(bonsai.path());

    // Bonsai: merge -> conflicts on a.txt + b.txt; resolve a=Ours, b=Theirs.
    match merge_branch(bonsai.path(), "topic").expect("merge") {
        MergeOutcome::Conflicts { paths, .. } => {
            assert_eq!(paths, vec!["a.txt".to_string(), "b.txt".to_string()])
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    resolve_conflict(bonsai.path(), "a.txt", ConflictResolution::Ours).expect("resolve a");
    resolve_conflict(bonsai.path(), "b.txt", ConflictResolution::Theirs).expect("resolve b");

    let msg = std::fs::read_to_string(bonsai.path().join(".git").join("MERGE_MSG"))
        .expect("MERGE_MSG")
        .trim_end()
        .to_string();
    let result = commit_merge(bonsai.path(), &msg, None).expect("commit merge");
    assert_eq!(result.oid, head_oid(bonsai.path()));
    assert_eq!(result.branch.as_deref(), Some("main"));
    assert_eq!(result.summary, "Merge branch 'topic'");

    // Twin: identical resolutions via the CLI, commit with the same text.
    git_fail(twin.path(), &["merge", "topic"]);
    git(twin.path(), &["checkout", "--ours", "--", "a.txt"]);
    git(twin.path(), &["add", "a.txt"]);
    git(twin.path(), &["checkout", "--theirs", "--", "b.txt"]);
    git(twin.path(), &["add", "b.txt"]);
    // Untracked helper file inside the twin repo — never staged, so it does
    // not affect the committed tree.
    let msg_file = twin.path().join("merge-msg.txt");
    std::fs::write(&msg_file, &msg).expect("write msg file");
    common::git_env(
        twin.path(),
        &["commit", "-F", &msg_file.to_string_lossy()],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );

    assert_eq!(tree_oid(bonsai.path()), tree_oid(twin.path()));
    let p = parents(bonsai.path());
    assert_eq!(p, parents(twin.path()));
    assert_eq!(p.len(), 2);
    assert_eq!(p[0], pre_head, "HEAD must be the first parent");
    assert_eq!(message(bonsai.path()), message(twin.path()));

    assert_eq!(repo_state(bonsai.path()), git2::RepositoryState::Clean);
    assert!(!bonsai.path().join(".git").join("MERGE_HEAD").exists());
}

#[test]
fn commit_merge_with_unresolved_conflicts_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict_two_files);
    let d = bonsai.path();
    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    resolve_conflict(d, "a.txt", ConflictResolution::Ours).expect("resolve a");
    // b.txt still conflicted.
    let err = commit_merge(d, "msg", None).expect_err("unresolved");
    assert!(
        matches!(err, AppError::UnresolvedConflicts(_)),
        "expected UnresolvedConflicts, got {err:?}"
    );
}

#[test]
fn commit_merge_without_a_merge_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = commit_merge(d, "msg", None).expect_err("no merge");
    assert!(
        matches!(err, AppError::NoOperationInProgress(_)),
        "expected NoOperationInProgress, got {err:?}"
    );
}

// ============================================================ §9.8 abort_merge

/// P8 + abort: a PRE-merge unstaged edit is moved onto the autostash before
/// the merge runs (matrix row #5, deferred re-apply). So during the paused
/// merge and after `abort_merge`, that edit is NOT in the worktree — the file
/// sits at its HEAD version and the edit is safe at stash@{0}. This differs
/// from pre-P8, where the edit stayed in the worktree. Abort still restores the
/// merge-touched file to HEAD; the retained stash guarantees no data loss.
#[test]
fn abort_after_autostashed_merge_keeps_unrelated_edit_on_stash() {
    require_git!();
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
    let (bonsai, _twin) = twin_pair(script);
    let d = bonsai.path();

    // Pre-merge UNSTAGED edit to a file the merge does not touch.
    let unrelated = "edited but not staged\n";
    write(d, "unrelated.txt", unrelated);
    let pre_a = std::fs::read(d.join("a.txt")).expect("read a.txt"); // main's a.txt
    let pre_head = head_oid(d);

    // Dirty tree -> autostash -> conflicting merge pauses; stash RETAINED.
    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { paths, stashed } => {
            assert_eq!(paths, vec!["a.txt".to_string()]);
            assert!(stashed, "the pre-merge edit must have been autostashed");
        }
        other => panic!("expected Conflicts{{stashed:true}}, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Merge);
    assert_eq!(stash_count(d), 1, "autostash retained during the paused merge");
    // Mid-merge, the edit is on the stash: worktree unrelated.txt is at HEAD.
    assert_eq!(
        std::fs::read_to_string(d.join("unrelated.txt")).expect("read unrelated"),
        "orig\n",
        "the pre-merge edit is on the stash, not in the worktree"
    );

    abort_merge(d).expect("abort");

    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(head_oid(d), pre_head, "HEAD must not move");
    assert_eq!(git(d, &["write-tree"]), tree_oid(d), "index tree must equal HEAD tree");
    assert!(git(d, &["ls-files", "-u"]).is_empty(), "no conflict stages may remain");
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt"),
        pre_a,
        "conflicted file must be restored to pre-merge (HEAD) bytes"
    );
    // The unrelated edit stays on the stash across the abort (not clobbered,
    // not in the worktree).
    assert_eq!(
        std::fs::read_to_string(d.join("unrelated.txt")).expect("read unrelated"),
        "orig\n",
        "after abort the worktree file is at HEAD; the edit is still stashed"
    );
    assert_eq!(stash_count(d), 1, "the autostash survives the abort (stash@{{0}})");

    // Data-safety proof: re-applying stash@{0} restores the edit byte-exactly.
    git(d, &["stash", "pop"]);
    assert_eq!(
        std::fs::read_to_string(d.join("unrelated.txt")).expect("read unrelated"),
        unrelated,
        "the user's edit is recoverable from stash@{{0}}"
    );
    assert_eq!(stash_count(d), 0, "pop consumed the stash");
}

#[test]
fn abort_without_a_merge_is_rejected() {
    require_git!();
    let repo = init_repo();
    let d = repo.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = abort_merge(d).expect_err("no merge");
    assert!(
        matches!(err, AppError::NoOperationInProgress(_)),
        "expected NoOperationInProgress, got {err:?}"
    );
}

// ============================================================ §9.9 create_commit gate

#[test]
fn plain_commit_during_paused_merge_is_rejected() {
    require_git!();
    let (bonsai, _twin) = twin_pair(script_conflict);
    let d = bonsai.path();
    match merge_branch(d, "topic").expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    resolve_conflict(d, "a.txt", ConflictResolution::Ours).expect("resolve");

    let err = create_commit(d, "sneaky plain commit", None).expect_err("gated");
    assert!(
        matches!(err, AppError::OperationInProgress(_)),
        "expected OperationInProgress, got {err:?}"
    );
    // Merge state untouched by the refused commit.
    assert_eq!(repo_state(d), git2::RepositoryState::Merge);
}
