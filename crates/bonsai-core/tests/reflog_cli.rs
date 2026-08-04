//! P38 CLI-oracle reflog tests (contract §9).
//!
//! Like the blame suite, reflog only READS existing ref history, so we run
//! Bonsai's `read_reflog` against the SAME scratch repo the real `git` CLI
//! inspects — the entries (oids, messages, `@{N}` indices) are directly
//! comparable. The fixture exercises the real reflog-writing operations
//! (commit / reset / amend / rebase) so several distinct reflog message kinds
//! are covered.
//!
//! Oracle: `git log -g --format=%H%x1f%gd%x1f%gs HEAD` (newest-first) parsed to
//! (index, new_oid, message).
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full). Each test
//! skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::git::reflog::read_reflog;
use common::{git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// `git add -A` then commit with the repo's configured identity.
fn commit_all(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", msg]);
}

/// One oracle reflog record: (index N in `<ref>@{N}`, new oid, message).
#[derive(Debug)]
struct OracleEntry {
    index: u32,
    new_oid: String,
    message: String,
}

/// `git log -g --format=%H\x1f%gd\x1f%gs HEAD`, newest-first. `%gd` is the
/// reflog selector (`HEAD@{N}`); `%gs` is the reflog subject.
fn oracle_reflog(dir: &Path) -> Vec<OracleEntry> {
    let raw = common::git_raw(
        dir,
        &["log", "-g", "--format=%H\x1f%gd\x1f%gs", "HEAD"],
        &[],
    );
    let text = String::from_utf8_lossy(&raw);
    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\x1f');
        let new_oid = parts.next().unwrap_or("").to_string();
        let selector = parts.next().unwrap_or("");
        let message = parts.next().unwrap_or("").to_string();
        // selector looks like "HEAD@{3}"; extract the digits between { and }.
        let index = selector
            .split_once('{')
            .and_then(|(_, rest)| rest.strip_suffix('}'))
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or_else(|| panic!("unparseable reflog selector: {selector:?}"));
        out.push(OracleEntry { index, new_oid, message });
    }
    out
}

/// (1) HEAD reflog matches `git log -g` across commit / reset / amend / rebase:
/// same length, and per index the new_oid (`%H`), index (`@{N}`) and message
/// (`%gs`) match; newest-first with index 0 == current HEAD oid.
#[test]
fn reflog_matches_git_log_g_head() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    // commit base, A, B.
    write(dir, "f.txt", "base\n");
    commit_all(dir, "c1: base");
    write(dir, "f.txt", "base\nA\n");
    commit_all(dir, "c2: add A");
    write(dir, "f.txt", "base\nA\nB\n");
    commit_all(dir, "c3: add B");

    // reset --soft HEAD~1 (writes "reset: moving to HEAD~1"); B stays staged.
    git(dir, &["reset", "--soft", "HEAD~1"]);
    // commit --amend rewrites c2 with the staged B (writes "commit (amend): ...").
    git(dir, &["commit", "--amend", "-m", "c2': A+B amended"]);
    // A rebase that replays the last commit, rewriting HEAD (writes rebase
    // reflog entries). `--exec` forces a real replay rather than a fast-forward.
    git(dir, &["rebase", "--exec", "git --version", "HEAD~1"]);

    let got = read_reflog(dir, "HEAD").expect("read_reflog HEAD");
    let oracle = oracle_reflog(dir);

    assert_eq!(got.len(), oracle.len(), "reflog length matches git log -g");
    assert!(got.len() >= 5, "several reflog entries recorded, got {}", got.len());

    for (i, (g, o)) in got.iter().zip(oracle.iter()).enumerate() {
        assert_eq!(g.index as usize, i, "index is the 0-based position");
        assert_eq!(g.index, o.index, "index matches @{{N}} at row {i}");
        assert_eq!(g.new_oid, o.new_oid, "new_oid matches %H at row {i}");
        assert_eq!(g.message.trim(), o.message.trim(), "message matches %gs at row {i}");
    }

    // Newest-first: index 0 is the current HEAD tip.
    let head = git(dir, &["rev-parse", "HEAD"]);
    assert_eq!(got[0].index, 0, "newest entry is index 0");
    assert_eq!(got[0].new_oid, head, "index 0 new_oid == current HEAD");
}

/// (2) The branch prefix path: `read_reflog(dir, "main")` newest new_oid equals
/// `git rev-parse main`.
#[test]
fn reflog_branch_prefixing_matches_rev_parse() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    write(dir, "f.txt", "base\n");
    commit_all(dir, "c1: base");
    write(dir, "f.txt", "base\nmore\n");
    commit_all(dir, "c2: more");

    let entries = read_reflog(dir, "main").expect("read_reflog main");
    assert!(!entries.is_empty(), "main has a reflog");
    let main_tip = git(dir, &["rev-parse", "main"]);
    assert_eq!(entries[0].new_oid, main_tip, "branch tip matches rev-parse main");
}

/// (3) A valid-but-never-updated ref yields `[]` (not an error).
#[test]
fn reflog_missing_ref_is_empty() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    write(dir, "f.txt", "base\n");
    commit_all(dir, "c1: base");

    let entries = read_reflog(dir, "does-not-exist").expect("missing ref is Ok");
    assert!(entries.is_empty(), "never-updated ref -> empty vec");
}
