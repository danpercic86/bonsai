//! P56a scratch-repo tests for `generate_changelog` (contract §7.4, §7.9, §7.10).
//!
//! §7.10 CLI oracle: the commit set the changelog walks matches
//! `git log --format=%H <from>..<to>` (membership + order, newest first) for both
//! a TAG range (`betweenRefs`) and `sinceLastTag`, asserted via the COMMITS block
//! in the payload the `dump_stdin` stub captures. §7.9 stub harness: the payload
//! carries a `COMMITS:` block and a `NET CHANGES (diffstat):` section, and the
//! returned `AiChangelog` carries the stub body + parsed cost.
//!
//! Lives in its OWN test binary so the process-global `BONSAI_CLAUDE_BIN` cannot
//! race the lib unit tests (mirrors `ai_digest_cli.rs` / `ai_compose_cli.rs`).
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full). Each test
//! skips (passes with a note) if `git` is not on PATH.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::git::ai_changelog::{generate_changelog, AiChangelog, ChangelogRange};
use crate::common;
use crate::common::{git, git_env, init_repo};

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

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// Linear main = A(tag v1) -> B -> C(tag v2), with DISTINCT committer dates so
/// libgit2's TOPOLOGICAL|TIME walk and `git log`'s order agree deterministically.
fn tagged_linear_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;

    let commit_at = |name: &str, content: &str, secs: i64| {
        write(d, "f.txt", content);
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
    commit_at("A base", "base\n", now - 300);
    git(d, &["tag", "v1"]);
    commit_at("B feat: add b", "base\nb\n", now - 200);
    commit_at("C fix: fix c", "base\nb\nc\n", now - 100);
    git(d, &["tag", "v2"]);
    dir
}

/// Runs `generate_changelog` with the `dump_stdin` stub and returns BOTH the
/// result (from/to echo, cost) and the captured payload (CRLF normalized).
fn run_dump(d: &Path, range: ChangelogRange) -> (AiChangelog, String) {
    let dump = d.join("changelog_dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);
    let out = generate_changelog(d, range, RunOpts::default());
    std::env::remove_var(STDIN_DUMP_ENV);
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);
    let result = out.unwrap_or_else(|e| panic!("generate_changelog should succeed: {e:?}"));
    // find.exe re-emits the payload with CRLF line endings; normalize to LF so
    // the section-splitting assertions match what generate_changelog assembled.
    let payload = std::fs::read_to_string(&dump)
        .expect("stub wrote stdin dump")
        .replace("\r\n", "\n");
    (result, payload)
}

/// Extracts the short7 hashes from the payload's COMMITS block, in order. Each
/// `render_commit_list` line starts with the 7-char short oid; the optional
/// "(+N more commits)" line (starts with '(') and blank lines are skipped.
fn payload_short7s(payload: &str) -> Vec<String> {
    let block = payload
        .split("COMMITS:\n")
        .nth(1)
        .and_then(|rest| rest.split("\nNET CHANGES (diffstat):").next())
        .unwrap_or_else(|| panic!("payload lacks COMMITS/NET CHANGES sections:\n{payload}"));
    block
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('('))
        .map(|l| l.chars().take(7).collect())
        .collect()
}

// ============================================================ §7.10 betweenRefs (tag) oracle

/// The COMMITS block for BetweenRefs{v1, v2} matches
/// `git log --format=%H v1..v2` (membership + order, newest first); the RESOLVED
/// range is echoed as-is; `commit_count == 2`.
#[test]
fn between_refs_tag_range_matches_git_log_oracle() {
    require_git!();
    let _g = env_lock();

    let dir = tagged_linear_repo();
    let d = dir.path();

    let (result, payload) = run_dump(
        d,
        ChangelogRange::BetweenRefs {
            from: "v1".to_string(),
            to: "v2".to_string(),
        },
    );
    let ours = payload_short7s(&payload);
    let oracle: Vec<String> = git(d, &["log", "--format=%H", "v1..v2"])
        .lines()
        .map(|h| h[..7].to_string())
        .collect();
    assert_eq!(ours, oracle, "changelog commit set must match `git log v1..v2`");
    assert_eq!(ours.len(), 2, "two commits (B, C) between v1 and v2");
    assert_eq!(result.commit_count, 2);
    assert_eq!(result.from_ref, "v1");
    assert_eq!(result.to_ref, "v2");
    // The net diffstat carries the range's changed file.
    assert!(payload.contains("f.txt"), "diffstat lacks the changed file:\n{payload}");
}

// ============================================================ §7.10 sinceLastTag oracle

/// `SinceLastTag{target:"v2"}` resolves from_ref = the previous tag reachable
/// from v2's tip = "v1" and to_ref = "v2"; the walked commit set equals
/// `git log --format=%H v1..v2`.
#[test]
fn since_last_tag_maps_and_matches_git_log_oracle() {
    require_git!();
    let _g = env_lock();

    let dir = tagged_linear_repo();
    let d = dir.path();

    let (result, payload) = run_dump(
        d,
        ChangelogRange::SinceLastTag {
            target: Some("v2".to_string()),
        },
    );
    assert_eq!(result.from_ref, "v1", "previous tag before v2");
    assert_eq!(result.to_ref, "v2", "to_ref echoes the target");

    let ours = payload_short7s(&payload);
    let oracle: Vec<String> = git(d, &["log", "--format=%H", "v1..v2"])
        .lines()
        .map(|h| h[..7].to_string())
        .collect();
    assert_eq!(ours, oracle, "sinceLastTag must equal `git log v1..v2`");
    assert_eq!(result.commit_count, 2);
}

// ============================================================ §7.9 stub harness

/// `generate_changelog(BetweenRefs)` returns the stub body + parsed cost; the
/// captured payload carries a COMMITS block and a NET CHANGES diffstat section.
#[test]
fn changelog_returns_stub_body_with_commits_and_diffstat() {
    require_git!();
    let _g = env_lock();

    let dir = tagged_linear_repo();
    let d = dir.path();

    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, "success");
    let result = generate_changelog(
        d,
        ChangelogRange::BetweenRefs {
            from: "v1".to_string(),
            to: "v2".to_string(),
        },
        RunOpts::default(),
    );
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);
    let result = result.unwrap_or_else(|e| panic!("changelog should succeed: {e:?}"));
    assert_eq!(result.text, STUB_BODY);
    assert_eq!(result.cost_usd, Some(0.012));

    let (_r, payload) = run_dump(
        d,
        ChangelogRange::BetweenRefs {
            from: "v1".to_string(),
            to: "v2".to_string(),
        },
    );
    assert!(payload.contains("COMMITS:\n"), "payload lacks COMMITS:\n{payload}");
    assert!(
        payload.contains("\nNET CHANGES (diffstat):\n"),
        "payload lacks NET CHANGES diffstat:\n{payload}"
    );
}
