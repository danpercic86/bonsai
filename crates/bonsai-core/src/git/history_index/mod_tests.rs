//! Unit tests for [`super`] (`history_index/mod.rs`) — kept in a sibling file
//! so the module itself stays under the ~500-line soft limit. Declared with
//! `#[path]` as a child module of `history_index`, so `super::*` still reaches
//! the private helpers without widening their visibility (the `external_tests`
//! / `session_drain_tests` convention).

use super::*;
use std::collections::HashMap;

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

    // End-to-end through the P57b public API (git-built store -> retrieval):
    // the term unique to commit 2 returns that oid first.
    let results = search_history(
        dir.path(),
        idx.path(),
        &HistoryQuery {
            text: "zebracorn".to_string(),
            top_k: 0,
        },
    )
    .expect("search");
    assert!(!results.hits.is_empty(), "the keyword commit is retrieved");
    assert_eq!(results.hits[0].oid, c2.to_string(), "unique-term commit ranks first");
    assert!(!results.index_stale);
    assert_eq!(results.indexed_commits, 4);
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

// ---------------------------------------------------- F-A9-2 ghost prune

/// A rebuild drops docs whose oid is no longer reachable (rewritten-away /
/// GC'd), so search never returns dead oids and idf isn't skewed.
#[test]
fn build_prunes_ghost_docs_absent_from_reachable() {
    let (dir, idx, _) = build_fixture();
    build_index(dir.path(), idx.path(), silent).expect("build 1");
    let mut store = store::load(idx.path()).expect("load");
    assert_eq!(store.docs.len(), 4);

    // Inject a ghost doc for an oid that is NOT in the repo (as if a commit
    // were rewritten away since the last build).
    let ghost = "f".repeat(40);
    store.docs.insert(
        ghost.clone(),
        CommitDoc {
            summary: "ghost rewritten commit".to_string(),
            author_name: "Nobody".to_string(),
            author_ts: 1,
            dl: 3,
            tf: HashMap::new(),
        },
    );
    store::save(idx.path(), &store).expect("save with ghost");
    assert_eq!(store::load(idx.path()).expect("reload").docs.len(), 5);

    // The next build prunes the ghost and keeps the 4 real docs.
    build_index(dir.path(), idx.path(), silent).expect("build 2");
    let after = store::load(idx.path()).expect("load after");
    assert_eq!(after.docs.len(), 4, "ghost pruned");
    assert!(!after.docs.contains_key(&ghost), "ghost oid dropped");
}

// ---------------------------------------------------- F-A9-3 skip-and-go

/// One unreadable object skips-and-counts instead of aborting the whole
/// build: a corrupt loose blob for c.txt makes extracting c2 fail, but the
/// other three commits still index and the build returns `Ok`.
#[test]
#[allow(clippy::permissions_set_readonly_false)] // test-only: un-protect a git object to corrupt it
fn build_skips_unreadable_object_and_indexes_the_rest() {
    let (dir, idx, [_c0, _c1, c2, _c3]) = build_fixture();

    // Locate + corrupt the loose blob object for c.txt ("zebracorn ...").
    let repo = git2::Repository::open(dir.path()).expect("open");
    let blob_oid = repo.blob(b"zebracorn special payload\n").expect("hash blob");
    let hex = blob_oid.to_string();
    let obj_path = dir
        .path()
        .join(".git")
        .join("objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    assert!(obj_path.exists(), "loose blob present: {}", obj_path.display());
    // Clear any read-only bit git set (Windows attr / *nix 0444) before overwrite.
    let mut perms = std::fs::metadata(&obj_path).expect("meta").permissions();
    perms.set_readonly(false);
    let _ = std::fs::set_permissions(&obj_path, perms);
    std::fs::write(&obj_path, b"corrupt-not-zlib").expect("corrupt object");

    let status =
        build_index(dir.path(), idx.path(), silent).expect("build tolerates bad object");
    assert!(status.built);
    let store = store::load(idx.path()).expect("load");
    assert_eq!(store.docs.len(), 3, "3 good commits indexed, corrupt one skipped");
    assert!(
        !store.docs.contains_key(&c2.to_string()),
        "the unreadable commit is skipped"
    );
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
    for ignorecase in [true, false] {
        let dir = index_dir_for(base, Path::new("/home/user/repo"), ignorecase);
        assert!(dir.starts_with(base.join("history-index")));
        // The leaf is the repo key (stable for the same workdir).
        assert_eq!(
            dir,
            index_dir_for(base, Path::new("/home/user/repo/"), ignorecase),
            "trailing slash normalizes to the same dir"
        );
    }
    // Case folding follows the injected flag, NOT the build target: a
    // case-insensitive workdir (Windows / macOS APFS) must resolve one dir.
    assert_eq!(
        index_dir_for(base, Path::new("/home/user/Repo"), true),
        index_dir_for(base, Path::new("/home/user/repo"), true)
    );
    assert_ne!(
        index_dir_for(base, Path::new("/home/user/Repo"), false),
        index_dir_for(base, Path::new("/home/user/repo"), false)
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
        skipped_commits: 3,
    };
    let v = serde_json::to_value(&base).expect("json");
    assert_eq!(v["built"], true);
    assert_eq!(v["indexedCommits"], 7);
    assert_eq!(v["headOid"], serde_json::Value::Null, "None -> null");
    assert_eq!(v["newCommits"], 0);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["builtAt"], 42);
    assert_eq!(v["skippedCommits"], 3);

    let some = IndexStatus {
        head_oid: Some("deadbeef".to_string()),
        ..base
    };
    assert_eq!(serde_json::to_value(&some).expect("json")["headOid"], "deadbeef");
}
