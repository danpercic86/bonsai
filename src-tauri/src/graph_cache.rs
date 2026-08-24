//! P86 B1 — per-repo commit-graph layout cache (command layer).
//!
//! A branch create/rename/delete at an EXISTING commit, or HEAD moving onto an
//! already-walked commit, leaves the walk (nodes, lanes, edges, ordering)
//! identical — only the ref pills differ. This module separates the expensive
//! O(commits) walk from the cheap O(refs) decoration and keys a cache on a
//! topology fingerprint derived from the walk *seed* ([`bonsai_core::graph::graph_seed`]),
//! so an unchanged-topology refresh is a replay (HitVerbatim) or a re-pill pass
//! (HitRedecorate) instead of a full re-walk (Miss).
//!
//! CORRECTNESS: a stale layout must NEVER be served. When in doubt we Miss (a
//! false miss is a perf loss; a false hit is a correctness bug). The seed
//! captures every input to the walk (all tip oids + HEAD + the hidden/stash
//! set); the store path brackets the walk with a second seed probe and only
//! caches when the topology is observably unchanged across it (defends against a
//! mutation racing the cold walk — see [`stream_graph_cached`]).

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

use bonsai_core::error::AppError;
use bonsai_core::graph::{
    graph_seed_with, redecorate_chunks, stream_graph_from_repo, GraphChunk, GraphSeed, RefKind,
    RefLabel, RefMap,
};

use crate::perf::PerfState;

/// The cached graph output for one repo, plus everything the classifier needs.
/// Stored behind [`GraphCache`] in each `RepoEntry`.
pub struct CachedGraph {
    /// Hash of the WALK identity: `(sorted tips, head, sorted hide)`. Stored as
    /// a compact digest (contract shape); NOT used as an equality proxy —
    /// [`classify`] compares the exact `tips`/`head`/`hide` sets below so a hash
    /// collision can never mis-serve TOPOLOGY.
    pub seed_fp: u64,
    /// Hash of the DECORATION identity (the full [`RefMap`]) — the contract's
    /// HitVerbatim-vs-HitRedecorate discriminator. A 64-bit collision (~2⁻⁶⁴)
    /// could at worst replay stale PILLS for one refresh; it can never affect
    /// topology (that is exact-set-checked). Self-heals on the next real change.
    pub deco_fp: u64,
    /// Deduped walk tips (topology inputs). Sorted set for `⊆` classification.
    pub tips: BTreeSet<git2::Oid>,
    pub head: Option<git2::Oid>,
    /// Stash synthetic parents skip-emitted by the walk (hide-set change ⇒ Miss).
    pub hide: BTreeSet<git2::Oid>,
    /// Every oid emitted as a node by the walk (tips AND interior commits). A new
    /// tip must already be a member for a HitRedecorate — proves no new commit.
    pub node_oids: HashSet<git2::Oid>,
    /// The exact `Meta … Batch* … Done` wire stream, replayed verbatim on a hit.
    pub chunks: Vec<GraphChunk>,
}

/// Per-repo cache slot. `None` until the first walk; reset to `None` on
/// `open_repo` re-arm (a fresh `RepoEntry` is inserted) and dropped on
/// `close_repo`.
pub type GraphCache = Mutex<Option<CachedGraph>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Classification {
    Miss,
    HitVerbatim,
    HitRedecorate,
}

/// Stable per-kind tag for the decoration fingerprint (independent of the
/// pill-sort rank; any injective mapping is fine).
fn kind_tag(k: RefKind) -> u8 {
    match k {
        RefKind::LocalBranch => 0,
        RefKind::RemoteBranch => 1,
        RefKind::Tag => 2,
        RefKind::Head => 3,
        RefKind::Stash => 4,
    }
}

/// `seed_fp` — the WALK identity. Hash of `(sorted tip oids, head oid, sorted
/// hide oids)`. Any topology-affecting change (a tip added/removed, HEAD moved,
/// a stash pushed/popped) changes it. The `BTreeSet` iteration order makes it
/// order-independent and deterministic.
fn seed_fingerprint(
    tips: &BTreeSet<git2::Oid>,
    head: Option<git2::Oid>,
    hide: &BTreeSet<git2::Oid>,
) -> u64 {
    let mut h = DefaultHasher::new();
    b"tips".hash(&mut h);
    for t in tips {
        t.as_bytes().hash(&mut h);
    }
    b"head".hash(&mut h);
    match head {
        Some(o) => o.as_bytes().hash(&mut h),
        None => 0u8.hash(&mut h),
    }
    b"hide".hash(&mut h);
    for o in hide {
        o.as_bytes().hash(&mut h);
    }
    h.finish()
}

/// `deco_fp` — the DECORATION identity. Hash of the full [`RefMap`] (oids sorted;
/// each oid's labels are already pill-sorted by `collect_refs`). Each
/// [`RefLabel`] carries `name` + `kind` + `is_head`, and `is_head`/the `Head`
/// pill encode head_branch + detached — so any rename/add/remove/HEAD-move at
/// existing oids flips this hash.
fn deco_fingerprint(refs: &RefMap) -> u64 {
    let mut entries: Vec<(&git2::Oid, &Vec<RefLabel>)> = refs.iter().collect();
    entries.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut h = DefaultHasher::new();
    (entries.len() as u64).hash(&mut h);
    for (oid, labels) in entries {
        oid.as_bytes().hash(&mut h);
        (labels.len() as u64).hash(&mut h);
        for l in labels {
            l.name.hash(&mut h);
            kind_tag(l.kind).hash(&mut h);
            l.is_head.hash(&mut h);
        }
    }
    h.finish()
}

/// Classify a fresh seed against the cached entry (contract §B1). See the module
/// doc for the soundness argument; the short version:
/// - identical `(tips, head, hide)` ⇒ same walk ⇒ HitVerbatim / HitRedecorate on
///   the deco fingerprint.
/// - same hide, `cache.tips ⊆ new_tips` (no tip removed ⇒ nothing dropped) and
///   `new_tips ⊆ cache.node_oids` (every new tip, incl. HEAD, already a walked
///   node ⇒ no new commit) ⇒ equal reachable set + same deterministic order ⇒
///   HitRedecorate (e.g. a branch created at an existing commit).
/// - otherwise Miss (full re-walk). A tip removed, a new commit, a HEAD advance
///   to an unwalked oid, or any hide-set change all land here.
fn classify(
    cache: Option<&CachedGraph>,
    tips: &BTreeSet<git2::Oid>,
    head: Option<git2::Oid>,
    hide: &BTreeSet<git2::Oid>,
    deco_fp: u64,
) -> Classification {
    let Some(c) = cache else {
        return Classification::Miss;
    };
    if tips == &c.tips && head == c.head && hide == &c.hide {
        return if deco_fp == c.deco_fp {
            Classification::HitVerbatim
        } else {
            Classification::HitRedecorate
        };
    }
    if hide == &c.hide
        && c.tips.is_subset(tips)
        && tips.iter().all(|t| c.node_oids.contains(t))
    {
        return Classification::HitRedecorate;
    }
    Classification::Miss
}

/// Blocking. Cache-aware core of the `stream_graph` command (P86 B1). Probes the
/// cheap seed, classifies against the per-repo `cache`, then either replays the
/// cached chunks (HitVerbatim), re-pills them (HitRedecorate), or walks and
/// repopulates (Miss) — teeing every walked chunk into both `emit` and the
/// cache. `emit` returns `false` when the sink is gone (channel dropped): the
/// pass stops promptly with `Ok`. The `cache` mutex is held for the classify +
/// hit replay/store but NEVER across the cold walk.
pub fn stream_graph_cached(
    workdir: &Path,
    cache: &GraphCache,
    perf: &PerfState,
    emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    // `&Path` entry (tests / non-routed callers): open ONE handle, then run the
    // same cache-aware body the routed `stream_graph` command drives through
    // `repo_handle::with_repo_mut`. The open is NOT counted here — `repo_opens`
    // is instrumented only at the handle-cache seam (P88b reconciliation), so
    // this direct path is a diagnostics no-op for that counter.
    let mut repo = git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?;
    stream_graph_cached_with(&mut repo, cache, perf, emit)
}

/// P88b/B2b: cache-aware graph stream from an ALREADY-OPEN handle (the round
/// handle cache). Byte-identical to [`stream_graph_cached`]; the `&Path` entry
/// point above opens then delegates here. `&mut` is required because the seed
/// probe runs `stash_foreach`. Opens are counted ONCE by
/// `repo_handle::with_repo_mut` at the command seam — never inline here.
pub fn stream_graph_cached_with(
    repo: &mut git2::Repository,
    cache: &GraphCache,
    perf: &PerfState,
    mut emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    let seed = graph_seed_with(repo)?;

    let tips: BTreeSet<git2::Oid> = seed.tips.iter().copied().collect();
    let hide: BTreeSet<git2::Oid> = seed.hide.iter().copied().collect();
    let deco_fp = deco_fingerprint(&seed.refs);
    let seed_fp = seed_fingerprint(&tips, seed.head, &hide);

    let mut guard = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match classify(guard.as_ref(), &tips, seed.head, &hide, deco_fp) {
        Classification::HitVerbatim => {
            perf.inc_graph_cache_hits();
            if let Some(c) = guard.as_ref() {
                for chunk in &c.chunks {
                    if !emit(chunk.clone()) {
                        return Ok(());
                    }
                }
            }
            Ok(())
        }
        Classification::HitRedecorate => {
            perf.inc_graph_redecorates();
            if let Some(c) = guard.as_mut() {
                // Re-pill the cached stream in place, then update the WALK
                // identity to the fresh seed. `node_oids` is unchanged by
                // construction (a redecorate never adds/removes nodes).
                redecorate_chunks(&mut c.chunks, &seed.refs, seed.head);
                c.tips = tips;
                c.hide = hide;
                c.head = seed.head;
                c.deco_fp = deco_fp;
                c.seed_fp = seed_fp;
                for chunk in &c.chunks {
                    if !emit(chunk.clone()) {
                        return Ok(());
                    }
                }
            }
            Ok(())
        }
        Classification::Miss => {
            perf.inc_graph_walks();
            // Never hold the cache lock across the cold walk (contract §B1
            // concurrency); the walk streams holding only the channel.
            drop(guard);

            let mut buf: Vec<GraphChunk> = Vec::new();
            let mut node_oids: HashSet<git2::Oid> = HashSet::new();
            let mut saw_done = false;
            // P88b/B2b: the walk reuses the SAME handle as the seed probe above
            // (was a second open) — one open serves both. `repo_opens` is bumped
            // once by `with_repo_mut` at the command seam, never here.
            stream_graph_from_repo(repo, |chunk| {
                match &chunk {
                    GraphChunk::Batch { nodes, .. } => {
                        for n in nodes {
                            if let Ok(o) = git2::Oid::from_str(&n.id) {
                                node_oids.insert(o);
                            }
                        }
                    }
                    GraphChunk::Done { .. } => saw_done = true,
                    GraphChunk::Meta { .. } => {}
                }
                buf.push(chunk.clone());
                emit(chunk)
            })?;

            // Only cache a COMPLETE walk whose topology is observably unchanged
            // across the walk window. If the sink died mid-stream (`!saw_done`)
            // the buffer is partial; if a mutation raced the walk the bracketing
            // seed differs — either way we skip the store (safe Miss next time)
            // rather than risk a stale hit. The bracket probe is an internal
            // consistency check, not a serving open, so it is not counted.
            if saw_done && seed_unchanged_with(repo, &tips, seed.head, &hide) {
                let mut guard = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                *guard = Some(CachedGraph {
                    seed_fp,
                    deco_fp,
                    tips,
                    head: seed.head,
                    hide,
                    node_oids,
                    chunks: buf,
                });
            }
            Ok(())
        }
    }
}

/// Re-probe the seed after a walk and report whether the WALK identity
/// (`tips`/`head`/`hide`) is unchanged — the store guard against a mutation
/// racing the cold walk. A probe failure is treated as "changed" (skip caching).
fn seed_unchanged_with(
    repo: &mut git2::Repository,
    tips: &BTreeSet<git2::Oid>,
    head: Option<git2::Oid>,
    hide: &BTreeSet<git2::Oid>,
) -> bool {
    match graph_seed_with(repo) {
        Ok(GraphSeed {
            tips: t2,
            head: h2,
            hide: hd2,
            ..
        }) => {
            let t2: BTreeSet<git2::Oid> = t2.into_iter().collect();
            let hd2: BTreeSet<git2::Oid> = hd2.into_iter().collect();
            &t2 == tips && h2 == head && &hd2 == hide
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests;
