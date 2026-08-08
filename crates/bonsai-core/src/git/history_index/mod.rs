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
pub mod store;

use std::path::{Path, PathBuf};

use crate::error::AppError;

pub use bm25::Bm25Index;
pub use doc::{extract_doc, tokenize, CommitDoc, MSG_BOOST};
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
/// No IO — the core owns the layout; the command supplies `app_data_base`.
pub fn index_dir_for(app_data_base: &Path, workdir: &Path) -> PathBuf {
    app_data_base
        .join("history-index")
        .join(store::repo_key(workdir))
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
    let repo = crate::git::stage::open_workdir_repo(workdir)?;
    let mut store = store::load(index_dir)
        .filter(|s| s.schema == HISTORY_INDEX_SCHEMA)
        .unwrap_or_else(IndexStore::empty);

    let reachable = reachable_oids(&repo)?;
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
    for (i, oid) in todo.iter().enumerate() {
        let document = doc::extract_doc(&repo, *oid)?;
        store.docs.insert(oid.to_string(), document);
        if i % PROGRESS_TICK == 0 {
            emit(IndexPhase::Extracting, i as u32);
        }
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
    let repo = crate::git::stage::open_workdir_repo(workdir)?;
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
mod tests {
    use super::*;

    /// git2-init a `main`-headed scratch repo with pinned identity + autocrlf off.
    fn init_scratch() -> (tempfile::TempDir, git2::Repository) {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init_opts(
            dir.path(),
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        (dir, repo)
    }

    /// One commit built from `parent`'s tree + text `files`, on HEAD, both times
    /// pinned to `t`. Returns the new oid.
    fn mk_commit(
        repo: &git2::Repository,
        parent: Option<git2::Oid>,
        files: &[(&str, &str)],
        msg: &str,
        t: i64,
    ) -> git2::Oid {
        let sig = git2::Signature::new("Ada Lovelace", "ada@example.com", &git2::Time::new(t, 0))
            .expect("sig");
        let parent_commit = parent.map(|p| repo.find_commit(p).expect("parent"));
        let mut tb = match &parent_commit {
            Some(pc) => repo
                .treebuilder(Some(&pc.tree().expect("parent tree")))
                .expect("tb"),
            None => repo.treebuilder(None).expect("tb"),
        };
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100_644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .expect("commit")
    }

    /// A 4-commit linear fixture on `main`; commit 2 carries the unique keyword
    /// "zebracorn". Returns (repo dir, index dir, [c0..c3]).
    fn build_fixture() -> (tempfile::TempDir, tempfile::TempDir, [git2::Oid; 4]) {
        let (dir, repo) = init_scratch();
        let c0 = mk_commit(&repo, None, &[("a.txt", "alpha groundwork\n")], "seed alpha", 1000);
        let c1 = mk_commit(&repo, Some(c0), &[("b.txt", "beta body\n")], "add beta module", 2000);
        let c2 = mk_commit(
            &repo,
            Some(c1),
            &[("c.txt", "zebracorn special payload\n")],
            "wire the zebracorn subsystem",
            3000,
        );
        let c3 = mk_commit(&repo, Some(c2), &[("d.txt", "delta done\n")], "delta cleanup", 4000);
        let idx = crate::testutil::scratch_dir();
        (dir, idx, [c0, c1, c2, c3])
    }

    fn silent(_p: IndexProgress) {}

    // ---------------------------------------------------- §7.6 build + search

    #[test]
    fn build_then_search_finds_expected() {
        let (dir, idx, [_c0, _c1, c2, _c3]) = build_fixture();
        let status = build_index(dir.path(), idx.path(), silent).expect("build");
        assert!(status.built);
        assert_eq!(status.indexed_commits, 4);
        assert!(!status.stale);
        assert_eq!(status.new_commits, 0);

        let store = store::load(idx.path()).expect("load store");
        let ranked = bm25::rank(&store.bm25, &store.docs, &tokenize("zebracorn"), 20);
        assert!(!ranked.is_empty(), "the keyword commit is retrieved");
        assert_eq!(ranked[0].0, c2.to_string(), "unique-term commit ranks first");
    }

    // ---------------------------------------------------- §7.7 incremental

    #[test]
    fn incremental_build_only_documents_new() {
        let (dir, idx, [_c0, _c1, _c2, c3]) = build_fixture();
        build_index(dir.path(), idx.path(), silent).expect("build 1");
        let count1 = store::load(idx.path()).expect("load 1").docs.len();
        assert_eq!(count1, 4);

        // Append one commit, then rebuild.
        let repo = git2::Repository::open(dir.path()).expect("open");
        mk_commit(&repo, Some(c3), &[("e.txt", "epsilon\n")], "add epsilon", 5000);
        let mut ticks: Vec<IndexProgress> = Vec::new();
        build_index(dir.path(), idx.path(), |p| ticks.push(p)).expect("build 2");

        let count2 = store::load(idx.path()).expect("load 2").docs.len();
        assert_eq!(count2, count1 + 1, "exactly one new doc added");

        let counting = ticks
            .iter()
            .find(|p| matches!(p.phase, IndexPhase::Counting))
            .expect("a Counting tick");
        assert_eq!(counting.new_commits, 1, "only the new commit is documented");
        assert_eq!(counting.total, 1, "existing docs are NOT re-extracted");
    }

    // ---------------------------------------------------- §7.8 schema bump

    #[test]
    fn schema_bump_forces_full_rebuild() {
        let (dir, idx, _) = build_fixture();
        build_index(dir.path(), idx.path(), silent).expect("build");

        // Downgrade the on-disk schema; the next build must discard + rebuild all.
        let mut store = store::load(idx.path()).expect("load");
        store.schema = HISTORY_INDEX_SCHEMA - 1;
        store::save(idx.path(), &store).expect("resave old schema");

        let mut ticks: Vec<IndexProgress> = Vec::new();
        build_index(dir.path(), idx.path(), |p| ticks.push(p)).expect("rebuild");
        let counting = ticks
            .iter()
            .find(|p| matches!(p.phase, IndexPhase::Counting))
            .expect("a Counting tick");
        assert_eq!(counting.new_commits, 4, "all commits re-documented");
        assert_eq!(
            store::load(idx.path()).expect("load").schema,
            HISTORY_INDEX_SCHEMA,
            "schema stamped back to current"
        );
    }

    // ---------------------------------------------------- §7.9 staleness

    #[test]
    fn status_reports_stale_after_new_ref() {
        let (dir, idx, [_c0, _c1, _c2, c3]) = build_fixture();
        build_index(dir.path(), idx.path(), silent).expect("build");

        let fresh = index_status(dir.path(), idx.path()).expect("status");
        assert!(fresh.built);
        assert!(!fresh.stale, "just built => not stale");
        assert_eq!(fresh.new_commits, 0);
        assert_eq!(fresh.indexed_commits, 4);

        // A new commit moves main's tip and adds a reachable-but-unindexed oid.
        let repo = git2::Repository::open(dir.path()).expect("open");
        mk_commit(&repo, Some(c3), &[("f.txt", "zeta\n")], "add zeta", 6000);
        let stale = index_status(dir.path(), idx.path()).expect("status 2");
        assert!(stale.stale, "moved ref tip => stale");
        assert!(stale.new_commits >= 1, "the new commit is not yet indexed");

        // A fresh build clears staleness.
        build_index(dir.path(), idx.path(), silent).expect("rebuild");
        let cleared = index_status(dir.path(), idx.path()).expect("status 3");
        assert!(!cleared.stale);
        assert_eq!(cleared.new_commits, 0);
        assert_eq!(cleared.indexed_commits, 5);
    }

    #[test]
    fn status_of_missing_index_is_not_built() {
        // A missing store returns not-built BEFORE opening the repo, so no fixture
        // is needed — the index dir simply does not exist.
        let dir = crate::testutil::scratch_dir();
        let status = index_status(dir.path(), &dir.path().join("absent")).expect("status");
        assert!(!status.built);
        assert_eq!(status.indexed_commits, 0);
        assert_eq!(status.schema, 0);
        assert!(status.head_oid.is_none());
    }

    #[test]
    fn index_dir_for_is_under_history_index() {
        let base = Path::new("/data/app");
        let dir = index_dir_for(base, Path::new("/home/user/repo"));
        assert!(dir.starts_with(base.join("history-index")));
        // The leaf is the repo key (stable for the same workdir).
        assert_eq!(
            dir,
            index_dir_for(base, Path::new("/home/user/repo/")),
            "trailing slash normalizes to the same dir"
        );
    }

    // ---------------------------------------------------- wire shapes

    #[test]
    fn index_progress_wire_shape_is_camel_case() {
        let v = serde_json::to_value(IndexProgress {
            phase: IndexPhase::Extracting,
            processed: 5,
            total: 10,
            new_commits: 3,
        })
        .expect("json");
        assert_eq!(v["phase"], "extracting");
        assert_eq!(v["processed"], 5);
        assert_eq!(v["total"], 10);
        assert_eq!(v["newCommits"], 3);
    }

    #[test]
    fn index_status_wire_shape_is_camel_case() {
        let base = IndexStatus {
            built: true,
            indexed_commits: 7,
            head_oid: None,
            stale: false,
            new_commits: 0,
            schema: 1,
            built_at: Some(42),
        };
        let v = serde_json::to_value(&base).expect("json");
        assert_eq!(v["built"], true);
        assert_eq!(v["indexedCommits"], 7);
        assert_eq!(v["headOid"], serde_json::Value::Null, "None -> null");
        assert_eq!(v["newCommits"], 0);
        assert_eq!(v["schema"], 1);
        assert_eq!(v["builtAt"], 42);

        let some = IndexStatus {
            head_oid: Some("deadbeef".to_string()),
            ..base
        };
        assert_eq!(serde_json::to_value(&some).expect("json")["headOid"], "deadbeef");
    }
}
