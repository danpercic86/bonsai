//! Lane-assignment and topology tests for the M2 graph fixtures (E1-E6).
//! Exercises `lane.rs`: row order, lane numbers, edge lanes, `lane_count`,
//! and determinism. Sibling of `tests_decorate.rs` / `tests_stream.rs`;
//! shared fixture builders live in `tests.rs`.

use super::{
    branch, build_criss_cross, commit, edge_tuples, ids, init_repo, lanes, parents, set_head,
};
use crate::graph::*;

/// E1 — linear chain (3 commits, branch `main` on tip, HEAD attached).
#[test]
fn linear_chain() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        vec![c2.to_string(), c1.to_string(), c0.to_string()]
    );
    assert_eq!(lanes(&l), vec![0, 0, 0]);
    assert_eq!(edge_tuples(&l), vec![(0, 1, 0), (1, 2, 0)]);
    assert_eq!(l.lane_count, 1);
    assert_eq!(l.head_index, Some(0));
    assert!(!l.truncated);
    assert_eq!(parents(&l), vec![vec![1], vec![2], vec![]]);
    assert_eq!(
        l.nodes[0].refs,
        vec![RefLabel {
            name: "main".to_string(),
            kind: RefKind::LocalBranch,
            is_head: true,
        }]
    );
    assert!(l.nodes[1].refs.is_empty());
    assert!(l.nodes[2].refs.is_empty());
    assert_eq!(l.nodes[0].summary, "C2");
    assert_eq!(l.nodes[0].author, "Test User");
    assert_eq!(l.nodes[0].ts, 3);
    // P51: committer time == author time here (the `commit` helper signs
    // both with the same signature).
    assert_eq!(l.nodes[0].committer_ts, 3);
}

/// P51 — `committer_ts` is populated from the COMMITTER signature and is
/// distinct from `ts` (the author time) when the two differ, as after a
/// rebase/amend. Proves the node reads the committer, not the author.
#[test]
fn committer_ts_reads_committer_time() {
    let (dir, repo) = init_repo();
    let author = git2::Signature::new("Author", "a@example.com", &git2::Time::new(100, 0))
        .expect("author signature");
    let committer = git2::Signature::new("Committer", "c@example.com", &git2::Time::new(500, 0))
        .expect("committer signature");
    let blob = repo.blob(b"x").expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("tree insert");
    let tree = repo
        .find_tree(tb.write().expect("write tree"))
        .expect("find tree");
    let oid = repo
        .commit(None, &author, &committer, "C0", &tree, &[])
        .expect("commit");
    branch(&repo, "main", oid);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(l.nodes[0].ts, 100, "ts is the author time");
    assert_eq!(
        l.nodes[0].committer_ts, 500,
        "committer_ts is the committer time"
    );
}

/// E2 — fork + merge: M{C3,F2} F2{F1} C3{C2} F1{C1} C2{C1} C1{C0} C0{}.
#[test]
fn fork_merge() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    let f1 = commit(&repo, "F1", &[c1], 4);
    let c3 = commit(&repo, "C3", &[c2], 5);
    let f2 = commit(&repo, "F2", &[f1], 6);
    let m = commit(&repo, "M", &[c3, f2], 7);
    branch(&repo, "main", m);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        [m, f2, c3, f1, c2, c1, c0]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(lanes(&l), vec![0, 1, 0, 1, 0, 0, 0]);
    assert_eq!(
        edge_tuples(&l),
        vec![
            (0, 1, 1),
            (0, 2, 0),
            (1, 3, 1),
            (2, 4, 0),
            (3, 5, 1),
            (4, 5, 0),
            (5, 6, 0),
        ]
    );
    assert_eq!(l.lane_count, 2);
    assert!(!l.truncated);
}

/// E3 — two parallel branches, no merge (walk order C3,T2,C2,T1,C1).
#[test]
fn parallel_branches() {
    let (dir, repo) = init_repo();
    let c1 = commit(&repo, "C1", &[], 1);
    let t1 = commit(&repo, "T1", &[c1], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    let t2 = commit(&repo, "T2", &[t1], 4);
    let c3 = commit(&repo, "C3", &[c2], 5);
    branch(&repo, "main", c3);
    branch(&repo, "topic", t2);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        [c3, t2, c2, t1, c1]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(lanes(&l), vec![0, 1, 0, 1, 0]);
    assert_eq!(
        edge_tuples(&l),
        vec![(0, 2, 0), (1, 3, 1), (2, 4, 0), (3, 4, 1)]
    );
    assert_eq!(l.lane_count, 2);
}

/// E4 — criss-cross: A2{A1,B1} B2{B1,A1} A1{R} B1{R} R{}.
/// Includes the general `edge.lane ∉ {fromLane, toLane}` shapes:
/// (1,2,0) fromLane 2 → toLane 0, and (1,3,2) fromLane 2, toLane 1.
#[test]
fn criss_cross() {
    let (dir, repo) = init_repo();
    let (l, oids) = build_criss_cross(&repo, dir.path());
    let [a2, b2, a1, b1, r] = oids;

    assert_eq!(
        ids(&l),
        [a2, b2, a1, b1, r]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(lanes(&l), vec![0, 2, 0, 1, 0]);
    assert_eq!(
        edge_tuples(&l),
        vec![
            (0, 2, 0),
            (0, 3, 1),
            (1, 2, 0),
            (1, 3, 2),
            (2, 4, 0),
            (3, 4, 1),
        ]
    );
    assert_eq!(l.lane_count, 3);
}

/// E5 — octopus merge (3 parents): M{A,B,C}, each linear to root R.
#[test]
fn octopus_merge() {
    let (dir, repo) = init_repo();
    let r = commit(&repo, "R", &[], 1);
    let c = commit(&repo, "C", &[r], 2);
    let b = commit(&repo, "B", &[r], 3);
    let a = commit(&repo, "A", &[r], 4);
    let m = commit(&repo, "M", &[a, b, c], 5);
    branch(&repo, "main", m);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        [m, a, b, c, r]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(l.nodes[0].parents.len(), 3);
    assert_eq!(l.nodes[0].parents, vec![1, 2, 3]);
    assert_eq!(lanes(&l), vec![0, 0, 1, 2, 0]);
    // Three edges out of r0 with lanes 0, 1, 2.
    assert_eq!(
        edge_tuples(&l),
        vec![
            (0, 1, 0),
            (0, 2, 1),
            (0, 3, 2),
            (1, 4, 0),
            (2, 4, 1),
            (3, 4, 2),
        ]
    );
    assert_eq!(l.lane_count, 3);
}

/// E6 — two orphan roots: main chain + disconnected `pages` (older times).
/// The second component reuses freed lane 0; NO edge crosses components.
#[test]
fn two_orphan_roots() {
    let (dir, repo) = init_repo();
    let p0 = commit(&repo, "P0", &[], 1);
    let p1 = commit(&repo, "P1", &[p0], 2);
    let c0 = commit(&repo, "C0", &[], 3);
    let c1 = commit(&repo, "C1", &[c0], 4);
    let c2 = commit(&repo, "C2", &[c1], 5);
    branch(&repo, "main", c2);
    branch(&repo, "pages", p1);
    set_head(&repo, "main");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        [c2, c1, c0, p1, p0]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(lanes(&l), vec![0, 0, 0, 0, 0]);
    assert_eq!(edge_tuples(&l), vec![(0, 1, 0), (1, 2, 0), (3, 4, 0)]);
    assert_eq!(l.lane_count, 1);
    // No edge between the two components (main rows 0..=2, pages 3..=4).
    assert!(!l.edges.iter().any(|e| e.from <= 2 && e.to >= 3));
}

/// `Repository::init` only → empty layout, Ok, not Err.
#[test]
fn unborn_repo() {
    let (dir, _repo) = init_repo();

    let l = compute_graph(dir.path()).expect("compute_graph on unborn repo");
    assert!(l.nodes.is_empty());
    assert!(l.edges.is_empty());
    assert_eq!(l.lane_count, 0);
    assert_eq!(l.head_index, None);
    assert!(!l.truncated);
}

/// Same repo state → identical layout (lane-color stability rule).
#[test]
fn determinism() {
    let (dir, repo) = init_repo();
    let (first, _) = build_criss_cross(&repo, dir.path());
    let second = compute_graph(dir.path()).expect("compute_graph again");
    assert_eq!(first, second);
}
