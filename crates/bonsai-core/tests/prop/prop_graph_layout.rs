//! T5 property suite (contract §2.1): structural invariants of
//! `graph::compute_graph` over random [`RepoShape`]s. 64 cases total;
//! `PROPTEST_CASES=<n>` overrides the baked per-band literal at runtime
//! (`PROPTEST_CASES=256` for the exhaustive local run).
//!
//! WALL-CLOCK NOTE: the whole 64-case run used to live in ONE test fn, and
//! nextest parallelizes across test *functions*, not across cases inside a
//! `proptest!` block — so that one fn was the suite's critical path (~57s).
//! The commit-count axis (`1..=200`) is therefore partitioned into 8 DISJOINT
//! bands that TILE `1..=200` exactly, one test fn each, via
//! `prop_common::repo_shape_sized`. Every band runs the identical body (all of
//! items 1-6 plus the item-7 determinism recompute), so no assertion is
//! dropped. Band widths follow a sqrt schedule and cases are allocated
//! PROPORTIONAL TO BAND WIDTH, which (a) keeps the marginal distribution over
//! commit count ~uniform, exactly as the single fn had it, (b) keeps the case
//! total at 64 and total CPU unchanged, and (c) equalizes per-band cost
//! (cost ∝ cases × mean commits), so every fn lands well under the 15s target.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use bonsai_core::graph::{compute_graph, GraphLayout, RefKind};
use proptest::prelude::*;

use crate::prop_common;
use crate::prop_common::{build_repo, repo_shape_sized, HeadSpec, RepoShape};

/// All structural invariants (contract §2.1 items 1-7) for one built repo.
fn assert_invariants(shape: &RepoShape, path: &Path, layout: &GraphLayout) {
    let repo = git2::Repository::open(path).expect("open");
    let n = layout.nodes.len();

    // (1) Node bijection: one node per commit, ids unique, == the reachable set.
    assert_eq!(n, shape.commits.len(), "node count == commit count");
    let ids: HashSet<&str> = layout.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(ids.len(), n, "node ids are unique");

    let index_of: HashMap<&str, u32> = layout
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i as u32))
        .collect();

    for (r, node) in layout.nodes.iter().enumerate() {
        let r = r as u32;
        // (2) Topological order: every parent is strictly BELOW its child.
        for &p in &node.parents {
            assert!(p > r, "parent {p} not below child row {r}");
            assert!((p as usize) < n, "parent index in range");
        }
        // (3) Parent truth: node.parents map, in order, to the real git parents.
        let oid = git2::Oid::from_str(&node.id).expect("oid");
        let commit = repo.find_commit(oid).expect("find commit");
        let real: Vec<String> = commit.parent_ids().map(|o| o.to_string()).collect();
        let via_layout: Vec<String> = node
            .parents
            .iter()
            .map(|p| layout.nodes[*p as usize].id.clone())
            .collect();
        assert_eq!(via_layout, real, "parents match real git parents in order");
    }

    // (4) Lanes dense + bounded.
    let mut used: BTreeSet<u32> = BTreeSet::new();
    for node in &layout.nodes {
        assert!(node.lane < layout.lane_count, "node lane < lane_count");
        used.insert(node.lane);
    }
    for e in &layout.edges {
        assert!(e.lane < layout.lane_count, "edge lane < lane_count");
        used.insert(e.lane);
    }
    if n == 0 {
        assert_eq!(layout.lane_count, 0);
    } else {
        let max = *used.iter().max().unwrap();
        assert_eq!(layout.lane_count, max + 1, "lane_count == max used lane + 1");
        let full: BTreeSet<u32> = (0..layout.lane_count).collect();
        assert_eq!(used, full, "every lane below lane_count is used (density)");
    }

    // (5) Edges well-formed: bounds, sorted, and set-equal to parent links.
    let mut sorted = layout.edges.clone();
    sorted.sort_by_key(|e| (e.from, e.to));
    assert_eq!(
        layout.edges.iter().map(|e| (e.from, e.to)).collect::<Vec<_>>(),
        sorted.iter().map(|e| (e.from, e.to)).collect::<Vec<_>>(),
        "edges sorted ascending by (from,to)"
    );
    let edge_pairs: HashSet<(u32, u32)> = layout
        .edges
        .iter()
        .map(|e| {
            assert!(e.from < e.to && (e.to as usize) < n, "edge from<to<len");
            (e.from, e.to)
        })
        .collect();
    let parent_pairs: HashSet<(u32, u32)> = layout
        .nodes
        .iter()
        .enumerate()
        .flat_map(|(r, node)| node.parents.iter().map(move |p| (r as u32, *p)))
        .collect();
    assert_eq!(edge_pairs, parent_pairs, "edge set == parent-link set");

    // (6) head_index points at the HEAD oid; detached HEAD ⇒ exactly one Head.
    let head = repo.head().ok();
    let head_oid = head.as_ref().and_then(|h| h.target()).map(|o| o.to_string());
    match (&layout.head_index, &head_oid) {
        (Some(i), Some(h)) => assert_eq!(&layout.nodes[*i as usize].id, h),
        (None, None) => {}
        other => panic!("head_index / HEAD mismatch: {other:?}"),
    }
    let head_labels = layout
        .nodes
        .iter()
        .flat_map(|node| node.refs.iter())
        .filter(|r| r.kind == RefKind::Head)
        .count();
    if matches!(shape.head, HeadSpec::Detached(_)) {
        assert_eq!(head_labels, 1, "detached HEAD ⇒ exactly one Head label");
        let has_head_flag = layout
            .nodes
            .iter()
            .flat_map(|node| node.refs.iter())
            .any(|r| r.kind == RefKind::Head && r.is_head);
        assert!(has_head_flag, "detached Head label carries is_head");
    } else {
        assert_eq!(head_labels, 0, "attached HEAD ⇒ no detached Head label");
    }

    let _ = index_of; // referenced for clarity; kept out of the hot asserts
}

/// One band of the commit-count partition: `$lo..=$hi` commits, `$cases`
/// cases. The body is the original `graph_invariants_and_determinism` body,
/// verbatim: build the repo, assert every structural invariant (items 1-6) via
/// the shared [`assert_invariants`], then recompute and assert byte-identical
/// output (item 7 — determinism).
macro_rules! band {
    ($name:ident, $lo:expr, $hi:expr, $cases:expr) => {
        proptest! {
            #![proptest_config(ProptestConfig { cases: $cases, ..ProptestConfig::default() })]

            #[test]
            fn $name(shape in repo_shape_sized($lo, $hi)) {
                let (dir, path) = build_repo(&shape);
                let first = compute_graph(&path).expect("compute_graph");
                assert_invariants(&shape, &path, &first);
                let second = compute_graph(&path).expect("compute_graph again");
                prop_assert_eq!(&first, &second, "same repo ⇒ identical layout");
                drop(dir);
            }
        }
    };
}

// 8 bands tiling 1..=200 (widths 71/29/22/19/17/15/14/13 = 200) with cases
// proportional to width (23/9/7/6/5/5/5/4 = 64).
band!(graph_invariants_and_determinism_b1_001_071, 1, 71, 23);
band!(graph_invariants_and_determinism_b2_072_100, 72, 100, 9);
band!(graph_invariants_and_determinism_b3_101_122, 101, 122, 7);
band!(graph_invariants_and_determinism_b4_123_141, 123, 141, 6);
band!(graph_invariants_and_determinism_b5_142_158, 142, 158, 5);
band!(graph_invariants_and_determinism_b6_159_173, 159, 173, 5);
band!(graph_invariants_and_determinism_b7_174_187, 174, 187, 5);
band!(graph_invariants_and_determinism_b8_188_200, 188, 200, 4);

// ---- F-T5-1: lane stability under HEAD append (DEMOTED to a pinned finding) --
//
// Contract §2.1 item 8 / §8.1 flagged the "appending a commit to the HEAD
// branch does not shift the lane of any commit outside HEAD's lane" clause as
// unproven. A 64-case proptest immediately found counterexamples: appending a
// commit re-seeds the whole topological+time walk, so lane assignment (which is
// order-of-emission + first-free) can reshuffle unrelated lanes. The PROVABLE
// invariant (item 7: identical input ⇒ identical layout) is asserted above.
// This clause is NOT silently weakened: it is pinned here as a FINDING for the
// orchestrator (FINDINGS.md F-T5-1) with a minimal deterministic reproduction.
//
// NOTE: this is scroll/color stability under a NEW COMMIT (full recompute), not
// under scrolling a fixed layout (which never recomputes). Whether it is a real
// UX regression is the orchestrator's call — the test only pins the behavior.

/// Minimal reproduction (6 commits): a two-branch fork where the side branch
/// `feat` has a LATER timestamp than `main`'s tip. Appending one commit to the
/// HEAD branch (`main`) shifts `feat`'s tip from lane 0 to lane 1 even though it
/// was never in HEAD's lane.
#[test]
fn regression_f_t5_1_lane_shift_on_head_append() {
    let dir = prop_common::common::scratch_dir();
    let repo = git2::Repository::init_opts(
        dir.path(),
        git2::RepositoryInitOptions::new().initial_head("main"),
    )
    .expect("init");
    {
        let mut cfg = repo.config().expect("cfg");
        cfg.set_str("user.name", "Prop Bot").unwrap();
        cfg.set_str("user.email", "prop@bonsai.local").unwrap();
    }
    let mk = |parents: &[git2::Oid], t: i64, tag: &str| {
        let sig = git2::Signature::new("Prop Bot", "prop@bonsai.local", &git2::Time::new(t, 0)).unwrap();
        let blob = repo.blob(tag.as_bytes()).unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        tb.insert("n.txt", blob, 0o100_644).unwrap();
        let tree = repo.find_tree(tb.write().unwrap()).unwrap();
        let pcs: Vec<git2::Commit> = parents.iter().map(|p| repo.find_commit(*p).unwrap()).collect();
        let prefs: Vec<&git2::Commit> = pcs.iter().collect();
        repo.commit(None, &sig, &sig, tag, &tree, &prefs).unwrap()
    };
    let c0 = mk(&[], 1, "c0");
    let c1 = mk(&[c0], 2, "c1");
    let c2 = mk(&[c1], 3, "c2"); // main tip
    let c3 = mk(&[c0], 5, "c3"); // feat tip — LATER than main
    repo.reference("refs/heads/main", c2, true, "").unwrap();
    repo.reference("refs/heads/feat", c3, true, "").unwrap();
    repo.set_head("refs/heads/main").unwrap();

    let before = compute_graph(dir.path()).expect("before");
    let head_lane = before.nodes[before.head_index.unwrap() as usize].lane;
    let lane_of = |l: &GraphLayout, id: &str| l.nodes.iter().find(|n| n.id == id).map(|n| n.lane);
    let c3_before = lane_of(&before, &c3.to_string()).unwrap();
    assert_ne!(c3_before, head_lane, "feat tip starts outside HEAD's lane");

    // Append one commit to main (HEAD) with the latest timestamp.
    let c4 = mk(&[c2], 6, "c4");
    repo.reference("refs/heads/main", c4, true, "").unwrap();

    let after = compute_graph(dir.path()).expect("after");
    let c3_after = lane_of(&after, &c3.to_string()).unwrap();

    // PINNED FINDING F-T5-1: feat's tip lane changed on an unrelated append.
    assert_ne!(
        c3_after, c3_before,
        "F-T5-1: expected the (known) lane shift on HEAD append to reproduce"
    );
}
