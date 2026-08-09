//! T5 property suite (contract §2.4): `read_status` must agree with the
//! `git status --porcelain` oracle after any random mutation sequence. Gated on
//! the git CLI; 32 cases (each shells out to git).

#[path = "prop_common/mod.rs"]
mod prop_common;

use std::path::Path;

use bonsai_core::git::status::read_status;
use proptest::prelude::*;

use prop_common::common;
use prop_common::{flatten_snapshot, porcelain_tuples};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return Ok(());
        }
    };
}

/// A repo-relative path: 1-2 ASCII lowercase segments (Windows-safe — no
/// reserved names, no trailing dot/space, forward slashes as git reports).
fn path_strat() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z]{1,5}", 1..=2).prop_map(|segs| segs.join("/"))
}

/// Multi-line content keyed by PATH (and a modify seed) so no two distinct
/// paths ever share bytes. This eliminates the git2-vs-CLI worktree-rename-to-
/// untracked divergence (pinned separately as F-T5-3 /
/// `regression_f_t5_3_untracked_worktree_rename`), keeping this broad
/// create/modify/delete/stage/unstage property meaningful and green.
fn content_for(path: &str, seed: u32) -> String {
    (0..6).map(|i| format!("{path}:line {seed}-{i}\n")).collect()
}

/// A raw op: (kind, path selector, content seed, new-name). Kinds:
/// 0 create, 1 modify, 2 delete, 3 stage, 4 unstage. (fs-rename is deliberately
/// excluded — it is the F-T5-3 divergence, covered by staged `git mv` in
/// `status_porcelain.rs`.)
type RawOp = (u8, usize, u32, String);

fn ops_strat() -> impl Strategy<Value = Vec<RawOp>> {
    prop::collection::vec(
        (0u8..=4, any::<usize>(), any::<u32>(), path_strat()),
        1..=12,
    )
}

/// Apply one op best-effort against the live repo, mutating `known` (paths that
/// currently exist on disk). Failures are ignored — the final status compare is
/// the oracle, regardless of how the state was reached.
fn apply(repo: &git2::Repository, root: &Path, known: &mut Vec<String>, op: &RawOp) {
    let (kind, sel, seed, newname) = op;
    let pick = |known: &Vec<String>| -> Option<String> {
        if known.is_empty() {
            None
        } else {
            Some(known[sel % known.len()].clone())
        }
    };
    match kind {
        0 => {
            // Create at a fresh path.
            let p = newname.clone();
            if !known.contains(&p) {
                let full = root.join(&p);
                if let Some(parent) = full.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&full, content_for(&p, *seed)).is_ok() {
                    known.push(p);
                }
            }
        }
        1 => {
            if let Some(p) = pick(known) {
                let _ = std::fs::write(root.join(&p), content_for(&p, seed.wrapping_add(1)));
            }
        }
        2 => {
            if let Some(p) = pick(known) {
                if std::fs::remove_file(root.join(&p)).is_ok() {
                    known.retain(|x| x != &p);
                }
            }
        }
        3 => {
            // Stage a path (add, or record its deletion).
            if let Some(p) = pick(known) {
                if let Ok(mut index) = repo.index() {
                    let rel = Path::new(&p);
                    if root.join(&p).exists() {
                        let _ = index.add_path(rel);
                    } else {
                        let _ = index.remove_path(rel);
                    }
                    let _ = index.write();
                }
            } else if let Ok(mut index) = repo.index() {
                let _ = index.update_all(["*"].iter(), None);
                let _ = index.write();
            }
        }
        _ => {
            // Unstage a path (reset index entry to HEAD).
            if let (Some(p), Ok(head)) = (pick(known), repo.head()) {
                if let Ok(obj) = head.peel(git2::ObjectType::Commit) {
                    let _ = repo.reset_default(Some(&obj), [Path::new(&p)].iter());
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn status_matches_porcelain(
        initial in prop::collection::vec((path_strat(), any::<u32>()), 1..=6),
        ops in ops_strat(),
    ) {
        require_git!();
        let dir = common::init_repo();
        let root = dir.path();

        // Commit an initial tree of distinct multi-line files. Best-effort:
        // a generated path can clash with another as a dir/file (e.g. "z" and
        // "z/a") — a state git itself cannot hold — so a failing path is simply
        // skipped rather than panicking the harness.
        let mut known: Vec<String> = Vec::new();
        for (p, seed) in &initial {
            if known.contains(p) {
                continue;
            }
            let full = root.join(p);
            if let Some(parent) = full.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    continue;
                }
            }
            if std::fs::write(&full, content_for(p, *seed)).is_ok() {
                known.push(p.clone());
            }
        }
        // Guarantee at least one committed file so the base commit is non-empty.
        if known.is_empty() {
            std::fs::write(root.join("seed.txt"), content_for("seed.txt", 0)).expect("seed write");
            known.push("seed.txt".to_string());
        }
        common::git(root, &["add", "-A"]);
        common::commit_fixed(root, "initial");

        let repo = git2::Repository::open(root).expect("open");
        for op in &ops {
            apply(&repo, root, &mut known, op);
        }

        let snapshot = read_status(root).expect("read_status");
        prop_assert_eq!(
            flatten_snapshot(&snapshot),
            porcelain_tuples(root),
            "read_status disagrees with git porcelain oracle"
        );
        drop(dir);
    }
}

// ---- F-T5-3: worktree rename to an UNTRACKED target (pinned divergence) -----
//
// When a tracked file is deleted from the worktree and an untracked file with
// identical bytes appears, `read_status` (git2, `renames_index_to_workdir`)
// reports a single UNSTAGED RENAME (orig -> new), whereas `git status`
// (porcelain v1) reports the two events SEPARATELY: `D <orig>` + `?? <new>`.
// git2 rename-detects an untracked destination; the git CLI does not. This
// VIOLATES the porcelain-equivalence contract (§2.4) and is logged as FINDINGS
// F-T5-3 for the orchestrator. Pinned here; the broad property above excludes
// fs-renames so it exercises every OTHER mutation cleanly.
#[test]
fn regression_f_t5_3_untracked_worktree_rename() {
    if !common::have_git() {
        return;
    }
    let dir = common::init_repo();
    let root = dir.path();
    let body = "line1\nline2\nline3\nline4\nline5\n";
    std::fs::write(root.join("a"), body).unwrap();
    common::git(root, &["add", "-A"]);
    common::commit_fixed(root, "base");
    std::fs::remove_file(root.join("a")).unwrap();
    std::fs::write(root.join("b"), body).unwrap();

    let read = flatten_snapshot(&read_status(root).unwrap());
    let porcelain = porcelain_tuples(root);
    // PINNED: the two disagree (git2 sees a rename; the CLI sees delete+untracked).
    assert_ne!(read, porcelain, "F-T5-3: expected the known divergence to reproduce");
    assert!(
        read.iter().any(|(list, path, orig, st)| list == "unstaged"
            && path == "b"
            && orig.as_deref() == Some("a")
            && st == "renamed"),
        "git2 reports an unstaged rename b<-a; got {read:?}"
    );
}
