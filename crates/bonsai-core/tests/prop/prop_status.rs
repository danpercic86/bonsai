//! T5 property suite (contract §2.4): `read_status` must agree with the
//! `git status --porcelain` oracle after any random mutation sequence. Gated on
//! the git CLI; 32 cases total (each shells out to git).
//!
//! WALL-CLOCK NOTE: the whole 32-case run used to live in ONE test fn, and
//! nextest parallelizes across test *functions*, not across cases inside a
//! `proptest!` block — so that one fn was the suite's critical path (~21s
//! isolated / ~23.5s under gate contention). The op-count axis (`1..=12`) is
//! therefore partitioned into 4 DISJOINT bands that TILE `1..=12` exactly, one
//! test fn each, via `ops_strat_sized`. Every band runs the identical body and
//! the identical porcelain-oracle assertion, so no coverage is dropped: the
//! union of the four bands is exactly the old input space and the case total is
//! unchanged at 32.
//!
//! Bands are EQUAL width here (3 each) with cases allocated proportional to
//! width (8 each), unlike `prop_graph_layout`'s sqrt schedule. Reason: the
//! per-case cost of this suite is dominated by FIXED setup — `init_repo` (5
//! `git` process spawns) + `add -A` + `commit` + the `git status --porcelain`
//! oracle ≈ 8 process spawns — while the ops themselves are cheap in-process
//! git2/fs calls. Cost is therefore ~flat in op count, so equal cases per band
//! is what equalizes per-band wall time, and proportional-to-width allocation
//! also keeps the marginal distribution over op count exactly uniform, as the
//! single fn had it.
//!
//! REGRESSION SEEDS: `prop_status.proptest-regressions` was deleted. proptest
//! keys that file per SOURCE FILE, so every band replayed all 3 `cc` seeds
//! (3 replays -> 12, ~25% of each band's wall time), and a `cc` seed only stores
//! an RNG seed — it regenerates through the CURRENT strategy, which changed when
//! op kind 5 (fs-rename) came back, so the seeds no longer reproduced the inputs
//! in their own `# shrinks to` comments. Those recorded inputs are now pinned
//! verbatim as explicit cases in `prop_status_pinned.rs`, which makes the
//! regression coverage exact instead of notional.

use std::path::Path;

use bonsai_core::git::status::read_status;
use proptest::prelude::*;

use crate::prop_common::common;
use crate::prop_common::{flatten_snapshot, porcelain_tuples};

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

/// Multi-line content keyed by PATH (and a modify seed).
fn content_for(path: &str, seed: u32) -> String {
    (0..6).map(|i| format!("{path}:line {seed}-{i}\n")).collect()
}

/// A raw op: (kind, path selector, content seed, new-name). Kinds:
/// 0 create, 1 modify, 2 delete, 3 stage, 4 unstage, 5 fs-rename. The fs-rename
/// (move a tracked file to an untracked destination with IDENTICAL bytes) is the
/// exact F-T5-3 scenario — now that index-to-workdir rename detection is off,
/// bonsai reports delete+untracked like git, so this mutation is back in the
/// broad property (previously excluded while F-T5-3 was open).
type RawOp = (u8, usize, u32, String);

/// The op sequence, restricted to `lo..=hi` ops. Identical in every other
/// respect (same kind/selector/seed/path distributions); exists so the suite can
/// partition the op-count axis into disjoint bands that run as separate test fns
/// (nextest parallelizes across fns, not across cases in one fn). The original
/// strategy was `ops_strat_sized(1, 12)`, so a set of bands whose ranges tile
/// `1..=12` covers exactly the same input space.
fn ops_strat_sized(lo: usize, hi: usize) -> impl Strategy<Value = Vec<RawOp>> {
    prop::collection::vec((0u8..=5, any::<usize>(), any::<u32>(), path_strat()), lo..=hi)
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
        4 => {
            // Unstage a path (reset index entry to HEAD).
            if let (Some(p), Ok(head)) = (pick(known), repo.head()) {
                if let Ok(obj) = head.peel(git2::ObjectType::Commit) {
                    let _ = repo.reset_default(Some(&obj), [Path::new(&p)].iter());
                }
            }
        }
        _ => {
            // fs-rename: move a known file to a fresh path, preserving bytes.
            if let Some(p) = pick(known) {
                let dst = newname.clone();
                if !known.contains(&dst) && dst != p {
                    let full_dst = root.join(&dst);
                    if let Some(parent) = full_dst.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::rename(root.join(&p), &full_dst).is_ok() {
                        known.retain(|x| x != &p);
                        known.push(dst);
                    }
                }
            }
        }
    }
}

/// Build a fixture repo from `initial`, apply `ops`, and return the
/// `(read_status, git-porcelain-oracle)` pair. This is the original
/// `status_matches_porcelain` body verbatim, lifted out of the `band!` macro so
/// the randomized bands AND the pinned regression cases in `prop_status_pinned`
/// exercise byte-identical setup, mutation and observation code.
fn run_case(
    initial: &[(String, u32)],
    ops: &[RawOp],
) -> (
    std::collections::BTreeSet<crate::prop_common::StatusTuple>,
    std::collections::BTreeSet<crate::prop_common::StatusTuple>,
) {
    let dir = common::init_repo();
    let root = dir.path();

    // Commit an initial tree of distinct multi-line files. Best-effort:
    // a generated path can clash with another as a dir/file (e.g. "z" and
    // "z/a") — a state git itself cannot hold — so a failing path is simply
    // skipped rather than panicking the harness.
    let mut known: Vec<String> = Vec::new();
    for (p, seed) in initial {
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
    for op in ops {
        apply(&repo, root, &mut known, op);
    }

    let snapshot = read_status(root).expect("read_status");
    let read = flatten_snapshot(&snapshot);
    let oracle = porcelain_tuples(root);
    drop(dir);
    (read, oracle)
}

/// One band of the op-count partition: `$lo..=$hi` ops, `$cases` cases. The
/// body is the original `status_matches_porcelain` body, verbatim: build a repo
/// with a committed initial tree, apply the random op sequence, then assert
/// `read_status` equals the `git status --porcelain` oracle exactly.
macro_rules! band {
    ($name:ident, $lo:expr, $hi:expr, $cases:expr) => {
        proptest! {
            #![proptest_config(ProptestConfig { cases: $cases, ..ProptestConfig::default() })]

            #[test]
            fn $name(
                initial in prop::collection::vec((path_strat(), any::<u32>()), 1..=6),
                ops in ops_strat_sized($lo, $hi),
            ) {
                require_git!();
                let (read, oracle) = run_case(&initial, &ops);
                prop_assert_eq!(
                    read,
                    oracle,
                    "read_status disagrees with git porcelain oracle"
                );
            }
        }
    };
}

// 4 bands tiling 1..=12 (widths 3/3/3/3 = 12) with cases proportional to width
// (8/8/8/8 = 32) — the same 32 cases the single fn ran.
band!(status_matches_porcelain_b1_01_03, 1, 3, 8);
band!(status_matches_porcelain_b2_04_06, 4, 6, 8);
band!(status_matches_porcelain_b3_07_09, 7, 9, 8);
band!(status_matches_porcelain_b4_10_12, 10, 12, 8);

#[path = "prop_status_pinned.rs"]
mod prop_status_pinned;

// ---- F-T5-3: worktree rename to an UNTRACKED target (FIXED) -----------------
//
// When a tracked file is deleted from the worktree and an untracked file with
// identical bytes appears, `read_status` must match `git status --porcelain`:
// the two events are reported SEPARATELY as `D <orig>` (unstaged delete) +
// `?? <new>` (untracked). Previously git2's `renames_index_to_workdir` collapsed
// them into one UNSTAGED RENAME, diverging from porcelain (FINDINGS F-T5-3). The
// fix disables index-to-workdir rename detection (git does not rename-detect an
// untracked destination), so bonsai now agrees with git. Pinned here.
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
    // FIXED: read_status now matches git porcelain exactly (delete + untracked).
    assert_eq!(read, porcelain, "F-T5-3 fixed: read_status must match git porcelain");
    assert!(
        read.iter()
            .any(|(list, path, orig, st)| list == "unstaged"
                && path == "a"
                && orig.is_none()
                && st == "deleted"),
        "expected unstaged delete of `a`; got {read:?}"
    );
    assert!(
        read.iter()
            .any(|(list, path, _orig, st)| list == "untracked" && path == "b" && st == "untracked"),
        "expected untracked `b`; got {read:?}"
    );
    assert!(
        !read.iter().any(|(_l, _p, _o, st)| st == "renamed"),
        "expected NO rename row; got {read:?}"
    );
}
