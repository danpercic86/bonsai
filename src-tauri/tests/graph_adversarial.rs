//! Adversarial / gap-probing tests for the M2a layout engine (tester pass).
//! Shapes intentionally NOT in the contract §2.6 list:
//!   (a) a merge commit whose two parents are the SAME commit (degenerate,
//!       hand-crafted via a raw odb commit object — `git commit-tree -p X -p X`
//!       equivalent),
//!   (b) a branch tip that is an ancestor of another tip (pill mid-history,
//!       no duplicate node),
//!   (c) a 1000-commit linear chain (structural sanity + loose timing bound).
//!
//! Fixtures are built with git2 in `tempfile::TempDir`s; only structural
//! invariants from the contract (§1.1) are asserted where the contract does
//! not pin exact geometry.

use bonsai_lib::graph::{compute_graph, GraphLayout, RefKind};

/// Init a repo in a fresh temp dir with local user config set.
fn init_repo() -> (tempfile::TempDir, git2::Repository) {
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

/// Commit from an in-memory tree with an explicit, strictly-increasing time.
fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
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

fn branch(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let c = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &c, true).expect("create branch");
}

fn set_head(repo: &git2::Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}")).expect("set head");
}

/// Contract §1.1 structural invariants every layout must satisfy:
/// - edges sorted ascending by (from, to); to > from; endpoints in range;
///   edge lane < lane_count,
/// - node lanes < lane_count (when nodes exist),
/// - parent indices strictly greater than the node's own index and in range,
///   (topological order: parents appear at a HIGHER index),
/// - head_index (if any) in range.
fn assert_invariants(l: &GraphLayout) {
    let n = l.nodes.len() as u32;
    let mut prev: Option<(u32, u32)> = None;
    for e in &l.edges {
        assert!(e.to > e.from, "edge to > from violated: {e:?}");
        assert!(e.from < n && e.to < n, "edge endpoint out of range: {e:?}");
        assert!(e.lane < l.lane_count, "edge lane >= lane_count: {e:?}");
        if let Some(p) = prev {
            assert!((e.from, e.to) >= p, "edges not sorted by (from, to)");
        }
        prev = Some((e.from, e.to));
    }
    for (i, node) in l.nodes.iter().enumerate() {
        assert!(node.lane < l.lane_count, "node {i} lane >= lane_count");
        for &p in &node.parents {
            assert!(p > i as u32, "node {i}: parent index {p} not > own index");
            assert!(p < n, "node {i}: parent index {p} out of range");
        }
    }
    if let Some(h) = l.head_index {
        assert!(h < n, "head_index out of range");
    }
}

/// (a) Merge whose two parents are the SAME commit. `repo.commit` may reject
/// duplicates, so the commit object is written raw into the odb — exactly what
/// `git commit-tree -p X -p X` produces. Must not panic; layout must satisfy
/// all structural invariants; the merge node reports both parent slots.
#[test]
fn duplicate_parent_merge_is_sane() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);

    // Raw commit with `parent c1` listed twice.
    let tree_id = repo.find_commit(c1).unwrap().tree_id();
    let raw = format!(
        "tree {tree_id}\n\
         parent {c1}\n\
         parent {c1}\n\
         author Test User <test@example.com> 3 +0000\n\
         committer Test User <test@example.com> 3 +0000\n\n\
         degenerate merge\n"
    );
    let merge = repo
        .odb()
        .unwrap()
        .write(git2::ObjectType::Commit, raw.as_bytes())
        .expect("write raw duplicate-parent commit");
    // Confirm the degenerate shape actually exists.
    assert_eq!(repo.find_commit(merge).unwrap().parent_count(), 2);

    branch(&repo, "main", merge);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph must not error");
    assert_invariants(&l);

    assert_eq!(l.nodes.len(), 3, "no duplicate node for the shared parent");
    assert_eq!(l.nodes[0].summary, "degenerate merge");
    // Both parent slots resolve to the same index (row 1 == c1).
    assert_eq!(l.nodes[0].parents, vec![1, 1]);
    // Everything fits in a single lane (the duplicate parent must not leak an
    // extra reserved lane that nothing ever frees).
    assert_eq!(l.lane_count, 1, "duplicate parent leaked an extra lane");
    // Both merge edges land on (0,1); duplicates tolerated, dangling edges not.
    let merge_edges: Vec<_> = l.edges.iter().filter(|e| e.from == 0).collect();
    assert!(
        !merge_edges.is_empty() && merge_edges.iter().all(|e| e.to == 1),
        "edges out of the merge must all target row 1: {:?}",
        l.edges
    );
}

/// (b) A branch tip that IS an ancestor of another tip: `old` points at C1,
/// `main` (HEAD) at C2 with C2 -> C1 -> C0. The walk is seeded from both tips;
/// C1 must appear exactly once, mid-history, carrying the `old` pill.
#[test]
fn ancestor_branch_tip_gets_mid_history_pill() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "old", c1);
    branch(&repo, "main", c2);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_invariants(&l);

    // No duplicate node despite two seeds sharing history.
    assert_eq!(l.nodes.len(), 3);
    let ids: Vec<String> = l.nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(ids, vec![c2.to_string(), c1.to_string(), c0.to_string()]);

    // Pure linear geometry: one lane, chain edges only.
    assert_eq!(l.nodes.iter().map(|n| n.lane).collect::<Vec<_>>(), vec![0, 0, 0]);
    assert_eq!(l.lane_count, 1);
    let tuples: Vec<_> = l.edges.iter().map(|e| (e.from, e.to, e.lane)).collect();
    assert_eq!(tuples, vec![(0, 1, 0), (1, 2, 0)]);

    // `old` pill sits on the mid-history row, `main` (is_head) on the tip.
    assert_eq!(l.nodes[1].refs.len(), 1);
    assert_eq!(l.nodes[1].refs[0].name, "old");
    assert_eq!(l.nodes[1].refs[0].kind, RefKind::LocalBranch);
    assert!(!l.nodes[1].refs[0].is_head);
    assert_eq!(l.nodes[0].refs.len(), 1);
    assert!(l.nodes[0].refs[0].is_head);
    assert_eq!(l.head_index, Some(0));
}

/// (c) 1000-commit linear chain: all lane 0, exactly n-1 chain edges, and no
/// quadratic blowup (loose debug-mode wall-clock bound on layout alone).
#[test]
fn thousand_commit_linear_chain() {
    let (dir, repo) = init_repo();
    const N: usize = 1000;
    let mut prev: Option<git2::Oid> = None;
    for i in 0..N {
        let parents: Vec<git2::Oid> = prev.into_iter().collect();
        prev = Some(commit(&repo, &format!("c{i}"), &parents, 1 + i as i64));
    }
    branch(&repo, "main", prev.unwrap());
    set_head(&repo, "main");

    let started = std::time::Instant::now();
    let l = compute_graph(dir.path()).expect("compute_graph");
    let elapsed = started.elapsed();

    assert_invariants(&l);
    assert_eq!(l.nodes.len(), N);
    assert!(l.nodes.iter().all(|n| n.lane == 0), "all lanes must be 0");
    assert_eq!(l.lane_count, 1);
    assert_eq!(l.edges.len(), N - 1);
    for (i, e) in l.edges.iter().enumerate() {
        assert_eq!((e.from, e.to, e.lane), (i as u32, i as u32 + 1, 0));
    }
    assert_eq!(
        l.nodes[0].parents,
        vec![1],
        "tip parent must be the next row"
    );
    assert!(l.nodes[N - 1].parents.is_empty(), "root has no parents");
    assert!(!l.truncated);
    // Loose ceiling: debug-mode git2 on 1k commits is ~tens of ms; anything
    // near quadratic would blow far past this.
    assert!(
        elapsed.as_secs() < 10,
        "layout of 1k linear commits took {elapsed:?} — suspicious blowup"
    );
}
