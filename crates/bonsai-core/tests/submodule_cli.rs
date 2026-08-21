//! P60d CLI-oracle: submodule add / deinit / remove parity with the `git` CLI.
//!
//! Add is done via git2 (`Repository::submodule` + clone + `add_finalize`);
//! deinit/remove shell out to `git submodule deinit` / `git rm`. The oracle
//! builds a hermetic superproject with a LOCAL (`file://`) submodule (no creds,
//! no network) and asserts our ops leave the same on-disk state the real `git`
//! sequence would.
//!
//! HARD RULE: all scratch repos live on D: via `common::scratch_dir()` (through
//! `init_repo`). Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::process::Command;

use bonsai_core::git::search::SpawnGitRunner;
use bonsai_core::git::submodule::{add_submodule, deinit_submodule, remove_submodule};
use common::{file_url, git, git_ok, git_raw, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Write `content` to `dir/name`, then stage + commit it (deterministic date).
fn commit_file(dir: &Path, name: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(name), content).expect("write file");
    git(dir, &["add", name]);
    common::commit_fixed(dir, msg);
}

/// `git config --get <key>` → Some(value) on success, None on non-zero exit.
fn config_get(dir: &Path, key: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["config", "--get", key])
        .current_dir(dir)
        .output()
        .expect("run git config");
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// The staged gitlink oid at `path`, or None when there is no 160000 entry.
/// `git ls-files --stage -- <path>` line format: "<mode> <oid> <stage>\t<path>".
fn staged_gitlink_opt(dir: &Path, path: &str) -> Option<String> {
    let raw = git_raw(dir, &["ls-files", "--stage", "--", path], &[]);
    let text = String::from_utf8_lossy(&raw);
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let mode = it.next().unwrap_or("");
        let oid = it.next().unwrap_or("");
        if mode == "160000" {
            return Some(oid.to_string());
        }
    }
    None
}

fn staged_gitlink(dir: &Path, path: &str) -> String {
    staged_gitlink_opt(dir, path).unwrap_or_else(|| panic!("no staged gitlink at {path}"))
}

/// True when `dir` does not exist or contains no entries.
fn dir_empty(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut rd) => rd.next().is_none(),
        Err(_) => true,
    }
}

/// Full round-trip: `add_submodule` (git2) matches `git submodule add`, then
/// `deinit_submodule` / `remove_submodule` (shell-out) match the real teardown.
#[test]
fn oracle_add_deinit_remove_roundtrip() {
    require_git!();

    // 1. Upstream sub-repo with one commit, referenced by a LOCAL file:// URL.
    let sub = init_repo();
    commit_file(sub.path(), "lib.txt", "sub v1\n", "sub: initial");
    let url = file_url(sub.path());
    let sub_head = git(sub.path(), &["rev-parse", "HEAD"]);

    // 2. Two superprojects: one driven by add_submodule, one by real git.
    let ours = init_repo();
    commit_file(ours.path(), "top.txt", "super\n", "super: initial");
    let cli = init_repo();
    commit_file(cli.path(), "top.txt", "super\n", "super: initial");

    let path = "vendor/sub";
    let url_key = format!("submodule.{path}.url");

    // --- add: git2 vs `git -c protocol.file.allow=always submodule add` ------
    let info = add_submodule(ours.path(), &url, path).expect("add_submodule");
    assert_eq!(info.path, path, "wire path");
    assert_eq!(info.name, path, "name defaults to the path");
    assert_eq!(info.url.as_deref(), Some(url.as_str()), "url recorded");

    // `protocol.file.allow=always` unblocks the file:// transport that modern
    // git refuses for submodules by default (CVE-2022-39253); libgit2 (our add)
    // is not subject to that CLI guard.
    let cli_added = git_ok(
        cli.path(),
        &["-c", "protocol.file.allow=always", "submodule", "add", &url, path],
    );
    assert!(cli_added, "real `git submodule add` should succeed");

    // .gitmodules + .git/config parity: both register submodule.<path>.url.
    assert!(ours.path().join(".gitmodules").exists(), "our .gitmodules written");
    assert_eq!(config_get(ours.path(), &url_key).as_deref(), Some(url.as_str()));
    // Staged gitlink parity: both index a 160000 entry at <path> pointing at the
    // SAME upstream HEAD (both cloned the same sub-repo).
    let ours_link = staged_gitlink(ours.path(), path);
    assert_eq!(ours_link, staged_gitlink(cli.path(), path), "gitlink oid == git");
    assert_eq!(ours_link, sub_head, "gitlink points at the sub-repo HEAD");

    // --- deinit: config cleared, worktree emptied, .gitmodules RETAINED ------
    deinit_submodule(ours.path(), &SpawnGitRunner, path, true).expect("deinit_submodule");
    assert!(
        config_get(ours.path(), &url_key).is_none(),
        "submodule config entry cleared by deinit",
    );
    assert!(
        ours.path().join(".gitmodules").exists(),
        ".gitmodules RETAINED after deinit (re-init-able)",
    );
    assert!(
        dir_empty(&ours.path().join(path)),
        "submodule worktree emptied by deinit",
    );
    // Deinit does NOT touch the index — the gitlink is still staged.
    assert!(staged_gitlink_opt(ours.path(), path).is_some(), "gitlink kept by deinit");

    // --- remove: gitlink + .gitmodules entry gone, worktree deleted ---------
    remove_submodule(ours.path(), &SpawnGitRunner, path, true).expect("remove_submodule");
    assert!(
        staged_gitlink_opt(ours.path(), path).is_none(),
        "gitlink dropped from the index by remove",
    );
    let gm = std::fs::read_to_string(ours.path().join(".gitmodules")).unwrap_or_default();
    assert!(
        !gm.contains(&format!("[submodule \"{path}\"]")),
        ".gitmodules entry dropped by remove; got: {gm:?}",
    );
    assert!(
        !ours.path().join(path).exists(),
        "submodule worktree directory deleted by remove",
    );
}
