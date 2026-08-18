//! Semantic commit-history search — the persisted per-commit document index
//! (P57a contract §1/§3). BM25 over commit message + a bounded diff sample,
//! pure-Rust and fully local (contract D1/OD1): no embeddings, no model
//! download, no new heavy dependency.
//!
//! This directory module splits the concern to honor the ~500-line limit:
//! * [`doc`] — per-commit document extraction + the shared tokenizer.
//! * [`bm25`] — the pure Okapi BM25 stats + scoring algorithm.
//! * [`store`] — atomic on-disk persistence + the FNV-1a repo key.
//! * this file — the wire types, consts, [`index_dir_for`] path builder, and
//!   the [`build_index`] / [`index_status`] orchestration.
//!
//! The command layer resolves the app-data base and passes it in; the core
//! stays runtime-free and unit-testable with a `tempfile::TempDir` index dir
//! (mirrors `settings.rs`'s path-parameterization). git2-only extraction (OQ8):
//! it never shells out, so the index builds even where the `git` binary is
//! absent. Ref-seeding reuses `search::seed_all_refs` (OQ9 visibility bump) for
//! the all-refs reachable walk.

pub mod bm25;
pub mod doc;
pub mod search;
pub mod store;

use std::path::{Path, PathBuf};

use crate::error::AppError;

pub use bm25::Bm25Index;
pub use doc::{extract_doc, tokenize, CommitDoc, MSG_BOOST};
pub use search::{search_history, HistoryHit, HistoryQuery, HistorySearchResults};
pub use store::{repo_key, IndexStore};

/// Persisted-index schema; bump on ANY format/tokenization change to force a
/// full rebuild (contract §3.4 invalidation).
pub const HISTORY_INDEX_SCHEMA: u32 = 1;
/// Hard cap on commits indexed in one build (contract §2.1; the 20k+ horizon).
pub const MAX_INDEX_COMMITS: usize = 50_000;
/// Per-commit diff bytes read for TOKENIZATION at build time — never stored raw
/// (D3). Bounds the walk (contract §2.1).
pub const MAX_DOC_DIFF_BYTES: usize = 4_096;
/// Emit an `Extracting` progress tick every N documented commits (contract §3.4).
pub const PROGRESS_TICK: usize = 200;
/// Default retrieval depth when a query asks for `top_k == 0` (contract §2.1).
pub const DEFAULT_TOP_K: u32 = 20;
/// Hard cap on retrieval depth; a larger `top_k` is clamped to this (contract §2.1).
pub const MAX_TOP_K: u32 = 50;

/// Streamed build progress — one per Channel tick (contract §2.1).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub phase: IndexPhase,
    /// Commits documented so far THIS build.
    pub processed: u32,
    /// Commits to document THIS build (0 until counted).
    pub total: u32,
    /// Of `total`, how many were newly-added (incremental).
    pub new_commits: u32,
}

/// Build phase (contract §2.1). Serialized lowercase camelCase.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexPhase {
    Counting,
    Extracting,
    Writing,
    Done,
}

/// Cheap status driving the UI affordance (contract §2.1).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    /// An index file exists and parsed at the CURRENT schema.
    pub built: bool,
    /// Documents in the store.
    pub indexed_commits: u32,
    /// HEAD (40-hex) at the last build.
    pub head_oid: Option<String>,
    /// Current ref tips differ from the last build's tips.
    pub stale: bool,
    /// Reachable commits not yet in the store (0 when fresh).
    pub new_commits: u32,
    /// Schema of the on-disk file (for a mismatch note).
    pub schema: u32,
    /// Unix secs of the last build.
    pub built_at: Option<i64>,
}

/// Pure path builder (contract §3.1): `base/history-index/<repo_key(workdir)>`.
/// No IO — the core owns the layout; the command supplies `app_data_base` AND
/// the `ignorecase` hint (see [`store::repo_key`]), so every branch is
/// unit-testable on any host. Prefer [`index_dir_for_repo`] in production.
pub fn index_dir_for(app_data_base: &Path, workdir: &Path, ignorecase: bool) -> PathBuf {
    app_data_base
        .join("history-index")
        .join(store::repo_key(workdir, ignorecase))
}

/// [`index_dir_for`] with `ignorecase` resolved from the repo's own
/// `core.ignorecase` ([`crate::git::repo::path_ignorecase`]) — one cheap config
/// read. This is what callers with a real workdir should use: keying off
/// `cfg!(windows)` instead split the cache per path-casing on macOS APFS, which
/// is case-insensitive by default.
pub fn index_dir_for_repo(app_data_base: &Path, workdir: &Path) -> PathBuf {
    index_dir_for(
        app_data_base,
        workdir,
        crate::git::repo::path_ignorecase(workdir),
    )
}

/// Build/refresh the persisted index (contract §3.4). Blocking + CPU-heavy (a
/// diff per new commit) ⇒ ALWAYS call under `spawn_blocking`. Loads any existing
/// store; a schema mismatch starts empty (invalidation). Walks all refs (bounded
/// at [`MAX_INDEX_COMMITS`]), documents every reachable oid absent from the store
/// (INCREMENTAL — a commit's content is immutable, so existing docs are never
/// re-extracted), rebuilds BM25 stats, stamps head/tips/built_at, and atomically
/// writes. Streams progress. Returns the fresh status (stale=false, new=0).
pub fn build_index(
    workdir: &Path,
    index_dir: &Path,
    mut on_progress: impl FnMut(IndexProgress) + Send,
) -> Result<IndexStatus, AppError> {
    let repo = open_repo_at(workdir)?;
    let mut store = store::load(index_dir)
        .filter(|s| s.schema == HISTORY_INDEX_SCHEMA)
        .unwrap_or_else(IndexStore::empty);

    let reachable = reachable_oids(&repo)?;
    // F-A9-2: drop docs for commits no longer reachable (rewritten by rebase/
    // amend, or GC'd) so search never returns dead oids and BM25 idf isn't
    // skewed by ghosts. SKIP pruning when the walk hit the cap — beyond
    // MAX_INDEX_COMMITS we cannot distinguish a ghost from a still-reachable
    // commit, so pruning there would wrongly evict live docs (the "cap-before-
    // filter" drift, F-A9-4).
    let truncated = reachable.len() >= MAX_INDEX_COMMITS;
    if !truncated {
        let reachable_set: std::collections::HashSet<String> =
            reachable.iter().map(|oid| oid.to_string()).collect();
        store.docs.retain(|oid, _| reachable_set.contains(oid));
    }
    let todo: Vec<git2::Oid> = reachable
        .into_iter()
        .filter(|oid| !store.docs.contains_key(&oid.to_string()))
        .collect();
    let total = todo.len() as u32;
    // Every tick carries the same total/new_commits; only phase + processed vary.
    let mut emit = |phase, processed| {
        on_progress(IndexProgress {
            phase,
            processed,
            total,
            new_commits: total,
        });
    };

    emit(IndexPhase::Counting, 0);
    // F-A9-3: one unreadable object (corrupt/missing blob or tree, a broken
    // pack) skips-and-counts rather than aborting the whole build — a partial
    // index over the good commits is far better than none, and the next build
    // retries the skipped oids (they stay out of the store).
    let mut skipped: u32 = 0;
    for (i, oid) in todo.iter().enumerate() {
        match doc::extract_doc(&repo, *oid) {
            Ok(document) => {
                store.docs.insert(oid.to_string(), document);
            }
            Err(e) => {
                skipped = skipped.saturating_add(1);
                eprintln!("bonsai: history-index skipping unreadable commit {oid}: {e}");
            }
        }
        if i % PROGRESS_TICK == 0 {
            emit(IndexPhase::Extracting, i as u32);
        }
    }
    if skipped > 0 {
        eprintln!("bonsai: history-index build skipped {skipped} unreadable commit(s)");
    }

    store.bm25 = Bm25Index::build_stats(&store.docs);
    store.head_oid = head_hex(&repo);
    store.tip_oids = collect_tip_hexes(&repo)?;
    store.built_at = Some(now_unix());

    emit(IndexPhase::Writing, total);
    store::save(index_dir, &store)?;
    emit(IndexPhase::Done, total);

    Ok(fresh_status(&store))
}

/// Cheap status of the persisted index (contract §3.1). Reads the store, then a
/// header-only ref-tip + reachable scan to compute `stale`/`new_commits`.
/// Missing/unparsable/schema-mismatched ⇒ `built: false`.
pub fn index_status(workdir: &Path, index_dir: &Path) -> Result<IndexStatus, AppError> {
    let store = match store::load(index_dir) {
        Some(s) if s.schema == HISTORY_INDEX_SCHEMA => s,
        Some(s) => return Ok(not_built(s.schema)),
        None => return Ok(not_built(0)),
    };
    let repo = open_repo_at(workdir)?;
    let stale = collect_tip_hexes(&repo)? != store.tip_oids;
    let new_commits = reachable_oids(&repo)?
        .iter()
        .filter(|oid| !store.docs.contains_key(&oid.to_string()))
        .count() as u32;
    Ok(IndexStatus {
        built: true,
        indexed_commits: store.docs.len() as u32,
        head_oid: store.head_oid.clone(),
        stale,
        new_commits,
        schema: store.schema,
        built_at: store.built_at,
    })
}

/// Status right after a successful build: fresh by construction.
fn fresh_status(store: &IndexStore) -> IndexStatus {
    IndexStatus {
        built: true,
        indexed_commits: store.docs.len() as u32,
        head_oid: store.head_oid.clone(),
        stale: false,
        new_commits: 0,
        schema: store.schema,
        built_at: store.built_at,
    }
}

/// The "no usable index" status; `schema` echoes the on-disk schema (0 when the
/// file is missing/unparsable) for a UI mismatch note.
fn not_built(schema: u32) -> IndexStatus {
    IndexStatus {
        built: false,
        indexed_commits: 0,
        head_oid: None,
        stale: false,
        new_commits: 0,
        schema,
        built_at: None,
    }
}

/// Open the repo at `workdir` with `NO_SEARCH` — the bare-agnostic, read-only
/// convention every `git/` module uses (`branches.rs`/`tags.rs`/`search.rs`).
/// The index is derived data (it never writes to `.git`), so this must NOT go
/// through `stage::open_workdir_repo`, which asserts a working directory and
/// yields a wrong-domain "cannot modify index" error on a bare repo (P57a nit).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// All-refs reachable walk (header-only), bounded at [`MAX_INDEX_COMMITS`].
/// Seeds via `search::seed_all_refs` (OQ9 shared ref-seeding).
fn reachable_oids(repo: &git2::Repository) -> Result<Vec<git2::Oid>, AppError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TIME)?;
    crate::git::search::seed_all_refs(repo, &mut walk)?;
    let mut out = Vec::new();
    for oid in walk {
        if out.len() >= MAX_INDEX_COMMITS {
            break;
        }
        out.push(oid?);
    }
    Ok(out)
}

/// The SORTED set of ref-tip oids (local + remote-tracking [skip `*/HEAD`] +
/// tags-peeled + HEAD) for the staleness compare. This collects TIPS (the same
/// ref set `search::seed_all_refs` walks from), which that helper does not
/// expose — a small, deduped parallel of its ref enumeration.
fn collect_tip_hexes(repo: &git2::Repository) -> Result<Vec<String>, AppError> {
    let mut tips: Vec<String> = Vec::new();
    for entry in repo.branches(Some(git2::BranchType::Local))? {
        let (b, _) = entry?;
        if let Ok(c) = b.get().peel_to_commit() {
            tips.push(c.id().to_string());
        }
    }
    for entry in repo.branches(Some(git2::BranchType::Remote))? {
        let (b, _) = entry?;
        if matches!(b.name(), Ok(Some(n)) if n.ends_with("/HEAD")) {
            continue;
        }
        if let Ok(c) = b.get().peel_to_commit() {
            tips.push(c.id().to_string());
        }
    }
    for entry in repo.references_glob("refs/tags/*")? {
        let reference = entry?;
        if let Ok(obj) = reference.peel(git2::ObjectType::Commit) {
            tips.push(obj.id().to_string());
        }
    }
    if let Ok(head) = repo.head() {
        if let Some(oid) = head.target() {
            tips.push(oid.to_string());
        }
    }
    tips.sort();
    tips.dedup();
    Ok(tips)
}

/// Current HEAD (40-hex); `None` when HEAD is unborn.
fn head_hex(repo: &git2::Repository) -> Option<String> {
    repo.head().ok().and_then(|h| h.target()).map(|o| o.to_string())
}

/// Unix seconds now (0 on a pre-epoch clock — never panics).
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
