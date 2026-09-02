//! P10 stash-node tests. Exercises `stash_seed.rs`: a stash renders as its
//! own node carrying the `stash@{n}` pill, its synthetic index commit stays
//! hidden, and an orphaned base is pulled into the walk. Needs a real
//! worktree, so it uses its own repo builders rather than the in-memory ones
//! in `tests.rs`.

use crate::graph::*;

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
