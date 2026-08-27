//! Spec-003 graph declutter filter — integration tests over the public graph
//! API: `compute_graph_with` (one-shot), `stream_graph_from_repo` (streaming),
//! `graph_seed` (the cheap probe carrying `seed_refs_applied`).
//!
//! Locked semantics under test (plan.md §Approach):
//! - default filter == today's behavior (regression guard);
//! - first-parent == `git log --first-parent` over the seed set (a merged ref
//!   that still exists keeps its tip + line; deleted → side nodes vanish);
//! - `Some(names)` solo == ancestry(names) ∪ ancestry(HEAD), HEAD pill always
//!   resolves (synthesized when its branch is filtered out);
//! - `Some([])` hide-all == HEAD-only seed, applied=true;
//! - `Some(non-empty)` all-stale == full-graph fallback, applied=false;
//! - stash tips excluded whenever `seed_refs` is `Some`.

use bonsai_core::graph::{
    compute_graph, compute_graph_with, graph_seed, stream_graph_from_repo, GraphChunk,
    GraphFilter, GraphLayout, RefKind,
};

// ---- fixture helpers (mirrors src/graph/tests.rs) --------------------------

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

fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0))
        .expect("signature");
    let blob = repo.blob(msg.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("tree insert");
    let tree = repo
        .find_tree(tb.write().expect("write tree"))
        .expect("find tree");
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

fn tag(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    repo.reference(&format!("refs/tags/{name}"), oid, true, "tag")
        .expect("create tag ref");
}

fn set_head(repo: &git2::Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}"))
        .expect("set head");
}

fn fp() -> GraphFilter {
    GraphFilter {
        first_parent: true,
        seed_refs: None,
        ..Default::default()
    }
}

fn seeds(names: &[&str]) -> GraphFilter {
    GraphFilter {
        first_parent: false,
        seed_refs: Some(names.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    }
}

fn ids(l: &GraphLayout) -> Vec<String> {
    l.nodes.iter().map(|n| n.id.clone()).collect()
}

/// Every pill name in the layout, in row order.
fn all_pill_names(l: &GraphLayout) -> Vec<String> {
    l.nodes
        .iter()
        .flat_map(|n| n.refs.iter().map(|r| r.name.clone()))
        .collect()
}

// ---- fixtures ---------------------------------------------------------------

/// Merge fixture: `a0 <- a1 <- m` on the first-parent line, side commit `s1`
/// (parent a0) merged by `m` (parents `[a1, s1]`). `main` on `m`, HEAD attached.
/// The side branch ref is created only when `keep_side_ref` is true.
fn merge_fixture(keep_side_ref: bool) -> (tempfile::TempDir, git2::Repository, [git2::Oid; 4]) {
    let (dir, repo) = init_repo();
    let a0 = commit(&repo, "a0", &[], 1);
    let a1 = commit(&repo, "a1", &[a0], 2);
    let s1 = commit(&repo, "s1", &[a0], 3);
    let m = commit(&repo, "m", &[a1, s1], 4);
    branch(&repo, "main", m);
    if keep_side_ref {
        branch(&repo, "side", s1);
    }
    set_head(&repo, "main");
    (dir, repo, [a0, a1, s1, m])
}

/// Multi-branch fixture: base `b0`; `x1 <- x2` (branch `x`); `y1` (branch `y`,
/// checked out); `z1` (branch `z`, tag `v1`). All branch from `b0`.
fn multi_branch_fixture(
) -> (tempfile::TempDir, git2::Repository, [git2::Oid; 5]) {
    let (dir, repo) = init_repo();
    let b0 = commit(&repo, "b0", &[], 1);
    let x1 = commit(&repo, "x1", &[b0], 2);
    let x2 = commit(&repo, "x2", &[x1], 3);
    let y1 = commit(&repo, "y1", &[b0], 4);
    let z1 = commit(&repo, "z1", &[b0], 5);
    branch(&repo, "x", x2);
    branch(&repo, "y", y1);
    branch(&repo, "z", z1);
    tag(&repo, "v1", z1);
    set_head(&repo, "y");
    (dir, repo, [b0, x1, x2, y1, z1])
}

/// Merge + tag + stash fixture for the default-regression guard.
fn kitchen_sink_fixture() -> (tempfile::TempDir, git2::Repository) {
    let (dir, repo, [_a0, a1, s1, m]) = merge_fixture(true);
    tag(&repo, "v1", a1);
    branch(&repo, "topic", s1);
    // Materialize the workdir at HEAD, then stash a tracked-file change.
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout head");
    std::fs::write(dir.path().join("f.txt"), "changed\n").expect("modify f.txt");
    {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut repo2 = git2::Repository::open(dir.path()).expect("reopen");
        repo2.stash_save(&sig, "wip", None).expect("stash save");
    }
    let _ = m;
    (dir, repo)
}

// ---- stream capture / parity -------------------------------------------------

fn capture_stream(dir: &std::path::Path, filter: &GraphFilter) -> Vec<GraphChunk> {
    let mut repo = git2::Repository::open(dir).expect("open repo");
    let mut chunks = Vec::new();
    stream_graph_from_repo(&mut repo, filter, |c| {
        chunks.push(c);
        true
    })
    .expect("stream_graph_from_repo");
    chunks
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

/// Folds a stream into the parity tuple `(ids, lanes, edges, lane_count,
/// head_index, truncated)` and compares with the one-shot layout.
fn assert_stream_matches_oneshot(dir: &std::path::Path, filter: &GraphFilter, label: &str) {
    let oracle = compute_graph_with(dir, filter).expect("compute_graph_with");
    let chunks = capture_stream(dir, filter);

    let mut s_ids: Vec<String> = Vec::new();
    let mut s_lanes: Vec<u32> = Vec::new();
    let mut s_edges: Vec<(u32, u32, u32)> = Vec::new();
    let mut s_lane_count = 0u32;
    let mut s_head_index: Option<u32> = None;
    let mut s_truncated = false;
    for c in &chunks {
        match c {
            GraphChunk::Meta { .. } => {}
            GraphChunk::Batch { nodes, edges, .. } => {
                for n in nodes {
                    s_ids.push(n.id.clone());
                    s_lanes.push(n.lane);
                }
                for e in edges {
                    s_edges.push((e.from, e.to, e.lane));
                }
            }
            GraphChunk::Done {
                lane_count,
                head_index,
                truncated,
                ..
            } => {
                s_lane_count = *lane_count;
                s_head_index = *head_index;
                s_truncated = *truncated;
            }
        }
    }
    s_edges.sort_unstable();

    assert_eq!(s_ids, ids(&oracle), "{label}: node ids");
    assert_eq!(
        s_lanes,
        oracle.nodes.iter().map(|n| n.lane).collect::<Vec<_>>(),
        "{label}: lanes"
    );
    assert_eq!(
        s_edges,
        oracle
            .edges
            .iter()
            .map(|e| (e.from, e.to, e.lane))
            .collect::<Vec<_>>(),
        "{label}: edges"
    );
    assert_eq!(s_lane_count, oracle.lane_count, "{label}: lane_count");
    assert_eq!(s_head_index, oracle.head_index, "{label}: head_index");
    assert_eq!(s_truncated, oracle.truncated, "{label}: truncated");
}

// ---- tests --------------------------------------------------------------------

/// Regression guard (locked): the default filter is byte-identical to the
/// unfiltered walk on a fixture with branches, a merge, a tag, and a stash.
#[test]
fn default_filter_is_a_byte_identical_regression() {
    let (dir, _repo) = kitchen_sink_fixture();
    let plain = compute_graph(dir.path()).expect("compute_graph");
    let filtered = compute_graph_with(dir.path(), &GraphFilter::default())
        .expect("compute_graph_with default");
    assert_eq!(plain, filtered, "default filter must change nothing");
    assert!(
        plain.nodes.iter().any(|n| n.summary == "s1"),
        "sanity: merge side present in the full graph"
    );
}

/// AC1 (merged ref DELETED): first-parent collapses to a single lane; the merge
/// commit stays, side nodes vanish; toggling off restores the full layout
/// byte-identically.
#[test]
fn first_parent_with_merged_ref_deleted() {
    let (dir, _repo, [a0, a1, s1, m]) = merge_fixture(false);
    let full = compute_graph(dir.path()).expect("full");
    assert_eq!(full.nodes.len(), 4);

    let l = compute_graph_with(dir.path(), &fp()).expect("first-parent");
    assert_eq!(
        ids(&l),
        vec![m.to_string(), a1.to_string(), a0.to_string()],
        "side commit absent; merge commit present"
    );
    assert!(!ids(&l).contains(&s1.to_string()));
    assert_eq!(l.lane_count, 1, "single lane");
    assert!(l.nodes.iter().all(|n| n.lane == 0));
    assert!(
        l.nodes.iter().all(|n| n.parents.len() <= 1),
        "no second-parent edges under first-parent"
    );

    let restored = compute_graph(dir.path()).expect("full again");
    assert_eq!(full, restored, "toggle off → byte-identical full layout");
}

/// Chosen semantics (documented): a merged side branch whose ref still exists
/// keeps its tip and its own first-parent line.
#[test]
fn first_parent_with_merged_ref_present_keeps_its_line() {
    let (dir, _repo, [a0, a1, s1, m]) = merge_fixture(true);
    let l = compute_graph_with(dir.path(), &fp()).expect("first-parent");
    let got = ids(&l);
    assert!(got.contains(&s1.to_string()), "live ref keeps its tip");
    assert_eq!(got.len(), 4, "all four commits still walked");
    // The merge edge m→s1 is gone; s1's own first-parent edge s1→a0 remains.
    let m_row = got.iter().position(|i| *i == m.to_string()).expect("m row") as u32;
    let s_row = got.iter().position(|i| *i == s1.to_string()).expect("s row") as u32;
    let a0_row = got.iter().position(|i| *i == a0.to_string()).expect("a0") as u32;
    let a1_row = got.iter().position(|i| *i == a1.to_string()).expect("a1") as u32;
    let pairs: Vec<(u32, u32)> = l.edges.iter().map(|e| (e.from, e.to)).collect();
    assert!(!pairs.contains(&(m_row, s_row)), "merge edge dropped");
    assert!(pairs.contains(&(m_row, a1_row)), "first-parent edge kept");
    assert!(pairs.contains(&(s_row, a0_row)), "side line kept");
}

/// AC3 solo: `seed_refs = [x]` with `y` checked out → ancestry(x) ∪
/// ancestry(HEAD); no pills for other refs; HEAD pill synthesized.
#[test]
fn solo_restricts_to_ref_plus_head_and_synthesizes_head_pill() {
    let (dir, _repo, [b0, x1, x2, y1, z1]) = multi_branch_fixture();
    let l = compute_graph_with(dir.path(), &seeds(&["refs/heads/x"])).expect("solo");

    let got = ids(&l);
    for want in [x2, x1, y1, b0] {
        assert!(got.contains(&want.to_string()), "ancestry keeps {want}");
    }
    assert!(!got.contains(&z1.to_string()), "z's line excluded");

    let pills = all_pill_names(&l);
    assert!(pills.contains(&"x".to_string()));
    assert!(pills.contains(&"HEAD".to_string()), "synthesized HEAD pill");
    for hidden in ["y", "z", "v1"] {
        assert!(!pills.contains(&hidden.to_string()), "{hidden} pill hidden");
    }
    // The synthesized label is a detached-style Head pill on the HEAD commit.
    let head_row = l.head_index.expect("head resolves") as usize;
    assert_eq!(l.nodes[head_row].id, y1.to_string());
    let head_ref = &l.nodes[head_row].refs[0];
    assert_eq!(head_ref.kind, RefKind::Head);
    assert!(head_ref.is_head);
}

/// Hide-all: `Some([])` → HEAD's ancestry only, applied=true, HEAD pill present.
#[test]
fn hide_all_seeds_head_only() {
    let (dir, _repo, [b0, _x1, _x2, y1, _z1]) = multi_branch_fixture();
    let filter = seeds(&[]);
    let l = compute_graph_with(dir.path(), &filter).expect("hide-all");
    assert_eq!(ids(&l), vec![y1.to_string(), b0.to_string()]);
    // The HEAD pill is the SYNTHESIZED detached-style label (the branch pill
    // is hidden too under hide-all).
    let head_ref = &l.nodes[l.head_index.expect("head resolves") as usize].refs[0];
    assert_eq!(head_ref.name, "HEAD");
    assert_eq!(head_ref.kind, RefKind::Head);
    assert!(head_ref.is_head);

    let seed = graph_seed(dir.path(), &filter).expect("seed");
    assert!(seed.seed_refs_applied, "hide-all is an APPLIED restriction");
}

/// Stale persistence: every name nonexistent → full graph, applied=false.
#[test]
fn all_stale_seed_refs_fall_back_to_full_graph() {
    let (dir, _repo, _oids) = multi_branch_fixture();
    let filter = seeds(&["refs/heads/nope", "refs/tags/gone"]);
    let full = compute_graph(dir.path()).expect("full");
    let l = compute_graph_with(dir.path(), &filter).expect("stale");
    assert_eq!(l, full, "stale whitelist == full graph");

    let seed = graph_seed(dir.path(), &filter).expect("seed");
    assert!(!seed.seed_refs_applied, "stale → NOT applied");
}

/// Partially stale: the valid subset applies; applied=true.
#[test]
fn partially_stale_seed_refs_apply_the_valid_subset() {
    let (dir, _repo, _oids) = multi_branch_fixture();
    let just_x = compute_graph_with(dir.path(), &seeds(&["refs/heads/x"])).expect("solo x");
    let with_stale =
        compute_graph_with(dir.path(), &seeds(&["refs/heads/x", "refs/heads/nope"]))
            .expect("partial stale");
    assert_eq!(with_stale, just_x, "stale names are simply ignored");

    let seed = graph_seed(dir.path(), &seeds(&["refs/heads/x", "refs/heads/nope"]))
        .expect("seed");
    assert!(seed.seed_refs_applied);
}

/// Stash tips: excluded from the seed when `seed_refs` is `Some`; still walked
/// (own node) under `first_parent` alone.
#[test]
fn stashes_excluded_under_seed_refs_but_kept_under_first_parent() {
    let (dir, _repo) = kitchen_sink_fixture();
    let full = compute_graph(dir.path()).expect("full");
    let stash_id = full
        .nodes
        .iter()
        .find(|n| n.refs.iter().any(|r| r.kind == RefKind::Stash))
        .map(|n| n.id.clone())
        .expect("full graph has the stash node");

    let solo = compute_graph_with(dir.path(), &seeds(&["refs/heads/main"])).expect("solo");
    assert!(
        !ids(&solo).contains(&stash_id),
        "stash tip excluded when seed_refs is Some"
    );

    let fp_only = compute_graph_with(dir.path(), &fp()).expect("first-parent");
    assert!(
        ids(&fp_only).contains(&stash_id),
        "stash node kept under first_parent alone"
    );
}

/// Meta flag matrix: `(filtered, seedRefsApplied)` per filter shape.
#[test]
fn meta_flags_matrix() {
    let (dir, _repo, _oids) = multi_branch_fixture();
    let cases: Vec<(GraphFilter, (bool, bool), &str)> = vec![
        (GraphFilter::default(), (false, false), "default"),
        (fp(), (true, false), "first-parent only"),
        (seeds(&["refs/heads/x"]), (true, true), "valid seedRefs"),
        (seeds(&[]), (true, true), "hide-all"),
        (seeds(&["refs/heads/nope"]), (false, false), "all stale"),
        (
            GraphFilter {
                first_parent: true,
                seed_refs: Some(vec!["refs/heads/nope".to_string()]),
                ..Default::default()
            },
            (true, false),
            "all stale + first-parent",
        ),
    ];
    for (filter, want, label) in cases {
        let chunks = capture_stream(dir.path(), &filter);
        assert_eq!(meta_flags(&chunks), want, "meta flags for {label}");
    }
}

/// Stream/one-shot parity under every filter shape (existing parity pattern,
/// parameterized by filter).
#[test]
fn stream_matches_oneshot_under_each_filter() {
    let (dir, _repo) = kitchen_sink_fixture();
    for (filter, label) in [
        (GraphFilter::default(), "default"),
        (fp(), "first-parent"),
        (seeds(&["refs/heads/main"]), "solo"),
        (seeds(&[]), "hide-all"),
        (seeds(&["refs/heads/nope"]), "stale"),
    ] {
        assert_stream_matches_oneshot(dir.path(), &filter, label);
    }
}

/// NIT-7: a whitelist name that RESOLVES but is skipped by the enumeration
/// (the symbolic `refs/remotes/origin/HEAD`) must count as stale — otherwise
/// `applied` would be true with an unintended HEAD-only graph.
#[test]
fn unenumerable_seed_ref_counts_as_stale() {
    let (dir, repo, _oids) = multi_branch_fixture();
    let x_tip = repo
        .find_branch("x", git2::BranchType::Local)
        .expect("branch x")
        .get()
        .target()
        .expect("x tip");
    repo.reference("refs/remotes/origin/main", x_tip, true, "remote main")
        .expect("remote ref");
    repo.reference_symbolic(
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
        true,
        "remote HEAD",
    )
    .expect("symbolic remote HEAD");

    let filter = seeds(&["refs/remotes/origin/HEAD"]);
    let seed = graph_seed(dir.path(), &filter).expect("seed");
    assert!(
        !seed.seed_refs_applied,
        "a resolvable-but-skipped ref must not count as a match"
    );
    let full = compute_graph(dir.path()).expect("full");
    let l = compute_graph_with(dir.path(), &filter).expect("filtered");
    assert_eq!(l, full, "falls back to the full graph");
}
