//! P86 B1 layout-cache classification tests. Self-contained git2 fixtures (no
//! AppState) drive [`stream_graph_cached`] directly and assert the walk/hit/
//! redecorate counters plus the served topology — proving no false hit: a
//! ref-only change at an existing commit redecorates (no walk), while a new
//! commit / a dropped tip re-walks (Miss).

use super::*;
use bonsai_core::graph::{GraphChunk, GraphFilter, RefKind, RefLabel};

// ---- fixtures ------------------------------------------------------------

fn init_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com")
        .expect("email");
    (dir, repo)
}

/// Commit an in-memory single-file tree with an explicit timestamp (distinct
/// times keep the TIME-sorted walk deterministic). Does NOT update any ref.
fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
    let sig =
        git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0)).expect("sig");
    let blob = repo.blob(msg.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("insert");
    let tree = repo
        .find_tree(tb.write().expect("write tree"))
        .expect("tree");
    let parent_commits: Vec<git2::Commit> = parents
        .iter()
        .map(|p| repo.find_commit(*p).expect("parent"))
        .collect();
    let refs: Vec<&git2::Commit> = parent_commits.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &refs)
        .expect("commit")
}

fn branch(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let c = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &c, true).expect("branch");
}

fn set_head(repo: &git2::Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}"))
        .expect("set head");
}

fn delete_branch(repo: &git2::Repository, name: &str) {
    repo.find_branch(name, git2::BranchType::Local)
        .expect("find branch")
        .delete()
        .expect("delete branch");
}

/// Standard 3-commit chain `c0<-c1<-c2` with `main` on the tip and HEAD attached.
fn linear_fixture() -> (tempfile::TempDir, git2::Repository, [git2::Oid; 3]) {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    set_head(&repo, "main");
    (dir, repo, [c0, c1, c2])
}

// ---- drivers / extractors ------------------------------------------------

fn run(path: &std::path::Path, cache: &GraphCache, perf: &PerfState) -> Vec<GraphChunk> {
    let mut out = Vec::new();
    stream_graph_cached(path, cache, perf, &GraphFilter::default(), |c| {
        out.push(c);
        true
    })
    .expect("stream_graph_cached");
    out
}

/// PB-1: drive the cache-aware stream at an explicit (tiny) store cap so both
/// the stored and store-skipped branches are exercisable without a 50k fixture.
fn run_capped(
    path: &std::path::Path,
    cache: &GraphCache,
    perf: &PerfState,
    max_nodes: usize,
) -> Vec<GraphChunk> {
    let mut repo = git2::Repository::open_ext(
        path,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .expect("open");
    let mut out = Vec::new();
    let mut recorder = crate::obs::phase::PhaseRecorder::start(crate::obs::phase::OP_GRAPH_GET);
    stream_graph_cached_capped(
        &mut repo,
        cache,
        perf,
        &GraphFilter::default(),
        max_nodes,
        &mut recorder,
        |c| {
            out.push(c);
            true
        },
    )
    .expect("stream_graph_cached_capped");
    out
}

/// `(id, lane)` per node in row order — the topology, decoration-independent.
fn rows(chunks: &[GraphChunk]) -> Vec<(String, u32)> {
    let mut v = Vec::new();
    for c in chunks {
        if let GraphChunk::Batch { nodes, .. } = c {
            for n in nodes {
                v.push((n.id.clone(), n.lane));
            }
        }
    }
    v
}

fn edge_tuples(chunks: &[GraphChunk]) -> Vec<(u32, u32, u32, u16)> {
    let mut v = Vec::new();
    for c in chunks {
        if let GraphChunk::Batch { edges, .. } = c {
            for e in edges {
                v.push((e.from, e.to, e.lane, e.ord));
            }
        }
    }
    v.sort_unstable();
    v
}

fn done_head_index(chunks: &[GraphChunk]) -> Option<u32> {
    for c in chunks.iter().rev() {
        if let GraphChunk::Done { head_index, .. } = c {
            return *head_index;
        }
    }
    None
}

fn refs_at(chunks: &[GraphChunk], id: &git2::Oid) -> Vec<RefLabel> {
    let hex = id.to_string();
    for c in chunks {
        if let GraphChunk::Batch { nodes, .. } = c {
            for n in nodes {
                if n.id == hex {
                    return n.refs.clone();
                }
            }
        }
    }
    Vec::new()
}

fn ref_names(chunks: &[GraphChunk], id: &git2::Oid) -> Vec<String> {
    refs_at(chunks, id).into_iter().map(|r| r.name).collect()
}

/// Exact wire JSON for the whole stream — byte-identical replay check.
fn wire(chunks: &[GraphChunk]) -> Vec<serde_json::Value> {
    chunks
        .iter()
        .map(|c| serde_json::to_value(c).expect("serialize chunk"))
        .collect()
}

mod classify;
mod memory_cap;
mod spans;
