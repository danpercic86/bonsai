//! M5 CLI-oracle branch tests (contract §6.1–§6.4).
//!
//! Fixtures built with the git CLI (repo-local identity, fixed dates where
//! twin-repo oid identity matters); our git2 op runs on repo A, the
//! equivalent CLI op on a twin repo B (or the CLI output is the direct
//! oracle). All scratch repos live under `D:\Data\Temp\bonsai-scratch`.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

use crate::common;
use crate::common::{commit_fixed, git, git_ok, init_repo};
use bonsai_core::error::AppError;
use bonsai_core::git::branches::{create_branch, list_refs};

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
    v.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
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
    let mut expected = lines(&git(
        path,
        &["for-each-ref", "refs/heads", "--format=%(refname:short)"],
    ));
    ci_sort(&mut expected);
    let ours: Vec<String> = snap.local.iter().map(|b| b.name.clone()).collect();
    assert_eq!(ours, expected);

    // is_head.
    let current = git(path, &["branch", "--show-current"]);
    for b in &snap.local {
        assert_eq!(
            b.is_head,
            b.name == current,
            "is_head mismatch for {}",
            b.name
        );
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
        let expected_upstream = if upstream.is_empty() {
            None
        } else {
            Some(upstream)
        };
        assert_eq!(
            b.upstream, expected_upstream,
            "upstream mismatch for {}",
            b.name
        );
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
    let zeta = snap
        .local
        .iter()
        .find(|b| b.name == "Zeta-topic")
        .expect("zeta");
    assert_eq!(
        (zeta.upstream.as_deref(), zeta.ahead, zeta.behind),
        (None, None, None)
    );
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
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );

    let snap = list_refs(path).expect("list_refs");

    let mut expected: Vec<String> = lines(&git(
        path,
        &["for-each-ref", "refs/remotes", "--format=%(refname:short)"],
    ))
    .into_iter()
    .filter(|n| n != "origin/HEAD" && n != "origin")
    .collect();
    ci_sort(&mut expected);
    assert!(
        !expected.is_empty(),
        "fixture must have remote-tracking refs"
    );
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
        "", " ", "a b", "a..b", "a.lock", "/a", "a/", "a~1", "a^", "a:b", "a?", "a[b", "@{u}", "-x",
    ];
    for name in invalid {
        assert!(
            !git_ok(path, &["check-ref-format", "--branch", name]),
            "oracle: git accepts {name:?} but the fixture assumes it is invalid"
        );
        let err =
            create_branch(path, name).expect_err(&format!("create_branch({name:?}) must fail"));
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "{name:?}: got {err:?}"
        );
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
            assert_eq!(
                m,
                "cannot create a branch: the repository has no commits yet"
            )
        }
        other => panic!("expected Git error, got {other:?}"),
    }
}

// §6.3 checkout and §6.4 delete live in their own files (~500-line limit),
// declared here as child modules so they share `require_git!` and `base_repo`.
#[path = "branches_checkout_cli.rs"]
mod branches_checkout_cli;
#[path = "branches_delete_cli.rs"]
mod branches_delete_cli;
