//! M5 CLI-oracle branch tests (contract §6.1–§6.4).
//!
//! Fixtures built with the git CLI (repo-local identity, fixed dates where
//! twin-repo oid identity matters); our git2 op runs on repo A, the
//! equivalent CLI op on a twin repo B (or the CLI output is the direct
//! oracle). All scratch repos live under `D:\Temp\bonsai-scratch`.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::branches::{
    checkout_branch, checkout_remote, create_branch, delete_branch, delete_remote_tracking,
    list_refs,
};
use common::{assert_same_status, commit_fixed, git, git_ok, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Case-insensitive sort matching `list_refs`'s ordering (ties broken
/// case-sensitively).
fn ci_sort(v: &mut [String]) {
    v.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
}

/// Lines of trimmed `git` stdout (empty output -> empty vec).
fn lines(out: &str) -> Vec<String> {
    if out.is_empty() {
        Vec::new()
    } else {
        out.lines().map(str::to_string).collect()
    }
}

/// Base fixture: one committed file on `main`.
fn base_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "main v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    dir
}

// ---------------------------------------------------------------- §6.1 list

/// §6.1.1: local names + order vs `git for-each-ref refs/heads` (sorted
/// case-insensitively); `is_head` vs `git branch --show-current`; upstream
/// vs `%(upstream:short)`; ahead/behind vs `git rev-list --left-right
/// --count`.
#[test]
fn list_local_branches_matches_cli() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();

    // A local bare "remote" gives us real upstreams without any network.
    let bare = common::scratch_dir();
    git(bare.path(), &["init", "--bare"]);
    let bare_url = bare.path().to_string_lossy().replace('\\', "/");
    git(path, &["remote", "add", "origin", &bare_url]);
    git(path, &["push", "origin", "main"]);
    git(path, &["fetch", "origin"]);

    // `old` stays at the first commit; origin/main advances to the second
    // (old: behind 1); main then gains an unpushed third (main: ahead 1).
    // Mixed case exercises the case-insensitive sort.
    git(path, &["branch", "old", "main"]);
    git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
    git(path, &["branch", "--set-upstream-to=origin/main", "old"]);
    git(path, &["branch", "Zeta-topic"]);
    git(path, &["branch", "alpha/topic"]);
    std::fs::write(path.join("file.txt"), "main v2\n").expect("write");
    git(path, &["add", "-A"]);
    commit_fixed(path, "second on main");
    git(path, &["push", "origin", "main"]);
    git(path, &["fetch", "origin"]);
    std::fs::write(path.join("file.txt"), "main v3\n").expect("write");
    git(path, &["add", "-A"]);
    commit_fixed(path, "third on main");

    let snap = list_refs(path).expect("list_refs");

    // Names + order.
    let mut expected =
        lines(&git(path, &["for-each-ref", "refs/heads", "--format=%(refname:short)"]));
    ci_sort(&mut expected);
    let ours: Vec<String> = snap.local.iter().map(|b| b.name.clone()).collect();
    assert_eq!(ours, expected);

    // is_head.
    let current = git(path, &["branch", "--show-current"]);
    for b in &snap.local {
        assert_eq!(b.is_head, b.name == current, "is_head mismatch for {}", b.name);
    }
    assert!(!snap.head.unborn && !snap.head.detached);
    assert_eq!(snap.head.branch_name.as_deref(), Some(current.as_str()));

    // Upstream shorthand per branch.
    for b in &snap.local {
        let upstream = git(
            path,
            &[
                "for-each-ref",
                &format!("refs/heads/{}", b.name),
                "--format=%(upstream:short)",
            ],
        );
        let expected_upstream = if upstream.is_empty() { None } else { Some(upstream) };
        assert_eq!(b.upstream, expected_upstream, "upstream mismatch for {}", b.name);
    }

    // Ahead/behind for the branches with an upstream.
    for b in snap.local.iter().filter(|b| b.upstream.is_some()) {
        let upstream = b.upstream.as_deref().expect("upstream present");
        let counts = git(
            path,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{upstream}...{}", b.name),
            ],
        );
        let mut parts = counts.split_whitespace();
        let behind: u32 = parts.next().expect("behind").parse().expect("behind u32");
        let ahead: u32 = parts.next().expect("ahead").parse().expect("ahead u32");
        assert_eq!(b.ahead, Some(ahead), "ahead mismatch for {}", b.name);
        assert_eq!(b.behind, Some(behind), "behind mismatch for {}", b.name);
    }
    // The fixture makes them non-trivial: main is ahead 1, old is behind 1.
    let main = snap.local.iter().find(|b| b.name == "main").expect("main");
    assert_eq!((main.ahead, main.behind), (Some(1), Some(0)));
    let old = snap.local.iter().find(|b| b.name == "old").expect("old");
    assert_eq!((old.ahead, old.behind), (Some(0), Some(1)));

    // No upstream -> all three None.
    let zeta = snap.local.iter().find(|b| b.name == "Zeta-topic").expect("zeta");
    assert_eq!((zeta.upstream.as_deref(), zeta.ahead, zeta.behind), (None, None, None));
}

/// §6.1.2: remote-tracking list matches `git for-each-ref refs/remotes`
/// minus the symbolic `origin/HEAD`.
#[test]
fn list_remote_branches_excludes_origin_head() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();

    let bare = common::scratch_dir();
    git(bare.path(), &["init", "--bare"]);
    let bare_url = bare.path().to_string_lossy().replace('\\', "/");
    git(path, &["remote", "add", "origin", &bare_url]);
    git(path, &["branch", "feature"]);
    git(path, &["push", "origin", "main", "feature"]);
    git(path, &["fetch", "origin"]);
    // Symbolic origin/HEAD entry, exactly what a clone would have.
    git(
        path,
        &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"],
    );

    let snap = list_refs(path).expect("list_refs");

    let mut expected: Vec<String> =
        lines(&git(path, &["for-each-ref", "refs/remotes", "--format=%(refname:short)"]))
            .into_iter()
            .filter(|n| n != "origin/HEAD" && n != "origin")
            .collect();
    ci_sort(&mut expected);
    assert!(!expected.is_empty(), "fixture must have remote-tracking refs");
    let ours: Vec<String> = snap.remote.iter().map(|r| r.name.clone()).collect();
    assert_eq!(ours, expected);
    assert!(!ours.iter().any(|n| n.ends_with("/HEAD")));
}

/// §6.1.3: one lightweight + one annotated tag -> both listed, sorted;
/// matches `git tag --list`.
#[test]
fn list_tags_matches_cli() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();
    git(path, &["tag", "v0.2.0"]);
    git(path, &["tag", "-a", "v0.1.0", "-m", "release v0.1.0"]);

    let snap = list_refs(path).expect("list_refs");

    let mut expected = lines(&git(path, &["tag", "--list"]));
    ci_sort(&mut expected);
    assert_eq!(snap.tags, expected);
    assert_eq!(snap.tags, vec!["v0.1.0".to_string(), "v0.2.0".to_string()]);
}

/// §6.1.4: detached HEAD -> every is_head == false, head.detached == true.
#[test]
fn list_detached_head() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();
    git(path, &["checkout", "--detach", "HEAD"]);

    let snap = list_refs(path).expect("list_refs");
    assert!(snap.head.detached);
    assert!(!snap.head.unborn);
    assert!(snap.local.iter().all(|b| !b.is_head));
}

/// §6.1.5: unborn repo -> empty lists, head.unborn == true, Ok not Err.
#[test]
fn list_unborn_repo() {
    require_git!();
    let dir = init_repo();

    let snap = list_refs(dir.path()).expect("list_refs on unborn repo");
    assert!(snap.local.is_empty());
    assert!(snap.remote.is_empty());
    assert!(snap.tags.is_empty());
    assert!(snap.head.unborn);
}

// -------------------------------------------------------------- §6.2 create

/// §6.2.1: create at HEAD -> new ref == HEAD oid; no checkout happened.
#[test]
fn create_branch_at_head_without_checkout() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();

    create_branch(path, "topic").expect("create_branch");

    assert_eq!(
        git(path, &["rev-parse", "refs/heads/topic"]),
        git(path, &["rev-parse", "HEAD"])
    );
    assert_eq!(git(path, &["branch", "--show-current"]), "main");
}

/// §6.2.2: duplicate name -> BranchExists, ref list unchanged.
#[test]
fn create_duplicate_branch_fails() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();
    let before = git(path, &["for-each-ref"]);

    let err = create_branch(path, "main").expect_err("duplicate must fail");
    assert!(matches!(err, AppError::BranchExists(_)), "got {err:?}");
    assert_eq!(git(path, &["for-each-ref"]), before);
}

/// §6.2.3: invalid names -> InvalidName and no ref created; the git CLI
/// (`git check-ref-format --branch`) agrees each is invalid.
#[test]
fn create_invalid_names_rejected() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();
    let before = git(path, &["for-each-ref"]);

    let invalid = [
        "", " ", "a b", "a..b", "a.lock", "/a", "a/", "a~1", "a^", "a:b", "a?", "a[b", "@{u}",
        "-x",
    ];
    for name in invalid {
        assert!(
            !git_ok(path, &["check-ref-format", "--branch", name]),
            "oracle: git accepts {name:?} but the fixture assumes it is invalid"
        );
        let err = create_branch(path, name)
            .expect_err(&format!("create_branch({name:?}) must fail"));
        assert!(matches!(err, AppError::InvalidName(_)), "{name:?}: got {err:?}");
    }

    assert_eq!(git(path, &["for-each-ref"]), before);
}

/// §6.2.4: unborn repo -> AppError::Git with the contract message.
#[test]
fn create_branch_on_unborn_repo_fails() {
    require_git!();
    let dir = init_repo();

    let err = create_branch(dir.path(), "topic").expect_err("create on unborn must fail");
    match err {
        AppError::Git(m) => {
            assert_eq!(m, "cannot create a branch: the repository has no commits yet")
        }
        other => panic!("expected Git error, got {other:?}"),
    }
}

// ------------------------------------------------------------ §6.3 checkout

/// Fixture for checkout tests: `main` (file.txt = "main v1", shared.txt) and
/// `side` (file.txt = "side v1"), currently on `main`. Deterministic dates so
/// twin repos are oid-identical.
fn checkout_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "main v1\n").expect("write file.txt");
    std::fs::write(path.join("shared.txt"), "shared v1\n").expect("write shared.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    git(path, &["checkout", "-b", "side"]);
    std::fs::write(path.join("file.txt"), "side v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "side change");
    git(path, &["checkout", "main"]);
    dir
}

fn read(path: &Path, name: &str) -> String {
    std::fs::read_to_string(path.join(name)).expect("read file")
}

/// §6.3.1: clean checkout — HEAD symref, worktree contents, and (empty)
/// porcelain all identical to the CLI twin.
#[test]
fn checkout_clean_matches_cli_twin() {
    require_git!();
    let a = checkout_repo();
    let b = checkout_repo();

    checkout_branch(a.path(), "side").expect("checkout_branch");
    git(b.path(), &["checkout", "side"]);

    assert_eq!(
        git(a.path(), &["symbolic-ref", "HEAD"]),
        git(b.path(), &["symbolic-ref", "HEAD"])
    );
    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), "refs/heads/side");
    assert_eq!(read(a.path(), "file.txt"), read(b.path(), "file.txt"));
    assert_eq!(read(a.path(), "file.txt"), "side v1\n");
    assert_same_status(a.path(), b.path());
    assert!(git(a.path(), &["status", "--porcelain"]).is_empty());
}

/// §6.3.2: checkout carrying a compatible local change (file untouched
/// between branches) succeeds and the modification survives.
#[test]
fn checkout_carries_compatible_changes() {
    require_git!();
    let a = checkout_repo();
    let b = checkout_repo();
    for p in [a.path(), b.path()] {
        std::fs::write(p.join("shared.txt"), "shared modified\n").expect("write shared.txt");
    }

    checkout_branch(a.path(), "side").expect("checkout_branch with compatible changes");
    git(b.path(), &["checkout", "side"]);

    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), "refs/heads/side");
    assert_eq!(read(a.path(), "shared.txt"), "shared modified\n");
    assert_eq!(read(a.path(), "file.txt"), "side v1\n");
    assert_same_status(a.path(), b.path());
}

/// §6.3.3: dirty conflict (modified file DIFFERS between branches) ->
/// CheckoutConflict and NOTHING moved; the CLI twin also refuses.
#[test]
fn checkout_dirty_conflict_changes_nothing() {
    require_git!();
    let a = checkout_repo();
    let b = checkout_repo();
    for p in [a.path(), b.path()] {
        std::fs::write(p.join("file.txt"), "local edit\n").expect("write file.txt");
    }
    let head_before = git(a.path(), &["symbolic-ref", "HEAD"]);
    let porcelain_before = common::porcelain_records(a.path());

    let err = checkout_branch(a.path(), "side").expect_err("conflicting checkout must fail");
    assert!(matches!(err, AppError::CheckoutConflict(_)), "got {err:?}");

    // Twin oracle: git checkout also refuses.
    assert!(!git_ok(b.path(), &["checkout", "side"]));

    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(read(a.path(), "file.txt"), "local edit\n");
    assert_eq!(common::porcelain_records(a.path()), porcelain_before);
}

/// §6.3.4: checkout of the current branch is an Ok no-op.
#[test]
fn checkout_current_branch_is_noop() {
    require_git!();
    let dir = checkout_repo();

    checkout_branch(dir.path(), "main").expect("checkout current branch");
    assert_eq!(git(dir.path(), &["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert!(git(dir.path(), &["status", "--porcelain"]).is_empty());
}

/// §6.3.5: nonexistent branch -> BranchNotFound.
#[test]
fn checkout_missing_branch() {
    require_git!();
    let dir = checkout_repo();

    let err = checkout_branch(dir.path(), "nope").expect_err("missing branch");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}

// -------------------------------------------------------------- §6.4 delete

/// §6.4.1: merged branch deletes; twin `git branch -d` agrees.
#[test]
fn delete_merged_branch() {
    require_git!();
    let a = base_repo();
    let b = base_repo();
    git(a.path(), &["branch", "merged"]);
    git(b.path(), &["branch", "merged"]);

    delete_branch(a.path(), "merged").expect("delete merged branch");
    assert!(!git_ok(a.path(), &["rev-parse", "--verify", "refs/heads/merged"]));
    assert!(git_ok(b.path(), &["branch", "-d", "merged"]));
}

/// §6.4.2: unmerged branch -> UnmergedBranch, ref still present; twin
/// `git branch -d` also fails.
#[test]
fn delete_unmerged_branch_blocked() {
    require_git!();
    let build = || {
        let dir = base_repo();
        let path = dir.path();
        git(path, &["checkout", "-b", "topic"]);
        std::fs::write(path.join("topic.txt"), "topic\n").expect("write topic.txt");
        git(path, &["add", "-A"]);
        commit_fixed(path, "topic commit");
        git(path, &["checkout", "main"]);
        dir
    };
    let a = build();
    let b = build();

    let err = delete_branch(a.path(), "topic").expect_err("unmerged delete must fail");
    match &err {
        AppError::UnmergedBranch(m) => {
            assert!(m.contains("not fully merged into HEAD"), "message: {m}");
            assert!(m.contains("git branch -D topic"), "message: {m}");
        }
        other => panic!("expected UnmergedBranch, got {other:?}"),
    }
    assert!(git_ok(a.path(), &["rev-parse", "--verify", "refs/heads/topic"]));
    assert!(!git_ok(b.path(), &["branch", "-d", "topic"]));
}

/// §6.4.3: current branch -> AppError::Git with the contract message.
#[test]
fn delete_current_branch_blocked() {
    require_git!();
    let dir = base_repo();

    let err = delete_branch(dir.path(), "main").expect_err("delete current must fail");
    match err {
        AppError::Git(m) => {
            assert_eq!(m, "cannot delete 'main': it is the currently checked-out branch")
        }
        other => panic!("expected Git error, got {other:?}"),
    }
    assert!(git_ok(dir.path(), &["rev-parse", "--verify", "refs/heads/main"]));
}

/// §6.4.4: nonexistent -> BranchNotFound.
#[test]
fn delete_missing_branch() {
    require_git!();
    let dir = base_repo();

    let err = delete_branch(dir.path(), "nope").expect_err("missing branch");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}

/// §6.4.5: detached on the merged tip -> delete succeeds (merged relative to
/// the detached HEAD commit).
#[test]
fn delete_merged_branch_while_detached() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();
    git(path, &["branch", "extra"]);
    git(path, &["checkout", "--detach", "HEAD"]);

    delete_branch(path, "extra").expect("delete merged branch while detached");
    assert!(!git_ok(path, &["rev-parse", "--verify", "refs/heads/extra"]));
}

// ---------------------------------------------------- P6 §2.1 tip / §2.2 / §2.3

/// Repo with a `file://` bare remote `origin` and a remote-tracking
/// `origin/topic` advanced one commit past `main` (topic changes file.txt so a
/// checkout touches the worktree). Currently on `main`, with NO local `topic`.
/// Returns `(working dir, bare remote dir)` — keep both alive.
fn remote_topic_repo() -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "main v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");

    let bare = common::scratch_dir();
    git(bare.path(), &["init", "--bare"]);
    let bare_url = bare.path().to_string_lossy().replace('\\', "/");
    git(path, &["remote", "add", "origin", &bare_url]);

    // topic advances past main, changing file.txt.
    git(path, &["checkout", "-b", "topic"]);
    std::fs::write(path.join("file.txt"), "topic v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic change");

    git(path, &["push", "origin", "main", "topic"]);
    git(path, &["checkout", "main"]);
    git(path, &["fetch", "origin"]);
    // Drop the local topic so tests exercise the create / collision paths.
    git(path, &["branch", "-D", "topic"]);
    (dir, bare)
}

/// P6 §2.1: BranchInfo.tip / RemoteBranchInfo.tip equal `git rev-parse <ref>`.
#[test]
fn list_refs_tip_matches_cli() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();

    let snap = list_refs(path).expect("list_refs");

    let main = snap.local.iter().find(|b| b.name == "main").expect("main");
    assert_eq!(main.tip, git(path, &["rev-parse", "refs/heads/main"]));

    let origin_topic = snap
        .remote
        .iter()
        .find(|r| r.name == "origin/topic")
        .expect("origin/topic");
    assert_eq!(
        origin_topic.tip,
        git(path, &["rev-parse", "refs/remotes/origin/topic"])
    );
    // Tips are full 40-char hex oids.
    assert_eq!(main.tip.len(), 40);
    assert_eq!(origin_topic.tip.len(), 40);
}

/// P6 §2.2 create path: `origin/topic` with NO local `topic` -> creates and
/// switches to a local tracking branch at the remote tip, upstream configured.
#[test]
fn checkout_remote_creates_and_tracks() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();

    checkout_remote(path, "origin/topic").expect("checkout_remote create path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    assert_eq!(
        git(path, &["rev-parse", "topic"]),
        git(path, &["rev-parse", "refs/remotes/origin/topic"])
    );
    // Upstream configured to origin/refs/heads/topic.
    assert_eq!(git(path, &["config", "branch.topic.remote"]), "origin");
    assert_eq!(
        git(path, &["config", "branch.topic.merge"]),
        "refs/heads/topic"
    );
    // Worktree now has topic's content.
    assert_eq!(read(path, "file.txt"), "topic v1\n");
}

/// P6 §2.2 fast-forward path: a local `topic` exists strictly BEHIND
/// `origin/topic` (at main's oid, an ancestor of the remote tip) -> checkout
/// fast-forwards the local ref onto the remote tip and ends on local `topic`.
#[test]
fn checkout_remote_fast_forwards_behind_local() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    // Local topic at main's oid — a strict ancestor of origin/topic's tip.
    git(path, &["branch", "topic", "main"]);
    let local_before = git(path, &["rev-parse", "topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);
    assert_ne!(local_before, remote_tip, "fixture must be behind");

    checkout_remote(path, "origin/topic").expect("checkout_remote fast-forward path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    // Ref fast-forwarded onto the remote tip.
    assert_eq!(git(path, &["rev-parse", "topic"]), remote_tip);
    // Worktree now has the remote's content.
    assert_eq!(read(path, "file.txt"), "topic v1\n");
}

/// P6 §2.2 ahead path: a local `topic` strictly AHEAD of `origin/topic` (the
/// remote tip is an ancestor of the local tip) -> check out local as-is, ref
/// NOT moved.
#[test]
fn checkout_remote_ahead_local_not_moved() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    // Recreate local topic at the remote tip, then advance it one commit.
    git(path, &["branch", "topic", "refs/remotes/origin/topic"]);
    git(path, &["checkout", "topic"]);
    std::fs::write(path.join("file.txt"), "topic v2\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic ahead");
    git(path, &["checkout", "main"]);
    let local_before = git(path, &["rev-parse", "topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);
    assert_ne!(local_before, remote_tip, "fixture must be ahead");

    checkout_remote(path, "origin/topic").expect("checkout_remote ahead path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    // Ref NOT moved: local retains its extra commit.
    assert_eq!(git(path, &["rev-parse", "topic"]), local_before);
    assert_eq!(read(path, "file.txt"), "topic v2\n");
}

/// P6 §2.2 equal path: a local `topic` at the SAME oid as `origin/topic` ->
/// check out as-is, no ref move, no error.
#[test]
fn checkout_remote_equal_tips_no_move() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    git(path, &["branch", "topic", "refs/remotes/origin/topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);

    checkout_remote(path, "origin/topic").expect("checkout_remote equal path");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), "refs/heads/topic");
    assert_eq!(git(path, &["rev-parse", "topic"]), remote_tip);
    assert_eq!(read(path, "file.txt"), "topic v1\n");
}

/// P6 §2.2 diverged path: a local `topic` that has diverged from `origin/topic`
/// (neither tip is an ancestor of the other) -> error, and HEAD + branch tip +
/// worktree are all untouched.
#[test]
fn checkout_remote_diverged_changes_nothing() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    // Local topic branches off main with its OWN divergent commit.
    git(path, &["branch", "topic", "main"]);
    git(path, &["checkout", "topic"]);
    std::fs::write(path.join("file.txt"), "topic divergent\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic divergent");
    git(path, &["checkout", "main"]);
    let local_before = git(path, &["rev-parse", "topic"]);
    let remote_tip = git(path, &["rev-parse", "refs/remotes/origin/topic"]);
    let head_before = git(path, &["symbolic-ref", "HEAD"]);
    let file_before = read(path, "file.txt");
    assert_ne!(local_before, remote_tip, "fixture must diverge");

    let err = checkout_remote(path, "origin/topic").expect_err("diverged checkout must fail");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");

    // Nothing changed.
    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(git(path, &["rev-parse", "topic"]), local_before);
    assert_eq!(read(path, "file.txt"), file_before);
}

/// P6 §2.2 conflict: a dirty worktree a safe checkout would overwrite ->
/// CheckoutConflict, HEAD + worktree unchanged, and NO new local branch.
#[test]
fn checkout_remote_conflict_changes_nothing() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "local edit\n").expect("write file.txt");
    let head_before = git(path, &["symbolic-ref", "HEAD"]);

    let err = checkout_remote(path, "origin/topic").expect_err("conflicting checkout must fail");
    assert!(matches!(err, AppError::CheckoutConflict(_)), "got {err:?}");

    assert_eq!(git(path, &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(read(path, "file.txt"), "local edit\n");
    // No local branch was created.
    assert!(lines(&git(path, &["branch", "--list", "topic"])).is_empty());
}

/// P6 §2.2 errors: no '/' -> InvalidName; unknown remote ref -> BranchNotFound.
#[test]
fn checkout_remote_error_taxonomy() {
    require_git!();
    let (dir, _bare) = remote_topic_repo();
    let path = dir.path();

    let err = checkout_remote(path, "nope").expect_err("no slash must fail");
    assert!(matches!(err, AppError::InvalidName(_)), "got {err:?}");

    let err = checkout_remote(path, "origin/ghost").expect_err("unknown remote ref must fail");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}

/// P6 §2.3: deletes only the LOCAL remote-tracking ref; the server's own refs
/// are untouched. Unknown ref -> BranchNotFound.
#[test]
fn delete_remote_tracking_local_only() {
    require_git!();
    let (dir, bare) = remote_topic_repo();
    let path = dir.path();

    // Sanity: the remote-tracking ref exists before deletion.
    assert!(!lines(&git(path, &["branch", "-r", "--list", "origin/topic"])).is_empty());

    delete_remote_tracking(path, "origin/topic").expect("delete_remote_tracking");

    assert!(lines(&git(path, &["branch", "-r", "--list", "origin/topic"])).is_empty());
    // The server's own branch is untouched.
    assert!(
        git(bare.path(), &["show-ref"]).contains("refs/heads/topic"),
        "server ref refs/heads/topic must survive a local remote-tracking delete"
    );

    // Unknown ref -> BranchNotFound.
    let err = delete_remote_tracking(path, "origin/ghost").expect_err("unknown ref must fail");
    assert!(matches!(err, AppError::BranchNotFound(_)), "got {err:?}");
}
