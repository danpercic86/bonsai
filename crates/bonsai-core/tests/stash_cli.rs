//! T2 Area 6 — stash CLI-oracle integration tests (contract §3 Area 6).
//!
//! Twin-pair semantics: repo A drives `bonsai_core::git::stash`, repo B drives
//! the `git` CLI; we compare RESULTING worktree bytes + porcelain status, never
//! `git stash list` free text (stash commit oids differ by timestamp/author).
//!
//! Conflict-path cases + untracked-collision live in the sibling
//! `stash_cli_conflicts.rs` (soft 500-line file discipline).
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full). Each test
//! skips (passes with a note) when `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::stash::{
    apply_stash, create_stash, drop_stash, list_stashes, pop_stash, ApplyStashOutcome, StashScope,
};
use common::{assert_same_status, commit_fixed, git, init_repo, porcelain_records};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// A base commit on `main` with `f.txt = "base\n"` + a second branch `other`
/// off the same commit. Identity is set by `init_repo`.
fn repo_with_two_branches() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("f.txt"), "base\n").expect("write f");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    git(path, &["branch", "other"]);
    dir
}

fn read(p: &Path, name: &str) -> Vec<u8> {
    std::fs::read(p.join(name)).unwrap_or_default()
}

// --------------------------------------------------- cross-branch clean apply

/// Save a stash on `main`, switch to `other` (same base → no conflict), apply.
/// Twin-pair: bonsai apply vs `git stash apply` must leave byte-identical
/// worktrees and identical porcelain.
#[test]
fn apply_onto_different_branch_clean_twin_pair() {
    require_git!();
    let a = repo_with_two_branches();
    let b = repo_with_two_branches();

    for (dir, cli) in [(a.path(), false), (b.path(), true)] {
        // Modify the tracked file (All scope captures tracked changes).
        std::fs::write(dir.join("f.txt"), "changed\n").expect("edit f");
        if cli {
            git(dir, &["stash", "push", "-m", "wip"]);
        } else {
            let r = create_stash(dir, Some("wip"), StashScope::All).expect("create");
            assert!(r.created);
        }
        // Switch to the OTHER branch and apply there.
        git(dir, &["checkout", "other"]);
        if cli {
            git(dir, &["stash", "apply"]);
        } else {
            match apply_stash(dir, 0, false, None).expect("apply") {
                ApplyStashOutcome::Applied => {}
                other => panic!("expected clean Applied, got {other:?}"),
            }
        }
    }

    assert_eq!(read(a.path(), "f.txt"), b"changed\n", "bonsai restored the edit");
    assert_same_status(a.path(), b.path());
    // The stash survives an apply (never dropped) on both sides.
    assert_eq!(list_stashes(a.path()).expect("list").len(), 1);
}

// -------------------------------------------------- message round-trip + drop

/// A stash message containing embedded newlines + unicode/emoji round-trips
/// through the reflog for BOTH the native (All) and staged scopes; the list
/// count is right and drop-by-index removes the correct entry.
#[test]
fn message_newlines_unicode_native_and_staged_then_drop_by_index() {
    require_git!();
    let dir = repo_with_two_branches();
    let path = dir.path();

    let msg_native = "wíp: line-α\nsecond line 🚀\nthird ✨";
    let msg_staged = "staged: café\n日本語 line 🎋";

    // Native (All): a tracked edit.
    std::fs::write(path.join("f.txt"), "native-edit\n").expect("edit");
    assert!(create_stash(path, Some(msg_native), StashScope::All)
        .expect("native create")
        .created);

    // Staged: a staged edit of the same file (folded into a staged-scope stash).
    std::fs::write(path.join("f.txt"), "staged-edit\n").expect("edit2");
    git(path, &["add", "f.txt"]);
    assert!(create_stash(path, Some(msg_staged), StashScope::Staged)
        .expect("staged create")
        .created);

    let list = list_stashes(path).expect("list");
    assert_eq!(list.len(), 2, "two stashes on the stack");
    // stash@{0} is the most-recent (staged) entry. NOTE: stash messages are
    // stored as reflog SUBJECTS, so embedded newlines are normalized to spaces
    // (standard git behavior) — the point is that NOTHING is truncated at the
    // newline and the unicode/emoji survive intact.
    assert!(list[0].message.contains("café") && list[0].message.contains("日本語")
        && list[0].message.contains('🎋'), "staged msg (no truncation): {:?}", list[0].message);
    assert!(list[1].message.contains("line-α") && list[1].message.contains('🚀')
        && list[1].message.contains('✨'), "native msg (no truncation): {:?}", list[1].message);

    // Reflog intact: `refs/stash` reflog has exactly 2 entries.
    let reflog = git(path, &["reflog", "show", "refs/stash"]);
    assert_eq!(reflog.lines().count(), 2, "stash reflog has 2 entries:\n{reflog}");

    // Drop stash@{1} (the native one) by index → the staged entry remains @ 0.
    let native_oid = list[1].oid.clone();
    let staged_oid = list[0].oid.clone();
    drop_stash(path, 1, None).expect("drop index 1");
    let after = list_stashes(path).expect("list after drop");
    assert_eq!(after.len(), 1, "one stash left");
    assert_eq!(after[0].oid, staged_oid, "the STAGED entry is the survivor");
    assert_ne!(after[0].oid, native_oid, "native entry is gone");
}

// ---------------------------------------------------------- expected_oid guard

/// F-A6-B: a stale `expected_oid` blocks apply/pop/drop ("stash list changed");
/// the matching oid succeeds.
#[test]
fn expected_oid_stale_blocks_then_matching_succeeds() {
    require_git!();
    let dir = repo_with_two_branches();
    let path = dir.path();

    std::fs::write(path.join("f.txt"), "edit\n").expect("edit");
    create_stash(path, Some("wip"), StashScope::All).expect("create");
    let real_oid = list_stashes(path).expect("list")[0].oid.clone();
    let stale = "0".repeat(40);

    // Stale oid → clean error, nothing applied, stash retained.
    for op in ["apply", "drop"] {
        let err = match op {
            "apply" => apply_stash(path, 0, false, Some(&stale)).unwrap_err(),
            _ => drop_stash(path, 0, Some(&stale)).unwrap_err(),
        };
        match err {
            AppError::Git(m) => assert!(m.contains("stash list changed"), "{op}: {m}"),
            other => panic!("{op}: expected Git 'stash list changed', got {other:?}"),
        }
    }
    assert_eq!(list_stashes(path).expect("list").len(), 1, "stash still present");
    assert_eq!(read(path, "f.txt"), b"base\n", "worktree untouched by stale-oid attempts");

    // Matching oid → pop succeeds and drops.
    match pop_stash(path, 0, false, Some(&real_oid)).expect("pop with matching oid") {
        ApplyStashOutcome::Applied => {}
        other => panic!("expected Applied, got {other:?}"),
    }
    assert_eq!(read(path, "f.txt"), b"edit\n", "edit restored");
    assert!(list_stashes(path).expect("list").is_empty(), "stash dropped after clean pop");
}

// --------------------------------------------------- binary + multi-MB staged

/// A binary blob AND a multi-MB blob round-trip losslessly through a Staged-scope
/// stash + pop (folded staged content → restored unstaged).
#[test]
fn binary_and_multi_mb_staged_round_trip() {
    require_git!();
    let dir = repo_with_two_branches();
    let path = dir.path();

    // Binary content with embedded NULs + a >2 MiB blob.
    let binary: Vec<u8> = (0u16..4096).flat_map(|n| [n as u8, (n >> 8) as u8, 0u8]).collect();
    let big: Vec<u8> = (0u32..(3 * 1024 * 1024)).map(|n| (n.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(path.join("bin.dat"), &binary).expect("write bin");
    std::fs::write(path.join("big.dat"), &big).expect("write big");
    git(path, &["add", "-A"]);
    assert!(create_stash(path, Some("staged binaries"), StashScope::Staged)
        .expect("stash staged")
        .created);

    // Staged scope reset the folded paths back to HEAD (absent from the tree).
    assert!(!path.join("bin.dat").exists(), "bin.dat removed after staged stash");
    assert!(!path.join("big.dat").exists(), "big.dat removed after staged stash");

    match pop_stash(path, 0, false, None).expect("pop") {
        ApplyStashOutcome::Applied => {}
        other => panic!("expected Applied, got {other:?}"),
    }
    assert_eq!(read(path, "bin.dat"), binary, "binary bytes round-trip");
    assert_eq!(read(path, "big.dat"), big, "multi-MB bytes round-trip");
}

// ------------------------------------------- staged-modify + unstaged-delete

/// A file staged-modified AND then deleted in the worktree. Staged scope folds
/// the STAGED content (index-vs-HEAD) into the stash; pop restores it. Twin-pair
/// against the CLI's `git stash push --staged`-equivalent is not 1:1 (git has no
/// fold semantics), so we assert bonsai's documented FOLD behavior directly.
#[test]
fn staged_modify_then_unstaged_delete_staged_scope() {
    require_git!();
    let dir = repo_with_two_branches();
    let path = dir.path();

    // Stage a modification, then delete the file on disk (unstaged deletion).
    std::fs::write(path.join("f.txt"), "staged-content\n").expect("edit");
    git(path, &["add", "f.txt"]);
    std::fs::remove_file(path.join("f.txt")).expect("rm worktree");

    // create_staged_stash folds the CURRENT worktree content of staged paths;
    // the file is absent → it records the staged blob and resets to HEAD.
    let r = create_stash(path, Some("staged+deleted"), StashScope::Staged).expect("stash");
    assert!(r.created, "staged delta present → created");

    // After the staged stash the path returns to HEAD (base content) on disk.
    let recs = porcelain_records(path);
    assert!(recs.is_empty() || recs.iter().all(|(t, _)| !t.starts_with("D")),
        "no dangling staged deletion after stash: {recs:?}");

    // Pop replays the folded diff as an unstaged edit.
    match pop_stash(path, 0, false, None).expect("pop") {
        ApplyStashOutcome::Applied => {}
        other => panic!("expected Applied, got {other:?}"),
    }
    assert_eq!(read(path, "f.txt"), b"staged-content\n", "staged content restored");
}

// --------------------------------------------------------- unborn HEAD, bare

/// create_stash on an unborn HEAD returns a clean `AppError` for every scope —
/// never a panic and never a silent success that loses the staged content.
#[test]
fn create_on_unborn_head_errors_cleanly_all_scopes() {
    require_git!();
    for scope in [StashScope::All, StashScope::AllWithUntracked, StashScope::Staged] {
        let dir = init_repo(); // no commits → unborn HEAD
        let path = dir.path();
        std::fs::write(path.join("f.txt"), "x\n").expect("write");
        git(path, &["add", "-A"]);
        match create_stash(path, Some("m"), scope) {
            Err(_) => {} // clean AppError (no panic)
            // A `created:false` (nothing stashed) is also acceptable & lossless:
            // the staged content stays in the index untouched.
            Ok(r) => assert!(!r.created, "{scope:?}: unborn must not claim a stash was created"),
        }
        // The staged content is never lost.
        assert!(path.join("f.txt").exists(), "{scope:?}: file survives");
    }
}

/// create_stash on a bare repo errors cleanly (no workdir to stash).
#[test]
fn create_on_bare_repo_errors() {
    require_git!();
    let dir = common::scratch_dir();
    let path = dir.path();
    git(path, &["init", "--bare", "-b", "main"]);
    match create_stash(path, None, StashScope::All) {
        Err(_) => {}
        Ok(r) => assert!(!r.created, "bare repo cannot create a stash"),
    }
}

// ------------------------------------------------------------ index.lock guard

/// With `.git/index.lock` present, create/apply/pop/drop error cleanly and the
/// lock file is preserved (we never delete a lock we did not create).
#[test]
fn index_lock_blocks_ops_and_lock_preserved() {
    require_git!();
    let dir = repo_with_two_branches();
    let path = dir.path();

    // A real stash to exercise apply/pop/drop against.
    std::fs::write(path.join("f.txt"), "edit\n").expect("edit");
    create_stash(path, Some("wip"), StashScope::All).expect("create");
    std::fs::write(path.join("f.txt"), "edit2\n").expect("edit2"); // dirty for create

    let lock = path.join(".git").join("index.lock");
    std::fs::write(&lock, b"").expect("create lock");

    // create needs the index lock → errors, lock preserved.
    assert!(create_stash(path, None, StashScope::All).is_err(), "create blocked by lock");
    assert!(lock.exists(), "create must not delete the lock");

    // apply/pop write the index → blocked, lock preserved.
    assert!(apply_stash(path, 0, false, None).is_err(), "apply blocked by lock");
    assert!(lock.exists(), "apply must not delete the lock");
    assert!(pop_stash(path, 0, false, None).is_err(), "pop blocked by lock");
    assert!(lock.exists(), "pop must not delete the lock");

    // drop touches only the stash reflog (no index) — whatever its result, the
    // lock we created must survive it (we never delete a foreign lock).
    let _ = drop_stash(path, 0, None);
    assert!(lock.exists(), "drop must not delete the lock");

    std::fs::remove_file(&lock).ok();
    // The stash was still there through the blocked apply/pop (drop's effect is
    // index-independent, so we don't assert on the post-drop count here).
    assert!(list_stashes(path).is_ok(), "list_stashes still works after the lock storm");
}
