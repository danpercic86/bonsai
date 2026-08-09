//! T2 Area 7 — autostash integration tests (contract §3 Area 7).
//!
//! `git/autostash.rs` already carries inline F-A7-6 identity tests (foreign
//! stash between save and pop, for both pop and rollback). THIS file adds the
//! `is_dirty` truth table, the rollback/pop CONFLICT-retention paths, and the
//! merge-path autostash (only cherry-pick/revert are exercised elsewhere).
//!
//! Scratch repos on D: via `init_repo`. Skips (passes with a note) w/o `git`.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::autostash::{is_dirty, pop_after_success, rollback_and_map, stash_save, PopResult};
use bonsai_core::git::merge::{merge_branch, MergeOutcome};
use bonsai_core::git::stash::list_stashes;
use common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn sig() -> git2::Signature<'static> {
    git2::Signature::now("Test User", "test@example.com").expect("sig")
}

fn open(p: &Path) -> git2::Repository {
    git2::Repository::open(p).expect("open git2 repo")
}

fn read(p: &Path, name: &str) -> String {
    std::fs::read_to_string(p.join(name)).unwrap_or_default()
}

/// base commit `f.txt="base\n"`.
fn repo_with_base() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("f.txt"), "base\n").expect("write");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    dir
}

// ------------------------------------------------------------ is_dirty table

/// `is_dirty` truth table: clean=false; untracked=false; ignored=false;
/// unstaged tracked edit=true; staged tracked edit=true.
#[test]
fn is_dirty_truth_table() {
    require_git!();
    let dir = repo_with_base();
    let path = dir.path();
    let repo = open(path);

    // Clean.
    assert!(!is_dirty(&repo).expect("clean"), "clean tree → not dirty");

    // Untracked only → excluded.
    std::fs::write(path.join("untracked.txt"), "u\n").expect("write untracked");
    assert!(!is_dirty(&repo).expect("untracked"), "untracked excluded");
    std::fs::remove_file(path.join("untracked.txt")).expect("rm untracked");

    // Ignored only → excluded.
    std::fs::write(path.join(".gitignore"), "ignored.txt\n").expect("gitignore");
    git(path, &["add", ".gitignore"]);
    commit_fixed(path, "add gitignore");
    std::fs::write(path.join("ignored.txt"), "x\n").expect("write ignored");
    let repo = open(path);
    assert!(!is_dirty(&repo).expect("ignored"), "ignored file excluded");

    // Unstaged tracked edit → dirty.
    std::fs::write(path.join("f.txt"), "unstaged\n").expect("edit f");
    assert!(is_dirty(&repo).expect("unstaged"), "unstaged tracked edit → dirty");

    // Staged tracked edit → dirty.
    git(path, &["add", "f.txt"]);
    assert!(is_dirty(&repo).expect("staged"), "staged tracked edit → dirty");
}

/// `is_dirty` on an unborn HEAD with a STAGED add is dirty (a staged add is an
/// index change, not an untracked file).
#[test]
fn is_dirty_unborn_head_staged_add() {
    require_git!();
    let dir = init_repo(); // no commits → unborn HEAD
    let path = dir.path();
    std::fs::write(path.join("f.txt"), "new\n").expect("write");
    git(path, &["add", "f.txt"]);
    let repo = open(path);
    assert!(is_dirty(&repo).expect("unborn staged"), "unborn + staged add → dirty");
}

// ---------------------------------------------- rollback_and_map conflicted

/// `rollback_and_map` when the auto-restore CONFLICTS: the stash is RETAINED and
/// the returned error names where the changes are safe (never a silent drop).
#[test]
fn rollback_and_map_conflicted_restore_retains_stash() {
    require_git!();
    let dir = repo_with_base();
    let path = dir.path();
    let mut repo = open(path);

    // Autostash our edit; tree returns to base.
    std::fs::write(path.join("f.txt"), "ours\n").expect("edit");
    let oid = stash_save(&mut repo, &sig(), "bonsai: autostash").expect("save");
    assert_eq!(read(path, "f.txt"), "base\n", "clean after save");

    // Make the worktree conflict with the stash's diff before rollback.
    std::fs::write(path.join("f.txt"), "conflicting-worktree\n").expect("edit2");

    let out = rollback_and_map(&mut repo, Some(oid), AppError::Git("boom".to_string()));
    match out {
        AppError::Git(m) => {
            assert!(m.contains("boom"), "original error preserved: {m}");
            assert!(m.contains("stash@{"), "message points at the safe stash: {m}");
        }
        other => panic!("expected Git error, got {other:?}"),
    }
    // The stash is RETAINED (conflicted restore never drops).
    assert_eq!(list_stashes(path).expect("list").len(), 1, "stash retained on conflicted rollback");
}

// ---------------------------------------------- pop_after_success conflict

/// `pop_after_success` content-conflict: worktree diverged so the re-apply
/// conflicts → `PopResult::Conflicted(paths)` with the path listed, stash
/// RETAINED.
#[test]
fn pop_after_success_content_conflict_retains_and_lists() {
    require_git!();
    let dir = repo_with_base();
    let path = dir.path();

    std::fs::write(path.join("f.txt"), "ours\n").expect("edit");
    let oid = {
        let mut repo = open(path);
        stash_save(&mut repo, &sig(), "bonsai: autostash").expect("save")
    };
    // COMMIT a diverging change (clean worktree) so the re-apply is a 3-way
    // CONTENT conflict — libgit2 returns Ok with markers, not a checkout block.
    std::fs::write(path.join("f.txt"), "diverged\n").expect("edit2");
    git(path, &["add", "f.txt"]);
    commit_fixed(path, "diverging commit");

    // Reopen so the git2 handle sees the CLI commit's index/HEAD.
    let mut repo = open(path);
    match pop_after_success(&mut repo, path, oid).expect("pop_after_success Ok") {
        PopResult::Conflicted(paths) => {
            assert!(paths.iter().any(|p| p == "f.txt"), "conflicted path listed: {paths:?}");
        }
        PopResult::Restored => panic!("expected a content conflict, got Restored"),
    }
    assert_eq!(list_stashes(path).expect("list").len(), 1, "stash retained on conflicted pop");
}

// ------------------------------------------------------- merge-path autostash

/// A fast-forward merge with a DIRTY tracked edit triggers the autostash:
/// `FastForwarded{stashed:true}`, the FF lands, and the unstaged edit is
/// restored afterward (no stash left on the stack).
#[test]
fn merge_ff_with_dirty_tree_autostashes_and_restores() {
    require_git!();
    let dir = repo_with_base();
    let path = dir.path();

    // topic advances main by one commit on a DIFFERENT file (FF-able).
    git(path, &["checkout", "-b", "topic"]);
    std::fs::write(path.join("g.txt"), "topic\n").expect("write g");
    git(path, &["add", "-A"]);
    commit_fixed(path, "topic commit");
    git(path, &["checkout", "main"]);

    // Dirty unstaged edit on a file topic does NOT touch → autostash-safe FF.
    std::fs::write(path.join("f.txt"), "dirty-edit\n").expect("dirty");

    match merge_branch(path, "topic", false).expect("merge") {
        MergeOutcome::FastForwarded { stashed, .. } => {
            assert!(stashed, "a dirty tree must be autostashed for the FF");
        }
        other => panic!("expected FastForwarded, got {other:?}"),
    }

    // FF brought in topic's file AND the dirty edit was restored.
    assert_eq!(read(path, "g.txt"), "topic\n", "FF pulled in topic's commit");
    assert_eq!(read(path, "f.txt"), "dirty-edit\n", "the autostashed edit was restored");
    assert!(list_stashes(path).expect("list").is_empty(), "autostash dropped after clean restore");
}
