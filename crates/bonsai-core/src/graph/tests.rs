//! Shared graph-test fixtures. The concrete test cases live in the sibling
//! `tests_*.rs` modules (`tests_lane`, `tests_decorate`, `tests_stash_seed`,
//! `tests_stream`, `tests_filter`, `tests_fold`); this module only holds the
//! repo/commit builders and layout accessors they have in common.

use super::*;

/// Initializes a repo in a fresh temp dir with local user config set.
pub(super) fn init_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    {
        let mut config = repo.config().expect("open config");
        config.set_str("user.name", "Test User").expect("set name");
        config
            .set_str("user.email", "test@example.com")
            .expect("set email");
    }
    (dir, repo)
}

/// Creates a commit from an in-memory tree with an EXPLICIT timestamp
/// (walk-order determinism depends on distinct times). No ref is updated.
pub(super) fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0))
        .expect("signature");
    let blob = repo.blob(msg.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("tree insert");
    let tree = repo.find_tree(tb.write().expect("write tree")).expect("find tree");
    let parent_commits: Vec<git2::Commit> = parents
        .iter()
        .map(|p| repo.find_commit(*p).expect("find parent"))
        .collect();
    let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &parent_refs)
        .expect("commit")
}

pub(super) fn branch(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let c = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &c, true).expect("create branch");
}

pub(super) fn set_head(repo: &git2::Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}")).expect("set head");
}

pub(super) fn ids(l: &GraphLayout) -> Vec<String> {
    l.nodes.iter().map(|n| n.id.clone()).collect()
}

pub(super) fn lanes(l: &GraphLayout) -> Vec<u32> {
    l.nodes.iter().map(|n| n.lane).collect()
}

pub(super) fn edge_tuples(l: &GraphLayout) -> Vec<(u32, u32, u32)> {
    l.edges.iter().map(|e| (e.from, e.to, e.lane)).collect()
}

pub(super) fn parents(l: &GraphLayout) -> Vec<Vec<u32>> {
    l.nodes.iter().map(|n| n.parents.clone()).collect()
}

/// Builds the E4 fixture in `repo` and computes its layout.
/// Returns the layout plus `[a2, b2, a1, b1, r]`.
pub(super) fn build_criss_cross(
    repo: &git2::Repository,
    workdir: &std::path::Path,
) -> (GraphLayout, [git2::Oid; 5]) {
    let r = commit(repo, "R", &[], 1);
    let b1 = commit(repo, "B1", &[r], 2);
    let a1 = commit(repo, "A1", &[r], 3);
    let b2 = commit(repo, "B2", &[b1, a1], 4);
    let a2 = commit(repo, "A2", &[a1, b1], 5);
    branch(repo, "a", a2);
    branch(repo, "b", b2);
    set_head(repo, "a");
    let l = compute_graph(workdir).expect("compute_graph");
    (l, [a2, b2, a1, b1, r])
}

// The concrete test cases, declared here (rather than from `graph.rs`) so the
// production module root keeps its size. Same `#[path]` sibling pattern that
// `stream.rs` uses for `stream_wire_tests.rs`.
#[path = "tests_decorate.rs"]
mod tests_decorate;
#[path = "tests_lane.rs"]
mod tests_lane;
#[path = "tests_stash_seed.rs"]
mod tests_stash_seed;
#[path = "tests_stream.rs"]
mod tests_stream;
