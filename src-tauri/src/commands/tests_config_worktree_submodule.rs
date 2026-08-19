//! T2 Area 1 (pass B) — config / worktree / submodule command inners,
//! runtime-free. Global-config WRITES are deliberately NOT exercised (they
//! would mutate the developer's real `~/.gitconfig` — the T2 §0 safety rule);
//! Global is covered read-only. Submodule/worktree CLI-dependent steps skip
//! cleanly when the `git` binary is absent.

use super::tests_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

// ============================================================ config

/// set_config (Local) writes a value that get_config (Local) reads back;
/// unset_config removes it and is idempotent (a second unset is still Ok).
#[test]
fn config_set_get_unset_local_round_trip_and_idempotent() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    block_on(set_config_inner(
        &state, &id, ConfigLevelArg::Local, "bonsai.testkey".into(), "hello".into(),
    ))
    .expect("set local");

    let cfg = git2::Repository::open(dir.path())
        .expect("open")
        .config()
        .expect("config")
        .get_string("bonsai.testkey")
        .expect("read back");
    assert_eq!(cfg, "hello");

    // get_config(Local) is the read side; it must succeed on an open repo.
    block_on(get_config_inner(&state, &id, ConfigLevelArg::Local)).expect("get local");

    block_on(unset_config_inner(&state, &id, ConfigLevelArg::Local, "bonsai.testkey".into()))
        .expect("unset");
    block_on(unset_config_inner(&state, &id, ConfigLevelArg::Local, "bonsai.testkey".into()))
        .expect("unset again is idempotent (NotFound swallowed)");
    assert!(
        git2::Repository::open(dir.path())
            .unwrap()
            .config()
            .unwrap()
            .get_string("bonsai.testkey")
            .is_err(),
        "key is gone after unset"
    );
}

/// get_config(Global) is a safe read-only view (never mutated here). An
/// invalid key shape is rejected with InvalidName before any write.
#[test]
fn config_global_read_and_invalid_key() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    block_on(get_config_inner(&state, &id, ConfigLevelArg::Global)).expect("read global view");

    let err = block_on(set_config_inner(
        &state, &id, ConfigLevelArg::Local, "nosectionkey".into(), "x".into(),
    ))
    .expect_err("a key with no section is invalid");
    assert!(matches!(err, AppError::InvalidName(_)), "{err:?}");
}

/// apply_identity_profile (via its `_inner` seam) writes user.name/email into
/// LOCAL config and returns a Local ConfigView; an unknown repo is NoRepo
/// (the gate fires before any git2 write).
#[test]
fn apply_identity_profile_happy_and_no_repo() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    let view = block_on(apply_identity_profile_inner(
        &state,
        &id,
        "Ada Lovelace".into(),
        "ada@example.com".into(),
        None,
    ))
    .expect("apply identity");
    // Returned view is the refreshed Local view (shape only asserted here).
    let _ = view.curated;

    let cfg = git2::Repository::open(dir.path()).unwrap().config().unwrap();
    assert_eq!(cfg.get_string("user.name").unwrap(), "Ada Lovelace");
    assert_eq!(cfg.get_string("user.email").unwrap(), "ada@example.com");

    let err = block_on(apply_identity_profile_inner(
        &state,
        MISSING_ID,
        "N".into(),
        "n@e.com".into(),
        None,
    ))
    .expect_err("unknown repo");
    assert!(matches!(err, AppError::NoRepo), "{err:?}");
}

// ============================================================ worktree

/// Full worktree lifecycle: add (checking out a non-current branch) → list →
/// lock → unlock → remove. The main row is always present.
#[test]
fn worktree_add_list_lock_unlock_remove() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "wtbranch".into())).expect("create branch");

    let created = block_on(add_worktree_inner(&state, &id, "wtbranch".into(), "wt-one".into()))
        .expect("add worktree");
    assert_eq!(created.branch.as_deref(), Some("wtbranch"));
    assert!(!created.is_main);
    let name = created.name.clone();

    let list = block_on(list_worktrees_inner(&state, &id)).expect("list");
    assert!(list.iter().any(|w| w.is_main), "main row present");
    assert!(list.iter().any(|w| w.name == name), "the new worktree is listed");

    block_on(lock_worktree_inner(&state, &id, name.clone(), Some("busy".into()))).expect("lock");
    assert!(
        block_on(list_worktrees_inner(&state, &id)).unwrap().iter().any(|w| w.name == name && w.locked),
        "listed as locked"
    );
    block_on(unlock_worktree_inner(&state, &id, name.clone())).expect("unlock");

    block_on(remove_worktree_inner(&state, &id, name.clone())).expect("remove");
    assert!(
        !block_on(list_worktrees_inner(&state, &id)).unwrap().iter().any(|w| w.name == name),
        "gone after remove"
    );
    let _ = dir;
}

/// remove refuses a DIRTY worktree (Git), and a blank name is InvalidName.
#[test]
fn worktree_remove_dirty_refusal_and_invalid_name() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "dirtybr".into())).expect("branch");
    let wt = block_on(add_worktree_inner(&state, &id, "dirtybr".into(), "wt-dirty".into()))
        .expect("add");

    // Dirty the linked worktree, then removing it must refuse.
    let list = block_on(list_worktrees_inner(&state, &id)).unwrap();
    let row = list.iter().find(|w| w.name == wt.name).expect("row");
    std::fs::write(std::path::Path::new(&row.abs_path).join("scratch.txt"), "dirty\n").expect("dirty");
    let err = block_on(remove_worktree_inner(&state, &id, wt.name.clone()))
        .expect_err("dirty worktree must not be removed");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");

    let err = block_on(add_worktree_inner(&state, &id, "  ".into(), "  ".into()))
        .expect_err("blank branch");
    assert!(matches!(err, AppError::InvalidName(_)), "{err:?}");

    // Cleanup: force by clearing the dirt then removing.
    std::fs::remove_file(std::path::Path::new(&row.abs_path).join("scratch.txt")).ok();
    block_on(remove_worktree_inner(&state, &id, wt.name)).ok();
}

/// list_copy_candidates / preview_worktree_copy / add_worktree_with_changes
/// happy path: an untracked file is a candidate, previews clean against a new
/// branch, and is copied into the created worktree.
#[test]
fn worktree_copy_candidates_preview_and_add_with_changes() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    block_on(create_branch_inner(&state, &id, "copybr".into())).expect("branch");
    std::fs::write(dir.path().join("carry.txt"), "carry me\n").expect("write untracked");

    let candidates = block_on(list_copy_candidates_inner(&state, &id)).expect("candidates");
    assert!(
        candidates.iter().any(|c| c.path == "carry.txt"),
        "the untracked file is a copy candidate"
    );

    let plan = block_on(preview_worktree_copy_inner(
        &state, &id, "copybr".into(), vec!["carry.txt".into()],
    ))
    .expect("preview");
    assert!(plan.iter().any(|p| p.path == "carry.txt"), "planned");

    let sel = vec![CopySelection {
        path: "carry.txt".into(),
        action: worktree_copy::CopyAction::Copy,
    }];
    let created = block_on(add_worktree_with_changes_inner(
        &state, &id, "copybr".into(), "wt-copy".into(), sel,
    ))
    .expect("add with changes");
    let carried = std::path::Path::new(&created.abs_path).join("carry.txt");
    assert_eq!(std::fs::read_to_string(&carried).expect("carried file"), "carry me\n");

    block_on(remove_worktree_inner(&state, &id, created.name)).ok();
}

// ============================================================ submodule

/// list_submodules is empty on a repo without submodules; init/sync/update with
/// a blank name are InvalidName (validated before any git work).
#[test]
fn submodule_list_empty_and_blank_name_invalid() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    assert!(
        block_on(list_submodules_inner(&state, &id)).expect("list").is_empty(),
        "no submodules yet"
    );

    for res in [
        block_on(init_submodule_inner(&state, &id, "  ".into())),
        block_on(sync_submodule_inner(&state, &id, "  ".into())),
        block_on(update_submodule_inner(&state, &id, "  ".into())),
    ] {
        assert!(matches!(res, Err(AppError::InvalidName(_))), "{res:?}");
    }
}

/// add(file:// url) → list → init → sync → update → deinit → remove over a
/// file:// twin submodule source. Requires the git binary (deinit/remove shell
/// out); skips cleanly when absent.
#[test]
fn submodule_add_lifecycle_over_file_url() {
    if !have_git() {
        eprintln!("skipping: git CLI not on PATH");
        return;
    }
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    // A tiny source repo to embed as a submodule (built with the CLI so its
    // HEAD is a real branch a clone can check out).
    let src = tempfile::TempDir::new().expect("src dir");
    git(src.path(), &["init", "-b", "main"]);
    git(src.path(), &["config", "user.name", "Src"]);
    git(src.path(), &["config", "user.email", "src@example.com"]);
    std::fs::write(src.path().join("lib.txt"), "sub\n").expect("write");
    git(src.path(), &["add", "-A"]);
    git(src.path(), &["commit", "-m", "sub base"]);

    let url = file_url(src.path());
    let added = block_on(add_submodule_inner(&state, &id, url, "vendor/sub".into()))
        .expect("add submodule");
    assert_eq!(added.path, "vendor/sub");
    let name = added.name.clone();

    assert!(
        block_on(list_submodules_inner(&state, &id)).unwrap().iter().any(|s| s.path == "vendor/sub"),
        "listed after add"
    );

    // init / sync / update are idempotent no-ops on an already-cloned submodule.
    block_on(init_submodule_inner(&state, &id, name.clone())).expect("init");
    block_on(sync_submodule_inner(&state, &id, name.clone())).expect("sync");
    block_on(update_submodule_inner(&state, &id, name.clone())).expect("update");

    // P73 (contract §8.3) — wedge-and-repair through the real command wrapper.
    use bonsai_core::git::submodule::SubmoduleStatus as SubStatus;
    // Wedge: keep `.git/modules/<name>` intact (plus a sentinel proving REUSE),
    // delete the gitlink and every workdir file, keep the now-empty dir. libgit2
    // alone dies here with "attempt to reinitialize"; the salvage reattaches.
    let sub_wd = dir.path().join("vendor/sub");
    let module_dir = dir.path().join(".git").join("modules").join(&name);
    assert!(module_dir.join("HEAD").exists(), "precondition: a cached module gitdir");
    let sentinel = module_dir.join("bonsai-sentinel");
    std::fs::write(&sentinel, "keep me").expect("plant sentinel");
    std::fs::remove_file(sub_wd.join(".git")).expect("remove gitlink");
    for e in std::fs::read_dir(&sub_wd).expect("read submodule workdir") {
        let e = e.expect("dir entry");
        if std::fs::symlink_metadata(e.path()).expect("stat").is_dir() {
            std::fs::remove_dir_all(e.path()).expect("rm dir");
        } else {
            std::fs::remove_file(e.path()).expect("rm file");
        }
    }
    // The row survives the wedge and reads `uninitialized`.
    let wedged = block_on(list_submodules_inner(&state, &id))
        .expect("list while wedged")
        .into_iter()
        .find(|s| s.path == "vendor/sub")
        .expect("the wedged row is still listed");
    assert_eq!(wedged.status, SubStatus::Uninitialized, "wedged row: {wedged:?}");

    // ...and the command repairs it, reusing the cached gitdir (no network).
    block_on(update_submodule_inner(&state, &id, name.clone()))
        .expect("update must reconnect the orphaned module gitdir");
    assert_eq!(
        std::fs::read_to_string(&sentinel).expect("sentinel survives"),
        "keep me",
        "the cached gitdir was reused, not re-cloned"
    );
    assert_eq!(
        // CRLF-normalized: the module gitdir is cloned by libgit2, which inherits
        // the developer's global `core.autocrlf`.
        std::fs::read_to_string(sub_wd.join("lib.txt"))
            .expect("lib.txt restored")
            .replace("\r\n", "\n"),
        "sub\n",
        "the submodule content is back on disk"
    );
    let repaired = block_on(list_submodules_inner(&state, &id))
        .expect("list after repair")
        .into_iter()
        .find(|s| s.path == "vendor/sub")
        .expect("the repaired row");
    // NOT `upToDate` in THIS fixture: the `.gitmodules` + gitlink are only STAGED
    // (this lifecycle never commits them), so `INDEX_ADDED` makes
    // `classify_status` report `outOfSync` regardless of how healthy the checkout
    // is — pre-existing P19 behaviour, unrelated to P73. What the repair must
    // prove here is that the row LEFT `uninitialized` and the checked-out commit
    // now matches the pinned one. (The `upToDate` end state is asserted on a
    // committed fixture in `bonsai-core/tests/submodule_wedge_cli.rs`.)
    assert_ne!(repaired.status, SubStatus::Uninitialized, "repaired row: {repaired:?}");
    assert_eq!(repaired.status, SubStatus::OutOfSync, "repaired row: {repaired:?}");
    assert!(repaired.wt_oid.is_some() && repaired.wt_oid == repaired.index_oid,
        "the checked-out commit matches the pinned one: {repaired:?}");
    assert!(sub_wd.join(".git").is_file(), "a gitlink FILE was written back");

    block_on(deinit_submodule_inner(&state, &id, name.clone())).expect("deinit");
    block_on(remove_submodule_inner(&state, &id, name.clone())).expect("remove");
    assert!(
        !block_on(list_submodules_inner(&state, &id)).unwrap().iter().any(|s| s.path == "vendor/sub"),
        "gone after remove"
    );
    let _ = dir;
}
