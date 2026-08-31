//! P13c scratch-repo tests for `ai_resolve_conflict` (contract §10.3).
//!
//! Drives real git2 merge conflicts on scratch repos (reusing the `common`
//! `init_repo`/`git`/`commit_fixed` harness), with the local `claude` CLI
//! replaced by the committed stub (`tests/fixtures/claude_stub.cmd`) selected
//! via `BONSAI_CLAUDE_BIN` + `BONSAI_STUB_MODE`. No network, no real CLI.
//!
//! Proves: (1) a proposal is produced and WRITES NOTHING; (2) feeding the
//! proposed text to `resolve_conflict_text` clears the conflict and lets
//! `commit_merge` finalize a clean 2-parent commit; (3) binary/too-large/
//! deletion-kind conflicts short-circuit to `AiFailed` before any CLI call;
//! (4) non-conflicted path → `git`, `../escape` → `invalidName`.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::{run_claude, RunOpts, DEFAULT_MODEL};
use bonsai_core::error::AppError;
use bonsai_core::git::ai_resolve::ai_resolve_conflict;
use bonsai_core::git::conflict::{resolve_conflict_text, MAX_CONFLICT_BYTES};
use bonsai_core::git::merge::{commit_merge, merge_branch, MergeOutcome};
use common::{commit_fixed, git, init_repo};

const STUB_BODY: &str = "MERGED_BODY_OK";
const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
/// process-global and the stub inherits them, so parallel tests would race.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn stub_path() -> std::path::PathBuf {
    common::claude_stub_path()
}

/// Point the AI layer at the committed stub in `success` mode.
fn set_success_stub() {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "success");
}

/// Point the AI layer at the committed stub in an arbitrary `BONSAI_STUB_MODE`.
fn set_stub_mode(mode: &str) {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, mode);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// True iff `path` currently has an index conflict record (any stage).
fn is_conflicted(dir: &Path, path: &str) -> bool {
    let repo = git2::Repository::open(dir).expect("open repo");
    let index = repo.index().expect("index");
    for c in index.conflicts().expect("conflicts") {
        let c = c.expect("conflict record");
        let p = c
            .our
            .as_ref()
            .or(c.their.as_ref())
            .or(c.ancestor.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).into_owned());
        if p.as_deref() == Some(path) {
            return true;
        }
    }
    false
}

/// Stage-0 blob bytes for `path`, or None when the path is still conflicted /
/// absent from the index at stage 0.
fn stage0_blob(dir: &Path, path: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(dir).expect("open repo");
    let index = repo.index().expect("index");
    let entry = index.get_path(Path::new(path), 0)?;
    let blob = repo.find_blob(entry.id).expect("blob");
    Some(blob.content().to_vec())
}

/// Parent oids of HEAD, in order.
fn parents(dir: &Path) -> Vec<String> {
    git(dir, &["log", "-1", "--format=%P"])
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// Builds a scratch repo paused in a `bothModified` merge conflict on `a.txt`
/// (ours = "main", theirs = "topic"), plus an untracked-safe tracked `keep.txt`.
fn both_modified_conflict() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "line1\nbase\nline3\n");
    write(d, "keep.txt", "keep\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");

    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { paths, .. } => {
            assert!(paths.iter().any(|p| p == "a.txt"), "expected a.txt conflict");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    dir
}

/// Builds a scratch repo paused in a `deletedByThem` conflict on `a.txt`
/// (topic deletes it, main modifies it).
fn deleted_by_them_conflict() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "base\n");
    write(d, "keep.txt", "keep\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    git(d, &["rm", "a.txt"]);
    commit_fixed(d, "topic deletes a.txt");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "modified by main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main modifies a.txt");

    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { paths, .. } => {
            assert!(paths.iter().any(|p| p == "a.txt"), "expected a.txt conflict");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    dir
}

// ============================================================ §10.3 (1) proposal writes nothing

#[test]
fn proposal_returns_stub_body_and_writes_nothing() {
    require_git!();
    let _g = env_lock();
    set_success_stub();

    let dir = both_modified_conflict();
    let d = dir.path();
    let before = std::fs::read(d.join("a.txt")).expect("read a.txt before");

    let proposal =
        ai_resolve_conflict(d, "a.txt", RunOpts::default()).expect("proposal on a text conflict");

    assert_eq!(proposal.path, "a.txt");
    assert_eq!(proposal.proposed_text, STUB_BODY, "text must be the stub body");
    assert_eq!(proposal.cost_usd, Some(0.012), "cost parsed from the envelope");

    // WRITES NOTHING: still conflicted, worktree bytes unchanged, no stage-0.
    assert!(is_conflicted(d, "a.txt"), "a.txt must still be conflicted");
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt after"),
        before,
        "worktree bytes must be untouched by a proposal"
    );
    assert!(
        stage0_blob(d, "a.txt").is_none(),
        "a conflicted path must have no stage-0 entry after a proposal"
    );
}

// ============================================================ §10.3 (2) apply + commit_merge

#[test]
fn applying_proposal_clears_conflict_and_commit_merge_finalizes() {
    require_git!();
    let _g = env_lock();
    set_success_stub();

    let dir = both_modified_conflict();
    let d = dir.path();
    let pre_head = git(d, &["rev-parse", "HEAD"]);

    let proposal = ai_resolve_conflict(d, "a.txt", RunOpts::default()).expect("proposal");

    // Apply via the EXISTING resolve_conflict_text primitive (no new command).
    resolve_conflict_text(d, "a.txt", &proposal.proposed_text).expect("apply proposal");

    // Conflict gone; stage-0 blob == the applied bytes; worktree bytes match.
    assert!(!is_conflicted(d, "a.txt"), "a.txt must no longer be conflicted");
    assert_eq!(
        stage0_blob(d, "a.txt").as_deref(),
        Some(proposal.proposed_text.as_bytes()),
        "stage-0 blob must equal the applied proposal bytes"
    );
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt"),
        proposal.proposed_text.as_bytes(),
        "worktree bytes must equal the applied proposal bytes"
    );

    // commit_merge finalizes a clean 2-parent merge commit.
    let result = commit_merge(d, "Merge branch 'topic'", None, false).expect("commit_merge");
    assert_eq!(result.oid, git(d, &["rev-parse", "HEAD"]));
    let p = parents(d);
    assert_eq!(p.len(), 2, "merge commit must have 2 parents");
    assert_eq!(p[0], pre_head, "first parent must be the pre-merge HEAD");
    assert_eq!(
        git2::Repository::open(d).expect("open").state(),
        git2::RepositoryState::Clean,
        "repo must be Clean after commit_merge"
    );
}

// ============================================================ §10.3 (3) guards short-circuit to AiFailed

#[test]
fn binary_too_large_and_deletion_kinds_short_circuit_to_ai_failed() {
    require_git!();
    let _g = env_lock();
    // The stub is pointed at even though the guards must fire BEFORE any CLI
    // call — a proposal here would prove the guard leaked.
    set_success_stub();

    // Binary worktree file.
    let dir = both_modified_conflict();
    std::fs::write(dir.path().join("a.txt"), b"\x00\x01binary blob").expect("write binary");
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("binary must be AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "got {err:?}");

    // Too large (> 1 MiB).
    let dir = both_modified_conflict();
    std::fs::write(
        dir.path().join("a.txt"),
        vec![b'a'; MAX_CONFLICT_BYTES as usize + 1],
    )
    .expect("write huge");
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("too_large must be AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "got {err:?}");

    // Deletion-kind conflict (deletedByThem) — no text merge.
    let dir = deleted_by_them_conflict();
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("deletion kind must be AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "got {err:?}");
}

// ============================================================ §10.3 (4) path guards

#[test]
fn non_conflicted_and_escape_paths_error() {
    require_git!();
    let _g = env_lock();
    set_success_stub();

    let dir = both_modified_conflict();
    let d = dir.path();

    // Non-conflicted (but valid) tracked path → git "has no conflict".
    let err = ai_resolve_conflict(d, "keep.txt", RunOpts::default())
        .expect_err("non-conflicted path must error");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // Traversal / absolute paths → invalidName (validated before get_conflict).
    for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
        let err = ai_resolve_conflict(d, bad, RunOpts::default()).expect_err("escape path");
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "path {bad:?}: expected InvalidName, got {err:?}"
        );
    }
}

// ============================================================ P13 tester: adversarial `ai_resolve` cases
//
// These extend §10.3 with the gaps the sub-increment tests skip: a proposal that
// still carries conflict markers, empty / whitespace-only CLI output, CRLF in the
// proposed body, and the default-model argv. All use the committed stub via
// `BONSAI_CLAUDE_BIN` + `BONSAI_STUB_MODE` — no real `claude`, no network.

// ---- Leftover conflict markers in the proposal ----
//
// DOCUMENTS ACTUAL BEHAVIOR (P3c/P12 trust model): `resolve_conflict_text` trusts
// the caller exactly like `git add` — it does NOT scan for or reject leftover
// `<<<<<<<`/`=======`/`>>>>>>>` markers. So if the model returns a body that is
// still conflicted and the frontend Save-gate is bypassed, the marker text is
// staged verbatim at stage 0 (the conflict record is still cleared). The
// frontend `hasUnresolvedMarkers` gate — NOT the backend — is what normally
// prevents this. This is the contract-implied behavior, not a bug.

/// The `result` body carried by the `success_markers` stub (see
/// `tests/fixtures/claude_envelope_markers.json`) after JSON `\n` unescaping.
const MARKER_BODY: &str = "<<<<<<< HEAD\nMINE\n=======\nTHEIRS\n>>>>>>> topic\n";

#[test]
fn leftover_markers_proposal_is_staged_verbatim_not_rejected() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("success_markers");

    let dir = both_modified_conflict();
    let d = dir.path();

    let proposal = ai_resolve_conflict(d, "a.txt", RunOpts::default())
        .expect("a markerful proposal is still a valid AiResult (run_claude does not scan markers)");
    assert_eq!(
        proposal.proposed_text, MARKER_BODY,
        "the proposal body carries leftover conflict markers verbatim"
    );
    assert!(
        proposal.proposed_text.contains("<<<<<<<")
            && proposal.proposed_text.contains("=======")
            && proposal.proposed_text.contains(">>>>>>>"),
        "sanity: all three marker kinds present in the proposal"
    );

    // Apply via the real primitive. It must NOT silently reject the markers.
    resolve_conflict_text(d, "a.txt", &proposal.proposed_text)
        .expect("resolve_conflict_text trusts the caller like `git add` — no marker rejection");

    // The conflict record is cleared (stage 0 written) even though markers remain
    // in the content — proving the trust model, not a marker-aware merge.
    assert!(
        !is_conflicted(d, "a.txt"),
        "conflict record cleared even with leftover markers (git-add trust model)"
    );
    assert_eq!(
        stage0_blob(d, "a.txt").as_deref(),
        Some(MARKER_BODY.as_bytes()),
        "the staged stage-0 blob is the marker text VERBATIM"
    );
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt"),
        MARKER_BODY.as_bytes(),
        "worktree bytes are the marker text verbatim"
    );
}

// ---- Empty / whitespace-only proposal → AiFailed (§3.3 step 4) ----

#[test]
fn empty_proposal_maps_to_ai_failed() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("empty");

    let dir = both_modified_conflict();
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("empty `result` must be AiFailed('Claude returned no output')");
    match err {
        AppError::AiFailed(m) => assert!(
            m.contains("no output"),
            "expected the empty-output message, got: {m}"
        ),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    // WRITES NOTHING on failure: a.txt is still conflicted.
    assert!(is_conflicted(dir.path(), "a.txt"), "still conflicted after a failed proposal");
}

#[test]
fn whitespace_only_proposal_maps_to_ai_failed() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("whitespace");

    let dir = both_modified_conflict();
    let err = ai_resolve_conflict(dir.path(), "a.txt", RunOpts::default())
        .expect_err("whitespace-only `result` trims to empty → AiFailed");
    match err {
        AppError::AiFailed(m) => assert!(
            m.contains("no output"),
            "whitespace-only trims to empty; expected the empty-output message, got: {m}"
        ),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    assert!(is_conflicted(dir.path(), "a.txt"), "still conflicted after a failed proposal");
}

// ---- CRLF in the proposed body ----
//
// DOCUMENTS ACTUAL NEWLINE BEHAVIOR: `resolve_conflict_text` does `fs::write` of
// the bytes verbatim and `index.add_path`. `init_repo` sets `core.autocrlf=false`
// (see tests/common/mod.rs), so libgit2 applies NO CRLF filter — the staged blob
// keeps the CR bytes exactly as proposed. This mirrors the existing
// `applying_proposal_clears_conflict_...` test (stage-0 blob == proposed bytes);
// it asserts NO normalization the code does not perform.

/// The `result` body carried by the `success_crlf` stub after JSON unescaping.
const CRLF_BODY: &str = "L1\r\nL2\r\nL3\r\n";

#[test]
fn crlf_proposal_is_staged_verbatim_no_normalization() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("success_crlf");

    let dir = both_modified_conflict();
    let d = dir.path();

    let proposal = ai_resolve_conflict(d, "a.txt", RunOpts::default()).expect("crlf proposal");
    assert_eq!(
        proposal.proposed_text, CRLF_BODY,
        "the proposed body keeps its CRLF line endings through parse + strip_fence"
    );
    assert!(
        proposal.proposed_text.contains("\r\n"),
        "sanity: CR bytes present in the proposal"
    );

    resolve_conflict_text(d, "a.txt", &proposal.proposed_text).expect("apply crlf proposal");

    assert!(!is_conflicted(d, "a.txt"), "conflict cleared");
    // With core.autocrlf=false, the staged blob keeps the CR bytes verbatim.
    assert_eq!(
        stage0_blob(d, "a.txt").as_deref(),
        Some(CRLF_BODY.as_bytes()),
        "staged stage-0 blob keeps CRLF verbatim (autocrlf=false; no normalization)"
    );
    assert_eq!(
        std::fs::read(d.join("a.txt")).expect("read a.txt"),
        CRLF_BODY.as_bytes(),
        "worktree bytes keep CRLF verbatim"
    );
}

// ---- Default model → `--model sonnet` ----

#[test]
fn default_run_opts_model_is_none_and_default_model_is_sonnet() {
    // Pure consts: RunOpts::default() carries no explicit model, so run_claude
    // substitutes DEFAULT_MODEL, which is "sonnet".
    assert!(RunOpts::default().model.is_none(), "RunOpts::default().model must be None");
    assert_eq!(DEFAULT_MODEL, "sonnet", "the default resolution model is sonnet");
}

#[test]
fn default_opts_spawn_model_sonnet_in_argv() {
    // No git needed: the `check_model` stub inspects its OWN argv and only emits
    // the success body when `--model sonnet` is present, so this asserts the
    // ACTUAL spawned command line, not just the const.
    let _g = env_lock();
    set_stub_mode("check_model");

    // RunOpts::default() → model None → run_claude passes `--model sonnet`.
    let res = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
        .expect("default opts must spawn --model sonnet");
    assert_eq!(
        res.text, "MODEL_IS_SONNET",
        "the stub confirms `--model sonnet` was on the argv"
    );

    // An explicit non-default model overrides it: the stub no longer sees sonnet.
    let opts = RunOpts { model: Some("opus".to_string()), ..RunOpts::default() };
    let err = run_claude(Path::new("."), "prompt", Some("payload"), opts)
        .expect_err("explicit --model opus is NOT sonnet");
    assert!(
        matches!(err, AppError::AiFailed(_)),
        "the stub reports an is_error envelope when sonnet is absent, got {err:?}"
    );
}
