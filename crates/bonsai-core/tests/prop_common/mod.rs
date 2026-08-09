//! Shared proptest strategies + repo builder for the T5 property suites
//! (contract §1). Included by each `prop_*.rs` via
//! `#[path = "prop_common/mod.rs"] mod prop_common;` — integration test
//! binaries do not share a crate root, so this is a `#[path]`-included module.
//!
//! Determinism + scratch discipline: repos are materialized under
//! `common::scratch_dir()` (D:\Temp\bonsai-scratch on Windows). Each commit
//! gets a UNIQUE tree (`n.txt` = commit index) so two specs can never collide
//! on the same oid; every commit is made reachable by an auto leaf branch so
//! the node-bijection invariant (nodes == commits) holds.
//!
//! `PROPTEST_CASES` default is 64 in-file (set per-suite via `ProptestConfig`);
//! local exhaustive runs use `PROPTEST_CASES=256` from the environment.

#![allow(dead_code)] // each suite uses a subset of these helpers

use std::path::PathBuf;

use proptest::prelude::*;

// The established CLI-oracle helpers (scratch_dir, git shellout, porcelain
// oracle). Re-exported so suites reach them as `prop_common::common::*`.
#[path = "../common/mod.rs"]
pub mod common;

/// Fixed base timestamp; per-commit `ts_offset` skews around it (clock skew).
pub const BASE_TS: i64 = 1_600_000_000;

/// One commit in a [`RepoShape`]. `parents` indices are each `< own index`
/// (DAG by construction); empty only for index 0; len 2 ⇒ merge; a duplicated
/// entry ⇒ degenerate duplicate-parent merge.
#[derive(Debug, Clone)]
pub struct CommitSpec {
    pub parents: Vec<usize>,
    pub message: String,
    pub ts_offset: i64,
}

/// Where HEAD points: an attached auto branch at a commit, or detached on one.
#[derive(Debug, Clone)]
pub enum HeadSpec {
    Branch(usize),
    Detached(usize),
}

/// A bounded random repository shape (contract §1).
#[derive(Debug, Clone)]
pub struct RepoShape {
    /// 1..=200 commits; index 0 is the root.
    pub commits: Vec<CommitSpec>,
    /// Extra local-branch refs (name, commit index). Materialized under
    /// `refs/heads/user/<name>` to avoid D/F clashes with the auto refs.
    pub branches: Vec<(String, usize)>,
    /// Lightweight tags (name, commit index).
    pub tags: Vec<(String, usize)>,
    pub head: HeadSpec,
}

/// Printable-ish message: arbitrary Unicode (incl. non-ASCII / astral / control
/// chars other than NUL — a NUL would truncate the C commit message). Bounded
/// to 40 chars for build speed (contract allows 0..=200).
fn message_strat() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>().prop_filter("no NUL", |c| *c != '\0'), 0..=40)
        .prop_map(|cs| cs.into_iter().collect())
}

/// Valid git ref shorthand: leading lowercase letter, then `[a-zA-Z0-9_-]` —
/// no leading `-`/`.`, no `..`, no trailing `.lock`, no slash (invalid names
/// are T5.3's job, not the generator's).
fn ref_name_strat() -> impl Strategy<Value = String> {
    "[a-z][a-zA-Z0-9_-]{0,8}"
}

/// proptest Strategy producing a [`RepoShape`] (contract §1). Merge probability
/// ~20%, duplicate-parent ~2%; the rest are linear (70% first-parent = i-1) or
/// branch-from-ancestor.
pub fn repo_shape() -> impl Strategy<Value = RepoShape> {
    let raw = prop::collection::vec(
        (
            any::<u32>(),
            any::<u32>(),
            any::<u8>(),
            message_strat(),
            -1000i64..=1000,
        ),
        1..=200,
    );
    raw.prop_flat_map(|raws| {
        let n = raws.len();
        let commits: Vec<CommitSpec> = raws
            .iter()
            .enumerate()
            .map(|(i, (p1, p2, ms, msg, ts))| {
                if i == 0 {
                    return CommitSpec {
                        parents: vec![],
                        message: msg.clone(),
                        ts_offset: *ts,
                    };
                }
                // First parent: 70% linear (i-1), else an arbitrary ancestor.
                let p1i = if (*p1 % 100) < 70 {
                    i - 1
                } else {
                    (*p1 as usize) % i
                };
                let is_dup = *ms < 5; // ~2%
                let is_merge = *ms < 51; // ~20% (superset of dup)
                let parents = if i >= 2 && is_dup {
                    vec![p1i, p1i]
                } else if i >= 2 && is_merge {
                    let mut p2i = (*p2 as usize) % i;
                    if p2i == p1i {
                        p2i = (p1i + 1) % i;
                    }
                    vec![p1i, p2i]
                } else {
                    vec![p1i]
                };
                CommitSpec {
                    parents,
                    message: msg.clone(),
                    ts_offset: *ts,
                }
            })
            .collect();

        let branches = prop::collection::vec((ref_name_strat(), 0..n), 0..=8);
        let tags = prop::collection::vec((ref_name_strat(), 0..n), 0..=8);
        let head = prop_oneof![
            (0..n).prop_map(HeadSpec::Branch),
            (0..n).prop_map(HeadSpec::Detached),
        ];
        (Just(commits), branches, tags, head).prop_map(|(commits, branches, tags, head)| {
            RepoShape {
                commits,
                branches: dedupe_named(branches),
                tags: dedupe_named(tags),
                head,
            }
        })
    })
}

/// Drop entries whose name repeats (a duplicate ref name would clash).
fn dedupe_named(items: Vec<(String, usize)>) -> Vec<(String, usize)> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|(name, _)| seen.insert(name.clone()))
        .collect()
}

/// Materialize `shape` under `common::scratch_dir()`. Every commit is written
/// with a unique tree; every leaf commit gets an auto branch so ALL commits are
/// reachable from the ref set (node-bijection precondition). Panics only on
/// infra errors (the point of the test is that `compute_graph` never panics).
pub fn build_repo(shape: &RepoShape) -> (tempfile::TempDir, PathBuf) {
    let dir = common::scratch_dir();
    let path = dir.path().to_path_buf();
    let repo = git2::Repository::init_opts(
        &path,
        git2::RepositoryInitOptions::new().initial_head("main"),
    )
    .expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Prop Bot").expect("name");
        cfg.set_str("user.email", "prop@bonsai.local").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }

    let mut oids: Vec<git2::Oid> = Vec::with_capacity(shape.commits.len());
    for (i, spec) in shape.commits.iter().enumerate() {
        let blob = repo.blob(i.to_string().as_bytes()).expect("blob");
        let mut tb = repo.treebuilder(None).expect("treebuilder");
        tb.insert("n.txt", blob, 0o100_644).expect("tree insert");
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("find tree");
        let t = BASE_TS + spec.ts_offset;
        let sig = git2::Signature::new("Prop Bot", "prop@bonsai.local", &git2::Time::new(t, 0))
            .expect("signature");
        let parent_commits: Vec<git2::Commit> = spec
            .parents
            .iter()
            .map(|p| repo.find_commit(oids[*p]).expect("find parent"))
            .collect();
        let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
        let oid = repo
            .commit(None, &sig, &sig, &spec.message, &tree, &parent_refs)
            .expect("commit");
        oids.push(oid);
    }

    // Auto leaf branches: any commit that is nobody's parent gets a ref so the
    // whole DAG is reachable from the ref set.
    let mut is_parent = vec![false; oids.len()];
    for spec in &shape.commits {
        for p in &spec.parents {
            is_parent[*p] = true;
        }
    }
    for (i, oid) in oids.iter().enumerate() {
        if !is_parent[i] {
            let _ = repo.reference(&format!("refs/heads/auto/leaf-{i}"), *oid, true, "leaf");
        }
    }

    // User branches (namespaced) + lightweight tags.
    for (name, idx) in &shape.branches {
        let _ = repo.reference(&format!("refs/heads/user/{name}"), oids[*idx], true, "b");
    }
    for (name, idx) in &shape.tags {
        if let Ok(obj) = repo.find_object(oids[*idx], None) {
            let _ = repo.tag_lightweight(name, &obj, true);
        }
    }

    match &shape.head {
        HeadSpec::Branch(idx) => {
            let _ = repo.reference("refs/heads/auto/head", oids[*idx], true, "h");
            repo.set_head("refs/heads/auto/head").expect("set head");
        }
        HeadSpec::Detached(idx) => {
            repo.set_head_detached(oids[*idx]).expect("detach head");
        }
    }

    (dir, path)
}

/// Two related text blobs for the intraline suite (contract §1): a base of
/// 0..=30 random lines and an edited copy with a few random line/char edits,
/// unicode-capable. Also yields the degenerate pairs (empty,x)/(x,x)/(x,empty).
pub fn diff_pair() -> impl Strategy<Value = (String, String)> {
    let line = prop::collection::vec(
        any::<char>().prop_filter("printable-ish, no newline/NUL", |c| {
            *c != '\n' && *c != '\r' && *c != '\0'
        }),
        0..=20,
    )
    .prop_map(|cs| cs.into_iter().collect::<String>());
    let base = prop::collection::vec(line.clone(), 0..=30);
    // Ops: 0=keep, 1=drop, 2=replace-with-random, plus intra-line char tweak.
    let ops = prop::collection::vec((0u8..=3, line.clone()), 0..=15);
    (base, ops).prop_map(|(base, ops)| {
        let a = base.join("\n");
        let mut edited: Vec<String> = base.clone();
        for (k, (op, repl)) in ops.into_iter().enumerate() {
            if edited.is_empty() {
                edited.push(repl);
                continue;
            }
            let idx = k % edited.len();
            match op {
                0 => {} // keep
                1 => {
                    edited.remove(idx);
                }
                2 => {
                    edited[idx] = repl;
                }
                _ => {
                    // intra-line: append a char + reverse (a bounded tweak)
                    let mut chars: Vec<char> = edited[idx].chars().collect();
                    chars.reverse();
                    edited[idx] = chars.into_iter().collect();
                }
            }
        }
        let b = edited.join("\n");
        (a, b)
    })
}

// ---- status oracle mapping (ported verbatim from tests/status_porcelain.rs) --

/// Canonical comparison tuple: (list, path, orig_path, status).
pub type StatusTuple = (String, String, Option<String>, String);

fn status_name(s: bonsai_core::git::status::FileStatus) -> &'static str {
    use bonsai_core::git::status::FileStatus::*;
    match s {
        Added => "added",
        Modified => "modified",
        Deleted => "deleted",
        Renamed => "renamed",
        Typechange => "typechange",
        Conflicted => "conflicted",
        Untracked => "untracked",
    }
}

/// Flatten a snapshot into the canonical tuple set (same mapping as the M1
/// status twin-pair tests).
pub fn flatten_snapshot(
    snapshot: &bonsai_core::git::status::StatusSnapshot,
) -> std::collections::BTreeSet<StatusTuple> {
    let mut set = std::collections::BTreeSet::new();
    for (list, entries) in [
        ("staged", &snapshot.staged),
        ("unstaged", &snapshot.unstaged),
        ("untracked", &snapshot.untracked),
        ("conflicted", &snapshot.conflicted),
    ] {
        for e in entries {
            set.insert((
                list.to_string(),
                e.path.clone(),
                e.orig_path.clone(),
                status_name(e.status).to_string(),
            ));
        }
    }
    set
}

fn is_conflict_code(x: char, y: char) -> bool {
    x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D')
}
fn index_column_status(x: char) -> Option<&'static str> {
    match x {
        'A' => Some("added"),
        'M' => Some("modified"),
        'D' => Some("deleted"),
        'R' => Some("renamed"),
        'T' => Some("typechange"),
        _ => None,
    }
}
fn worktree_column_status(y: char) -> Option<&'static str> {
    match y {
        'M' => Some("modified"),
        'D' => Some("deleted"),
        'R' => Some("renamed"),
        'T' => Some("typechange"),
        _ => None,
    }
}

/// Run `git status --porcelain=v1 -z --untracked-files=all` and map to the
/// canonical tuple set (same mapping as the M1 status twin-pair tests).
pub fn porcelain_tuples(dir: &std::path::Path) -> std::collections::BTreeSet<StatusTuple> {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(dir)
        .output()
        .expect("run git status");
    assert!(
        out.status.success(),
        "git status failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let raw = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut tokens = raw.split('\0').filter(|t| !t.is_empty());
    let mut set = std::collections::BTreeSet::new();
    while let Some(token) = tokens.next() {
        let mut chars = token.chars();
        let x = chars.next().expect("X column");
        let y = chars.next().expect("Y column");
        assert_eq!(chars.next(), Some(' '), "porcelain separator in {token:?}");
        let path: String = chars.collect();
        let orig = if x == 'R' || y == 'R' {
            Some(tokens.next().expect("rename orig path token").to_string())
        } else {
            None
        };
        if x == '?' && y == '?' {
            set.insert(("untracked".to_string(), path, None, "untracked".to_string()));
            continue;
        }
        if is_conflict_code(x, y) {
            set.insert(("conflicted".to_string(), path, None, "conflicted".to_string()));
            continue;
        }
        if let Some(status) = index_column_status(x) {
            let orig_for_row = if x == 'R' { orig.clone() } else { None };
            set.insert(("staged".to_string(), path.clone(), orig_for_row, status.to_string()));
        }
        if let Some(status) = worktree_column_status(y) {
            let orig_for_row = if y == 'R' { orig.clone() } else { None };
            set.insert(("unstaged".to_string(), path.clone(), orig_for_row, status.to_string()));
        }
    }
    set
}
