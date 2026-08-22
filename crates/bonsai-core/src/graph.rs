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

mod decorate;
mod lane;
mod seed;
mod stream;
use lane::LaneWalker;
pub use decorate::redecorate_chunks;
pub use seed::{graph_seed, GraphSeed};
pub use stream::{
    stream_graph_core, GraphChunk, GraphStreamEdge, StreamNode, STREAM_BATCH, STREAM_FIRST_BATCH,
    STREAM_MAX_COMMITS,
};

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
    /// Attached to a stash's OWN node `W`; name is `stash@{n}`.
    Stash,
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
    /// Committer commit time, seconds since epoch (UTC). P51: powers the
    /// author-vs-committer date basis toggle. Often == `ts` (rebases/amends
    /// differ). Additive; the frontend defaults to the author basis.
    pub committer_ts: i64,
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

/// Ref pills keyed by the commit oid they decorate. Public so the P86 layout
/// cache (command layer) can drive [`redecorate_chunks`] with a fresh decoration.
pub type RefMap = HashMap<git2::Oid, Vec<RefLabel>>;

/// The deterministic walk SEED shared by both graph paths (see [`collect_seed`]):
/// `(labels per oid, deduped tips in push order, head oid, hidden oids)`.
type WalkSeed = (RefMap, Vec<git2::Oid>, Option<git2::Oid>, Vec<git2::Oid>);

/// Blocking. Opens the repo at `workdir` (no upward search, same as
/// `read_status`) and computes the full layout. Unborn HEAD / zero refs →
/// empty layout, NOT an error.
pub fn compute_graph(workdir: &std::path::Path) -> Result<GraphLayout, AppError> {
    let mut repo = open_no_search(workdir)?;
    let (refs, tips, head_oid, hide) = collect_seed(&mut repo)?;
    if tips.is_empty() {
        return Ok(GraphLayout::empty());
    }
    layout_walk(&repo, &tips, refs, head_oid, &hide)
}

/// Opens the repo at `workdir` with NO upward search (same as `read_status`).
/// Shared by the one-shot and streaming paths so both observe the identical
/// repository.
fn open_no_search(workdir: &std::path::Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Collects the deterministic walk SEED shared by BOTH the one-shot and the
/// streaming path (contract §3.1: "determinism inputs … unchanged and shared").
/// Returns `(labels per oid, deduped tips in push order, head oid, hidden
/// oids)`. `&mut repo` is required for `stash_foreach`.
///
/// Resolves stashes BEFORE the immutable `collect_refs` borrow. Each stash
/// commit `W` is injected as a walk TIP so it renders as its own node; its
/// synthetic parents (`I`/`U`) go into `hide` so they never become nodes. Stash
/// tips are appended AFTER the branch/remote/tag/HEAD tips in ascending stash
/// index order (determinism, §1.6), then tips are re-deduped preserving first
/// occurrence (a stash `W` could coincide with an existing tip).
fn collect_seed(repo: &mut git2::Repository) -> Result<WalkSeed, AppError> {
    let stashes = collect_stashes(repo)?;
    let (mut refs, mut tips, head_oid) = collect_refs(repo)?;

    let mut hide: Vec<git2::Oid> = Vec::new();
    for s in &stashes {
        refs.entry(s.stash_oid).or_default().push(RefLabel {
            name: format!("stash@{{{}}}", s.index),
            kind: RefKind::Stash,
            is_head: false,
        });
        tips.push(s.stash_oid);
        hide.extend(s.hide.iter().copied());
    }
    let mut seen: HashSet<git2::Oid> = HashSet::new();
    tips.retain(|o| seen.insert(*o));

    Ok((refs, tips, head_oid, hide))
}

/// Builds the revwalk shared by both paths: `TOPOLOGICAL | TIME`, with the tips
/// pushed in the given deterministic order.
fn seeded_revwalk<'r>(
    repo: &'r git2::Repository,
    tips: &[git2::Oid],
) -> Result<git2::Revwalk<'r>, AppError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    for &tip in tips {
        revwalk.push(tip)?;
    }
    Ok(revwalk)
}

/// One stash resolved for the walk. `stash_oid` (= commit `W`) is pushed as a
/// revwalk TIP so the stash appears as its own node; `hide` = the stash's
/// synthetic parents (index commit `I` = parent 1, untracked commit `U` =
/// parent 2 if present) which are skip-emitted in `layout_walk` so they never
/// become nodes (NOT via `revwalk.hide`, which would also exclude `I`'s parent
/// `B` — see the note there). `W`'s FIRST parent (the base `B`) is left visible
/// and reached naturally, yielding the single `W → B` edge.
struct StashSeed {
    index: usize,
    stash_oid: git2::Oid,
    hide: Vec<git2::Oid>,
}

/// O(stashes). Enumerate the stash stack (ascending index, `stash@{0}` first);
/// for each, resolve `W` via the `refs/stash` reflog (entry `i`.`id_new()`), then
/// derive `hide` from `W`'s parents `[1..]` (skip parent 0 = base). A missing
/// `refs/stash` → empty; unresolvable entries are skipped. Requires `&mut` for
/// `stash_foreach`.
///
/// Perf: stashes add O(few) extra tips/hides to the walk. The M2d 20k perf
/// fixture contains no stashes, so this is a no-op there and the criterion
/// benchmark is unaffected.
fn collect_stashes(repo: &mut git2::Repository) -> Result<Vec<StashSeed>, AppError> {
    let mut idxs: Vec<usize> = Vec::new();
    repo.stash_foreach(|index, _msg, _oid| {
        idxs.push(index);
        true
    })?;
    let reflog = match repo.reflog("refs/stash") {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // no stash ref → nothing to inject
    };
    let mut out = Vec::with_capacity(idxs.len());
    for &index in &idxs {
        if let Some(entry) = reflog.get(index) {
            let stash_oid = entry.id_new();
            if let Ok(commit) = repo.find_commit(stash_oid) {
                // parents 1.. = the index commit `I` and optional untracked
                // commit `U`; parent 0 = base `B` is left visible.
                let hide: Vec<git2::Oid> = commit.parent_ids().skip(1).collect();
                out.push(StashSeed {
                    index,
                    stash_oid,
                    hide,
                });
            }
        }
    }
    Ok(out) // ascending by index (stash@{0} first)
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
        RefKind::Stash => 4,
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
                head_branch = head.shorthand().ok().map(str::to_string);
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
        let name = match reference.shorthand().ok() {
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

/// Core lane-assignment walk (§2.4). Tips must be pre-deduped and in
/// deterministic order; `refs` labels are moved into the emitted nodes.
fn layout_walk(
    repo: &git2::Repository,
    tips: &[git2::Oid],
    mut refs: RefMap,
    head_oid: Option<git2::Oid>,
    hide: &[git2::Oid],
) -> Result<GraphLayout, AppError> {
    let revwalk = seeded_revwalk(repo, tips)?;
    // Stash synthetic parents (`I` = staged index, `U` = untracked) must never
    // become nodes. We do NOT use `revwalk.hide` for this: `hide(I)` marks I AND
    // its ancestors uninteresting, and `I`'s parent IS the stash base `B`, so it
    // would wrongly exclude `B` (and its history) from the walk — the opposite
    // of the intended single `W → B` edge. Instead the `LaneWalker` skip-emits
    // these oids: they are dropped when reached in the walk and filtered out of
    // any commit's parent list, so `B` stays reachable via the branch/stash
    // tips. A hidden oid absent from the graph is a tolerated no-op.
    let hidden: HashSet<git2::Oid> = hide.iter().copied().collect();
    let mut walker = LaneWalker::new(hidden);

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut raw_parents: Vec<Vec<git2::Oid>> = Vec::new();
    let mut truncated = false;

    for oid in revwalk {
        let oid = oid?;
        // Stash `I`/`U` synthetic parents are never emitted as nodes.
        if walker.is_hidden(&oid) {
            continue;
        }
        if nodes.len() >= MAX_COMMITS {
            truncated = true;
            break;
        }
        // Row == final node index (skipped oids leave no gap).
        let row = nodes.len() as u32;
        let (node, node_edges) = walker.step(repo, oid, row, &mut refs)?;
        // Keep this row's raw parent oids for the `index_of` resolution below
        // (byte-identical to the pre-`LaneWalker` code); streaming skips this.
        raw_parents.push(walker.take_last_parents());
        nodes.push(GraphNode {
            id: node.id,
            lane: node.lane,
            parents: Vec::new(), // resolved below
            refs: node.refs,
            summary: node.summary,
            author: node.author,
            ts: node.ts,
            committer_ts: node.committer_ts,
        });
        for e in node_edges {
            edges.push(GraphEdge {
                from: e.from,
                to: e.to,
                lane: e.lane,
            });
        }
    }

    // 6. Resolve parent oids → indices; parents outside the emitted set are
    //    dropped (truncation only — a complete walk emits every ancestor).
    //    Pending edges never finalized are dropped with the walker.
    for (node, ps) in nodes.iter_mut().zip(raw_parents.iter()) {
        node.parents = ps.iter().filter_map(|p| walker.row_of(p)).collect();
    }

    edges.sort_unstable_by_key(|e| (e.from, e.to)); // required wire order (§1.1)
    let head_index = head_oid.and_then(|h| walker.row_of(&h));
    let lane_count = walker.lane_count();

    Ok(GraphLayout {
        nodes,
        edges,
        lane_count,
        head_index,
        truncated,
    })
}

#[cfg(test)]
mod tests;
