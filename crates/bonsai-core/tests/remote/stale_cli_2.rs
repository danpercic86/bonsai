//! T2 Area 7 — stale-branch cleanup HARDENING extensions (contract §3 Area 7).
//!
//! The base-protection audit fixes (F-A7-1/3/4/5/9) have inline git2 unit tests
//! in `git/stale.rs`; this file exercises them end-to-end through the public API
//! on repos built with the `git` CLI, plus the read-only-purity guarantee.
//!
//! NOTE: F-A7-3 (tip-moved-since-scan) requires an external mutation in the
//! window BETWEEN scan and delete — a window that lives entirely INSIDE a single
//! `delete_branches` call — so it is only reproducible at unit level
//! (`recheck_tip_detects_moved_tip` in `git/stale.rs`) and is not re-created here.
//!
//! Scratch repos on D:. Skips (passes with a note) w/o `git`.

use bonsai_core::error::AppError;
use bonsai_core::git::stale::{delete_branches, find_stale_branches, BranchDeleteStatus, StaleReason};
use crate::common;
use crate::common::{commit_fixed, git, git_ok, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// main: C0 → C1; `feat` merged (@ C0, ancestor of main); tag `v1` @ main tip.
/// Returns (dir, c0, c1_main_tip).
fn fixture() -> (tempfile::TempDir, String, String) {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("a.txt"), "a\n").unwrap();
    git(path, &["add", "-A"]);
    commit_fixed(path, "C0");
    let c0 = git(path, &["rev-parse", "HEAD"]);
    std::fs::write(path.join("b.txt"), "b\n").unwrap();
    git(path, &["add", "-A"]);
    commit_fixed(path, "C1");
    let c1 = git(path, &["rev-parse", "HEAD"]);
    git(path, &["branch", "feat", &c0]); // merged (ancestor of main tip)
    git(path, &["tag", "v1"]); // annotated-free lightweight tag @ main tip
    (dir, c0, c1)
}

// -------------------------------------- F-A7-1: base as refname / OID / tag

/// The base branch `main` must NEVER be exposed as stale/deletable regardless of
/// how the base is spelled: full refname, raw OID, or a tag at the base tip.
#[test]
fn base_protected_for_refname_oid_and_tag() {
    require_git!();
    for form in ["refname", "oid", "tag"] {
        let (dir, _c0, c1) = fixture();
        let path = dir.path();
        let base: String = match form {
            "refname" => "refs/heads/main".to_string(),
            "oid" => c1.clone(),
            _ => "v1".to_string(),
        };

        // Read-only classify: `main` absent, `feat` present as merged.
        let report = find_stale_branches(path, Some(&base)).expect("classify");
        assert!(!report.branches.iter().any(|b| b.name == "main"),
            "[{form}] main must never be classified stale: {:?}", report.branches);
        let feat = report.branches.iter().find(|b| b.name == "feat")
            .unwrap_or_else(|| panic!("[{form}] feat must be listed"));
        assert_eq!(feat.reason, StaleReason::Merged, "[{form}] feat merged");

        // Destructive: feat deleted, main REFUSED and surviving.
        let names = vec!["feat".to_string(), "main".to_string()];
        let results = delete_branches(path, &names, Some(&base)).expect("delete");
        let status = |n: &str| results.iter().find(|r| r.name == n).map(|r| r.status).unwrap();
        assert_eq!(status("feat"), BranchDeleteStatus::Deleted, "[{form}] feat deleted");
        assert_ne!(status("main"), BranchDeleteStatus::Deleted, "[{form}] main NOT deleted");
        assert!(git_ok(path, &["rev-parse", "--verify", "refs/heads/main"]),
            "[{form}] main ref survives");
    }
}

// ---------------------------------------- F-A7-5: Deleted rows carry the tip

/// A Deleted row's `message` is `"was at <short-oid>"` (recovery aid for undo).
#[test]
fn deleted_row_carries_was_at_short_oid() {
    require_git!();
    let (dir, c0, _c1) = fixture();
    let path = dir.path();

    let results = delete_branches(path, &["feat".to_string()], Some("main")).expect("delete");
    let feat = results.iter().find(|r| r.name == "feat").expect("feat row");
    assert_eq!(feat.status, BranchDeleteStatus::Deleted);
    let msg = feat.message.as_deref().unwrap_or("");
    assert!(msg.starts_with("was at "), "message names the deleted tip: {msg:?}");
    assert!(c0.starts_with(&msg["was at ".len()..]), "short-oid is feat's tip (C0): {msg:?}");
}

// -------------------------------------- F-A7-4: remote base protects local

/// A remote-tracking base (`origin/main`) protects the LOCAL `main` counterpart:
/// `main` is neither classified stale nor deletable.
#[test]
fn remote_base_protects_local_counterpart() {
    require_git!();
    let (dir, c0, _c1) = fixture();
    let path = dir.path();

    // Fabricate a remote-tracking ref origin/main at main's tip.
    git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    let _ = c0;

    let report = find_stale_branches(path, Some("origin/main")).expect("classify");
    assert_eq!(report.base, "origin/main");
    assert!(!report.branches.iter().any(|b| b.name == "main"),
        "local main protected under a remote base: {:?}", report.branches);

    let results = delete_branches(path, &["main".to_string()], Some("origin/main")).expect("delete");
    assert_ne!(results[0].status, BranchDeleteStatus::Deleted, "local main NOT deleted");
    assert!(git_ok(path, &["rev-parse", "--verify", "refs/heads/main"]), "main survives");
}

// -------------------------------------------- F-A7-9: dangling ref skipped

/// A dangling/corrupt loose branch ref must be skipped, not abort the scan.
#[test]
fn dangling_branch_ref_skipped_not_fatal() {
    require_git!();
    let (dir, _c0, _c1) = fixture();
    let path = dir.path();

    // Handcraft a loose branch ref pointing at a non-existent object.
    let refs = path.join(".git").join("refs").join("heads");
    std::fs::write(refs.join("dangling"), format!("{}\n", "0".repeat(40))).expect("write bad ref");

    // Scan still succeeds and still classifies the healthy `feat` branch.
    let report = find_stale_branches(path, Some("main")).expect("scan survives a dangling ref");
    assert!(report.branches.iter().any(|b| b.name == "feat"),
        "healthy branch still classified despite the dangling ref");
}

// ------------------------------------------------- unicode + empty batch

/// A unicode branch name is classified and deleted correctly.
#[test]
fn unicode_branch_name_stale_and_deletable() {
    require_git!();
    let (dir, c0, _c1) = fixture();
    let path = dir.path();
    let uni = "feature-café-日本";
    git(path, &["branch", uni, &c0]); // merged

    let report = find_stale_branches(path, Some("main")).expect("classify");
    assert!(report.branches.iter().any(|b| b.name == uni),
        "unicode branch classified stale: {:?}", report.branches.iter().map(|b| &b.name).collect::<Vec<_>>());

    let results = delete_branches(path, &[uni.to_string()], Some("main")).expect("delete");
    assert_eq!(results[0].status, BranchDeleteStatus::Deleted);
    assert!(!git_ok(path, &["rev-parse", "--verify", &format!("refs/heads/{uni}")]),
        "unicode branch deleted");
}

/// An empty names batch is a clean no-op (never errors, deletes nothing).
#[test]
fn empty_names_batch_is_noop() {
    require_git!();
    let (dir, _c0, _c1) = fixture();
    let path = dir.path();
    let results = delete_branches(path, &[], Some("main")).expect("empty batch Ok");
    assert!(results.is_empty(), "no result rows for an empty batch");
    assert!(git_ok(path, &["rev-parse", "--verify", "refs/heads/feat"]), "feat untouched");
}

// ------------------------------------------------------ bare / unborn errs

/// A bare repo and an unborn HEAD both surface a clean `AppError` (no panic).
#[test]
fn bare_and_unborn_error_cleanly() {
    require_git!();
    // Bare repo.
    let bare = common::scratch_dir();
    git(bare.path(), &["init", "--bare", "-b", "main"]);
    match find_stale_branches(bare.path(), Some("main")) {
        Err(AppError::Git(_)) => {}
        other => panic!("bare repo must be a clean Git error, got {other:?}"),
    }

    // Unborn HEAD (no commits) with base "main".
    let unborn = init_repo();
    match find_stale_branches(unborn.path(), Some("main")) {
        Err(_) => {} // clean AppError (unresolvable base)
        Ok(r) => panic!("unborn base should not resolve, got {r:?}"),
    }
}

// --------------------------------------------------- scan is READ-ONLY pure

/// `find_stale_branches` mutates nothing: all refs + the HEAD reflog are
/// byte-identical before and after the scan.
#[test]
fn scan_is_pure_refs_and_reflog_unchanged() {
    require_git!();
    let (dir, _c0, _c1) = fixture();
    let path = dir.path();

    let refs_before = git(path, &["for-each-ref", "--format=%(refname) %(objectname)"]);
    let reflog_before = git(path, &["reflog", "show", "--format=%H %gs", "HEAD"]);

    let _ = find_stale_branches(path, Some("main")).expect("scan");
    let _ = find_stale_branches(path, None).expect("scan auto-base");

    let refs_after = git(path, &["for-each-ref", "--format=%(refname) %(objectname)"]);
    let reflog_after = git(path, &["reflog", "show", "--format=%H %gs", "HEAD"]);

    assert_eq!(refs_before, refs_after, "scan must not move any ref");
    assert_eq!(reflog_before, reflog_after, "scan must not touch the reflog");
}
