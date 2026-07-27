//! Commit-graph layout engine (M2a).
//!
//! Rust owns ALL layout math: this module walks the commit history
//! (topological, then commit date), assigns lanes, and routes edges. The
//! frontend receives a finished [`GraphLayout`] and only rasterizes it.
//!
//! Wire invariants (M2 contract §1):
//! - `nodes` is in walk order and **row == node index** — there is no `row`
//!   field; `GraphEdge.from`/`to` double as row numbers.
//! - `GraphNode.parents` are indices into `nodes` (parents always appear at a
//!   higher index; first entry = first parent). Truncated walks silently drop
//!   parents that were not emitted.
//! - `edges` is sorted ascending by `(from, to)`.

use std::collections::{HashMap, HashSet};

use crate::error::AppError;

/// Hard cap on the walk; beyond it the layout is truncated (§2.8).
pub const MAX_COMMITS: usize = 100_000;

/// Kind of a ref pill shown beside a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    LocalBranch,
    /// Name already includes the remote: `"origin/main"`.
    RemoteBranch,
    Tag,
    /// ONLY emitted when HEAD is detached.
    Head,
}

/// A single ref pill.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefLabel {
    /// Shorthand: `"main"`, `"origin/main"`, `"v1.0"`, `"HEAD"`.
    pub name: String,
    pub kind: RefKind,
    /// true on the local branch HEAD points at (attached), or on the Head
    /// label (detached).
    pub is_head: bool,
}

/// One commit row of the layout. Row number == index in `GraphLayout.nodes`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    /// Full 40-char hex oid (M4 needs it for commit diffs).
    pub id: String,
    pub lane: u32,
    /// Indices into `GraphLayout.nodes` (parents always appear at a HIGHER
    /// index — topological order guarantees it). First entry = first parent.
    /// Truncated walks (§2.8) silently drop parents that were not emitted.
    pub parents: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<RefLabel>,
    /// First line of the message, char-safe cap at 120 chars.
    pub summary: String,
    /// Author name only (no email).
    pub author: String,
    /// Author commit time, seconds since epoch (UTC).
    pub ts: i64,
}

/// Logical commit→parent edge with the lane of its vertical run (§1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    /// Child node index == child ROW.
    pub from: u32,
    /// Parent node index == parent row; always `to > from`.
    pub to: u32,
    /// Lane of the vertical run between the rows.
    pub lane: u32,
}

/// Complete precomputed layout, sent as a single command response.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLayout {
    pub nodes: Vec<GraphNode>,
    /// Sorted ascending by `(from, to)` — required wire order.
    pub edges: Vec<GraphEdge>,
    /// Max lanes ever active; drives graph-area width.
    pub lane_count: u32,
    /// Node index of the HEAD commit (`None` if unborn/no HEAD).
    pub head_index: Option<u32>,
    /// Walk stopped at [`MAX_COMMITS`].
    pub truncated: bool,
}

impl GraphLayout {
    fn empty() -> Self {
        GraphLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
            lane_count: 0,
            head_index: None,
            truncated: false,
        }
    }
}

type RefMap = HashMap<git2::Oid, Vec<RefLabel>>;

/// Blocking. Opens the repo at `workdir` (no upward search, same as
/// `read_status`) and computes the full layout. Unborn HEAD / zero refs →
/// empty layout, NOT an error.
pub fn compute_graph(workdir: &std::path::Path) -> Result<GraphLayout, AppError> {
    let repo = git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?;
    let (refs, tips, head_oid) = collect_refs(&repo)?;
    if tips.is_empty() {
        return Ok(GraphLayout::empty());
    }
    layout_walk(&repo, &tips, refs, head_oid)
}

/// Sort rank for pill order (§2.2): detached Head first, then LocalBranch
/// (is_head first, then name asc), then RemoteBranch name asc, then Tag
/// name asc.
fn pill_rank(kind: RefKind) -> u8 {
    match kind {
        RefKind::Head => 0,
        RefKind::LocalBranch => 1,
        RefKind::RemoteBranch => 2,
        RefKind::Tag => 3,
    }
}

/// Collects ref labels per commit and the deterministic tip list for the walk.
/// Returns `(labels per oid, deduped tips in push order, head oid)`.
fn collect_refs(
    repo: &git2::Repository,
) -> Result<(RefMap, Vec<git2::Oid>, Option<git2::Oid>), AppError> {
    let mut labels: RefMap = HashMap::new();
    let mut tips: Vec<git2::Oid> = Vec::new();

    let mut head_oid: Option<git2::Oid> = None;
    let mut head_branch: Option<String> = None;
    let mut detached = false;

    match repo.head() {
        Ok(head) => {
            head_oid = head.target();
            detached = repo.head_detached()?;
            if !detached {
                head_branch = head.shorthand().map(str::to_string);
            }
        }
        Err(e)
            if e.code() == git2::ErrorCode::UnbornBranch
                || e.code() == git2::ErrorCode::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    // 1. Local branches, sorted by name ascending (byte-wise).
    let mut locals: Vec<(String, git2::Oid)> = Vec::new();
    for entry in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = entry?;
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => continue,
        };
        let oid = match branch.get().peel_to_commit() {
            Ok(c) => c.id(),
            Err(_) => continue, // unresolvable tip: skip
        };
        locals.push((name, oid));
    }
    locals.sort();
    for (name, oid) in locals {
        let is_head = !detached && head_branch.as_deref() == Some(name.as_str());
        labels.entry(oid).or_default().push(RefLabel {
            name,
            kind: RefKind::LocalBranch,
            is_head,
        });
        tips.push(oid);
    }

    // 2. Remote-tracking branches, sorted by shorthand; skip "*/HEAD".
    let mut remotes: Vec<(String, git2::Oid)> = Vec::new();
    for entry in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = entry?;
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.ends_with("/HEAD") {
            continue;
        }
        let oid = match branch.get().peel_to_commit() {
            Ok(c) => c.id(),
            Err(_) => continue,
        };
        remotes.push((name, oid));
    }
    remotes.sort();
    for (name, oid) in remotes {
        labels.entry(oid).or_default().push(RefLabel {
            name,
            kind: RefKind::RemoteBranch,
            is_head: false,
        });
        tips.push(oid);
    }

    // 3. Tags, sorted by name; peel annotated tags to the target commit; skip
    //    tags that do not peel to a commit (tag→blob/tree).
    let mut tags: Vec<(String, git2::Oid)> = Vec::new();
    for entry in repo.references_glob("refs/tags/*")? {
        let reference = entry?;
        let name = match reference.shorthand() {
            Some(s) => s.to_string(),
            None => continue, // non-UTF-8 ref name: skip
        };
        let oid = match reference.peel(git2::ObjectType::Commit) {
            Ok(obj) => obj.id(),
            Err(_) => continue, // tag→blob/tree: skip
        };
        tags.push((name, oid));
    }
    tags.sort();
    for (name, oid) in tags {
        labels.entry(oid).or_default().push(RefLabel {
            name,
            kind: RefKind::Tag,
            is_head: false,
        });
        tips.push(oid);
    }

    // 4. HEAD last: detached gets its own label; attached is covered by (1).
    if let Some(oid) = head_oid {
        if detached {
            labels.entry(oid).or_default().push(RefLabel {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
                is_head: true,
            });
        }
        tips.push(oid);
    }

    // Sort each commit's labels into pill order.
    for v in labels.values_mut() {
        v.sort_by(|a, b| {
            (pill_rank(a.kind), !a.is_head, a.name.as_str())
                .cmp(&(pill_rank(b.kind), !b.is_head, b.name.as_str()))
        });
    }

    // Dedupe tips preserving first occurrence (push order stays deterministic).
    let mut seen: HashSet<git2::Oid> = HashSet::new();
    tips.retain(|o| seen.insert(*o));

    Ok((labels, tips, head_oid))
}

/// An edge created at child time, finalized when the parent row is emitted.
struct PendingEdge {
    from: u32,
    lane: u32,
}

/// Lowest free lane index; grows the vector when all lanes are busy.
/// Scanning always starts at 0 — simple and deterministic (§8.5).
fn first_free(lanes: &mut Vec<Option<git2::Oid>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// First line of `summary`, char-safe capped at `max` chars.
fn first_line_capped(bytes: Option<&[u8]>, max: usize) -> String {
    let s = String::from_utf8_lossy(bytes.unwrap_or_default());
    let first_line = s.lines().next().unwrap_or("");
    first_line.chars().take(max).collect()
}

/// Core lane-assignment walk (§2.4). Tips must be pre-deduped and in
/// deterministic order; `refs` labels are moved into the emitted nodes.
fn layout_walk(
    repo: &git2::Repository,
    tips: &[git2::Oid],
    mut refs: RefMap,
    head_oid: Option<git2::Oid>,
) -> Result<GraphLayout, AppError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    for &tip in tips {
        revwalk.push(tip)?;
    }

    // lanes[i] == Some(p): an edge runs down lane i, waiting for commit p.
    // Multiple lanes may wait for the same oid (parallel lines converging).
    let mut lanes: Vec<Option<git2::Oid>> = Vec::new();
    let mut pending: HashMap<git2::Oid, Vec<PendingEdge>> = HashMap::new();
    let mut index_of: HashMap<git2::Oid, u32> = HashMap::new();

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut raw_parents: Vec<Vec<git2::Oid>> = Vec::new();
    let mut truncated = false;

    for (row, oid) in revwalk.enumerate() {
        if row >= MAX_COMMITS {
            truncated = true;
            break;
        }
        let oid = oid?;
        let row_u = row as u32;
        let commit = repo.find_commit(oid)?;

        // 1. Which lanes were waiting for this commit? (ascending)
        let reserved: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| **l == Some(oid))
            .map(|(i, _)| i)
            .collect();

        // 2. Pick this commit's lane.
        let lane = if reserved.is_empty() {
            first_free(&mut lanes) // tip / new branch head / orphan root
        } else {
            // Leftmost waiting lane wins; converging lines free their lanes.
            for &i in &reserved[1..] {
                lanes[i] = None;
            }
            reserved[0]
        };

        // 3. Finalize every edge that was waiting for this commit.
        for pe in pending.remove(&oid).unwrap_or_default() {
            edges.push(GraphEdge {
                from: pe.from,
                to: row_u,
                lane: pe.lane,
            });
        }

        // 4. Route edges to parents / update reservations.
        let parents: Vec<git2::Oid> = commit.parent_ids().collect();
        if parents.is_empty() {
            lanes[lane] = None; // root: line ends here
        } else {
            let p0 = parents[0];
            // First parent inherits the lane — even if p0 is ALSO reserved
            // elsewhere (convergence happens at p0 via leftmost-wins).
            lanes[lane] = Some(p0);
            pending.entry(p0).or_default().push(PendingEdge {
                from: row_u,
                lane: lane as u32,
            });
            for &pk in &parents[1..] {
                // Merge parents (octopus-safe): join an existing line if one
                // is already waiting for pk, else open a new lane.
                let j = match lanes.iter().position(|l| *l == Some(pk)) {
                    Some(j) => j,
                    None => {
                        let j = first_free(&mut lanes);
                        lanes[j] = Some(pk);
                        j
                    }
                };
                pending.entry(pk).or_default().push(PendingEdge {
                    from: row_u,
                    lane: j as u32,
                });
            }
        }

        // 5. Emit the node.
        index_of.insert(oid, row_u);
        let author = commit.author();
        nodes.push(GraphNode {
            id: oid.to_string(),
            lane: lane as u32,
            parents: Vec::new(), // resolved below
            refs: refs.remove(&oid).unwrap_or_default(),
            summary: first_line_capped(commit.summary_bytes(), 120),
            author: String::from_utf8_lossy(author.name_bytes()).into_owned(),
            ts: author.when().seconds(),
        });
        raw_parents.push(parents);
    }

    // 6. Resolve parent oids → indices; parents outside the emitted set are
    //    dropped (truncation only — a complete walk emits every ancestor).
    //    Pending edges never finalized are dropped with `pending`.
    for (node, ps) in nodes.iter_mut().zip(raw_parents.iter()) {
        node.parents = ps.iter().filter_map(|p| index_of.get(p).copied()).collect();
    }

    edges.sort_unstable_by_key(|e| (e.from, e.to)); // required wire order (§1.1)
    let head_index = head_oid.and_then(|h| index_of.get(&h).copied());
    let lane_count = lanes.len() as u32;

    Ok(GraphLayout {
        nodes,
        edges,
        lane_count,
        head_index,
        truncated,
    })
}

#[cfg(test)]
mod tests {
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
}
