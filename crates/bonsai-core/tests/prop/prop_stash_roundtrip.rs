//! T5 property suite (contract §2.5): a random dirty state survives a
//! create_stash → apply_stash round-trip byte-identically in the WORKTREE.
//!
//! Pinned semantics (contract §8.4): `apply_stash` uses libgit2's default
//! options WITHOUT `REINSTATE_INDEX`, so the staged-vs-unstaged split is NOT
//! restored (staged edits return as unstaged). The identity check is therefore
//! relaxed to WORKTREE-bytes identity ("all changes present"), NOT index-entry
//! identity — this is the behavior we pin.
//!
//! WALL-CLOCK NOTE: both properties used to run all their cases in ONE test fn
//! each (48 cases, ~13.2s / ~11.4s), and nextest parallelizes across test
//! *functions*, not across cases inside a `proptest!` block. The base-repo-size
//! axis (`n in 1..=5`, the count of committed files) is therefore partitioned
//! into 5 DISJOINT bands that TILE `1..=5` exactly — one value each, since the
//! axis is small and discrete — emitted by one `band!` invocation per value
//! that carries BOTH properties. The bodies and assertions are unchanged, so
//! the union of the bands is exactly the old input space.
//!
//! Cases are 10 per band per property (50 total, up from 48 — bands are equal
//! width, and 10 divides evenly where 48/5 = 9.6 does not, so the marginal
//! distribution over `n` stays exactly uniform rather than being skewed by
//! rounding). Equal cases per band is also what equalizes per-band wall time:
//! this suite spawns no `git` process, and its per-case cost (git2 repo init +
//! base commit + stash create/apply + worktree snapshot) is dominated by fixed
//! setup, so cost is ~flat in `n` rather than proportional to it.

use std::collections::BTreeMap;
use std::path::Path;

use bonsai_core::git::stash::{apply_stash, create_stash, ApplyStashOutcome, StashScope};
use bonsai_core::git::status::read_status;
use proptest::prelude::*;

use crate::prop_common::common;

/// Snapshot every worktree file (path → bytes), skipping `.git`.
fn worktree_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name();
            if name == ".git" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(bytes) = std::fs::read(&p) {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, bytes);
            }
        }
    }
    out
}

fn content(seed: u32, tag: &str) -> String {
    (0..5).map(|i| format!("{tag} {seed}-{i}\n")).collect()
}

fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(full, body).expect("write");
}

/// Build a base repo with `n` committed files `t0..t{n-1}`.
fn base_repo(n: usize) -> (tempfile::TempDir, git2::Repository) {
    let dir = common::scratch_dir();
    let repo = git2::Repository::init_opts(
        dir.path(),
        git2::RepositoryInitOptions::new().initial_head("main"),
    )
    .expect("init");
    {
        let mut cfg = repo.config().expect("cfg");
        cfg.set_str("user.name", "Prop Bot").unwrap();
        cfg.set_str("user.email", "prop@bonsai.local").unwrap();
        cfg.set_bool("core.autocrlf", false).unwrap();
    }
    {
        let mut index = repo.index().expect("index");
        for i in 0..n {
            write(
                dir.path(),
                &format!("t{i}"),
                &content(1000 + i as u32, "base"),
            );
            index.add_path(Path::new(&format!("t{i}"))).expect("add");
        }
        index.write().expect("write index");
        let tree_oid = index.write_tree().expect("wt");
        let tree = repo.find_tree(tree_oid).expect("tree");
        let sig = git2::Signature::new("Prop Bot", "prop@bonsai.local", &git2::Time::new(1000, 0))
            .unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .expect("commit");
    }
    (dir, repo)
}

/// One band of the base-repo-size partition: `$lo..=$hi` committed files,
/// `$cases` cases, both properties. The bodies are the original
/// `stash_all_with_untracked_roundtrip` / `stash_all_tracked_only_roundtrip`
/// bodies, verbatim.
macro_rules! band {
    ($with_untracked:ident, $tracked_only:ident, $lo:expr, $hi:expr, $cases:expr) => {
        proptest! {
            #![proptest_config(ProptestConfig { cases: $cases, ..ProptestConfig::default() })]

            /// AllWithUntracked: staged + unstaged + untracked all round-trip in the
            /// worktree; the worktree is clean between stash and apply.
            #[test]
            fn $with_untracked(
                n in $lo..=$hi,
                staged in prop::collection::vec((any::<usize>(), any::<u32>()), 0..=6),
                unstaged in prop::collection::vec((any::<usize>(), any::<u32>()), 0..=6),
                untracked in prop::collection::vec((0usize..=6, any::<u32>()), 0..=6),
            ) {
                let (dir, repo) = base_repo(n);
                let root = dir.path();

                // Staged edits to tracked files (modify + git2 add).
                {
                    let mut index = repo.index().expect("index");
                    for (sel, seed) in &staged {
                        let rel = format!("t{}", sel % n);
                        write(root, &rel, &content(*seed, "staged"));
                        index.add_path(Path::new(&rel)).expect("add");
                    }
                    index.write().expect("write");
                }
                // Unstaged edits (modify only) — overlaps a staged file ⇒ staged-then-modified.
                for (sel, seed) in &unstaged {
                    write(root, &format!("t{}", sel % n), &content(seed.wrapping_add(7), "unstaged"));
                }
                // Untracked files.
                for (k, seed) in &untracked {
                    write(root, &format!("u{k}"), &content(*seed, "untracked"));
                }

                let before = worktree_snapshot(root);
                let res = create_stash(root, None, StashScope::AllWithUntracked).expect("create_stash");
                prop_assume!(res.created); // clean worktree ⇒ nothing to stash, skip

                // Worktree is clean between stash and apply (baseline only).
                let mid = read_status(root).expect("status");
                prop_assert!(
                    mid.staged.is_empty() && mid.unstaged.is_empty()
                        && mid.untracked.is_empty() && mid.conflicted.is_empty(),
                    "worktree not clean after stash: {mid:?}"
                );

                let outcome = apply_stash(root, 0, false, None).expect("apply_stash");
                prop_assume!(matches!(outcome, ApplyStashOutcome::Applied));

                let after = worktree_snapshot(root);
                prop_assert_eq!(before, after, "worktree bytes not identical after stash round-trip");
                drop(dir);
            }

            /// All (no untracked): tracked changes round-trip; untracked files are left
            /// in place by the stash and excluded from the identity check (§2.5 step 2).
            #[test]
            fn $tracked_only(
                n in $lo..=$hi,
                staged in prop::collection::vec((any::<usize>(), any::<u32>()), 0..=6),
                unstaged in prop::collection::vec((any::<usize>(), any::<u32>()), 0..=6),
                untracked in prop::collection::vec((0usize..=4, any::<u32>()), 0..=4),
            ) {
                let (dir, repo) = base_repo(n);
                let root = dir.path();
                {
                    let mut index = repo.index().expect("index");
                    for (sel, seed) in &staged {
                        let rel = format!("t{}", sel % n);
                        write(root, &rel, &content(*seed, "staged"));
                        index.add_path(Path::new(&rel)).expect("add");
                    }
                    index.write().expect("write");
                }
                for (sel, seed) in &unstaged {
                    write(root, &format!("t{}", sel % n), &content(seed.wrapping_add(7), "unstaged"));
                }
                let untracked_paths: Vec<String> = untracked.iter().map(|(k, _)| format!("u{k}")).collect();
                for (k, seed) in &untracked {
                    write(root, &format!("u{k}"), &content(*seed, "untracked"));
                }

                let tracked_before: BTreeMap<String, Vec<u8>> = worktree_snapshot(root)
                    .into_iter()
                    .filter(|(p, _)| !untracked_paths.contains(p))
                    .collect();

                let res = create_stash(root, None, StashScope::All).expect("create_stash");
                prop_assume!(res.created);

                let outcome = apply_stash(root, 0, false, None).expect("apply_stash");
                prop_assume!(matches!(outcome, ApplyStashOutcome::Applied));

                let tracked_after: BTreeMap<String, Vec<u8>> = worktree_snapshot(root)
                    .into_iter()
                    .filter(|(p, _)| !untracked_paths.contains(p))
                    .collect();
                prop_assert_eq!(tracked_before, tracked_after, "tracked worktree bytes not identical");
                drop(dir);
            }
        }
    };
}

// 5 bands tiling 1..=5 (width 1 each) with 10 cases per band per property.
band!(
    stash_all_with_untracked_roundtrip_b1_n1,
    stash_all_tracked_only_roundtrip_b1_n1,
    1usize,
    1,
    10
);
band!(
    stash_all_with_untracked_roundtrip_b2_n2,
    stash_all_tracked_only_roundtrip_b2_n2,
    2usize,
    2,
    10
);
band!(
    stash_all_with_untracked_roundtrip_b3_n3,
    stash_all_tracked_only_roundtrip_b3_n3,
    3usize,
    3,
    10
);
band!(
    stash_all_with_untracked_roundtrip_b4_n4,
    stash_all_tracked_only_roundtrip_b4_n4,
    4usize,
    4,
    10
);
band!(
    stash_all_with_untracked_roundtrip_b5_n5,
    stash_all_tracked_only_roundtrip_b5_n5,
    5usize,
    5,
    10
);
