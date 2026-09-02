//! P3c CLI-oracle merge tests (contract §9, `merge_cli.rs`).
//!
//! Twin-repo pattern: two scratch repos are built by the IDENTICAL scripted
//! CLI setup (fixed dates -> identical base oids). Bonsai's merge fns run on
//! one; the real `git` CLI runs on the other; results are compared
//! byte-exactly (tree oids, parents, messages, conflicted sets).
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

use std::path::Path;

use bonsai_core::git::merge::{merge_branch, MergeOutcome};
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::merge_support::{
    cli_conflicted, cli_merge, git_fail, head_oid, message, parents, repo_state, require_git,
    script_clean_diverged, script_conflict, tree_oid, twin_pair, write,
};

// ============================================================ §9.1 clean merge

#[test]
fn clean_merge_matches_cli_twin() {
    require_git!();
    let (bonsai, twin) = twin_pair(script_clean_diverged);
    let pre_head = head_oid(bonsai.path());

    let outcome = merge_branch(bonsai.path(), "topic", false).expect("merge");
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

    let outcome = merge_branch(bonsai.path(), "topic", false).expect("merge");
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

    assert_eq!(merge_branch(d, "topic", false).expect("merge"), MergeOutcome::UpToDate);
    assert_eq!(head_oid(d), pre, "HEAD must not move");

    // Merging the current branch by name also falls out as UpToDate.
    assert_eq!(merge_branch(d, "main", false).expect("merge self"), MergeOutcome::UpToDate);
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

    let outcome = merge_branch(work, "origin/topic", false).expect("merge origin/topic");
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

    let outcome = merge_branch(bonsai.path(), "topic", false).expect("merge");
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
