//! T2 Area 6 — stash CONFLICT + adversarial cases (split from `stash_cli.rs`).
//!
//! Twin-pair semantics on the clean/conflict apply; direct-behavior assertions
//! on the untracked-collision (never-clobber) and the "second op while the index
//! still carries conflict entries" corner. Scratch repos on D:. Skips w/o `git`.

mod common;

use std::path::Path;

use bonsai_core::git::stash::{
    apply_stash, create_stash, list_stashes, pop_stash, ApplyStashOutcome, StashScope,
};
use common::{assert_same_status, commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn read(p: &Path, name: &str) -> Vec<u8> {
    std::fs::read(p.join(name)).unwrap_or_default()
}

/// base commit (`f.txt="base\n"`) + branch `other` diverged with a conflicting
/// edit committed on it.
fn repo_conflicting_branches() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("f.txt"), "base\n").expect("write");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    git(path, &["checkout", "-b", "other"]);
    std::fs::write(path.join("f.txt"), "other-side\n").expect("edit other");
    git(path, &["add", "-A"]);
    commit_fixed(path, "other change");
    git(path, &["checkout", "main"]);
    dir
}

// ---------------------------------------------- conflicting cross-branch apply

/// Save on `main` (edit f.txt), switch to the diverged `other`, apply → the
/// 3-way merge conflicts. Twin-pair: bonsai must leave the SAME porcelain as
/// `git stash apply`, must return `Conflicts`, and must RETAIN the stash.
#[test]
fn apply_onto_different_branch_conflicting_twin_pair() {
    require_git!();
    let a = repo_conflicting_branches();
    let b = repo_conflicting_branches();

    for (dir, cli) in [(a.path(), false), (b.path(), true)] {
        std::fs::write(dir.join("f.txt"), "main-side\n").expect("edit");
        if cli {
            git(dir, &["stash", "push", "-m", "wip"]);
            git(dir, &["checkout", "other"]);
            // apply is expected to fail (conflict) — do not assert success.
            let _ = common::git_ok(dir, &["stash", "apply"]);
        } else {
            create_stash(dir, Some("wip"), StashScope::All).expect("create");
            git(dir, &["checkout", "other"]);
            match apply_stash(dir, 0, false, None).expect("apply") {
                ApplyStashOutcome::Conflicts { paths } => {
                    assert!(paths.iter().any(|p| p == "f.txt"), "f.txt conflicted: {paths:?}");
                }
                other => panic!("expected Conflicts, got {other:?}"),
            }
        }
    }

    // Same conflicted porcelain on both sides (both show f.txt unmerged).
    assert_same_status(a.path(), b.path());
    // bonsai never drops on conflict.
    assert_eq!(list_stashes(a.path()).expect("list").len(), 1, "stash retained on conflict");
}

// -------------------------------------------------- untracked-file collision

/// apply/pop of a stash holding an UNTRACKED file that collides with an existing
/// untracked file of the same name must NOT clobber the on-disk content — it
/// errors or reports a conflict, and the stash is retained.
#[test]
fn untracked_collision_never_clobbers() {
    require_git!();
    for pop in [false, true] {
        let dir = init_repo();
        let path = dir.path();
        std::fs::write(path.join("f.txt"), "base\n").expect("write");
        git(path, &["add", "-A"]);
        commit_fixed(path, "base");

        // Stash an UNTRACKED file, then recreate it with DIFFERENT content.
        std::fs::write(path.join("u.txt"), "stashed-content\n").expect("write u");
        assert!(create_stash(path, Some("wip"), StashScope::AllWithUntracked)
            .expect("create")
            .created);
        assert!(!path.join("u.txt").exists(), "untracked file moved into stash");
        std::fs::write(path.join("u.txt"), "existing-content\n").expect("recreate u");

        let outcome = if pop {
            pop_stash(path, 0, false, None)
        } else {
            apply_stash(path, 0, false, None)
        };
        // Either a clean AppError or a Conflicts/Reserved outcome — never an
        // `Applied` that silently overwrote the on-disk file.
        match &outcome {
            Err(_) => {}
            Ok(ApplyStashOutcome::Applied) => {
                panic!("{}: collision must not resolve to a clean Applied", if pop { "pop" } else { "apply" })
            }
            Ok(_) => {}
        }
        // The KEY property: the on-disk untracked file is untouched.
        assert_eq!(read(path, "u.txt"), b"existing-content\n",
            "existing untracked content must never be clobbered");
        // The stash is retained (blobs live only there).
        assert_eq!(list_stashes(path).expect("list").len(), 1,
            "stash retained after a blocked collision");
    }
}

// ------------------------------- second op while the index carries conflicts

/// After a conflicted apply leaves conflict entries in the index, applying or
/// popping a SECOND stash must fail with a clean error and corrupt nothing:
/// both stashes stay on the stack (neither dropped) and the list is readable.
#[test]
fn second_op_with_conflicted_index_errors_no_corruption() {
    require_git!();
    for pop in [false, true] {
        let dir = init_repo();
        let path = dir.path();
        std::fs::write(path.join("f.txt"), "base\n").expect("write");
        git(path, &["add", "-A"]);
        commit_fixed(path, "base");

        // Two stashes off the base.
        std::fs::write(path.join("f.txt"), "s1\n").expect("e1");
        create_stash(path, Some("s1"), StashScope::All).expect("stash1");
        std::fs::write(path.join("f.txt"), "s2\n").expect("e2");
        create_stash(path, Some("s2"), StashScope::All).expect("stash2");
        // Commit a diverging change so applying a stash conflicts.
        std::fs::write(path.join("f.txt"), "committed\n").expect("e3");
        git(path, &["add", "-A"]);
        commit_fixed(path, "committed change");

        // Apply stash@{1} (s1) → conflict; index now holds conflict entries.
        match apply_stash(path, 1, false, None).expect("first apply") {
            ApplyStashOutcome::Conflicts { .. } => {}
            other => panic!("expected first apply to conflict, got {other:?}"),
        }
        assert_eq!(list_stashes(path).expect("list").len(), 2, "both stashes still present");

        // A SECOND op with the conflicted index must not corrupt anything.
        let outcome = if pop {
            pop_stash(path, 0, false, None)
        } else {
            apply_stash(path, 0, false, None)
        };
        // Whatever the outcome, no stash may be silently dropped and the stack
        // must remain readable — the load-bearing anti-corruption property.
        match &outcome {
            Ok(ApplyStashOutcome::Applied) if pop => {
                panic!("pop must not drop a stash while the index is conflicted")
            }
            _ => {}
        }
        let after = list_stashes(path).expect("list still readable — no corruption");
        assert_eq!(after.len(), 2, "no stash lost by the second op: {after:?}");
    }
}
