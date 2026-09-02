//! P3c CLI-oracle merge tests — `commit_merge` and `abort_merge`
//! (contract §9.7–§9.8).
//!
//! Split out of `merge_cli.rs`; the twin-repo scaffolding, helpers, and
//! fixtures live in `merge_support.rs`.

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::conflict::{resolve_conflict, ConflictResolution};
use bonsai_core::git::merge::{abort_merge, commit_merge, merge_branch, MergeOutcome};
use crate::common;
use crate::common::{commit_fixed, git, init_repo, FIXED_DATE};
use crate::merge_support::{
    git_fail, head_oid, message, parents, repo_state, require_git, script_conflict_two_files,
    stash_count, tree_oid, twin_pair, write,
};

// ============================================================ §9.7 commit_merge

#[test]
fn commit_merge_after_resolving_matches_cli_twin() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_conflict_two_files);
    let pre_head = head_oid(bonsai.path());

    // Bonsai: merge -> conflicts on a.txt + b.txt; resolve a=Ours, b=Theirs.
    match merge_branch(bonsai.path(), "topic", false).expect("merge") {
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
    let result = commit_merge(bonsai.path(), &msg, None, false).expect("commit merge");
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
    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { .. } => {}
        other => panic!("expected Conflicts, got {other:?}"),
    }
    resolve_conflict(d, "a.txt", ConflictResolution::Ours).expect("resolve a");
    // b.txt still conflicted.
    let err = commit_merge(d, "msg", None, false).expect_err("unresolved");
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

    let err = commit_merge(d, "msg", None, false).expect_err("no merge");
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
    match merge_branch(d, "topic", false).expect("merge") {
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
