//! P15c scratch-repo tests for `summarize_range` (contract §8.4).
//!
//! Drives real diverged git2 histories on scratch repos, with the local
//! `claude` CLI replaced by the committed stub (`tests/fixtures/claude_stub.cmd`)
//! via `BONSAI_CLAUDE_BIN` + `BONSAI_STUB_MODE`. No network, no real CLI.
//!
//! Proves: (1) a diverged base/feature repo → `AiSummary` with
//! `commit_count == unique commits`, base/target echoed, text == stub body, and
//! MERGE-BASE semantics (a commit added to base AFTER divergence is NOT counted,
//! and a base-only file does NOT appear in the diffstat); cross-checked against
//! `git rev-list --count base..target`. (2) an empty range → `AiFailed("nothing
//! to summarize …")` before any CLI call; a bad ref → `Git`. (3) the
//! `AI_SUMMARY_MAX_COMMITS` truncation "(+N more commits)" note when exceeded,
//! using a cheap git2 commit loop (NOT thousands of CLI commits).
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_summary::{summarize_range, AI_SUMMARY_MAX_COMMITS};
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

/// `git rev-list --count <range>` as an integer (the CLI oracle).
fn rev_list_count(dir: &Path, range: &str) -> u32 {
    git(dir, &["rev-list", "--count", range])
        .parse()
        .expect("rev-list --count parses")
}

/// A diverged repo:
///   main:    C0 ── M1 (adds main_only.txt AFTER divergence)
///              \
///   feature:   F1 (feature1.txt) ── F2 (feature2.txt)
/// merge-base(main, feature) == C0. Returns the temp dir.
fn diverged_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0 base");

    git(d, &["checkout", "-b", "feature"]);
    write(d, "feature1.txt", "f1\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "F1 add feature1");
    write(d, "feature2.txt", "f2\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "F2 add feature2");

    git(d, &["checkout", "main"]);
    write(d, "main_only.txt", "main only\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "M1 add main_only (after divergence)");

    dir
}

// ============================================================ §8.4 (1) diverged range + merge-base

#[test]
fn diverged_range_counts_unique_commits_and_uses_merge_base() {
    require_git!();
    let _g = env_lock();

    let dir = diverged_repo();
    let d = dir.path();

    // CLI oracle: commits unique to feature vs main.
    let expected = rev_list_count(d, "main..feature");
    assert_eq!(expected, 2, "fixture sanity: feature has 2 unique commits");

    // Capture stdin to inspect the rendered payload (commit list + diffstat).
    let dump = d.join("dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);

    let summary =
        summarize_range(d, "main", "feature", RunOpts::default()).expect("diverged range → Ok");

    std::env::remove_var(STDIN_DUMP_ENV);

    assert_eq!(summary.text, STUB_BODY, "text must be the stub body");
    assert_eq!(summary.cost_usd, Some(0.012), "cost parsed from the envelope");
    assert_eq!(summary.base, "main", "base echoed verbatim");
    assert_eq!(summary.target, "feature", "target echoed verbatim");
    assert_eq!(
        summary.commit_count, expected,
        "commit_count must equal `git rev-list --count main..feature`"
    );

    let payload = std::fs::read_to_string(&dump).expect("stub wrote stdin dump");
    // The two feature commit summaries reach the commit-list section.
    assert!(
        payload.contains("F1 add feature1") && payload.contains("F2 add feature2"),
        "payload commit list should carry both feature commits; got:\n{payload}"
    );
    // MERGE-BASE semantics: the base-only file (added to main AFTER divergence)
    // is in NEITHER the merge-base tree nor the target tree, so it must NOT
    // appear in the net diffstat. A direct base..target tree diff WOULD surface
    // it as a reversed deletion — this assertion is what distinguishes the two.
    assert!(
        !payload.contains("main_only.txt"),
        "merge-base diffstat must NOT include the base-only file; got:\n{payload}"
    );
    // The feature-added files DO appear in the diffstat.
    assert!(
        payload.contains("feature1.txt") && payload.contains("feature2.txt"),
        "diffstat should include the feature-added files; got:\n{payload}"
    );
}

// ============================================================ §8.4 (2) empty range / bad ref

#[test]
fn empty_range_target_equals_base_maps_to_ai_failed_no_cli_call() {
    require_git!();
    let _g = env_lock();
    // `nonzero` would surface loudly as AiFailed('...something broke...') if it
    // ran; the precise "nothing to summarize" message proves no CLI call.
    set_stub_mode("nonzero");

    let dir = diverged_repo();
    let d = dir.path();

    let err = summarize_range(d, "main", "main", RunOpts::default())
        .expect_err("target == base must be AiFailed");
    match err {
        AppError::AiFailed(m) => {
            assert!(m.contains("nothing to summarize"), "got: {m}");
            assert!(m.contains("main"), "message should name the refs; got: {m}");
        }
        other => panic!("expected AiFailed('nothing to summarize …'), got {other:?}"),
    }
}

#[test]
fn target_behind_base_has_no_unique_commits_maps_to_ai_failed() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("nonzero");

    let dir = diverged_repo();
    let d = dir.path();

    // feature is AHEAD of C0; main..feature has commits but feature has all of
    // C0. Summarizing base=feature, target=main: main's only unique commit vs
    // feature is M1 — so this is NOT empty. Use an ANCESTOR as target instead:
    // base=feature, target=C0 (an ancestor of feature) → zero unique commits.
    let c0 = git(d, &["rev-list", "--max-parents=0", "feature"]);

    let err = summarize_range(d, "feature", &c0, RunOpts::default())
        .expect_err("ancestor target must be AiFailed (no unique commits)");
    match err {
        AppError::AiFailed(m) => assert!(m.contains("nothing to summarize"), "got: {m}"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
}

#[test]
fn bad_ref_maps_to_git() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("success");

    let dir = diverged_repo();
    let d = dir.path();

    // Bad target ref.
    let err = summarize_range(d, "main", "no-such-branch", RunOpts::default())
        .expect_err("bad target ref must be Git");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");

    // Bad base ref.
    let err = summarize_range(d, "no-such-base", "feature", RunOpts::default())
        .expect_err("bad base ref must be Git");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
}

// ============================================================ §8.4 (3) commit-cap truncation note
//
// AI_SUMMARY_MAX_COMMITS (200) is a plain `pub const` (not #[cfg(test)]-lowered),
// so we build a cheap many-commit fixture with a git2 commit loop (NOT thousands
// of CLI `git commit` calls) — one base commit via the CLI, then N commits added
// in-process. `summarize_range` must cap the listed count at the const and append
// a "(+<total-cap> more commits)" note to the payload.

/// Adds `n` commits on `branch` (must be current HEAD) via git2, each touching
/// `churn.txt`. Cheap: no per-commit subprocess. Returns the final commit oid.
fn add_commits_git2(dir: &Path, n: usize) -> String {
    let repo = git2::Repository::open(dir).expect("open repo");
    let sig = git2::Signature::new("Loop Author", "loop@example.com", &git2::Time::new(1_600_000_000, 0))
        .expect("signature");
    let mut parent = repo.head().expect("head").peel_to_commit().expect("head commit");
    let mut last = parent.id();
    for i in 0..n {
        let churn = dir.join("churn.txt");
        std::fs::write(&churn, format!("iteration {i}\n")).expect("write churn");
        let mut index = repo.index().expect("index");
        index.add_path(Path::new("churn.txt")).expect("add churn");
        index.write().expect("write index");
        let tree_oid = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");
        last = repo
            .commit(Some("HEAD"), &sig, &sig, &format!("loop commit {i}"), &tree, &[&parent])
            .expect("commit");
        parent = repo.find_commit(last).expect("find new commit");
    }
    last.to_string()
}

#[test]
fn exceeding_commit_cap_appends_truncation_note() {
    require_git!();
    let _g = env_lock();

    let dir = init_repo();
    let d = dir.path();
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0 base");

    // Diverge onto `feature`, then add CAP + 1 commits cheaply via git2.
    git(d, &["checkout", "-b", "feature"]);
    let over = AI_SUMMARY_MAX_COMMITS + 1;
    add_commits_git2(d, over);

    // Oracle: feature has exactly `over` unique commits vs main.
    let unique = rev_list_count(d, "main..feature");
    assert_eq!(unique as usize, over, "fixture sanity: {over} unique commits");

    let dump = d.join("dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);

    let summary = summarize_range(d, "main", "feature", RunOpts::default())
        .expect("over-cap range → Ok");

    std::env::remove_var(STDIN_DUMP_ENV);

    // commit_count is the CAPPED display count (§4.5), not the pre-truncation total.
    assert_eq!(
        summary.commit_count as usize, AI_SUMMARY_MAX_COMMITS,
        "commit_count must be capped at AI_SUMMARY_MAX_COMMITS"
    );

    let payload = std::fs::read_to_string(&dump).expect("stub wrote stdin dump");
    // The pre-truncation total exceeds the cap by exactly 1 → "(+1 more commits)".
    let expected_note = format!("(+{} more commits)", over - AI_SUMMARY_MAX_COMMITS);
    assert!(
        payload.contains(&expected_note),
        "payload should carry the truncation note {expected_note:?}; got tail:\n{}",
        &payload[payload.len().saturating_sub(400)..]
    );
}
