//! P86 B1 layout-cache classification tests. Self-contained git2 fixtures (no
//! AppState) drive [`stream_graph_cached`] directly and assert the walk/hit/
//! redecorate counters plus the served topology — proving no false hit: a
//! ref-only change at an existing commit redecorates (no walk), while a new
//! commit / a dropped tip re-walks (Miss).

use super::*;
use bonsai_core::graph::{GraphChunk, RefKind, RefLabel};

// ---- fixtures ------------------------------------------------------------

fn init_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    (dir, repo)
}

/// Commit an in-memory single-file tree with an explicit timestamp (distinct
/// times keep the TIME-sorted walk deterministic). Does NOT update any ref.
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
    stream_graph_cached(path, cache, perf, |c| {
        out.push(c);
        true
    })
    .expect("stream_graph_cached");
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

// ---- tests ---------------------------------------------------------------

/// AC-B1c: two identical requests with no repo change → 2nd is HitVerbatim
/// (no walk, `graph_cache_hits`+1); output byte-identical.
#[test]
fn hit_verbatim_on_unchanged_repo() {
    let (dir, _repo, _oids) = linear_fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let first = run(dir.path(), &cache, &perf);
    let second = run(dir.path(), &cache, &perf);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "one cold walk");
    assert_eq!(c.graph_cache_hits, 1, "second request is a verbatim hit");
    assert_eq!(c.graph_redecorates, 0);
    assert_eq!(wire(&first), wire(&second), "replayed stream is byte-identical");
}

/// AC-B1a: create a branch at an EXISTING commit → HitRedecorate (`graph_walks`
/// unchanged, `graph_redecorates`+1); topology byte-identical, only the new pill
/// appears.
#[test]
fn redecorate_on_branch_create_at_existing_commit() {
    let (dir, repo, [c0, c1, _c2]) = linear_fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let before = run(dir.path(), &cache, &perf);
    assert!(!ref_names(&before, &c1).contains(&"feature".to_string()));

    branch(&repo, "feature", c1); // existing commit, HEAD stays on main
    let after = run(dir.path(), &cache, &perf);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "no re-walk on a ref-only add at an existing oid");
    assert_eq!(c.graph_redecorates, 1);
    assert_eq!(c.graph_cache_hits, 0);

    assert_eq!(rows(&before), rows(&after), "nodes/lanes identical");
    assert_eq!(edge_tuples(&before), edge_tuples(&after), "edges identical");
    assert_eq!(done_head_index(&before), done_head_index(&after), "HEAD unmoved");
    assert!(
        ref_names(&after, &c1).contains(&"feature".to_string()),
        "new pill present after redecorate"
    );
    // Unrelated commit keeps whatever pills it had (c0 has none).
    assert!(refs_at(&after, &c0).is_empty());
}

/// AC-B1b: a new commit / HEAD advance → Miss (`graph_walks`+1); the new node is
/// present. Counters prove no false hit.
#[test]
fn miss_on_new_commit() {
    let (dir, repo, [_c0, _c1, c2]) = linear_fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let before = run(dir.path(), &cache, &perf);
    let c3 = commit(&repo, "C3", &[c2], 4);
    // Advance `main` (the checked-out branch) via a direct ref update — the
    // `branch(force)` helper refuses to move the current HEAD.
    repo.reference("refs/heads/main", c3, true, "advance main")
        .expect("advance main");
    let after = run(dir.path(), &cache, &perf);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 2, "a new commit forces a full re-walk");
    assert_eq!(c.graph_cache_hits, 0);
    assert_eq!(c.graph_redecorates, 0);

    let hex3 = c3.to_string();
    assert!(!rows(&before).iter().any(|(id, _)| *id == hex3));
    assert!(rows(&after).iter().any(|(id, _)| *id == hex3), "new commit walked");
}

/// AC-B1d: a branch delete that drops commits → Miss (`graph_walks`+1); output
/// correctly shrinks (the offshoot-only commit disappears).
#[test]
fn miss_on_branch_delete_dropping_commits() {
    let (dir, repo, [c0, _c1, _c2]) = linear_fixture();
    let f1 = commit(&repo, "F1", &[c0], 5); // offshoot commit off c0
    branch(&repo, "feature", f1);
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let before = run(dir.path(), &cache, &perf);
    let hexf = f1.to_string();
    assert!(rows(&before).iter().any(|(id, _)| *id == hexf), "offshoot walked");

    delete_branch(&repo, "feature"); // drops the only tip reaching f1
    let after = run(dir.path(), &cache, &perf);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 2, "dropping a tip re-walks");
    assert_eq!(c.graph_redecorates, 0);
    assert!(
        !rows(&after).iter().any(|(id, _)| *id == hexf),
        "offshoot commit gone after delete"
    );
}

/// HitRedecorate via the EXACT-tips branch of the classifier (not the `⊆` one):
/// a lightweight tag added at an EXISTING tip leaves `(tips, head, hide)`
/// identical but changes decoration → re-pill, no re-walk.
#[test]
fn redecorate_on_tag_at_existing_tip() {
    let (dir, repo, [_c0, _c1, c2]) = linear_fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let before = run(dir.path(), &cache, &perf);
    let obj = repo.find_object(c2, None).expect("object");
    repo.tag_lightweight("v1", &obj, false).expect("tag");
    let after = run(dir.path(), &cache, &perf);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "a tag at an existing tip does not re-walk");
    assert_eq!(c.graph_redecorates, 1);
    assert_eq!(rows(&before), rows(&after), "topology identical");
    assert!(
        ref_names(&after, &c2).contains(&"v1".to_string()),
        "tag pill added by the redecorate"
    );
}

/// Correctness beyond the ACs: a HEAD move onto an ALREADY-WALKED commit
/// (detached checkout of an older commit) is a HitRedecorate, not a Miss — the
/// reachable set is unchanged, only the HEAD pill + head_index move. Proves the
/// `⊆ node_oids` branch handles HEAD-as-a-tip correctly (no false miss, no wrong
/// graph).
#[test]
fn redecorate_on_head_move_to_existing_commit() {
    let (dir, repo, [_c0, c1, c2]) = linear_fixture();
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let before = run(dir.path(), &cache, &perf);
    let head_row_before = done_head_index(&before);

    repo.set_head_detached(c1).expect("detach HEAD to c1");
    let after = run(dir.path(), &cache, &perf);

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "HEAD onto a walked commit does not re-walk");
    assert_eq!(c.graph_redecorates, 1);

    assert_eq!(rows(&before), rows(&after), "topology identical");
    assert_eq!(edge_tuples(&before), edge_tuples(&after));
    assert_ne!(
        done_head_index(&after),
        head_row_before,
        "head_index moved from c2's row to c1's row"
    );
    // c1 now carries a detached HEAD pill; main's pill stays on c2 (no is_head).
    assert!(
        refs_at(&after, &c1).iter().any(|r| r.kind == RefKind::Head),
        "detached HEAD pill on c1"
    );
    assert!(
        refs_at(&after, &c2)
            .iter()
            .all(|r| !(r.kind == RefKind::LocalBranch && r.is_head)),
        "main no longer marked is_head"
    );
}

/// Fingerprints: identical seeds hash equal; any topology or decoration change
/// flips the corresponding fingerprint (guards the classifier's fast path).
#[test]
fn fingerprints_track_changes() {
    let (dir, repo, [_c0, c1, c2]) = linear_fixture();
    let s1 = bonsai_core::graph::graph_seed(dir.path()).expect("seed");
    let tips1: BTreeSet<git2::Oid> = s1.tips.iter().copied().collect();
    let hide1: BTreeSet<git2::Oid> = s1.hide.iter().copied().collect();
    let seed_fp1 = seed_fingerprint(&tips1, s1.head, &hide1);
    let deco_fp1 = deco_fingerprint(&s1.refs);

    // Ref-only add at an existing commit: seed_fp changes (new tip), deco_fp too.
    branch(&repo, "feature", c1);
    let s2 = bonsai_core::graph::graph_seed(dir.path()).expect("seed");
    let tips2: BTreeSet<git2::Oid> = s2.tips.iter().copied().collect();
    let hide2: BTreeSet<git2::Oid> = s2.hide.iter().copied().collect();
    assert_ne!(seed_fp1, seed_fingerprint(&tips2, s2.head, &hide2));
    assert_ne!(deco_fp1, deco_fingerprint(&s2.refs));

    // Deco-only change at the SAME tips: rename main? Instead, a tag at c2 (a tip
    // already) keeps tips/head/hide identical but changes decoration.
    delete_branch(&repo, "feature");
    let obj = repo.find_object(c2, None).expect("obj");
    repo.tag_lightweight("v1", &obj, false).expect("tag");
    let s3 = bonsai_core::graph::graph_seed(dir.path()).expect("seed");
    let tips3: BTreeSet<git2::Oid> = s3.tips.iter().copied().collect();
    let hide3: BTreeSet<git2::Oid> = s3.hide.iter().copied().collect();
    // Tips/head/hide back to the c0..c2 set (tag target c2 is already a tip).
    assert_eq!(seed_fingerprint(&tips3, s3.head, &hide3), seed_fp1, "same topology");
    assert_ne!(deco_fp1, deco_fingerprint(&s3.refs), "decoration differs (tag)");
}
