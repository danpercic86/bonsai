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
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

use std::path::Path;

use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::rebase_support::{
    checkout, cli_rebase, count_ahead, has_rebase_dir, head_oid, repo_state, require_git,
    rev_parse, script_clean_linear, top_infos, tree_oid, twin_pair, write,
};
use bonsai_core::git::rebase::{rebase_branch, RebaseOutcome};
use bonsai_core::git::remote::fetch_all;

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
        RebaseOutcome::Rebased {
            branch,
            head,
            steps,
            ..
        } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, &2, "topic..main range is 2 commits");
            assert_eq!(head, &head_oid(b), "returned head must be HEAD");
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    cli_rebase(t, "main");

    // Final HEAD tree identical.
    assert_eq!(
        tree_oid(b),
        tree_oid(t),
        "final HEAD tree oid must match twin"
    );
    // Each replayed commit: tree + author identity/time + message, in order.
    assert_eq!(
        top_infos(b, 2),
        top_infos(t, 2),
        "replayed commits differ from twin"
    );
    // Linear parent chain rooted at main's (unmoved) tip.
    assert_eq!(
        rev_parse(b, "HEAD~2"),
        onto_tip,
        "chain must root at main tip"
    );
    assert_eq!(
        count_ahead(b, "main", "HEAD"),
        2,
        "exactly 2 replayed commits"
    );
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
    assert_eq!(
        rebase_branch(d, "topic").expect("rebase"),
        RebaseOutcome::UpToDate
    );
    assert_eq!(head_oid(d), pre, "HEAD must not move");

    // Rebasing the current branch onto itself also falls out as UpToDate.
    assert_eq!(
        rebase_branch(d, "main").expect("self"),
        RebaseOutcome::UpToDate
    );
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
    assert_eq!(
        head_oid(b),
        head_oid(t),
        "FF HEAD must equal twin (no rewrite)"
    );
    assert_eq!(tree_oid(b), onto_tree, "worktree/tree == onto's tree");
    assert_eq!(
        count_ahead(b, "main", "HEAD"),
        0,
        "no commits ahead of onto"
    );
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
    assert!(!has_rebase_dir(b));
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
    git(
        root,
        &["clone", "-c", "core.autocrlf=false", &bare_s, "seed"],
    );
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
        RebaseOutcome::Rebased {
            branch,
            head,
            steps,
            ..
        } => {
            assert_eq!(branch, "topic");
            assert_eq!(steps, &1);
            assert_eq!(head, &head_oid(work));
        }
        other => panic!("expected Rebased, got {other:?}"),
    }

    cli_rebase(twin, "origin/main");

    assert_eq!(
        tree_oid(work),
        tree_oid(twin),
        "final HEAD tree must match twin"
    );
    assert_eq!(
        top_infos(work, 1),
        top_infos(twin, 1),
        "replayed commit differs from twin"
    );
    assert_eq!(
        rev_parse(work, "HEAD~1"),
        onto_tip,
        "commit sits on origin/main tip"
    );
    assert_eq!(count_ahead(work, "refs/remotes/origin/main", "HEAD"), 1);
    assert_eq!(repo_state(work), git2::RepositoryState::Clean);
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
    assert_eq!(
        bonsai_ahead, twin_ahead,
        "replayed-commit count must match twin"
    );
    assert_eq!(bonsai_ahead, 1, "the already-applied pick must be dropped");
    assert_eq!(tree_oid(b), tree_oid(t), "final HEAD tree must match twin");
    assert_eq!(
        top_infos(b, 1),
        top_infos(t, 1),
        "surviving commit differs from twin"
    );
    assert_eq!(repo_state(b), git2::RepositoryState::Clean);
}
