//! P57c scratch-repo tests for `answer_history` (contract §7.12/§7.13,
//! end-to-end).
//!
//! Drives a real git2 fixture + a persisted BM25 index with the local `claude`
//! CLI replaced by the committed stub (`tests/fixtures/claude_stub.cmd` / `.sh`)
//! selected via `BONSAI_CLAUDE_BIN` + `BONSAI_STUB_MODE`. No network, no real
//! CLI. Mirrors the `ai_compose_cli.rs` harness and lives in its OWN test binary
//! so the process-global `BONSAI_CLAUDE_BIN` cannot race the lib unit tests.
//!
//! Proves: (1) the grounding payload reaching the CLI's stdin carries the labeled
//! QUESTION / RELEVANT COMMITS / TOP MATCHES IN DETAIL sections and the REAL
//! first-parent diff for the top match (the WHY-not-WHAT grounding), and the
//! returned `HistoryAnswer` carries the retrieved set + parsed cost; (2) a MISSING
//! index resolves to `AiFailed` BEFORE any CLI call (OQ3 — the CLI is never
//! spawned).
//!
//! Index build + retrieval + diff re-fetch are pure git2 (no `git` CLI), so these
//! tests do not depend on `git` being on PATH.

mod common;

use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_history::answer_history;
use bonsai_core::git::history_index::build_index;

const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
const STDIN_DUMP_ENV: &str = "BONSAI_STUB_STDIN_DUMP";

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
/// process-global and the stub inherits them, so parallel tests would race.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// git2-init a `main`-headed scratch repo with pinned identity + autocrlf off.
fn init_scratch() -> (tempfile::TempDir, git2::Repository) {
    let dir = common::scratch_dir();
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

/// One commit from `parent`'s tree + text `files`, on HEAD, both times pinned to
/// `t`. Returns the new oid.
fn mk_commit(
    repo: &git2::Repository,
    parent: Option<git2::Oid>,
    files: &[(&str, &str)],
    msg: &str,
    t: i64,
) -> git2::Oid {
    let sig =
        git2::Signature::new("Ada Lovelace", "ada@example.com", &git2::Time::new(t, 0)).expect("sig");
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

/// A 4-commit fixture on `main` (commit 2 carries "zebracorn") + a freshly-built
/// index. Returns (repo dir, index dir), both held for the test's lifetime.
fn fixture_with_index() -> (tempfile::TempDir, tempfile::TempDir) {
    let (dir, repo) = init_scratch();
    let c0 = mk_commit(&repo, None, &[("a.txt", "alpha\n")], "seed alpha", 1000);
    let c1 = mk_commit(&repo, Some(c0), &[("b.txt", "beta\n")], "add beta", 2000);
    let c2 = mk_commit(
        &repo,
        Some(c1),
        &[("c.txt", "zebracorn payload\n")],
        "wire the zebracorn subsystem",
        3000,
    );
    let _c3 = mk_commit(&repo, Some(c2), &[("d.txt", "delta\n")], "delta cleanup", 4000);
    let idx = common::scratch_dir();
    build_index(dir.path(), idx.path(), |_p| {}).expect("build index");
    (dir, idx)
}

/// A fixture where THREE commits share the term "zebracorn" (in their messages)
/// alongside two that do not, plus a freshly-built index. Lets a test prove that
/// the retrieval depth for `top_k = 0` is the DEFAULT (many), not one. Returns
/// (repo dir, index dir), both held for the test's lifetime.
fn fixture_shared_term_index() -> (tempfile::TempDir, tempfile::TempDir) {
    let (dir, repo) = init_scratch();
    let c0 = mk_commit(&repo, None, &[("a.txt", "alpha\n")], "seed alpha", 1000);
    let c1 = mk_commit(&repo, Some(c0), &[("b.txt", "beta\n")], "wire the zebracorn intake", 2000);
    let c2 = mk_commit(
        &repo,
        Some(c1),
        &[("c.txt", "gamma\n")],
        "extend the zebracorn pipeline",
        3000,
    );
    let c3 = mk_commit(&repo, Some(c2), &[("d.txt", "delta\n")], "tune the zebracorn cache", 4000);
    let _c4 = mk_commit(&repo, Some(c3), &[("e.txt", "epsilon\n")], "unrelated cleanup", 5000);
    let idx = common::scratch_dir();
    build_index(dir.path(), idx.path(), |_p| {}).expect("build index");
    (dir, idx)
}

/// P57c regression (reviewer MUST-FIX): `top_k = 0` is the "default depth"
/// sentinel (⇒ `DEFAULT_TOP_K`), NOT "one commit". `answer_history(top_k = 0)`
/// over a fixture with THREE matching commits must retrieve ALL THREE — locking
/// that the `0` sentinel resolves to the default depth through the whole AI path
/// (`answer_history` → `search_history` → `effective_top_k`). A min-1 clamp
/// anywhere on that path would collapse the retrieved set back to a single commit
/// and fail this assertion.
#[test]
fn answer_history_top_k_zero_retrieves_default_depth_not_one() {
    let _g = env_lock();
    let (dir, idx) = fixture_shared_term_index();

    // Default (`:success`) stub path — we assert on the RETRIEVED set, not stdin.
    std::env::set_var(CLAUDE_BIN_ENV, common::claude_stub_path());
    std::env::remove_var(STUB_MODE_ENV);

    let answer = answer_history(dir.path(), idx.path(), "why zebracorn?", 0, RunOpts::default())
        .expect("answer (success stub, top_k = 0)");

    std::env::remove_var(CLAUDE_BIN_ENV);

    assert!(
        answer.retrieved.len() > 1,
        "top_k = 0 must not collapse to a single commit: {:?}",
        answer.retrieved
    );
    assert_eq!(
        answer.retrieved.len(),
        3,
        "top_k = 0 ⇒ default depth: ALL THREE matching commits are retrieved: {:?}",
        answer.retrieved
    );
    assert!(
        answer.retrieved.iter().all(|h| h.summary.contains("zebracorn")),
        "every retrieved commit matches the query term: {:?}",
        answer.retrieved
    );
}

/// §7.12: over a built index, `answer_history` grounds the CLI stdin (labeled
/// sections + the top match's REAL first-parent diff) and returns a
/// `HistoryAnswer` with the retrieved set + the parsed cost.
#[test]
fn answer_history_grounds_stdin_and_returns_retrieved() {
    let _g = env_lock();
    let (dir, idx) = fixture_with_index();

    // Capture the stdin the stub receives; `dump_stdin` still emits the success
    // envelope body ("MERGED_BODY_OK", cost 0.012).
    let dump = dir.path().join("stdin_dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, common::claude_stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);

    let answer = answer_history(dir.path(), idx.path(), "why zebracorn?", 20, RunOpts::default())
        .expect("answer (dump_stdin stub)");

    std::env::remove_var(STDIN_DUMP_ENV);
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);

    // The stub returned the canned success envelope; the retrieved set is echoed.
    assert_eq!(answer.text, "MERGED_BODY_OK");
    assert_eq!(answer.cost_usd, Some(0.012), "cost parsed from the stub envelope");
    assert!(!answer.retrieved.is_empty(), "the retrieved set is populated");
    assert!(
        answer.retrieved.iter().any(|h| h.summary.contains("zebracorn")),
        "the zebracorn commit is among the retrieved: {:?}",
        answer.retrieved
    );

    // The grounding payload reached the CLI's stdin: labeled sections + the real
    // first-parent diff (`===== FILE:` + the added line) for the top match.
    // NB: the Windows `find.exe` stdin capture re-emits with CRLF, so assert on
    // single-line substrings only (never a newline-spanning slice); the exact
    // `QUESTION:\n<question>` shape is covered by the LF-exact unit test.
    let payload = std::fs::read_to_string(&dump).expect("stub wrote the stdin dump");
    assert!(payload.contains("QUESTION:"), "{payload}");
    assert!(payload.contains("why zebracorn?"), "{payload}");
    assert!(
        payload.contains("RELEVANT COMMITS (most relevant first):"),
        "{payload}"
    );
    assert!(payload.contains("===== TOP MATCHES IN DETAIL ====="), "{payload}");
    assert!(
        payload.contains("MESSAGE:") && payload.contains("CHANGES:"),
        "{payload}"
    );
    assert!(payload.contains("===== FILE: c.txt"), "{payload}");
    assert!(payload.contains("+zebracorn payload"), "{payload}");
}

/// §7.13: a MISSING index resolves to `AiFailed` BEFORE any CLI call. The fake
/// bin is a NON-EXISTENT path: if the guard regressed and the CLI were reached,
/// `run_claude`'s spawn would fail `NotFound` → `AiUnavailable` (a DIFFERENT
/// kind), so this test fails loudly rather than making a real `claude` call.
#[test]
fn answer_history_no_index_fails_before_cli() {
    let _g = env_lock();
    let (dir, _repo) = init_scratch();

    std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/bonsai-claude-must-not-run.exe");
    std::env::remove_var(STUB_MODE_ENV);
    let missing_index = dir.path().join("no-index-here");

    let err = answer_history(dir.path(), &missing_index, "anything", 20, RunOpts::default())
        .expect_err("a missing index must fail before any CLI call");

    std::env::remove_var(CLAUDE_BIN_ENV);

    match err {
        AppError::AiFailed(m) => assert!(
            m.contains("not built"),
            "expected the no-index guard message, got: {m}"
        ),
        other => panic!("expected AiFailed (guard fires before the CLI), got {other:?}"),
    }
}
