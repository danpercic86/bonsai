//! P19 CLI-oracle submodule tests (contract §8).
//!
//! Every "remote" is a LOCAL repo referenced by a plain absolute path (the
//! local transport needs NO network and NO credentials — mirrors
//! `tests/remote_cli.rs`). Fixture (contract §8.1): a bare submodule origin +
//! seed clone that publishes commits A then B; a superproject `super` with the
//! submodule added (pinned at B); and a fresh clone `work` Bonsai operates on.
//!
//! Honest coverage note: the local transport never invokes the credentials
//! callback (the retry guard / error mapping are covered structurally by the
//! unit tests in `src/git/remote.rs`); the real credential path is a USER
//! CHECKPOINT. `protocol.file.allow=always` is set on the CLI submodule
//! commands so recent Git allows the local-path submodule transport.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::git::submodule::{
    init_submodule, list_submodules, sync_submodule, update_submodule, SubmoduleStatus,
};
use common::{commit_fixed, git, git_raw};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// git URLs want forward slashes even on Windows (the local transport parses
/// backslashes poorly). Absolute path only — no `file://` scheme (matches
/// remote_cli's proven Windows approach).
fn url_for(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn configure_identity(repo: &Path) {
    git(repo, &["config", "user.name", "Test User"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "core.autocrlf", "false"]);
}

/// Superproject `super`, a fresh `work` clone of it (submodule NOT recursed →
/// `sub` registered but uninitialized), and the submodule commit oids A/B.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    work: PathBuf,
    /// First submodule commit (parent of the pinned tip).
    oid_a: String,
    /// Pinned submodule commit (recorded in the superproject).
    oid_b: String,
}

/// Builds the §8.1 fixture. The submodule name defaults to its path, "sub".
fn setup() -> Fixture {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();

    // 1. Bare submodule origin + seed clone publishing commits A then B.
    git(&root, &["init", "--bare", "-b", "main", "sub-origin.git"]);
    let sub_origin = root.join("sub-origin.git");

    git(&root, &["clone", &url_for(&sub_origin), "sub-seed"]);
    let sub_seed = root.join("sub-seed");
    configure_identity(&sub_seed);
    git(&sub_seed, &["checkout", "-B", "main"]);
    std::fs::write(sub_seed.join("mod.txt"), "A\n").expect("write A");
    git(&sub_seed, &["add", "-A"]);
    commit_fixed(&sub_seed, "submodule commit A");
    let oid_a = git(&sub_seed, &["rev-parse", "HEAD"]);
    std::fs::write(sub_seed.join("mod.txt"), "B\n").expect("write B");
    git(&sub_seed, &["add", "-A"]);
    commit_fixed(&sub_seed, "submodule commit B");
    let oid_b = git(&sub_seed, &["rev-parse", "HEAD"]);
    git(&sub_seed, &["push", "-u", "origin", "main"]);

    // 2. Superproject with the submodule added (pins at tip B) + committed.
    git(&root, &["init", "-b", "main", "super"]);
    let superp = root.join("super");
    configure_identity(&superp);
    std::fs::write(superp.join("README.md"), "super\n").expect("write readme");
    git(&superp, &["add", "-A"]);
    commit_fixed(&superp, "superproject initial");
    git(
        &superp,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            &url_for(&sub_origin),
            "sub",
        ],
    );
    git(&superp, &["add", "-A"]);
    commit_fixed(&superp, "add submodule sub");

    // 3. Fresh clone `work` WITHOUT recursing → `sub` registered but empty.
    git(&root, &["clone", &url_for(&superp), "work"]);
    let work = root.join("work");
    configure_identity(&work);

    Fixture {
        _dir: dir,
        root,
        work,
        oid_a,
        oid_b,
    }
}

/// Leading sigil of `git -C <super> submodule status` for the single submodule
/// (`-` uninitialized, ` ` up-to-date, `+` out-of-sync). Read RAW (not trimmed)
/// so the space sigil survives.
fn status_sigil(superp: &Path) -> char {
    let raw = git_raw(superp, &["submodule", "status"], &[]);
    let s = String::from_utf8_lossy(&raw);
    s.chars().next().unwrap_or('?')
}

/// Convenience: the single submodule's status via the system under test.
fn only_status(workdir: &Path) -> SubmoduleStatus {
    let subs = list_submodules(workdir).expect("list_submodules");
    assert_eq!(subs.len(), 1, "fixture has exactly one submodule");
    assert_eq!(subs[0].name, "sub", "submodule name defaults to its path");
    subs[0].status
}

/// §8.2 #1 + #3 + #4: uninitialized → init+update reaches the pinned commit.
#[test]
fn init_then_update_reaches_pinned_commit() {
    require_git!();
    let fx = setup();
    let work = &fx.work;
    let sub = work.join("sub");

    // Uninitialized before we touch it. Parity: `-` sigil.
    assert_eq!(only_status(work), SubmoduleStatus::Uninitialized);
    assert_eq!(status_sigil(work), '-', "git sees uninitialized as `-`");
    // wt_oid is null while uninitialized.
    assert!(
        list_submodules(work).unwrap()[0].wt_oid.is_none(),
        "uninitialized submodule has no checked-out commit"
    );

    // System under test: init then update (fetch over the local transport).
    init_submodule(work, "sub").expect("init_submodule");
    update_submodule(work, "sub").expect("update_submodule");

    // Now up-to-date; wt_oid == index_oid == pinned B.
    let subs = list_submodules(work).expect("list after update");
    assert_eq!(subs[0].status, SubmoduleStatus::UpToDate);
    assert_eq!(status_sigil(work), ' ', "git sees up-to-date as ` `");
    assert_eq!(subs[0].wt_oid.as_deref(), subs[0].index_oid.as_deref());
    assert_eq!(subs[0].index_oid.as_deref(), Some(fx.oid_b.as_str()));

    // Oracle cross-checks: the submodule HEAD is the recorded oid, and a
    // subsequent CLI `submodule update` is a no-op (already at the pin).
    assert_eq!(git(&sub, &["rev-parse", "HEAD"]), fx.oid_b);
    git(
        work,
        &["-c", "protocol.file.allow=always", "submodule", "update"],
    );
    assert_eq!(git(&sub, &["rev-parse", "HEAD"]), fx.oid_b, "update was a no-op");
}

/// §8.2 #1: a checked-out commit different from the pin → outOfSync / `+`.
#[test]
fn checked_out_other_commit_is_out_of_sync() {
    require_git!();
    let fx = setup();
    let work = &fx.work;
    let sub = work.join("sub");

    init_submodule(work, "sub").expect("init");
    update_submodule(work, "sub").expect("update");
    assert_eq!(only_status(work), SubmoduleStatus::UpToDate);

    // Detach the submodule onto A (≠ the pinned B).
    git(&sub, &["checkout", &fx.oid_a]);

    assert_eq!(only_status(work), SubmoduleStatus::OutOfSync);
    assert_eq!(status_sigil(work), '+', "git sees a commit mismatch as `+`");
}

/// §8.2 #2: pinned commit matches but the submodule worktree is dirty →
/// modifiedWorkdir, cross-checked against the submodule's own porcelain status.
#[test]
fn dirty_but_matching_is_modified_workdir() {
    require_git!();
    let fx = setup();
    let work = &fx.work;
    let sub = work.join("sub");

    init_submodule(work, "sub").expect("init");
    update_submodule(work, "sub").expect("update");
    assert_eq!(only_status(work), SubmoduleStatus::UpToDate);

    // Edit a TRACKED file inside the submodule (no commit → pin still matches).
    std::fs::write(sub.join("mod.txt"), "dirty\n").expect("dirty edit");

    assert_eq!(only_status(work), SubmoduleStatus::ModifiedWorkdir);
    // Oracle: the submodule's own status is non-empty, yet the commit still
    // matches the pin (no `+` sigil).
    let porcelain = git(&sub, &["status", "--porcelain"]);
    assert!(!porcelain.is_empty(), "submodule worktree must be dirty");
    assert_ne!(status_sigil(work), '+', "commit still matches the pin");
}

/// §8.2 #5: sync propagates a changed .gitmodules URL into .git/config.
#[test]
fn sync_propagates_changed_url() {
    require_git!();
    let fx = setup();
    let work = &fx.work;

    // Initialize so .git/config carries submodule.sub.url.
    init_submodule(work, "sub").expect("init");
    update_submodule(work, "sub").expect("update");

    // Rewrite the .gitmodules URL to a second (arbitrary) path — sync only
    // copies the config string, so the target need not exist.
    let new_url = url_for(&fx.root.join("sub-origin2.git"));
    git(
        work,
        &[
            "config",
            "-f",
            ".gitmodules",
            "submodule.sub.url",
            &new_url,
        ],
    );

    // System under test.
    sync_submodule(work, "sub").expect("sync_submodule");

    assert_eq!(
        git(work, &["config", "submodule.sub.url"]),
        new_url,
        "sync must copy the .gitmodules URL into .git/config"
    );
}
