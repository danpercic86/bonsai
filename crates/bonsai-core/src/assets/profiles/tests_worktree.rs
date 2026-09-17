//! P31 tests: schema v2 migration + per-worktree activation.

use tempfile::TempDir;

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::stage::validate_rel_path;

use super::test_support::{profile, target};
use super::worktree::open_repo_at;
use super::*;

// ---------- P31: schema v2 migration + per-worktree activation ----------

/// Scratch git fixture under D:\Data\Temp\bonsai-scratch: main repo with a
/// committed CLAUDE.md + two branches and two linked worktrees
/// ("feature-x", "feature-y").
fn git_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    let repo = git2::Repository::init(&repo_dir).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "Test").unwrap();
    cfg.set_str("user.email", "test@example.com").unwrap();
    std::fs::write(repo_dir.join("CLAUDE.md"), b"# base\n").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("CLAUDE.md")).unwrap();
    idx.write().unwrap();
    let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
    let sig = repo.signature().unwrap();
    let head = repo
        .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();
    let commit = repo.find_commit(head).unwrap();
    repo.branch("feature/x", &commit, false).unwrap();
    repo.branch("feature/y", &commit, false).unwrap();
    let wx = crate::git::worktree::add_worktree(&repo_dir, "feature/x", "feature/x").unwrap();
    let wy = crate::git::worktree::add_worktree(&repo_dir, "feature/y", "feature/y").unwrap();
    assert_eq!(wx.name, "feature-x");
    assert_eq!(wy.name, "feature-y");
    (
        dir,
        repo_dir,
        PathBuf::from(wx.abs_path),
        PathBuf::from(wy.abs_path),
    )
}

const V1_FIXTURE: &str = r##"{
  "version": 1,
  "profiles": [
    { "name": "opus", "targets": [ { "assetId": "claude", "content": "# opus\n" } ] }
  ],
  "activeProfile": "opus"
}"##;

// §9.1 — migration: v1 loads unchanged, read is byte-safe, save stamps v2.
#[test]
fn v1_store_loads_byte_safe_and_migrates_on_save() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join(".bonsai").join("profiles.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, V1_FIXTURE.as_bytes()).unwrap();

    // Read: v1 parses, empty map, legacy active honored as "@main".
    let store = list_profiles(tmp.path()).unwrap();
    assert_eq!(store.version, 1);
    assert!(store.worktree_activations.is_empty());
    assert_eq!(store.active_profile.as_deref(), Some("opus"));
    assert_eq!(store.effective_activation(MAIN_WORKTREE_KEY), Some("opus"));
    // A pure read leaves the file BYTE-identical.
    assert_eq!(std::fs::read(&path).unwrap(), V1_FIXTURE.as_bytes());

    // First save stamps version 2 + materializes the "@main" mirror.
    let saved = save_profile(tmp.path(), profile("haiku", vec![])).unwrap();
    assert_eq!(saved.version, 2);
    assert_eq!(
        saved
            .worktree_activations
            .get(MAIN_WORKTREE_KEY)
            .map(String::as_str),
        Some("opus")
    );
    assert_eq!(saved.active_profile.as_deref(), Some("opus"));

    // v2 round-trips.
    let reloaded = list_profiles(tmp.path()).unwrap();
    assert_eq!(reloaded, saved);
}

// §3 — delete_profile clears matching worktree-activation entries.
#[test]
fn delete_profile_clears_matching_map_entries() {
    let (_dir, main, _wx, _wy) = git_fixture();
    save_profile(&main, profile("p", vec![target("claude", "# p\n")])).unwrap();
    activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
    activate_profile_for_worktree(&main, MAIN_WORKTREE_KEY, "p").unwrap();
    let store = delete_profile(&main, "p").unwrap();
    assert!(store.worktree_activations.is_empty());
    assert_eq!(store.active_profile, None);
}

// §3 key hygiene — stale keys are GC'd on the next persist.
#[test]
fn persist_garbage_collects_stale_worktree_keys() {
    let (_dir, main, _wx, _wy) = git_fixture();
    save_profile(&main, profile("p", vec![])).unwrap();
    // Inject a stale key directly into the store file.
    let path = main.join(".bonsai").join("profiles.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    v["worktreeActivations"] = serde_json::json!({ "ghost": "p", "feature-x": "p" });
    std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

    let store = save_profile(&main, profile("q", vec![])).unwrap();
    assert!(
        !store.worktree_activations.contains_key("ghost"),
        "stale key GC'd"
    );
    assert_eq!(
        store
            .worktree_activations
            .get("feature-x")
            .map(String::as_str),
        Some("p"),
        "live worktree key kept"
    );
}

// §9.2 — shared-store resolution from a linked worktree.
#[test]
fn linked_worktree_reads_and_writes_the_main_store() {
    let (_dir, main, wx, _wy) = git_fixture();
    assert_eq!(worktree_key_for(&main).unwrap(), MAIN_WORKTREE_KEY);
    assert_eq!(worktree_key_for(&wx).unwrap(), "feature-x");
    assert_eq!(resolve_store_root(&wx), resolve_store_root(&main));

    // save via the LINKED worktree lands in the MAIN store.
    save_profile(&wx, profile("p", vec![target("claude", "# p\n")])).unwrap();
    assert!(main.join(".bonsai").join("profiles.json").is_file());
    assert!(
        !wx.join(".bonsai").exists(),
        "no .bonsai in the linked worktree"
    );
    assert_eq!(list_profiles(&wx).unwrap().profiles.len(), 1);
    assert_eq!(list_profiles(&main).unwrap().profiles.len(), 1);
}

// §9.3 — activation writes into THAT worktree only; map persisted;
// second run idempotent.
#[test]
fn activate_into_linked_worktree_writes_only_there() {
    let (_dir, main, wx, wy) = git_fixture();
    save_profile(
        &main,
        profile(
            "p",
            vec![
                target("claude", "# wt claude\n"),
                target("agents", "# wt agents\n"),
            ],
        ),
    )
    .unwrap();
    let main_claude_before = std::fs::read(main.join("CLAUDE.md")).unwrap();
    let wy_claude_before = std::fs::read(wy.join("CLAUDE.md")).unwrap();

    let act = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
    assert_eq!(act.results.len(), 2);

    // Byte-exact writes INSIDE the linked worktree.
    assert_eq!(
        std::fs::read(wx.join("CLAUDE.md")).unwrap(),
        b"# wt claude\n"
    );
    assert_eq!(
        std::fs::read(wx.join("AGENTS.md")).unwrap(),
        b"# wt agents\n"
    );
    assert!(!wx.join("CLAUDE.md.bonsai-tmp").exists());
    assert!(!wx.join("AGENTS.md.bonsai-tmp").exists());
    // Main + sibling worktree untouched (byte-compare).
    assert_eq!(
        std::fs::read(main.join("CLAUDE.md")).unwrap(),
        main_claude_before
    );
    assert!(!main.join("AGENTS.md").exists());
    assert_eq!(
        std::fs::read(wy.join("CLAUDE.md")).unwrap(),
        wy_claude_before
    );
    assert!(!wy.join("AGENTS.md").exists());

    // Map persisted; legacy mirror NOT set (key != "@main").
    let store = list_profiles(&main).unwrap();
    assert_eq!(
        store
            .worktree_activations
            .get("feature-x")
            .map(String::as_str),
        Some("p")
    );
    assert_eq!(store.active_profile, None);

    // Second identical run: all unchanged, nothing blocked.
    let again = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
    assert!(again
        .results
        .iter()
        .all(|r| r.action == TargetWriteAction::Unchanged));
}

// "@main" keying + legacy mirror via the D5 wrapper.
#[test]
fn legacy_activate_records_main_key_and_mirror() {
    let (_dir, main, wx, _wy) = git_fixture();
    save_profile(&main, profile("p", vec![target("agents", "# a\n")])).unwrap();
    // Wrapper from the MAIN worktree → "@main" + legacy mirror.
    let act = activate_profile(&main, "p").unwrap();
    assert_eq!(
        act.store
            .worktree_activations
            .get(MAIN_WORKTREE_KEY)
            .map(String::as_str),
        Some("p")
    );
    assert_eq!(act.store.active_profile.as_deref(), Some("p"));
    // Wrapper from the LINKED worktree tab → records under its own key (D5).
    save_profile(&main, profile("q", vec![target("gemini", "# g\n")])).unwrap();
    let act2 = activate_profile(&wx, "q").unwrap();
    assert_eq!(
        act2.store
            .worktree_activations
            .get("feature-x")
            .map(String::as_str),
        Some("q")
    );
    assert!(wx.join("GEMINI.md").is_file());
    assert!(!main.join("GEMINI.md").exists());
}

// §9.4 — D7 dirty-target guard: tracked+modified blocks BEFORE any write;
// untracked does not block.
#[test]
fn dirty_tracked_target_blocks_all_writes() {
    let (_dir, main, wx, _wy) = git_fixture();
    // Target #1 (agents) is missing/clean; target #2 (claude) is tracked +
    // human-modified in the target worktree.
    std::fs::write(wx.join("CLAUDE.md"), b"# human edit\n").unwrap();
    save_profile(
        &main,
        profile(
            "p",
            vec![target("agents", "# a\n"), target("claude", "# machine\n")],
        ),
    )
    .unwrap();

    let err = activate_profile_for_worktree(&main, "feature-x", "p").unwrap_err();
    assert!(
        matches!(&err, AppError::Git(m) if m.contains("uncommitted changes")),
        "expected dirty-target Git error, got {err:?}"
    );
    // ZERO files written: target #1 not created, target #2 byte-preserved.
    assert!(
        !wx.join("AGENTS.md").exists(),
        "all targets checked before any write"
    );
    assert_eq!(
        std::fs::read(wx.join("CLAUDE.md")).unwrap(),
        b"# human edit\n"
    );

    // UNTRACKED target file does NOT block (prior uncommitted activation).
    std::fs::write(wx.join("GEMINI.md"), b"# old untracked\n").unwrap();
    save_profile(&main, profile("q", vec![target("gemini", "# new\n")])).unwrap();
    let act = activate_profile_for_worktree(&main, "feature-x", "q").unwrap();
    assert_eq!(act.results[0].action, TargetWriteAction::Written);
    assert_eq!(std::fs::read(wx.join("GEMINI.md")).unwrap(), b"# new\n");
}

// Carry-forward (a): a GITIGNORED target file never blocks activation —
// like untracked, git holds no committed version of it to protect.
#[test]
fn gitignored_target_does_not_block_activation() {
    let (_dir, main, wx, _wy) = git_fixture();
    // GEMINI.md is ignored in the target worktree and holds stale content.
    std::fs::write(wx.join(".gitignore"), b"GEMINI.md\n").unwrap();
    std::fs::write(wx.join("GEMINI.md"), b"# old ignored\n").unwrap();
    save_profile(&main, profile("p", vec![target("gemini", "# fresh\n")])).unwrap();

    let act = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
    assert_eq!(act.results[0].action, TargetWriteAction::Written);
    assert_eq!(std::fs::read(wx.join("GEMINI.md")).unwrap(), b"# fresh\n");
}

// Carry-forward (b): the D5 wrappers fall back to "@main" ONLY for
// non-repo dirs. A real linked worktree whose identity cannot be resolved
// propagates the error instead of silently retargeting MAIN.
#[test]
fn wrapper_propagates_identity_errors_for_real_repos() {
    let (_dir, main, wx, _wy) = git_fixture();
    save_profile(&main, profile("p", vec![target("gemini", "# g\n")])).unwrap();

    // Break feature-x's identity while keeping its repo openable: move the
    // admin dir OUT of `.git/worktrees/` and repoint the worktree's `.git`
    // file at it. The repo still opens (the gitdir layout is intact), but
    // `find_worktree(<basename>)` fails (not registered under worktrees/)
    // and the canonical-path fallback scan finds no registered worktrees.
    let admin_old = main.join(".git").join("worktrees").join("feature-x");
    let admin_new = main.join(".git").join("ghost");
    std::fs::rename(&admin_old, &admin_new).unwrap();
    std::fs::write(
        wx.join(".git"),
        format!("gitdir: {}\n", admin_new.display()),
    )
    .unwrap();

    // Precondition: the worktree still opens as a repo, but its identity
    // cannot be established.
    assert!(open_repo_at(&wx).is_ok(), "worktree repo must still open");
    assert!(worktree_key_for(&wx).is_err());

    let err = activate_profile(&wx, "p").unwrap_err();
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    // Nothing was written anywhere, and no activation was recorded.
    assert!(!main.join("GEMINI.md").exists());
    assert!(!wx.join("GEMINI.md").exists());
    assert!(list_profiles(&main)
        .unwrap()
        .worktree_activations
        .is_empty());
    assert!(matches!(
        preview_profile(&wx, "p").unwrap_err(),
        AppError::Git(_)
    ));
}

// §9.5 — D6 eligibility: locked / invalid worktrees refuse preview AND activate.
#[test]
fn locked_and_invalid_worktrees_are_refused() {
    let (_dir, main, wx, _wy) = git_fixture();
    save_profile(&main, profile("p", vec![target("claude", "# p\n")])).unwrap();

    crate::git::worktree::lock_worktree(&main, "feature-y", Some("pinned")).unwrap();
    for res in [
        preview_profile_for_worktree(&main, "feature-y", "p").map(|_| ()),
        activate_profile_for_worktree(&main, "feature-y", "p").map(|_| ()),
    ] {
        match res {
            Err(AppError::Git(m)) => assert!(m.contains("locked"), "got: {m}"),
            other => panic!("expected locked refusal, got {other:?}"),
        }
    }

    // Invalid: delete the linked worktree's working directory.
    std::fs::remove_dir_all(&wx).unwrap();
    for res in [
        preview_profile_for_worktree(&main, "feature-x", "p").map(|_| ()),
        activate_profile_for_worktree(&main, "feature-x", "p").map(|_| ()),
    ] {
        match res {
            Err(AppError::Git(_)) => {}
            other => panic!("expected invalid/prunable refusal, got {other:?}"),
        }
    }

    // Unknown worktree key → precise Git error.
    match activate_profile_for_worktree(&main, "nope", "p") {
        Err(AppError::Git(m)) => assert!(m.contains("not found")),
        other => panic!("expected not-found, got {other:?}"),
    }
}

// §9.7 — every written path stays under the target worktree root.
#[test]
fn written_paths_are_contained_in_the_target_worktree() {
    let (_dir, main, wx, _wy) = git_fixture();
    save_profile(
        &main,
        profile(
            "p",
            vec![target("agents", "# a\n"), target("gemini", "# g\n")],
        ),
    )
    .unwrap();
    let act = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
    for r in &act.results {
        validate_rel_path(&r.path).unwrap();
        let full = wx.join(&r.path);
        assert!(full.starts_with(&wx), "{} escapes the worktree", r.path);
        assert!(full.is_file());
    }
}
