//! Spec-003: layout-cache behavior under an active [`GraphFilter`] — filter
//! equality gates every hit, and a HitRedecorate under a seed restriction can
//! never resurrect a hidden pill.

use super::*;
use bonsai_core::graph::{GraphChunk, GraphFilter};

fn init_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    (dir, repo)
}

fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0))
        .expect("sig");
    let blob = repo.blob(msg.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("insert");
    let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
    let parent_commits: Vec<git2::Commit> = parents
        .iter()
        .map(|p| repo.find_commit(*p).expect("parent"))
        .collect();
    let refs: Vec<&git2::Commit> = parent_commits.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &refs).expect("commit")
}

fn branch(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let c = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &c, true).expect("branch");
}

fn set_head(repo: &git2::Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}")).expect("set head");
}

/// `c0<-c1<-c2` with `main` on the tip, `other` on `c1`, HEAD attached to main.
fn fixture() -> (tempfile::TempDir, git2::Repository, [git2::Oid; 3]) {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    branch(&repo, "other", c1);
    set_head(&repo, "main");
    (dir, repo, [c0, c1, c2])
}

fn run(path: &std::path::Path, cache: &GraphCache, perf: &PerfState, f: &GraphFilter) -> Vec<GraphChunk> {
    let mut out = Vec::new();
    stream_graph_cached(path, cache, perf, f, |c| {
        out.push(c);
        true
    })
    .expect("stream_graph_cached");
    out
}

fn solo(names: &[&str]) -> GraphFilter {
    GraphFilter {
        first_parent: false,
        seed_refs: Some(names.iter().map(|s| s.to_string()).collect()),
    }
}

fn all_pill_names(chunks: &[GraphChunk]) -> Vec<String> {
    let mut v = Vec::new();
    for c in chunks {
        if let GraphChunk::Batch { nodes, .. } = c {
            for n in nodes {
                v.extend(n.refs.iter().map(|r| r.name.clone()));
            }
        }
    }
    v
}

fn wire(chunks: &[GraphChunk]) -> Vec<serde_json::Value> {
    chunks
        .iter()
        .map(|c| serde_json::to_value(c).expect("serialize"))
        .collect()
}

/// Same filter twice → HitVerbatim; a DIFFERENT filter → Miss + re-walk.
#[test]
fn filter_equality_gates_the_cache() {
    let (dir, _repo, _oids) = fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();
    let f = solo(&["refs/heads/main"]);

    let first = run(dir.path(), &cache, &perf, &f);
    let second = run(dir.path(), &cache, &perf, &f);
    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "one cold walk under the filter");
    assert_eq!(c.graph_cache_hits, 1, "same filter → verbatim hit");
    assert_eq!(wire(&first), wire(&second), "replay byte-identical");

    // A different filter (the default) is an unconditional Miss...
    let full = run(dir.path(), &cache, &perf, &GraphFilter::default());
    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 2, "filter change → re-walk");
    assert!(
        all_pill_names(&full).contains(&"other".to_string()),
        "full walk restores the hidden pill"
    );
    // ...and switching back to the first filter re-walks again (single slot).
    let third = run(dir.path(), &cache, &perf, &f);
    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 3, "toggle back → one more walk (determinism, not caching)");
    assert_eq!(wire(&first), wire(&third), "deterministic filtered stream");
}

/// A HitRedecorate under an active seed restriction re-pills from the FILTERED
/// seed: a whitelisted ref created at an existing commit appears, while a
/// non-whitelisted one can never resurrect.
#[test]
fn redecorate_under_filter_never_resurrects_hidden_pills() {
    let (dir, repo, [_c0, c1, _c2]) = fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();
    // Whitelist main + feature (feature does not exist yet → partial-stale,
    // applied via main).
    let f = solo(&["refs/heads/main", "refs/heads/feature"]);

    let before = run(dir.path(), &cache, &perf, &f);
    assert!(!all_pill_names(&before).contains(&"other".to_string()));

    // Create BOTH a whitelisted and a non-whitelisted branch at an existing
    // walked commit — a decoration-only change under this filter.
    branch(&repo, "feature", c1);
    branch(&repo, "secret", c1);
    let after = run(dir.path(), &cache, &perf, &f);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "ref-only change at an existing oid → no re-walk");
    assert_eq!(c.graph_redecorates, 1, "served as HitRedecorate");
    let pills = all_pill_names(&after);
    assert!(pills.contains(&"feature".to_string()), "whitelisted pill appears");
    assert!(!pills.contains(&"secret".to_string()), "hidden pill never resurrects");
    assert!(!pills.contains(&"other".to_string()), "hidden pill never resurrects");
}

/// The store path caches a FILTERED walk (the bracketing re-probe runs under
/// the same filter): first-parent twice → one walk + one verbatim hit.
#[test]
fn first_parent_walk_is_cached_and_replayed() {
    let (dir, _repo, _oids) = fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();
    let f = GraphFilter {
        first_parent: true,
        seed_refs: None,
    };
    let first = run(dir.path(), &cache, &perf, &f);
    let second = run(dir.path(), &cache, &perf, &f);
    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1);
    assert_eq!(c.graph_cache_hits, 1);
    assert_eq!(wire(&first), wire(&second));
}

fn meta_flags(chunks: &[GraphChunk]) -> (bool, bool) {
    match chunks.first() {
        Some(GraphChunk::Meta {
            filtered,
            seed_refs_applied,
            ..
        }) => (*filtered, *seed_refs_applied),
        other => panic!("first chunk must be Meta, got {other:?}"),
    }
}

/// MUST-FIX (review round 1): a HitRedecorate rewrites the Meta truth flags
/// from the FRESH seed. Renaming the soloed branch (same oid) flips the filter
/// into the stale fallback while the tip OIDs stay equal → the redecorated
/// replay must carry `seedRefsApplied:false` (and the full-graph pills), not
/// the cached `true`.
#[test]
fn redecorate_refreshes_meta_flags_on_stale_flip() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    branch(&repo, "other", c1);
    // Detached HEAD so the branch rename below cannot orphan HEAD.
    repo.set_head_detached(c2).expect("detach head");

    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();
    let f = solo(&["refs/heads/other"]);

    // Cold walk under the solo filter: applied, `main` pill hidden.
    let before = run(dir.path(), &cache, &perf, &f);
    assert_eq!(meta_flags(&before), (true, true));
    assert!(!all_pill_names(&before).contains(&"main".to_string()));

    // Rename the soloed ref at the SAME oid: tips/head/hide unchanged (the
    // stale fallback seeds the full graph, whose tips are the same {c1, c2}),
    // only the decoration + the resolved plan differ → HitRedecorate.
    repo.find_branch("other", git2::BranchType::Local)
        .expect("find other")
        .rename("renamed", false)
        .expect("rename");
    let after = run(dir.path(), &cache, &perf, &f);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "served without a re-walk");
    assert_eq!(c.graph_redecorates, 1, "rename at same oid → HitRedecorate");
    assert_eq!(
        meta_flags(&after),
        (false, false),
        "flags rewritten from the fresh seed (stale fallback)"
    );
    let pills = all_pill_names(&after);
    assert!(pills.contains(&"main".to_string()), "fallback shows all pills");
    assert!(pills.contains(&"renamed".to_string()));
}
