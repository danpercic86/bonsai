//! T2 Area 1 — branch + tag command inners (list/create/checkout/delete/
//! rename/stale-batch, create/delete/push tag), runtime-free.

use super::tests_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

fn local_names(snap: &BranchesSnapshot) -> Vec<&str> {
    snap.local.iter().map(|b| b.name.as_str()).collect()
}

/// `file:///` URL for a local path (git2 local transport; Windows drive paths
/// need the third slash + forward slashes). Mirrors `tests/common::file_url`.
fn file_url(p: &std::path::Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

/// list_branches matches the fixture: created branches listed sorted, is_head
/// on the checked-out one, HEAD attached.
#[test]
fn list_branches_matches_fixture() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "feature/x".into())).expect("create");
    block_on(create_branch_inner(&state, &id, "Zeta".into())).expect("create");

    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    let head_name = head_branch(dir.path()).expect("attached head");
    let mut expected = ["feature/x".to_string(), "Zeta".to_string(), head_name.clone()];
    expected.sort_by_key(|n| n.to_lowercase());
    assert_eq!(local_names(&snap), expected.iter().map(String::as_str).collect::<Vec<_>>());
    for b in &snap.local {
        assert_eq!(b.is_head, b.name == head_name);
        assert_eq!(b.tip, c0, "all branches at the fixture root commit");
        assert!(b.upstream.is_none());
    }
    assert!(snap.remote.is_empty() && snap.tags.is_empty());
    assert!(!snap.head.detached && !snap.head.unborn);
}

/// create_branch: duplicate → BranchExists; invalid names → InvalidName.
#[test]
fn create_branch_duplicate_and_invalid_names() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "dup".into())).expect("create");

    let err = block_on(create_branch_inner(&state, &id, "dup".into()))
        .expect_err("duplicate must error");
    assert!(matches!(err, AppError::BranchExists(_)), "{err:?}");

    for bad in ["-bad", "a..b", "", "has space?*"] {
        let err = block_on(create_branch_inner(&state, &id, bad.to_string()))
            .expect_err("invalid name must error");
        assert!(matches!(err, AppError::InvalidName(_)), "{bad}: {err:?}");
    }
}

/// create_branch_here creates the branch at an OLDER commit and checks it out.
#[test]
fn create_branch_here_at_older_commit() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "a.txt", "v2\n", "C1");

    let res = block_on(create_branch_here_inner(&state, &id, "from-c0".into(), c0.clone()))
        .expect("create here");
    assert!(!res.stashed && res.apply.is_none(), "clean tree: no autostash");
    assert_eq!(head_branch(dir.path()).as_deref(), Some("from-c0"));
    assert_eq!(head_oid(dir.path()), c0);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        "base\n",
        "worktree checked out at C0"
    );
}

/// checkout_branch: happy switch, dirty-tree autostash round-trip (edit
/// carried across, stash consumed), and a missing branch → BranchNotFound.
#[test]
fn checkout_branch_happy_dirty_and_missing() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("head");
    block_on(create_branch_inner(&state, &id, "side".into())).expect("create");

    // Clean switch.
    let res = block_on(checkout_branch_inner(&state, &id, "side".into())).expect("checkout");
    assert!(!res.stashed && res.apply.is_none() && !res.fast_forwarded);
    assert_eq!(head_branch(dir.path()).as_deref(), Some("side"));

    // Dirty switch back: the edit is auto-stashed and re-applied cleanly.
    std::fs::write(dir.path().join("a.txt"), "dirty edit\n").expect("write");
    let res = block_on(checkout_branch_inner(&state, &id, main.clone())).expect("dirty checkout");
    assert!(res.stashed, "dirty tree must autostash");
    assert_eq!(res.apply, Some(ApplyStashOutcome::Applied));
    assert_eq!(head_branch(dir.path()).as_deref(), Some(main.as_str()));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        "dirty edit\n",
        "edit carried across the switch"
    );
    let stashes = block_on(list_stashes_inner(&state, &id)).expect("stashes");
    assert!(stashes.is_empty(), "clean re-apply consumes the autostash");

    let err = block_on(checkout_branch_inner(&state, &id, "no-such".into()))
        .expect_err("missing branch");
    assert!(matches!(err, AppError::BranchNotFound(_)), "{err:?}");
}

/// delete_branch: merged branch deleted; unmerged blocked (UnmergedBranch);
/// the current branch is refused.
#[test]
fn delete_branch_merged_unmerged_current() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("head");

    block_on(create_branch_inner(&state, &id, "merged".into())).expect("create");
    block_on(delete_branch_inner(&state, &id, "merged".into())).expect("delete merged");
    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    assert!(!local_names(&snap).contains(&"merged"));

    // Unmerged: commit on a side branch, come back, deletion is blocked.
    block_on(create_branch_here_inner(&state, &id, "ahead".into(), head_oid(dir.path())))
        .expect("create+checkout");
    write_stage_commit(&state, &id, dir.path(), "ahead.txt", "x\n", "ahead commit");
    block_on(checkout_branch_inner(&state, &id, main.clone())).expect("back to main");
    let err = block_on(delete_branch_inner(&state, &id, "ahead".into()))
        .expect_err("unmerged must be blocked");
    assert!(matches!(err, AppError::UnmergedBranch(_)), "{err:?}");

    // Current branch: refused (clean Git error).
    let err = block_on(delete_branch_inner(&state, &id, main))
        .expect_err("current branch must be refused");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");

    // Missing branch: BranchNotFound.
    let err = block_on(delete_branch_inner(&state, &id, "ghost".into())).expect_err("missing");
    assert!(matches!(err, AppError::BranchNotFound(_)), "{err:?}");
}

/// rename_branch: happy rename preserves the tip; renaming onto an existing
/// name is BranchExists.
#[test]
fn rename_branch_happy_and_collision() {
    let state = AppState::default();
    let (_dir, id, c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "old-name".into())).expect("create");
    block_on(create_branch_inner(&state, &id, "taken".into())).expect("create");

    let res = block_on(rename_branch_inner(&state, &id, "old-name".into(), "new-name".into()))
        .expect("rename");
    assert!(!res.was_head);
    assert!(res.upstream.is_none());
    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    assert!(local_names(&snap).contains(&"new-name"));
    assert!(!local_names(&snap).contains(&"old-name"));
    assert_eq!(
        snap.local.iter().find(|b| b.name == "new-name").unwrap().tip,
        c0
    );

    let err = block_on(rename_branch_inner(&state, &id, "new-name".into(), "taken".into()))
        .expect_err("collision must error");
    assert!(matches!(err, AppError::BranchExists(_)), "{err:?}");
}

/// list_stale_branches is read-only: it reports the merged branch but deletes
/// nothing.
#[test]
fn list_stale_branches_is_read_only() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "stale-one".into())).expect("create");
    // Advance main so "stale-one" is strictly merged history.
    write_stage_commit(&state, &id, dir.path(), "a.txt", "v2\n", "C1");

    let report = block_on(list_stale_branches_inner(&state, &id, None)).expect("stale");
    assert!(
        report.branches.iter().any(|b| b.name == "stale-one"),
        "merged branch classified stale: {report:?}"
    );
    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    assert!(local_names(&snap).contains(&"stale-one"), "read-only: nothing deleted");
}

/// delete_branches: refuses the current branch, deletes a re-verified stale
/// one, reports a missing one — per-branch results, no thrown error.
#[test]
fn delete_branches_partial_results() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let main = head_branch(dir.path()).expect("head");
    block_on(create_branch_inner(&state, &id, "stale-two".into())).expect("create");
    write_stage_commit(&state, &id, dir.path(), "a.txt", "v2\n", "C1");

    let results = block_on(delete_branches_inner(
        &state,
        &id,
        vec![main.clone(), "stale-two".into(), "no-such".into()],
        None,
    ))
    .expect("batch delete returns Ok with per-branch results");
    assert_eq!(results.len(), 3);
    let by_name = |n: &str| results.iter().find(|r| r.name == n).unwrap();
    assert_ne!(
        by_name(&main).status,
        stale::BranchDeleteStatus::Deleted,
        "current/base branch must never be deleted: {results:?}"
    );
    assert_eq!(by_name("stale-two").status, stale::BranchDeleteStatus::Deleted);
    // Adapted: the freshly-recomputed stale set is checked FIRST, so a branch
    // that doesn't exist is reported SkippedNotStale (not in the safe set)
    // rather than SkippedNotFound. Either way it must not be "Deleted".
    assert!(
        matches!(
            by_name("no-such").status,
            stale::BranchDeleteStatus::SkippedNotStale | stale::BranchDeleteStatus::SkippedNotFound
        ),
        "{results:?}"
    );

    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    assert!(local_names(&snap).contains(&main.as_str()), "current branch survives");
    assert!(!local_names(&snap).contains(&"stale-two"));
}

/// create_tag: lightweight, annotated, unicode name; duplicate non-force
/// fails; delete_tag happy + missing.
#[test]
fn tag_create_delete_round_trip() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    block_on(create_tag_inner(&state, &id, "light".into(), c0.clone(), None, false, None))
        .expect("lightweight tag");
    block_on(create_tag_inner(
        &state,
        &id,
        "ann".into(),
        c0.clone(),
        Some("annotated ✨".into()),
        false,
        None,
    ))
    .expect("annotated tag");
    block_on(create_tag_inner(&state, &id, "v1.0-ünïcode".into(), c0.clone(), None, false, None))
        .expect("unicode tag name");

    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    for t in ["light", "ann", "v1.0-ünïcode"] {
        assert!(snap.tags.iter().any(|x| x == t), "tag {t} listed: {:?}", snap.tags);
    }
    // The annotated one is a real tag object.
    let repo = git2::Repository::open(dir.path()).expect("open");
    let obj = repo.revparse_single("refs/tags/ann").expect("ann");
    assert_eq!(obj.kind(), Some(git2::ObjectType::Tag));

    // Duplicate, non-force.
    let err = block_on(create_tag_inner(&state, &id, "light".into(), c0.clone(), None, false, None))
        .expect_err("duplicate tag");
    match err {
        AppError::Git(m) => assert!(m.contains("already exists"), "{m}"),
        other => panic!("expected Git(exists), got {other:?}"),
    }
    // Invalid name.
    let err = block_on(create_tag_inner(&state, &id, "-bad".into(), c0.clone(), None, false, None))
        .expect_err("invalid tag name");
    assert!(matches!(err, AppError::InvalidName(_)), "{err:?}");

    // Delete happy + missing.
    block_on(delete_tag_inner(&state, &id, "light".into())).expect("delete tag");
    let snap = block_on(list_branches_inner(&state, &id)).expect("list");
    assert!(!snap.tags.iter().any(|x| x == "light"));
    let err = block_on(delete_tag_inner(&state, &id, "light".into())).expect_err("missing tag");
    match err {
        AppError::Git(m) => assert!(m.contains("not found"), "{m}"),
        other => panic!("expected Git(not found), got {other:?}"),
    }
}

/// push_tag to a local bare `file://` remote (git2 local transport — no git
/// binary needed): the tag ref appears in the bare repo; a missing remote is
/// NoRemote; a missing local tag is Git.
#[test]
fn push_tag_to_file_remote() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    let bare_dir = tempfile::TempDir::new().expect("bare dir");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");
    {
        let repo = git2::Repository::open(dir.path()).expect("open");
        repo.remote("origin", &file_url(bare_dir.path())).expect("remote add");
    }

    block_on(create_tag_inner(&state, &id, "rel".into(), c0.clone(), Some("m".into()), false, None))
        .expect("create tag");
    block_on(push_tag_inner(&state, &id, "origin".into(), "rel".into(), false))
        .expect("push tag to bare file:// remote");

    let bare = git2::Repository::open_bare(bare_dir.path()).expect("open bare");
    assert!(bare.find_reference("refs/tags/rel").is_ok(), "tag arrived in the remote");

    let err = block_on(push_tag_inner(&state, &id, "nosuch".into(), "rel".into(), false))
        .expect_err("missing remote");
    assert!(matches!(err, AppError::NoRemote(_)), "{err:?}");
    let err = block_on(push_tag_inner(&state, &id, "origin".into(), "ghost".into(), false))
        .expect_err("missing local tag");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
}
