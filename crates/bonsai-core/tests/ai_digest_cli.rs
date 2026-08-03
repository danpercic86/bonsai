//! P28a scratch-repo tests for `digest_changes` (contract §10.2–§10.3).
//!
//! §10.2 CLI oracles: the commit set the digest walks matches `git log` for
//! both `betweenRefs` (`main..feature`) and `lastDays` (`--first-parent
//! --since=<cutoff>`), asserted via the COMMITS block in the payload the
//! `dump_stdin` stub captures. §10.3 stub harness: the payload carries both a
//! `COMMITS` block and a `DIFF` section; an empty range errors BEFORE spawning.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_explain::digest_changes;
use bonsai_core::git::ai_explain::AiDigestRange;
use common::{commit_fixed, git, git_env, init_repo};

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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude_stub.cmd")
}

fn set_stub_mode(mode: &str) {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, mode);
    std::env::remove_var(STDIN_DUMP_ENV);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// Runs `digest_changes` with the dump_stdin stub and returns the captured
/// payload text.
fn digest_dump(d: &Path, range: AiDigestRange) -> String {
    let dump = d.join("digest_dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);
    let out = digest_changes(d, range, RunOpts::default());
    std::env::remove_var(STDIN_DUMP_ENV);
    out.unwrap_or_else(|e| panic!("digest_changes should succeed: {e:?}"));
    // find.exe re-emits the payload with CRLF line endings; normalize to LF so
    // the section-splitting assertions match what digest_changes assembled.
    std::fs::read_to_string(&dump)
        .expect("stub wrote stdin dump")
        .replace("\r\n", "\n")
}

/// Extracts the short7 hashes from the payload's COMMITS block, in order.
fn payload_short7s(payload: &str) -> Vec<String> {
    let commits_block = payload
        .split("\nCOMMITS\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\nDIFF\n").next())
        .unwrap_or_else(|| panic!("payload lacks COMMITS/DIFF sections:\n{payload}"));
    commits_block
        .lines()
        .filter(|l| l.starts_with("- "))
        .map(|l| l[2..9].to_string())
        .collect()
}

/// main = base+B, feature branched off with two commits (fixed dates).
fn repo_with_feature() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    write(d, "a.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    write(d, "a.txt", "base\nmain2\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main second");
    git(d, &["checkout", "-b", "feature"]);
    write(d, "f.txt", "feature one\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "feature one");
    write(d, "f.txt", "feature one\nfeature two\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "feature two");
    git(d, &["checkout", "main"]);
    dir
}

// ============================================================ §10.2 betweenRefs oracle

/// The COMMITS block for BetweenRefs{main, feature} matches
/// `git log --format=%H main..feature` (membership + order, newest first).
#[test]
fn between_refs_commit_set_matches_git_log_oracle() {
    require_git!();
    let _g = env_lock();

    let dir = repo_with_feature();
    let d = dir.path();

    let payload = digest_dump(
        d,
        AiDigestRange::BetweenRefs {
            from: "main".to_string(),
            to: "feature".to_string(),
        },
    );
    let ours = payload_short7s(&payload);

    let oracle: Vec<String> = git(d, &["log", "--format=%H", "main..feature"])
        .lines()
        .map(|h| h[..7].to_string())
        .collect();
    assert_eq!(ours, oracle, "digest commit set must match `git log main..feature`");
    assert_eq!(ours.len(), 2, "two feature-only commits expected");
    assert!(payload.contains("RANGE main..feature (2 commits)"), "{payload}");
    // The range diff carries the feature-only content.
    assert!(payload.contains("+feature two"), "{payload}");
}

/// SinceCommit{<main oid>} on a HEAD at `feature` equals BetweenRefs to HEAD.
#[test]
fn since_commit_matches_between_refs_oracle() {
    require_git!();
    let _g = env_lock();

    let dir = repo_with_feature();
    let d = dir.path();
    git(d, &["checkout", "feature"]);
    let main_oid = git(d, &["rev-parse", "main"]);

    let payload = digest_dump(d, AiDigestRange::SinceCommit { oid: main_oid });
    let ours = payload_short7s(&payload);
    let oracle: Vec<String> = git(d, &["log", "--format=%H", "main..HEAD"])
        .lines()
        .map(|h| h[..7].to_string())
        .collect();
    assert_eq!(ours, oracle, "sinceCommit must equal main..HEAD");
}

// ============================================================ §10.2 lastDays oracle

/// lastDays first-parent walk matches `git log --first-parent --since=<cutoff>`
/// with pinned committer times (now−1d / −2d / −10d, days=7).
#[test]
fn last_days_commit_set_matches_git_log_oracle() {
    require_git!();
    let _g = env_lock();

    let dir = init_repo();
    let d = dir.path();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let day = 86_400i64;

    let commit_at = |name: &str, content: &str, secs: i64| {
        write(d, name, content);
        git(d, &["add", "-A"]);
        let date = format!("{secs} +0000");
        git_env(
            d,
            &["commit", "-m", name],
            &[
                ("GIT_AUTHOR_DATE", date.as_str()),
                ("GIT_COMMITTER_DATE", date.as_str()),
            ],
        );
    };
    commit_at("old.txt", "ten days ago\n", now - 10 * day);
    commit_at("mid.txt", "two days ago\n", now - 2 * day);
    commit_at("new.txt", "yesterday\n", now - day);

    let payload = digest_dump(d, AiDigestRange::LastDays { days: 7 });
    let ours = payload_short7s(&payload);

    // git approxidate parses a large bare integer as epoch seconds.
    let cutoff = (now - 7 * day).to_string();
    let since = format!("--since={cutoff}");
    let oracle: Vec<String> = git(d, &["log", "--first-parent", &since, "--format=%H", "HEAD"])
        .lines()
        .map(|h| h[..7].to_string())
        .collect();
    assert_eq!(ours, oracle, "lastDays commit set must match the git oracle");
    assert_eq!(ours.len(), 2, "the 10-day-old commit is outside the window");
    // The diff is anchored at the boundary commit's tree: it must NOT re-add
    // the out-of-window file but must add both in-window files.
    assert!(payload.contains("+two days ago"), "{payload}");
    assert!(payload.contains("+yesterday"), "{payload}");
    assert!(!payload.contains("+ten days ago"), "{payload}");
}

// ============================================================ §10.3 stub harness

/// `digest_changes(BetweenRefs)` returns the stub body; the captured payload
/// carries both a COMMITS block and a DIFF section (payload shape §4).
#[test]
fn digest_returns_stub_body_with_commits_and_diff_sections() {
    require_git!();
    let _g = env_lock();

    let dir = repo_with_feature();
    let d = dir.path();

    set_stub_mode("success");
    let analysis = digest_changes(
        d,
        AiDigestRange::BetweenRefs {
            from: "main".to_string(),
            to: "feature".to_string(),
        },
        RunOpts::default(),
    )
    .unwrap_or_else(|e| panic!("digest should succeed: {e:?}"));
    assert_eq!(analysis.text, STUB_BODY);
    assert_eq!(analysis.cost_usd, Some(0.012));

    let payload = digest_dump(
        d,
        AiDigestRange::BetweenRefs {
            from: "main".to_string(),
            to: "feature".to_string(),
        },
    );
    assert!(payload.contains("\nCOMMITS\n"), "payload lacks COMMITS:\n{payload}");
    assert!(payload.contains("\n\nDIFF\n"), "payload lacks DIFF:\n{payload}");
    assert!(payload.starts_with("RANGE "), "payload lacks RANGE header:\n{payload}");
}

/// An empty range (`from == to`) errors BEFORE spawning the CLI (`nonzero`
/// would surface loudly as a different AiFailed message if the stub ran).
#[test]
fn empty_range_errors_before_cli_spawn() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("nonzero");

    let dir = repo_with_feature();
    let d = dir.path();

    let err = digest_changes(
        d,
        AiDigestRange::BetweenRefs {
            from: "main".to_string(),
            to: "main".to_string(),
        },
        RunOpts::default(),
    )
    .expect_err("empty range must fail");
    match err {
        AppError::AiFailed(m) => assert_eq!(m, "no changes in the selected range", "got: {m}"),
        other => panic!("expected AiFailed('no changes in the selected range'), got {other:?}"),
    }
}

/// A bad ref maps to `Git`; days=0 maps to `InvalidName` — both before any
/// CLI spawn.
#[test]
fn bad_ref_and_zero_days_error_kinds() {
    require_git!();
    let _g = env_lock();
    set_stub_mode("nonzero");

    let dir = repo_with_feature();
    let d = dir.path();

    let err = digest_changes(
        d,
        AiDigestRange::BetweenRefs {
            from: "does-not-exist".to_string(),
            to: "main".to_string(),
        },
        RunOpts::default(),
    )
    .expect_err("bad ref must fail");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");

    let err = digest_changes(d, AiDigestRange::LastDays { days: 0 }, RunOpts::default())
        .expect_err("days=0 must fail");
    assert!(matches!(err, AppError::InvalidName(_)), "got {err:?}");
}
