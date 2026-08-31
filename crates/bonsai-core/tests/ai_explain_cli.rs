//! P15b scratch-repo tests for `analyze_diff` (contract §8.3).
//!
//! Drives real git2 diffs (commit vs first parent, working-dir file, staged
//! set) on scratch repos, with the local `claude` CLI replaced by the committed
//! stub (`tests/fixtures/claude_stub.cmd`) via `BONSAI_CLAUDE_BIN` +
//! `BONSAI_STUB_MODE`. No network, no real CLI. Mirrors `ai_resolve_cli.rs`.
//!
//! Proves: (1) `Commit{oid}` → the stub body is returned for BOTH `Explain` and
//! `Review`; (2) `WorkdirFile{staged}`, `WorkdirFile{unstaged}` and `Staged`
//! each build a non-empty payload → Ok; (3) an empty diff (clean workdir file)
//! → `AiFailed("no changes to analyze")` before any CLI call; a bad oid → `Git`;
//! a `../escape` path → `InvalidName`.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_explain::{analyze_diff, AiAnalysisMode, AiDiffTarget};
use common::{commit_fixed, git, init_repo};

const STUB_BODY: &str = "MERGED_BODY_OK";
const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
const STDIN_DUMP_ENV: &str = "BONSAI_STUB_STDIN_DUMP";

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn stub_path() -> std::path::PathBuf {
    common::claude_stub_path()
}

fn set_stub_mode(mode: &str) {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, mode);
    std::env::remove_var(STDIN_DUMP_ENV);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// A scratch repo whose HEAD commit modifies `a.txt` (an add + a del vs its
/// parent), so `Commit{HEAD}` has analyzable content. Returns (dir, head_oid).
fn repo_with_change() -> (tempfile::TempDir, String) {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "line1\nORIGINAL\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    write(d, "a.txt", "line1\nCHANGED_IN_COMMIT\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "modify a.txt");
    let head = git(d, &["rev-parse", "HEAD"]);
    (dir, head)
}

// ============================================================ §8.3 (1) Commit target, both modes

#[test]
fn commit_target_explain_and_review_both_return_stub_body() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("success");

    let (dir, head) = repo_with_change();
    let d = dir.path();

    for mode in [AiAnalysisMode::Explain, AiAnalysisMode::Review] {
        let analysis = analyze_diff(
            d,
            AiDiffTarget::Commit { oid: head.clone() },
            mode,
            RunOpts::default(),
        )
        .unwrap_or_else(|e| panic!("Commit target ({mode:?}) should succeed: {e:?}"));
        assert_eq!(analysis.text, STUB_BODY, "mode {mode:?}: text must be the stub body");
        assert_eq!(analysis.cost_usd, Some(0.012), "mode {mode:?}: cost parsed");
    }
}

// ============================================================ §8.3 (2) workdir + staged targets build a payload

#[test]
fn workdir_file_and_staged_targets_build_nonempty_payload() {
    require_git!();
    let _g = env_lock();

    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "one\ntwo\nthree\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    // Stage a modification to a.txt (HEAD vs index has content).
    write(d, "a.txt", "one\nTWO_STAGED\nthree\n");
    git(d, &["add", "a.txt"]);
    // Then make an UNSTAGED further edit (index vs workdir has content).
    write(d, "a.txt", "one\nTWO_STAGED\nTHREE_UNSTAGED\n");

    // Capture stdin to prove the payload is non-empty and carries the change.
    let dump = d.join("dump.txt");
    let run = |target: AiDiffTarget| -> Result<String, AppError> {
        std::env::set_var(CLAUDE_BIN_ENV, stub_path());
        std::env::set_var(STUB_MODE_ENV, "dump_stdin");
        std::env::set_var(STDIN_DUMP_ENV, &dump);
        let out = analyze_diff(d, target, AiAnalysisMode::Explain, RunOpts::default());
        std::env::remove_var(STDIN_DUMP_ENV);
        out.map(|_| std::fs::read_to_string(&dump).expect("stub wrote stdin dump"))
    };

    // staged=true → HEAD vs index → carries the staged line.
    let staged_payload = run(AiDiffTarget::WorkdirFile {
        path: "a.txt".to_string(),
        orig_path: None,
        staged: true,
    })
    .expect("WorkdirFile{staged:true} → Ok");
    assert!(
        staged_payload.contains("+TWO_STAGED"),
        "staged payload should carry the staged line; got:\n{staged_payload}"
    );

    // staged=false → index vs workdir → carries the unstaged line.
    let unstaged_payload = run(AiDiffTarget::WorkdirFile {
        path: "a.txt".to_string(),
        orig_path: None,
        staged: false,
    })
    .expect("WorkdirFile{staged:false} → Ok");
    assert!(
        unstaged_payload.contains("+THREE_UNSTAGED"),
        "unstaged payload should carry the unstaged line; got:\n{unstaged_payload}"
    );

    // Staged set target → non-empty payload carrying the staged line.
    let staged_set_payload = run(AiDiffTarget::Staged).expect("Staged → Ok");
    assert!(
        staged_set_payload.contains("+TWO_STAGED"),
        "staged-set payload should carry the staged line; got:\n{staged_set_payload}"
    );
}

// ============================================================ P25 §9.3 Worktree + Branch review targets

/// P25 §9.3: `analyze_diff(Worktree, Review)` over a scratch repo with an
/// uncommitted change returns the stub body; a CLEAN worktree short-circuits to
/// `AiFailed("no changes to analyze")` before any CLI spawn.
#[test]
fn worktree_target_reviews_and_empty_fails() {
    require_git!();
    let _g = env_lock();

    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "one\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    // Clean worktree → AiFailed BEFORE any CLI call (`nonzero` would surface
    // loudly if the stub actually ran).
    set_stub_mode("nonzero");
    let err = analyze_diff(d, AiDiffTarget::Worktree, AiAnalysisMode::Review, RunOpts::default())
        .expect_err("clean worktree must be AiFailed");
    match err {
        AppError::AiFailed(m) => assert_eq!(m, "no changes to analyze", "got: {m}"),
        other => panic!("expected AiFailed('no changes to analyze'), got {other:?}"),
    }

    // A staged change + an unstaged edit + an untracked file → analyzable.
    write(d, "a.txt", "one\nTWO_STAGED\n");
    git(d, &["add", "a.txt"]);
    write(d, "a.txt", "one\nTWO_STAGED\nTHREE_UNSTAGED\n");
    write(d, "untracked.txt", "brand new\n");

    set_stub_mode("success");
    let analysis =
        analyze_diff(d, AiDiffTarget::Worktree, AiAnalysisMode::Review, RunOpts::default())
            .unwrap_or_else(|e| panic!("Worktree review should succeed: {e:?}"));
    assert_eq!(analysis.text, STUB_BODY, "Worktree review returns the stub body");

    // The assembled payload carries all three change kinds.
    let dump = d.join("dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);
    analyze_diff(d, AiDiffTarget::Worktree, AiAnalysisMode::Review, RunOpts::default())
        .expect("Worktree dump → Ok");
    std::env::remove_var(STDIN_DUMP_ENV);
    let payload = std::fs::read_to_string(&dump).expect("stub wrote stdin dump");
    for needle in ["+TWO_STAGED", "+THREE_UNSTAGED", "+brand new"] {
        assert!(
            payload.contains(needle),
            "worktree payload should carry {needle:?}; got:\n{payload}"
        );
    }
}

/// P25 §9.3: `analyze_diff(Branch{name, base:None}, Review)` diffs the branch vs
/// the merge-base with the auto-resolved base (here local `main`) and returns
/// the stub body; the payload carries the branch-only change + the header.
#[test]
fn branch_target_reviews_via_stub() {
    require_git!();
    let _g = env_lock();

    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    // Diverge a feature branch with one branch-only file.
    git(d, &["checkout", "-b", "feature"]);
    write(d, "feature.txt", "feature work\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "feature work");
    // Back to main so HEAD != feature (the branch review targets `feature` by name).
    git(d, &["checkout", "main"]);

    set_stub_mode("success");
    let analysis = analyze_diff(
        d,
        AiDiffTarget::Branch {
            name: "feature".to_string(),
            base: None, // auto → local `main`
        },
        AiAnalysisMode::Review,
        RunOpts::default(),
    )
    .unwrap_or_else(|e| panic!("Branch review should succeed: {e:?}"));
    assert_eq!(analysis.text, STUB_BODY, "Branch review returns the stub body");

    // Payload carries the merge-base header + the branch-only addition.
    let dump = d.join("dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);
    analyze_diff(
        d,
        AiDiffTarget::Branch {
            name: "feature".to_string(),
            base: None,
        },
        AiAnalysisMode::Review,
        RunOpts::default(),
    )
    .expect("Branch dump → Ok");
    std::env::remove_var(STDIN_DUMP_ENV);
    let payload = std::fs::read_to_string(&dump).expect("stub wrote stdin dump");
    assert!(
        payload.contains("BRANCH feature vs main (merge-base)"),
        "branch payload should carry the header; got:\n{payload}"
    );
    assert!(
        payload.contains("+feature work"),
        "branch payload should carry the branch-only change; got:\n{payload}"
    );
}

/// P25 §9.3: an explicit `base` that cannot resolve maps to `Git` (bad ref);
/// a bad branch `name` likewise → `Git`.
#[test]
fn branch_bad_ref_maps_to_git() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("nonzero"); // would surface loudly if a CLI call slipped through

    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    let err = analyze_diff(
        d,
        AiDiffTarget::Branch {
            name: "does-not-exist".to_string(),
            base: None,
        },
        AiAnalysisMode::Review,
        RunOpts::default(),
    )
    .expect_err("bad branch name must be Git");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
}

// ============================================================ §8.3 (3) empty diff / bad oid / bad path

#[test]
fn clean_workdir_file_maps_to_ai_failed_no_cli_call() {
    require_git!();
    let _g = env_lock();
    // A mode that would blow up loudly if it ran; getting the precise
    // "no changes to analyze" message proves the guard fired BEFORE any CLI call.
    set_stub_mode("nonzero");

    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "unchanged\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    // No edits: a.txt is clean → workdir_file_diff(staged=false) is empty.

    let err = analyze_diff(
        d,
        AiDiffTarget::WorkdirFile {
            path: "a.txt".to_string(),
            orig_path: None,
            staged: false,
        },
        AiAnalysisMode::Explain,
        RunOpts::default(),
    )
    .expect_err("clean workdir file must be AiFailed");
    match err {
        AppError::AiFailed(m) => assert_eq!(m, "no changes to analyze", "got: {m}"),
        other => panic!("expected AiFailed('no changes to analyze'), got {other:?}"),
    }
}

#[test]
fn bad_oid_maps_to_git() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("success");

    let (dir, _head) = repo_with_change();
    let d = dir.path();

    // Valid-hex but nonexistent oid → find_commit fails → Git.
    let err = analyze_diff(
        d,
        AiDiffTarget::Commit {
            oid: "0123456789abcdef0123456789abcdef01234567".to_string(),
        },
        AiAnalysisMode::Explain,
        RunOpts::default(),
    )
    .expect_err("nonexistent oid must be Git");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");

    // Non-hex garbage oid → Oid::from_str fails → Git.
    let err = analyze_diff(
        d,
        AiDiffTarget::Commit {
            oid: "not-a-real-oid".to_string(),
        },
        AiAnalysisMode::Review,
        RunOpts::default(),
    )
    .expect_err("garbage oid must be Git");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
}

// DISCREPANCY (contract vs implementation): §7.1 and §8.3(3) both say a bad path
// ("../escape") → `InvalidName` "via validate_rel_path inside the diff fns". In
// fact the diff fns call the SHARED `validate_rel_path` in `git/stage.rs`, which
// returns `AppError::Other("invalid path: …")` (wire `kind:"other"`), NOT
// `InvalidName` (`kind:"invalidName"`). The `ai_resolve` path (a DIFFERENT
// validator) does return `InvalidName`, which is why the P13 escape test passes —
// hence the contract author's assumption. Impact is low: the traversal path IS
// rejected BEFORE any CLI or git tree access (the security property holds), only
// the error VARIANT/`kind` differs from the IPC contract's declared
// `invalidName`. Reported to the orchestrator; NOT fixed here (test code only).
// This test locks the ACTUAL behavior; if senior-dev reconciles it to
// `InvalidName`, flip the assertion below.
#[test]
fn escape_path_is_rejected_before_any_cli_call() {
    require_git!();
    let _g = env_lock();
    // `nonzero` would surface loudly as AiFailed if the CLI ran; a path-rejection
    // error instead proves the guard fires before any subprocess spawn.
    set_stub_mode("nonzero");

    let (dir, _head) = repo_with_change();
    let d = dir.path();

    for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
        let err = analyze_diff(
            d,
            AiDiffTarget::WorkdirFile {
                path: bad.to_string(),
                orig_path: None,
                staged: false,
            },
            AiAnalysisMode::Explain,
            RunOpts::default(),
        )
        .expect_err("escape path must error");
        // ACTUAL: Other("invalid path: …"); CONTRACT says InvalidName (see note).
        match &err {
            AppError::Other(m) => assert!(
                m.contains("invalid path"),
                "path {bad:?}: expected an 'invalid path' rejection, got Other({m:?})"
            ),
            AppError::InvalidName(_) => {
                // Accept the contract-specified variant too, so a future fix that
                // reconciles the validator does NOT spuriously fail this test.
            }
            other => panic!(
                "path {bad:?}: expected a path-rejection error (Other/InvalidName), got {other:?}"
            ),
        }
    }
}
