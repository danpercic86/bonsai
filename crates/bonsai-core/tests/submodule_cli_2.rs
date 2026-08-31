//! T2 Area 7 — submodule HARDENING extensions (contract §3 Area 7).
//!
//! `submodule_cli.rs` covers the add/deinit/remove round-trip vs the `git` CLI.
//! This file adds: list/init/sync/update round-trip + status-letter parity vs
//! `git submodule status`, safe-checkout refusal on a dirty submodule, the
//! missing-`.gitmodules`-with-gitlink corner, the F-A7-2 traversal regression,
//! `ext::` URL rejection, and the F-A7-10 add-rollback retry.
//!
//! All submodules use a LOCAL `file://` URL (no creds, no network). Scratch on
//! D:. Skips (passes with a note) w/o `git`.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::search::SpawnGitRunner;
use bonsai_core::git::submodule::{
    add_submodule, init_submodule, list_submodules, remove_submodule, sync_submodule,
    update_submodule, SubmoduleStatus,
};
use common::{commit_fixed, file_url, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

const SUB_PATH: &str = "vendor/sub";

/// Upstream sub-repo with two commits (v1, v2 on `lib.txt`). Returns
/// (dir, url, v1_oid, v2_oid); HEAD is at v2.
fn build_sub() -> (tempfile::TempDir, String, String, String) {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("lib.txt"), "sub v1\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "sub v1");
    let v1 = git(p, &["rev-parse", "HEAD"]);
    std::fs::write(p.join("lib.txt"), "sub v2\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "sub v2");
    let v2 = git(p, &["rev-parse", "HEAD"]);
    let url = file_url(p);
    (dir, url, v1, v2)
}

/// Superproject with one commit and a submodule at SUB_PATH added + committed.
fn build_super_with_sub(url: &str) -> tempfile::TempDir {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: initial");
    add_submodule(p, url, SUB_PATH).expect("add_submodule");
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: add submodule");
    dir
}

/// Leading status char from `git submodule status <path>` (` `/`+`/`-`). Uses
/// RAW output — the trimming `git()` helper would strip the space (UpToDate).
fn cli_status_char(super_dir: &Path) -> char {
    let raw = common::git_raw(super_dir, &["submodule", "status", "--", SUB_PATH], &[]);
    raw.first().map(|&b| b as char).unwrap_or('?')
}

fn only(super_dir: &Path) -> bonsai_core::git::submodule::SubmoduleInfo {
    let mut v = list_submodules(super_dir).expect("list");
    assert_eq!(v.len(), 1, "exactly one submodule: {v:?}");
    v.pop().unwrap()
}

// -------------------------------------- status-letter parity + init/sync

/// list/init/sync round-trip and status-letter parity: UpToDate (` `) after
/// add, then OutOfSync (`+`) once the submodule workdir checks out an older
/// commit than the superproject pins.
#[test]
fn status_letter_parity_and_init_sync_roundtrip() {
    require_git!();
    let (_sub, url, v1, v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();

    // Fresh add → checked out at the pinned commit (v2), clean.
    let info = only(p);
    assert_eq!(info.status, SubmoduleStatus::UpToDate, "clean matching → UpToDate");
    assert_eq!(cli_status_char(p), ' ', "git status char is space for UpToDate");
    assert_eq!(info.wt_oid.as_deref(), Some(v2.as_str()), "workdir at v2");

    // init + sync are no-op-safe on an already-registered submodule.
    init_submodule(p, SUB_PATH).expect("init idempotent");
    sync_submodule(p, SUB_PATH).expect("sync idempotent");

    // Check out v1 INSIDE the submodule → workdir commit != pinned (v2).
    git(&p.join(SUB_PATH), &["checkout", &v1]);
    let info = only(p);
    assert_eq!(info.status, SubmoduleStatus::OutOfSync, "workdir != pinned → OutOfSync");
    assert_eq!(cli_status_char(p), '+', "git status char is '+' for OutOfSync");
}

// -------------------------------------------------- update safe-checkout

/// `update_submodule` uses a SAFE checkout: a dirty tracked edit in the
/// submodule that a checkout-to-pinned would clobber makes update REFUSE, and
/// the dirty content survives.
#[test]
fn update_refuses_to_clobber_dirty_submodule() {
    require_git!();
    let (_sub, url, v1, _v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let sub_wt = p.join(SUB_PATH);

    // Move the submodule workdir to v1, then dirty lib.txt so a checkout back to
    // the pinned v2 would overwrite local modifications.
    git(&sub_wt, &["checkout", &v1]);
    std::fs::write(sub_wt.join("lib.txt"), "LOCAL UNCOMMITTED EDIT\n").expect("dirty");

    let res = update_submodule(p, SUB_PATH);
    assert!(res.is_err(), "safe checkout must refuse to clobber a dirty submodule: {res:?}");
    assert_eq!(
        std::fs::read_to_string(sub_wt.join("lib.txt")).unwrap(),
        "LOCAL UNCOMMITTED EDIT\n",
        "the dirty edit must survive the refused update",
    );
}

// ------------------------------------------- missing .gitmodules + gitlink

/// A superproject with a staged gitlink but a DELETED `.gitmodules` must not
/// panic list_submodules — it degrades cleanly.
#[test]
fn missing_gitmodules_with_gitlink_is_clean() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();

    std::fs::remove_file(p.join(".gitmodules")).expect("rm .gitmodules");
    // Must not panic; whatever git2 reports, it is a clean Result.
    let listed = list_submodules(p);
    assert!(listed.is_ok(), "list must not panic on a gitlink without .gitmodules: {listed:?}");
}

// ---------------------------------------------- F-A7-2 traversal refusal

/// A hostile `.gitmodules` name (`../escape`) fed to `remove_submodule` is
/// refused BEFORE any destructive git step — the validation fires first.
#[test]
fn remove_submodule_refuses_traversal_name() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super");

    // A sentinel file OUTSIDE .git/modules that a traversal delete could reach.
    let sentinel = p.join(".git").join("sentinel.txt");
    std::fs::write(&sentinel, b"keep me").expect("sentinel");

    match remove_submodule(p, &SpawnGitRunner, "../../sentinel", false) {
        Err(AppError::Git(m)) => assert!(m.contains("unsafe name"), "got: {m}"),
        other => panic!("traversal name must be refused, got {other:?}"),
    }
    assert!(sentinel.exists(), "no file outside .git/modules was touched");
}

// ------------------------------------------------------- ext:: rejected

/// An `ext::` transport URL is rejected by `add_submodule` (libgit2 has no ext
/// transport) — the helper command is NEVER spawned.
#[test]
fn add_submodule_rejects_ext_url() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super");

    let res = add_submodule(p, "ext::sh -c \"touch pwned\"", "evil");
    assert!(res.is_err(), "ext:: URL must be rejected: {res:?}");
    assert!(!p.join("pwned").exists(), "the ext:: helper command must not run");
}

// ------------------------------------------ F-A7-10 add rollback + retry

/// A failed add (bogus URL) rolls back its residue so a subsequent add at the
/// SAME path succeeds — never a lingering "submodule already exists".
#[test]
fn add_failure_rolls_back_and_retry_succeeds() {
    require_git!();
    let (_sub, good_url, _v1, _v2) = build_sub();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super");

    // Point at a nonexistent local repo → clone fails.
    let bogus = file_url(&p.join("does-not-exist"));
    let first = add_submodule(p, &bogus, SUB_PATH);
    assert!(first.is_err(), "add with a bogus URL must fail: {first:?}");

    // Retry with the good URL at the same path must NOT hit "already exists".
    let retry = add_submodule(p, &good_url, SUB_PATH);
    assert!(retry.is_ok(), "retry after rollback must succeed: {retry:?}");
}
