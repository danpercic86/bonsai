use super::*;

/// §2.9: the wire shape must match the TS mirror — camelCase keys, full oid.
#[test]
fn worktree_info_serializes_camel_case_keys() {
    let info = WorktreeInfo {
        name: "feature-login".to_string(),
        abs_path: "/repo/.worktrees/feature-login".to_string(),
        rel_path: None,
        branch: Some("feature/login".to_string()),
        head_oid: Some("a".repeat(40)),
        locked: true,
        lock_reason: Some("pinned for QA".to_string()),
        is_main: false,
        is_current: false,
        prunable: false,
        valid: true,
    };
    let v = serde_json::to_value(&info).expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "name": "feature-login",
            "absPath": "/repo/.worktrees/feature-login",
            "relPath": null,
            "branch": "feature/login",
            "headOid": "a".repeat(40),
            "locked": true,
            "lockReason": "pinned for QA",
            "isMain": false,
            "isCurrent": false,
            "prunable": false,
            "valid": true
        })
    );
}

/// §2.9: the slug table, including the priority tie-breaks and rejections.
#[test]
fn sanitize_slug_table() {
    // Happy cases.
    assert_eq!(sanitize_slug("feature/login").expect("slug"), "feature-login");
    assert_eq!(sanitize_slug("feature/x").expect("slug"), "feature-x");
    assert_eq!(sanitize_slug("feat/x").expect("slug"), "feat-x");
    assert_eq!(sanitize_slug("a//b").expect("slug"), "a-b"); // collapse runs
    assert_eq!(sanitize_slug("--weird--").expect("slug"), "weird"); // trim ends
    assert_eq!(sanitize_slug("release/1.2").expect("slug"), "release-1.2"); // dots kept
    assert_eq!(sanitize_slug("hot fix").expect("slug"), "hot-fix"); // space → '-'

    // Rejections → InvalidName.
    for bad in ["", "   ", "..", "/", "---", "..."] {
        match sanitize_slug(bad) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for {bad:?}, got {other:?}"),
        }
    }
}

/// §2.4: derive_worktree yields `<parent>/.worktrees/<slug>`, stays inside
/// the container, and appends `-2` on a directory collision.
#[test]
fn derive_worktree_containment_and_collision() {
    // Init the repo in a subdir so its PARENT is the unique tempdir — the
    // derived `.worktrees/` container then never leaks into the shared
    // scratch root across runs.
    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    let repo = git2::Repository::init(&repo_dir).expect("init");
    let main_dir = repo.workdir().expect("workdir").to_path_buf();
    let container = main_dir
        .parent()
        .expect("parent")
        .join(".worktrees")
        .join(dir_basename(&main_dir));

    // First derivation: exact slug.
    let (name, path) = derive_worktree(&main_dir, &repo, "feature/login").expect("derive");
    assert_eq!(name, "feature-login");
    assert_eq!(path, container.join("feature-login"));
    assert!(path.starts_with(&container), "leaf must stay in container");

    // Force a collision: create the directory, re-derive → "-2".
    std::fs::create_dir_all(&path).expect("mkdir collision");
    let (name2, path2) = derive_worktree(&main_dir, &repo, "feature/login").expect("derive");
    assert_eq!(name2, "feature-login-2");
    assert_eq!(path2, container.join("feature-login-2"));
    assert!(path2.starts_with(&container));
}

/// §2.4 runtime containment: paths outside (or equal to) the container are
/// rejected with a `Git` error, never silently accepted.
#[test]
fn ensure_contained_runtime_check() {
    let container = Path::new("/repo/.worktrees");
    assert!(ensure_contained(Path::new("/repo/.worktrees/feat"), container).is_ok());
    // The container itself is not a valid leaf.
    match ensure_contained(container, container) {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git error for container itself, got {other:?}"),
    }
    for escapee in ["/repo/elsewhere", "/repo", "/other/.worktrees/feat"] {
        match ensure_contained(Path::new(escapee), container) {
            Err(AppError::Git(_)) => {}
            other => panic!("expected Git error for {escapee:?}, got {other:?}"),
        }
    }
}

/// §2.9: blank args are rejected up-front with `InvalidName`.
#[test]
fn blank_args_are_invalid_name() {
    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    git2::Repository::init(&repo_dir).expect("init");
    match add_worktree(&repo_dir, "   ", "   ") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName for blank branch, got {other:?}"),
    }
    match remove_worktree(&repo_dir, "") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName for blank name, got {other:?}"),
    }
    match lock_worktree(&repo_dir, " ", None) {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName for blank name, got {other:?}"),
    }
    match unlock_worktree(&repo_dir, "") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName for blank name, got {other:?}"),
    }
}

/// A branch whose sanitized slug is empty is rejected before any path work.
#[test]
fn derive_worktree_rejects_empty_slug() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path().join("repo")).expect("init");
    let main_dir = repo.workdir().expect("workdir").to_path_buf();
    match derive_worktree(&main_dir, &repo, "///") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName, got {other:?}"),
    }
}

// ---------------------------------- P36 §1.2: branch_checked_out_elsewhere

/// Deterministic identity + autocrlf off (== discard.rs / branches.rs init).
fn wt_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch.
fn wt_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    crate::git::stage::stage_paths(
        dir,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
}

/// The short branch name HEAD points at (default "master"/"main").
fn wt_head_branch(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let head = repo.head().expect("HEAD");
    head.shorthand().expect("shorthand").to_string()
}

/// §1.2: a linked worktree on `feature` is flagged; the returned path equals
/// that worktree's abs_path (forward slashes).
#[test]
fn branch_checked_out_elsewhere_flags_linked_worktree() {
    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    wt_init(&repo_dir);
    wt_commit(&repo_dir, "base", &[("a.txt", "base\n")]);
    crate::git::branches::create_branch(&repo_dir, "feature").expect("create feature");

    let created = add_worktree(&repo_dir, "feature", "feature").expect("add worktree");

    let flagged = branch_checked_out_elsewhere(&repo_dir, "feature").expect("guard");
    assert_eq!(
        flagged.as_deref(),
        Some(created.abs_path.as_str()),
        "must name the linked worktree's abs_path"
    );
}

/// §1.2: the caller's OWN worktree is never a collision — asking about the
/// currently checked-out branch returns None even while a linked worktree for
/// a different branch exists.
#[test]
fn branch_checked_out_elsewhere_never_flags_self() {
    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    wt_init(&repo_dir);
    wt_commit(&repo_dir, "base", &[("a.txt", "base\n")]);
    let current = wt_head_branch(&repo_dir);
    crate::git::branches::create_branch(&repo_dir, "feature").expect("create feature");
    add_worktree(&repo_dir, "feature", "feature").expect("add worktree");

    // The main worktree's own branch is free from the main worktree.
    let self_flag = branch_checked_out_elsewhere(&repo_dir, &current).expect("guard");
    assert_eq!(self_flag, None, "own worktree must never be flagged");

    // A branch that exists but is NOT checked out in any worktree is free.
    crate::git::branches::create_branch(&repo_dir, "idle").expect("create idle");
    let idle_flag = branch_checked_out_elsewhere(&repo_dir, "idle").expect("guard");
    assert_eq!(idle_flag, None, "branch not in any worktree must be free");
}
