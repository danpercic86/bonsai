use super::*;

/// Initializes a repo in a fresh temp dir with local user config set.
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

/// Creates a commit from an in-memory tree with an EXPLICIT timestamp
/// (walk-order determinism depends on distinct times). No ref is updated.
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

fn ids(l: &GraphLayout) -> Vec<String> {
    l.nodes.iter().map(|n| n.id.clone()).collect()
}

fn lanes(l: &GraphLayout) -> Vec<u32> {
    l.nodes.iter().map(|n| n.lane).collect()
}

fn edge_tuples(l: &GraphLayout) -> Vec<(u32, u32, u32)> {
    l.edges.iter().map(|e| (e.from, e.to, e.lane)).collect()
}

fn parents(l: &GraphLayout) -> Vec<Vec<u32>> {
    l.nodes.iter().map(|n| n.parents.clone()).collect()
}

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
    let committer =
        git2::Signature::new("Committer", "c@example.com", &git2::Time::new(500, 0))
            .expect("committer signature");
    let blob = repo.blob(b"x").expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("tree insert");
    let tree = repo.find_tree(tb.write().expect("write tree")).expect("find tree");
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

/// Builds the E4 fixture in `repo` and computes its layout.
/// Returns the layout plus `[a2, b2, a1, b1, r]`.
fn build_criss_cross(
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

/// One commit that is simultaneously local branch tip (HEAD attached),
/// remote-tracking `origin/main`, lightweight tag, and annotated tag.
/// Label order per §2.2 pill_order; `is_head` only on the local branch.
#[test]
fn ref_pills_stacking() {
    let (dir, repo) = init_repo();
    let c = commit(&repo, "C", &[], 1);
    branch(&repo, "main", c);
    set_head(&repo, "main");
    repo.reference("refs/remotes/origin/main", c, true, "test")
        .expect("create remote ref");
    let obj = repo.find_object(c, None).expect("find object");
    repo.tag_lightweight("v1.0", &obj, true)
        .expect("lightweight tag");
    let sig =
        git2::Signature::new("Test User", "test@example.com", &git2::Time::new(2, 0))
            .expect("signature");
    repo.tag("v1.1-notes", &obj, &sig, "annotated notes tag", true)
        .expect("annotated tag");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(l.nodes.len(), 1);
    assert_eq!(l.head_index, Some(0));
    assert_eq!(
        l.nodes[0].refs,
        vec![
            RefLabel {
                name: "main".to_string(),
                kind: RefKind::LocalBranch,
                is_head: true,
            },
            RefLabel {
                name: "origin/main".to_string(),
                kind: RefKind::RemoteBranch,
                is_head: false,
            },
            RefLabel {
                name: "v1.0".to_string(),
                kind: RefKind::Tag,
                is_head: false,
            },
            RefLabel {
                name: "v1.1-notes".to_string(),
                kind: RefKind::Tag,
                is_head: false,
            },
        ]
    );
}

/// Detached HEAD on a mid-history commit gets a Head label; `head_index`
/// points at it; no Head label anywhere else; the branch loses `is_head`.
#[test]
fn detached_head() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    repo.set_head_detached(c1).expect("detach HEAD");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        [c2, c1, c0]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(l.head_index, Some(1));
    assert_eq!(
        l.nodes[1].refs,
        vec![RefLabel {
            name: "HEAD".to_string(),
            kind: RefKind::Head,
            is_head: true,
        }]
    );
    // Branch pill still present on the tip, but not marked HEAD.
    assert_eq!(
        l.nodes[0].refs,
        vec![RefLabel {
            name: "main".to_string(),
            kind: RefKind::LocalBranch,
            is_head: false,
        }]
    );
    // No Head label anywhere else.
    let head_labels = l
        .nodes
        .iter()
        .flat_map(|n| n.refs.iter())
        .filter(|r| r.kind == RefKind::Head)
        .count();
    assert_eq!(head_labels, 1);
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

/// A tag object pointing at a blob is ignored — no label, no panic.
#[test]
fn annotated_tag_to_blob_skipped() {
    let (dir, repo) = init_repo();
    let c = commit(&repo, "C", &[], 1);
    branch(&repo, "main", c);
    set_head(&repo, "main");
    let blob = repo.blob(b"just a blob").expect("blob");
    let obj = repo.find_object(blob, None).expect("find blob object");
    let sig =
        git2::Signature::new("Test User", "test@example.com", &git2::Time::new(2, 0))
            .expect("signature");
    repo.tag("blob-tag", &obj, &sig, "tag on a blob", false)
        .expect("tag blob");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(l.nodes.len(), 1);
    assert!(l.nodes[0]
        .refs
        .iter()
        .all(|r| r.kind != RefKind::Tag));
}

// ---------- P10: stash as its own graph node ----------

/// Init a repo with a real worktree + local user config.
fn init_worktree_repo() -> (tempfile::TempDir, git2::Repository) {
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

/// Writes `content` to `path` in the worktree, stages it, and commits it on
/// the current HEAD branch (updating HEAD). Returns the new commit oid.
fn commit_file(repo: &git2::Repository, path: &str, content: &str, msg: &str) -> git2::Oid {
    let workdir = repo.workdir().expect("workdir");
    std::fs::write(workdir.join(path), content).expect("write file");
    let mut index = repo.index().expect("open index");
    index
        .add_path(std::path::Path::new(path))
        .expect("stage file");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("signature");
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|o| repo.find_commit(o).ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
        .expect("commit")
}

/// Force-checks out `refname` (`refs/heads/...`) and moves HEAD onto it.
fn checkout_ref(repo: &git2::Repository, refname: &str) {
    repo.set_head(refname).expect("set head");
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_head(Some(&mut co)).expect("checkout head");
}

fn stash_labels(l: &GraphLayout) -> Vec<String> {
    l.nodes
        .iter()
        .flat_map(|n| n.refs.iter())
        .filter(|r| r.kind == RefKind::Stash)
        .map(|r| r.name.clone())
        .collect()
}

/// A stash renders as its OWN node `W` carrying the `stash@{n}` pill, linked
/// by a single edge to its base `B`; the base no longer carries the pill.
/// The synthetic index commit `I` is hidden (never emitted). An orphaned
/// base is now PULLED INTO the walk by the stash node (reversed P9b rule).
#[test]
fn stash_appears_as_own_node() {
    // --- Scenario 1: stash on a branch tip → own node, single base edge,
    //     no synthetic nodes, deterministic. ---
    {
        let (dir, repo) = init_worktree_repo();
        commit_file(&repo, "f.txt", "v0", "C0");
        let c1 = commit_file(&repo, "f.txt", "v1", "C1"); // HEAD tip == base
        std::fs::write(dir.path().join("f.txt"), "v2-dirty").expect("dirty");
        let res =
            crate::git::stash::create_stash(dir.path(), None, crate::git::stash::StashScope::All).expect("create_stash");
        assert!(res.created, "worktree was dirty → a stash must be created");

        let l = compute_graph(dir.path()).expect("compute_graph");

        // Exactly one node carries a Stash label `stash@{0}`.
        let stash_rows: Vec<usize> = l
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.refs.iter().any(|r| r.kind == RefKind::Stash))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(stash_rows.len(), 1, "exactly one stash node");
        let sr = stash_rows[0];
        assert_eq!(stash_labels(&l), vec!["stash@{0}".to_string()]);

        // Its summary is git's default "WIP on <branch>: …".
        assert!(
            l.nodes[sr].summary.starts_with("WIP"),
            "stash node summary starts with WIP, got {:?}",
            l.nodes[sr].summary
        );

        // Single parent resolving to the base row whose id == c1.
        assert_eq!(l.nodes[sr].parents.len(), 1, "single parent (base only)");
        let base_row = l.nodes[sr].parents[0] as usize;
        assert_eq!(l.nodes[base_row].id, c1.to_string());

        // The base row still exists and carries NO stash label.
        assert!(
            !l.nodes[base_row]
                .refs
                .iter()
                .any(|r| r.kind == RefKind::Stash),
            "stash pill moved off the base"
        );

        // Exactly one edge originates at `sr`, points to the base row, on
        // the stash node's own (offshoot) lane.
        let sr_u = sr as u32;
        let out_edges: Vec<&GraphEdge> =
            l.edges.iter().filter(|e| e.from == sr_u).collect();
        assert_eq!(out_edges.len(), 1, "one edge out of the stash node");
        assert_eq!(out_edges[0].to, base_row as u32);
        assert_eq!(out_edges[0].lane, l.nodes[sr].lane, "offshoot lane");

        // No synthetic nodes: C0, C1, W reachable; I hidden → 3 nodes.
        assert_eq!(l.nodes.len(), 3, "index commit I must not be emitted");

        // Determinism.
        let l2 = compute_graph(dir.path()).expect("compute_graph again");
        assert_eq!(l, l2);
    }

    // --- Scenario 2: two stashes on the SAME base → two distinct stash
    //     nodes, each on that base. ---
    {
        let (dir, repo) = init_worktree_repo();
        commit_file(&repo, "f.txt", "v0", "C0");
        let c1 = commit_file(&repo, "f.txt", "v1", "C1"); // base, HEAD stays

        std::fs::write(dir.path().join("f.txt"), "edit-a").expect("dirty a");
        assert!(crate::git::stash::create_stash(dir.path(), None, crate::git::stash::StashScope::All)
            .expect("create_stash a")
            .created); // becomes stash@{1}
        std::fs::write(dir.path().join("f.txt"), "edit-b").expect("dirty b");
        assert!(crate::git::stash::create_stash(dir.path(), None, crate::git::stash::StashScope::All)
            .expect("create_stash b")
            .created); // stash@{0}

        let l = compute_graph(dir.path()).expect("compute_graph");

        let mut names = stash_labels(&l);
        names.sort();
        assert_eq!(
            names,
            vec!["stash@{0}".to_string(), "stash@{1}".to_string()]
        );

        // Exactly two distinct nodes carry a Stash label.
        let stash_rows: Vec<usize> = l
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.refs.iter().any(|r| r.kind == RefKind::Stash))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(stash_rows.len(), 2, "two distinct stash nodes");

        // Each has a single parent resolving to the same base row (c1).
        let base_row = l
            .nodes
            .iter()
            .position(|n| n.id == c1.to_string())
            .expect("base present");
        for &sr in &stash_rows {
            assert_eq!(l.nodes[sr].parents.len(), 1);
            assert_eq!(l.nodes[sr].parents[0] as usize, base_row);
        }

        // The base carries no stash label.
        assert!(
            !l.nodes[base_row]
                .refs
                .iter()
                .any(|r| r.kind == RefKind::Stash),
            "base carries no stash pill"
        );
    }

    // --- Scenario 3: orphaned base → stash node PRESENT and base now
    //     present (reversed P9b rule). ---
    {
        let (dir, repo) = init_worktree_repo();
        commit_file(&repo, "f.txt", "v0", "C0");
        let c1 = commit_file(&repo, "f.txt", "v1", "C1");
        let main_ref = repo
            .head()
            .expect("head")
            .name()
            .expect("head ref name")
            .to_string();

        // Branch off c1 onto `temp`, commit X there, stash on X.
        let c1_commit = repo.find_commit(c1).expect("find c1");
        repo.branch("temp", &c1_commit, false).expect("create temp");
        checkout_ref(&repo, "refs/heads/temp");
        let x = commit_file(&repo, "f.txt", "vX", "X"); // base-to-be
        std::fs::write(dir.path().join("f.txt"), "vX-dirty").expect("dirty");
        let res =
            crate::git::stash::create_stash(dir.path(), None, crate::git::stash::StashScope::All).expect("create_stash");
        assert!(res.created);

        // Return to main and delete `temp` → X unreachable from any branch.
        checkout_ref(&repo, &main_ref);
        repo.find_branch("temp", git2::BranchType::Local)
            .expect("find temp")
            .delete()
            .expect("delete temp");

        let l = compute_graph(dir.path()).expect("compute_graph");

        // A stash node exists.
        assert_eq!(stash_labels(&l), vec!["stash@{0}".to_string()]);
        let sr = l
            .nodes
            .iter()
            .position(|n| n.refs.iter().any(|r| r.kind == RefKind::Stash))
            .expect("stash node present");

        // X is now PULLED INTO the walk (reversed from P9b).
        assert!(
            l.nodes.iter().any(|n| n.id == x.to_string()),
            "orphaned base X now pulled in by the stash node"
        );

        // The stash node's single parent resolves to the X row.
        assert_eq!(l.nodes[sr].parents.len(), 1);
        let base_row = l.nodes[sr].parents[0] as usize;
        assert_eq!(l.nodes[base_row].id, x.to_string());
    }
}

// ---------- P65a: streaming lane-stability (equivalence) ----------

/// Runs the streaming walk with the batch/cap constants forced to the given
/// values and captures every emitted chunk, in wire order.
fn capture_stream(
    dir: &std::path::Path,
    first: usize,
    batch: usize,
    max: usize,
) -> Vec<GraphChunk> {
    let mut chunks: Vec<GraphChunk> = Vec::new();
    super::stream::stream_graph_core_with(dir, first, batch, max, |c| {
        chunks.push(c);
        true
    })
    .expect("stream_graph_core_with");
    chunks
}

/// Folds a `Meta -> Batch* -> Done` chunk sequence back into a `GraphLayout`,
/// exactly as the P65b frontend assembler will (§4.2): nodes pushed in row
/// order, `parents` rebuilt from each edge's `from`/`to`/`ord`. Test-only
/// mirror that lets us assert the streamed walk reproduces `compute_graph`.
fn assemble(chunks: &[GraphChunk]) -> GraphLayout {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    // Per node: (ord, parent_row) pairs, to rebuild ordered `parents`.
    let mut parent_edges: Vec<Vec<(u16, u32)>> = Vec::new();
    let mut oid_to_row: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut meta_head_oid: Option<String> = None;
    let mut lane_count = 0u32;
    let mut head_index: Option<u32> = None;
    let mut truncated = false;

    for chunk in chunks {
        match chunk {
            GraphChunk::Meta { head_oid, .. } => {
                meta_head_oid = head_oid.clone();
            }
            GraphChunk::Batch {
                start_row,
                lane_count_so_far,
                nodes: bn,
                edges: be,
            } => {
                assert_eq!(
                    *start_row as usize,
                    nodes.len(),
                    "batch start_row must be contiguous with prior rows"
                );
                for sn in bn {
                    let row = nodes.len() as u32;
                    oid_to_row.insert(sn.id.clone(), row);
                    nodes.push(GraphNode {
                        id: sn.id.clone(),
                        lane: sn.lane,
                        parents: Vec::new(), // filled after the full stream
                        refs: sn.refs.clone(),
                        summary: sn.summary.clone(),
                        author: sn.author.clone(),
                        ts: sn.ts,
                        committer_ts: sn.committer_ts,
                    });
                    parent_edges.push(Vec::new());
                }
                for se in be {
                    edges.push(GraphEdge {
                        from: se.from,
                        to: se.to,
                        lane: se.lane,
                    });
                    parent_edges[se.from as usize].push((se.ord, se.to));
                }
                lane_count = lane_count.max(*lane_count_so_far);
            }
            GraphChunk::Done {
                total_rows,
                lane_count: lc,
                head_index: hi,
                truncated: tr,
            } => {
                assert_eq!(
                    *total_rows as usize,
                    nodes.len(),
                    "Done.total_rows must equal the emitted node count"
                );
                lane_count = *lc;
                head_index = *hi;
                truncated = *tr;
            }
        }
    }

    // Rebuild ordered parents: sort each node's edges by `ord`, take the
    // parent rows. A parent dropped by truncation simply has no edge and is
    // skipped — identical COMPACTION to compute_graph's `index_of`
    // filter_map on a complete walk (contract §3.1).
    for (node, pe) in nodes.iter_mut().zip(parent_edges.iter_mut()) {
        pe.sort_by_key(|(ord, _)| *ord);
        node.parents = pe.iter().map(|(_, to)| *to).collect();
    }

    // The Meta.head_oid → row resolution must agree with Done.head_index
    // (the frontend resolves head from head_oid; §4.2).
    let head_from_oid = meta_head_oid.and_then(|h| oid_to_row.get(&h).copied());
    assert_eq!(
        head_from_oid, head_index,
        "Meta.head_oid resolves to the same row as Done.head_index"
    );

    GraphLayout {
        nodes,
        edges,
        lane_count,
        head_index,
        truncated,
    }
}

/// Byte-identity assertion: nodes (incl. ordered `parents`), edges AS A SET,
/// `lane_count`, `head_index`, `truncated`.
fn assert_layouts_eq(got: &GraphLayout, want: &GraphLayout, label: &str, bs: usize) {
    assert_eq!(got.nodes, want.nodes, "{label} (batch={bs}): nodes");
    assert_eq!(
        got.lane_count, want.lane_count,
        "{label} (batch={bs}): lane_count"
    );
    assert_eq!(
        got.head_index, want.head_index,
        "{label} (batch={bs}): head_index"
    );
    assert_eq!(
        got.truncated, want.truncated,
        "{label} (batch={bs}): truncated"
    );
    let mut ge = got.edges.clone();
    let mut we = want.edges.clone();
    ge.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
    we.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
    assert_eq!(ge, we, "{label} (batch={bs}): edges as a set");
}

/// Forced batch sizes for the equivalence sweep. Varying these proves batch
/// boundaries never move a lane / color (the key P65 test, §7 item 1).
const BATCH_SIZES: [usize; 5] = [1, 2, 3, 7, 512];

/// Streams `dir` at every `BATCH_SIZES` value and asserts each assembled
/// layout is byte-identical to `compute_graph`'s one-shot output.
fn check_equivalence(label: &str, dir: &std::path::Path) {
    let oracle = compute_graph(dir).expect("compute_graph");
    assert!(
        !oracle.truncated,
        "{label}: fixture must be a complete (non-truncated) walk"
    );
    for &bs in &BATCH_SIZES {
        let chunks = capture_stream(dir, bs, bs, STREAM_MAX_COMMITS);
        let assembled = assemble(&chunks);
        assert_layouts_eq(&assembled, &oracle, label, bs);
    }
}

/// The crux: the streamed walk reproduces `compute_graph` byte-for-byte on
/// every M2 fixture (E1–E6) AND a mid-size generated fixture, at five batch
/// sizes. If this fails, the `LaneWalker` extraction changed lane behavior.
#[test]
fn stream_matches_compute_graph_across_batch_sizes() {
    // E1 — linear chain.
    {
        let (dir, repo) = init_repo();
        let c0 = commit(&repo, "C0", &[], 1);
        let c1 = commit(&repo, "C1", &[c0], 2);
        let c2 = commit(&repo, "C2", &[c1], 3);
        branch(&repo, "main", c2);
        set_head(&repo, "main");
        check_equivalence("E1-linear", dir.path());
    }
    // E2 — fork + merge.
    {
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
        check_equivalence("E2-fork-merge", dir.path());
    }
    // E3 — two parallel branches, no merge.
    {
        let (dir, repo) = init_repo();
        let c1 = commit(&repo, "C1", &[], 1);
        let t1 = commit(&repo, "T1", &[c1], 2);
        let c2 = commit(&repo, "C2", &[c1], 3);
        let t2 = commit(&repo, "T2", &[t1], 4);
        let c3 = commit(&repo, "C3", &[c2], 5);
        branch(&repo, "main", c3);
        branch(&repo, "topic", t2);
        set_head(&repo, "main");
        check_equivalence("E3-parallel", dir.path());
    }
    // E4 — criss-cross (shared builder).
    {
        let (dir, repo) = init_repo();
        let _ = build_criss_cross(&repo, dir.path());
        check_equivalence("E4-criss-cross", dir.path());
    }
    // E5 — octopus merge (3 parents).
    {
        let (dir, repo) = init_repo();
        let r = commit(&repo, "R", &[], 1);
        let c = commit(&repo, "C", &[r], 2);
        let b = commit(&repo, "B", &[r], 3);
        let a = commit(&repo, "A", &[r], 4);
        let m = commit(&repo, "M", &[a, b, c], 5);
        branch(&repo, "main", m);
        set_head(&repo, "main");
        check_equivalence("E5-octopus", dir.path());
    }
    // E6 — two orphan roots.
    {
        let (dir, repo) = init_repo();
        let p0 = commit(&repo, "P0", &[], 1);
        let p1 = commit(&repo, "P1", &[p0], 2);
        let c0 = commit(&repo, "C0", &[], 3);
        let c1 = commit(&repo, "C1", &[c0], 4);
        let c2 = commit(&repo, "C2", &[c1], 5);
        branch(&repo, "main", c2);
        branch(&repo, "pages", p1);
        set_head(&repo, "main");
        check_equivalence("E6-two-orphans", dir.path());
    }
    // Mid-size generated fixture: parallel lanes, merges, tags, long
    // branches (git2 objects only — fast).
    {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let spec = crate::fixture::FixtureSpec {
            main_len: 300,
            branch_every: 25,
            branch_len: 8,
            merge_after: 12,
            long_branches: 2,
            long_branch_len: 30,
            tag_every: 100,
            keep_branch_ref_every: 3,
        };
        crate::fixture::generate_fixture(dir.path(), &spec).expect("generate_fixture");
        check_equivalence("mid-size", dir.path());
    }
}

/// Empty / unborn repo: the stream is exactly `Meta{total:None, head:None}`
/// then `Done{0,0,None,false}` — never an error (contract §2.1).
#[test]
fn stream_unborn_repo_emits_meta_then_done() {
    let (dir, _repo) = init_repo();
    let chunks = capture_stream(dir.path(), 512, 512, STREAM_MAX_COMMITS);
    assert_eq!(chunks.len(), 2, "exactly Meta + Done");
    match &chunks[0] {
        GraphChunk::Meta { total, head_oid } => {
            assert_eq!(*total, None, "v1 grows-as-you-go (OQ2)");
            assert!(head_oid.is_none(), "unborn HEAD");
        }
        other => panic!("first chunk must be Meta, got {other:?}"),
    }
    match &chunks[1] {
        GraphChunk::Done {
            total_rows,
            lane_count,
            head_index,
            truncated,
        } => {
            assert_eq!(*total_rows, 0);
            assert_eq!(*lane_count, 0);
            assert!(head_index.is_none());
            assert!(!*truncated);
        }
        other => panic!("second chunk must be Done, got {other:?}"),
    }
    // Assembles to the same empty layout compute_graph produces.
    let oracle = compute_graph(dir.path()).expect("compute_graph");
    assert_layouts_eq(&assemble(&chunks), &oracle, "unborn", 512);
}

/// Truncation is also batch-boundary-invariant: a tiny `STREAM_MAX_COMMITS`
/// stops at the same row for every batch size, with identical lanes / edges
/// / lane_count (parents may differ only by truncation compaction, so they
/// are NOT compared — §7 item 1).
#[test]
fn stream_truncation_is_batch_invariant() {
    // E2 topology (7 nodes); truncate at 4.
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

    let max = 4usize;
    let mut reference: Option<GraphLayout> = None;
    for &bs in &[1usize, 2, 3, 512] {
        let chunks = capture_stream(dir.path(), bs, bs, max);
        let a = assemble(&chunks);
        assert!(a.truncated, "batch={bs}: truncated flag set at the cap");
        assert_eq!(a.nodes.len(), max, "batch={bs}: stopped at the cap");
        match &reference {
            None => reference = Some(a),
            Some(r) => {
                assert_eq!(
                    lanes(&a),
                    lanes(r),
                    "batch={bs}: lanes stable under truncation"
                );
                assert_eq!(a.lane_count, r.lane_count, "batch={bs}: lane_count");
                let mut ae = a.edges.clone();
                let mut re = r.edges.clone();
                ae.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
                re.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
                assert_eq!(ae, re, "batch={bs}: edges stable under truncation");
            }
        }
    }
}
