//! T2 Area 9 — history-index integration tests (contract §3 Area 9).
//!
//! Store persistence + repo_key + concurrent-tmp isolation have strong inline
//! coverage in `git/history_index/{store,bm25}.rs`. This file exercises the
//! PUBLIC build/status/search API end-to-end: garbage-store resilience, the
//! F-A9-2 ghost-doc pruning after a history rewrite, an adversarial VALID store
//! (finite/non-panic), concurrent build+search, deterministic tokenization, and
//! the unborn/empty repo. Scratch on D:. Skips (passes with a note) w/o `git`.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use bonsai_core::git::history_index::{
    self, bm25::Bm25Index, store, tokenize, CommitDoc, HistoryQuery, IndexStore,
};
use crate::common;
use crate::common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// No-op progress sink for `build_index`.
fn noprog(_p: history_index::IndexProgress) {}

fn build(workdir: &Path, index_dir: &Path) -> history_index::IndexStatus {
    history_index::build_index(workdir, index_dir, noprog).expect("build_index")
}

fn search(index_dir: &Path, text: &str) -> history_index::HistorySearchResults {
    history_index::search_history(
        Path::new("."),
        index_dir,
        &HistoryQuery { text: text.to_string(), top_k: 20 },
    )
    .expect("search_history")
}

/// A repo with two commits carrying distinctive message tokens.
fn repo_two_commits() -> (tempfile::TempDir, String) {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "a\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "alpha keepmetoken widget");
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "beta ghosttoken sprocket");
    let head = git(p, &["rev-parse", "HEAD"]);
    (dir, head)
}

// -------------------------------------------------- garbage store resilience

/// Every flavor of a corrupt on-disk store makes `index_status` report
/// not-built and `search_history` return empty+stale, never panics, and a
/// subsequent `build_index` rebuilds cleanly.
#[test]
fn garbage_store_not_built_search_empty_then_rebuilds() {
    require_git!();
    let (dir, _head) = repo_two_commits();
    let workdir = dir.path();

    // schema+1: a well-formed store at an unknown schema.
    let mut future = IndexStore::empty();
    future.schema = history_index::HISTORY_INDEX_SCHEMA + 1;
    let future_json = serde_json::to_vec(&future).unwrap();

    let garbage: Vec<(&str, Vec<u8>)> = vec![
        ("zero_byte", Vec::new()),
        ("truncated", br#"{"schema":1,"docs":{"#.to_vec()),
        ("random_4mb", (0u32..(4 * 1024 * 1024)).map(|n| (n.wrapping_mul(2654435761) >> 15) as u8).collect()),
        ("wrong_shape", br#"{"not":"an index","list":[1,2,3]}"#.to_vec()),
        ("schema_plus_one", future_json),
    ];

    for (label, bytes) in garbage {
        let idx = dir.path().join(format!("idx-{label}"));
        std::fs::create_dir_all(&idx).unwrap();
        std::fs::write(idx.join(store::STORE_FILE), &bytes).unwrap();

        // Status: not built (never a panic on garbage bytes).
        let st = history_index::index_status(workdir, &idx).expect("status on garbage");
        assert!(!st.built, "[{label}] garbage store must report not-built");

        // Search: empty + stale hint.
        let res = search(&idx, "ghosttoken");
        assert!(res.hits.is_empty(), "[{label}] no hits from a garbage store");
        assert!(res.index_stale, "[{label}] stale hint offered");

        // Rebuild: recovers to a real index.
        let after = build(workdir, &idx);
        assert!(after.built, "[{label}] build rebuilds over garbage");
        assert_eq!(after.indexed_commits, 2, "[{label}] both commits indexed after rebuild");
        assert!(!search(&idx, "ghosttoken").hits.is_empty(), "[{label}] search works post-rebuild");
    }
}

// -------------------------------------------------- F-A9-2 ghost-doc pruning

/// After a history rewrite (amend) the old commit's doc is pruned — search
/// never returns the dead oid and the indexed count stays at the reachable set.
#[test]
fn history_rewrite_prunes_ghost_docs() {
    require_git!();
    let (dir, old_head) = repo_two_commits();
    let workdir = dir.path();
    let idx = dir.path().join("idx");

    let first = build(workdir, &idx);
    assert_eq!(first.indexed_commits, 2);
    // The ghost token is findable and points at the soon-to-be-rewritten commit.
    let before = search(&idx, "ghosttoken");
    assert_eq!(before.hits.first().map(|h| h.oid.as_str()), Some(old_head.as_str()));

    // Rewrite HEAD: amend the message, dropping the ghost token.
    common::git_env(
        workdir,
        &["commit", "--amend", "-m", "beta cleaned sprocket"],
        &[("GIT_AUTHOR_DATE", common::FIXED_DATE), ("GIT_COMMITTER_DATE", common::FIXED_DATE)],
    );
    let new_head = git(workdir, &["rev-parse", "HEAD"]);
    assert_ne!(new_head, old_head, "amend produced a new oid");

    let after = build(workdir, &idx);
    assert_eq!(after.indexed_commits, 2, "still exactly 2 reachable docs (no ghost accretion)");
    // The dead oid's token is gone; the new message is searchable at the new oid.
    assert!(search(&idx, "ghosttoken").hits.is_empty(), "ghost token pruned");
    let cleaned = search(&idx, "cleaned");
    assert_eq!(cleaned.hits.first().map(|h| h.oid.as_str()), Some(new_head.as_str()),
        "new message indexed at the new oid");
    // No hit ever references the dead oid.
    assert!(!cleaned.hits.iter().any(|h| h.oid == old_head), "no dead oid surfaces");
}

// ------------------------------------------ adversarial VALID store: finite

/// A hand-built store with impossible stats (avgdl:0, df>n, dl:0) must not
/// panic and must yield only finite scores — the div-by-zero NaN path is
/// filtered by `rank`, never surfaced.
#[test]
fn adversarial_valid_store_is_finite_non_panic() {
    let dir = common::scratch_dir();
    let idx = dir.path().join("idx");

    let mut docs: BTreeMap<String, CommitDoc> = BTreeMap::new();
    let mut tf = HashMap::new();
    tf.insert("term".to_string(), 2u16);
    docs.insert(
        "a".repeat(40),
        CommitDoc { summary: "s".into(), author_name: "n".into(), author_ts: 1, dl: 0, tf },
    );
    let mut df = HashMap::new();
    df.insert("term".to_string(), 5u32); // df > n (impossible)
    let store = IndexStore {
        schema: history_index::HISTORY_INDEX_SCHEMA,
        head_oid: None,
        tip_oids: Vec::new(),
        built_at: None,
        docs,
        bm25: Bm25Index { n: 1, avgdl: 0.0, df }, // avgdl 0 → NaN denom internally
    };
    history_index::store::save(&idx, &store).expect("save adversarial store");

    // Must not panic; every returned score is finite.
    let res = search(&idx, "term");
    assert!(res.hits.iter().all(|h| h.score.is_finite()), "no NaN/inf score leaks: {res:?}");
}

// -------------------------------------------------- concurrency: build+search

/// A build and a search racing on the same index dir both complete cleanly, and
/// two concurrent builds leave a loadable store.
#[test]
fn concurrent_build_and_search_are_safe() {
    require_git!();
    let (dir, _head) = repo_two_commits();
    let workdir = dir.path().to_path_buf();
    let idx = dir.path().join("idx");
    build(&workdir, &idx); // seed one store so search has something to read

    // build || search
    let (w1, i1) = (workdir.clone(), idx.clone());
    let (w2, i2) = (workdir.clone(), idx.clone());
    let b = std::thread::spawn(move || history_index::build_index(&w1, &i1, noprog));
    let s = std::thread::spawn(move || {
        history_index::search_history(&w2, &i2, &HistoryQuery { text: "widget".into(), top_k: 20 })
    });
    assert!(b.join().expect("build join").is_ok(), "concurrent build Ok");
    assert!(s.join().expect("search join").is_ok(), "concurrent search Ok");

    // build || build → store still loadable afterward.
    let (w3, i3) = (workdir.clone(), idx.clone());
    let (w4, i4) = (workdir.clone(), idx.clone());
    let b1 = std::thread::spawn(move || history_index::build_index(&w3, &i3, noprog));
    let b2 = std::thread::spawn(move || history_index::build_index(&w4, &i4, noprog));
    let _ = b1.join().expect("b1");
    let _ = b2.join().expect("b2");
    let st = history_index::index_status(&workdir, &idx).expect("status after two builds");
    assert!(st.built && st.indexed_commits == 2, "store intact after concurrent builds: {st:?}");
}

// -------------------------------------------------- tokenizer determinism

/// `tokenize` is deterministic and handles unicode / CJK / emoji without panic.
#[test]
fn tokenize_unicode_cjk_emoji_deterministic() {
    let input = "Fix café login 🚀 中文 日本語 CJK-tokens_here";
    let a = tokenize(input);
    let b = tokenize(input);
    assert_eq!(a, b, "tokenization is deterministic");
    assert!(!a.is_empty(), "produces tokens: {a:?}");
    // A pure-emoji / whitespace string never panics and yields no useful tokens.
    let _ = tokenize("🚀🎉  \t\n");
    let _ = tokenize("");
    // Lowercasing: no uppercase ASCII survives.
    assert!(a.iter().all(|t| t == &t.to_lowercase()), "tokens lowercased: {a:?}");
}

// -------------------------------------------------- unborn / empty repo

/// An unborn-HEAD (empty) repo builds an empty index cleanly; search is empty.
#[test]
fn unborn_empty_repo_builds_empty_index() {
    require_git!();
    let dir = init_repo(); // no commits → unborn HEAD
    let workdir = dir.path();
    let idx = dir.path().join("idx");

    let st = build(workdir, &idx);
    assert!(st.built, "empty repo still produces a (built) index");
    assert_eq!(st.indexed_commits, 0, "no commits → 0 docs");
    assert!(search(&idx, "anything").hits.is_empty(), "empty index → no hits");
}

// -------------------------------------------------- repo_key unicode/case

/// `repo_key` is a stable digest for unicode paths, and separator-insensitive
/// everywhere. The CASE half is driven by the injected `ignorecase` flag rather
/// than by `cfg!(windows)`, so BOTH filesystem behaviours are asserted on every
/// host — the previous `#[cfg(windows)]` gate gave macOS (APFS, case-insensitive
/// by default) zero coverage of the folding it actually needs.
#[test]
fn repo_key_unicode_and_case() {
    let uni_path = Path::new("/tmp/reposé/日本語-project");
    for ignorecase in [true, false] {
        let uni = store::repo_key(uni_path, ignorecase);
        assert_eq!(uni.len(), 16, "16 hex chars");
        assert_eq!(uni, store::repo_key(uni_path, ignorecase), "stable");
        assert_ne!(
            uni,
            store::repo_key(Path::new("/tmp/reposé/other"), ignorecase),
            "distinct paths differ"
        );
        // Separators normalize regardless of case sensitivity.
        assert_eq!(
            store::repo_key(Path::new("D:\\Repos\\Café"), ignorecase),
            store::repo_key(Path::new("D:/Repos/Café"), ignorecase),
            "separator normalized"
        );
    }
    // Case-insensitive FS (Windows, macOS APFS by default): one key for both
    // casings, non-ASCII included. Case-sensitive FS (ext4): distinct dirs.
    assert_eq!(
        store::repo_key(Path::new("D:\\Repos\\Café"), true),
        store::repo_key(Path::new("d:/repos/café"), true),
        "case folded when ignorecase"
    );
    assert_ne!(
        store::repo_key(Path::new("D:\\Repos\\Café"), false),
        store::repo_key(Path::new("d:/repos/café"), false),
        "case preserved when the FS is case-sensitive"
    );
}

/// End-to-end: the index dir a REAL repo resolves to is keyed off git's own
/// `core.ignorecase`, not the build target — the bug that split the history-index
/// cache per path-casing on macOS APFS. Runs on EVERY host because the
/// expectation is derived from the injected/detected flag, not from `cfg!`.
#[test]
fn index_dir_follows_git_ignorecase_not_build_target() {
    require_git!();
    let dir = init_repo();
    let workdir = dir.path();
    let base = workdir.join("appdata");

    // The production helper resolves exactly the detected `core.ignorecase`.
    let detected = bonsai_core::git::repo::path_ignorecase(workdir);
    assert_eq!(
        history_index::index_dir_for_repo(&base, workdir),
        history_index::index_dir_for(&base, workdir, detected),
        "index_dir_for_repo == index_dir_for(detected ignorecase)"
    );

    for ignorecase in [true, false] {
        let value = if ignorecase { "true" } else { "false" };
        git(workdir, &["config", "core.ignorecase", value]);
        assert_eq!(
            bonsai_core::git::repo::path_ignorecase(workdir),
            ignorecase,
            "path_ignorecase reads core.ignorecase = {value}"
        );

        // Case variants of one workdir share an index dir IFF the filesystem
        // is case-insensitive (otherwise they are genuinely different repos).
        let shared = history_index::index_dir_for(&base, Path::new("/repos/Bonsai"), ignorecase)
            == history_index::index_dir_for(&base, Path::new("/repos/bonsai"), ignorecase);
        assert_eq!(shared, ignorecase, "case-variant index dirs (ignorecase={value})");
    }
}
