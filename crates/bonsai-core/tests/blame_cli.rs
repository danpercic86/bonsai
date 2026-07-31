//! P23c CLI-oracle blame + file-history tests (contract §13.2).
//!
//! Unlike the rebase/merge suites there is NO twin: blame and file-history only
//! READ existing commits, so we run Bonsai's `blame_file` / `file_history`
//! against the SAME scratch repo the real `git` CLI inspects. The commit oids
//! are therefore directly comparable (no committer-time rewrite involved).
//!
//! Blame oracle: `git blame --line-porcelain -- <path>` (one full header block
//! per line -> trivially parsed to (finalLine -> sha, author, content)).
//! History oracle: `git log --follow --format=%H -- <path>`.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (C: is full). Each test
//! skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::blame::{blame_file, file_history, MAX_HISTORY};
use common::{git, git_env, init_repo, FIXED_DATE};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ------------------------------------------------------------ small helpers

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn write_bytes(dir: &Path, name: &str, content: &[u8]) {
    std::fs::write(dir.join(name), content).expect("write fixture bytes");
}

/// `git add -A` then commit with a DISTINCT author identity (name+email) and
/// fixed dates, so blame attribution across authors is deterministic.
fn commit_as(dir: &Path, author_name: &str, author_email: &str, msg: &str) {
    git(dir, &["add", "-A"]);
    git_env(
        dir,
        &["commit", "-m", msg],
        &[
            ("GIT_AUTHOR_NAME", author_name),
            ("GIT_AUTHOR_EMAIL", author_email),
            ("GIT_COMMITTER_NAME", author_name),
            ("GIT_COMMITTER_EMAIL", author_email),
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

/// One oracle blame record per line: (final line no, commit sha, author name,
/// line content). Parsed from `git blame --line-porcelain`.
#[derive(Debug)]
struct OracleLine {
    final_line: usize,
    sha: String,
    author: String,
    content: String,
}

fn is_hex40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Runs `git blame --line-porcelain [rev] -- <path>` and parses one record per
/// line. Each block starts with `<40-hex> <orig> <final> <num>`, carries an
/// `author <name>` line, and ends with a TAB-prefixed content line.
fn oracle_blame(dir: &Path, rev: Option<&str>, path: &str) -> Vec<OracleLine> {
    let mut args = vec!["blame", "--line-porcelain"];
    if let Some(r) = rev {
        args.push(r);
    }
    args.push("--");
    args.push(path);
    let raw = common::git_raw(dir, &args, &[]);
    let text = String::from_utf8_lossy(&raw);

    let mut out = Vec::new();
    let mut cur_sha: Option<String> = None;
    let mut cur_final: usize = 0;
    let mut cur_author: Option<String> = None;
    for line in text.split('\n') {
        if let Some(content) = line.strip_prefix('\t') {
            // Content line ends the current block.
            out.push(OracleLine {
                final_line: cur_final,
                sha: cur_sha.clone().expect("sha before content"),
                author: cur_author.clone().expect("author before content"),
                content: content.to_string(),
            });
            continue;
        }
        let mut parts = line.split(' ');
        if let Some(first) = parts.next() {
            if is_hex40(first) {
                cur_sha = Some(first.to_string());
                // header: <sha> <orig> <final> [<num>]
                let _orig = parts.next();
                if let Some(f) = parts.next() {
                    cur_final = f.parse().unwrap_or(cur_final);
                }
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("author ") {
            cur_author = Some(rest.to_string());
        }
    }
    out
}

/// `git log --follow --format=%H -- <path>`, newest-first.
fn oracle_follow_log(dir: &Path, path: &str) -> Vec<String> {
    git(dir, &["log", "--follow", "--format=%H", "--", path])
        .lines()
        .map(String::from)
        .collect()
}

// ------------------------------------------------------------ blame tests

/// (1) Blame at HEAD matches `git blame --line-porcelain` per line: oid +
/// author + content, with contiguous 1-based final line numbers.
#[test]
fn blame_matches_git_porcelain_at_head() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    // c1 (Alice): three lines.
    write(dir, "file.txt", "line1\nline2\nline3\n");
    commit_as(dir, "Alice", "alice@example.com", "c1: create");
    // c2 (Bob): change the middle line.
    write(dir, "file.txt", "line1\nline2-bob\nline3\n");
    commit_as(dir, "Bob", "bob@example.com", "c2: edit line2");
    // c3 (Carol): append a line.
    write(dir, "file.txt", "line1\nline2-bob\nline3\nline4\n");
    commit_as(dir, "Carol", "carol@example.com", "c3: append line4");

    let lines = blame_file(dir, "file.txt", None).expect("blame ok");
    let oracle = oracle_blame(dir, None, "file.txt");

    assert_eq!(lines.len(), oracle.len(), "line count matches oracle");
    assert_eq!(lines.len(), 4, "fixture has four lines");

    for (i, (got, want)) in lines.iter().zip(oracle.iter()).enumerate() {
        assert_eq!(got.final_line_no as usize, i + 1, "final line 1-based contiguous");
        assert_eq!(got.final_line_no as usize, want.final_line, "final line matches oracle");
        assert_eq!(got.oid, want.sha, "oid matches oracle at line {}", i + 1);
        assert_eq!(got.author_name, want.author, "author matches oracle at line {}", i + 1);
        assert_eq!(got.line_text, want.content, "content matches oracle at line {}", i + 1);
    }

    // Sanity: the three distinct authors landed on the expected lines.
    assert_eq!(lines[0].author_name, "Alice");
    assert_eq!(lines[1].author_name, "Bob");
    assert_eq!(lines[2].author_name, "Alice");
    assert_eq!(lines[3].author_name, "Carol");
}

/// (2) Blame as of an OLDER commit attributes lines as of that revision — fewer
/// lines, and the not-yet-made edits are absent.
#[test]
fn blame_at_older_commit() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    write(dir, "file.txt", "line1\nline2\nline3\n");
    commit_as(dir, "Alice", "alice@example.com", "c1");
    let c1 = git(dir, &["rev-parse", "HEAD"]);

    write(dir, "file.txt", "line1\nline2-bob\nline3\n");
    commit_as(dir, "Bob", "bob@example.com", "c2");
    let c2 = git(dir, &["rev-parse", "HEAD"]);

    write(dir, "file.txt", "line1\nline2-bob\nline3\nline4\n");
    commit_as(dir, "Carol", "carol@example.com", "c3");

    // As of c2 the file has three lines; line4 does not exist yet.
    let lines = blame_file(dir, "file.txt", Some(&c2)).expect("blame at c2");
    let oracle = oracle_blame(dir, Some(&c2), "file.txt");
    assert_eq!(lines.len(), 3, "three lines as of c2");
    assert_eq!(lines.len(), oracle.len());
    for (got, want) in lines.iter().zip(oracle.iter()) {
        assert_eq!(got.oid, want.sha);
        assert_eq!(got.author_name, want.author);
        assert_eq!(got.line_text, want.content);
    }
    assert_eq!(lines[0].oid, c1, "line1 introduced at c1");
    assert_eq!(lines[1].oid, c2, "line2 edited at c2");
    assert_eq!(lines[2].oid, c1, "line3 introduced at c1");
}

/// (3) Blame errors: binary -> Git; `..` path -> Other; unknown path -> Git.
#[test]
fn blame_error_cases() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    write(dir, "text.txt", "hi\n");
    write_bytes(dir, "bin.dat", &[0u8, 159, 146, 150, 0, 1, 2, 3]);
    commit_as(dir, "Alice", "alice@example.com", "c1");

    let err = blame_file(dir, "bin.dat", None).expect_err("binary rejected");
    assert!(matches!(err, AppError::Git(_)), "binary -> Git, got {err:?}");

    let err = blame_file(dir, "../escape", None).expect_err("traversal rejected");
    assert!(matches!(err, AppError::Other(_)), "`..` -> Other, got {err:?}");

    let err = blame_file(dir, "does-not-exist.txt", None).expect_err("unknown rejected");
    assert!(matches!(err, AppError::Git(_)), "unknown -> Git, got {err:?}");
}

// ------------------------------------------------------------ history tests

/// (4) File history matches `git log --follow --oneline` across several edits
/// AND one mid-history rename (OPEN #10 best-effort single-rename follow).
#[test]
fn file_history_follows_single_rename() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    // c1: create old.txt
    write(dir, "old.txt", "a\n");
    commit_as(dir, "Alice", "alice@example.com", "c1: create old.txt");
    let c1 = git(dir, &["rev-parse", "HEAD"]);

    // c2: edit old.txt
    write(dir, "old.txt", "a\nb\n");
    commit_as(dir, "Bob", "bob@example.com", "c2: edit old.txt");
    let c2 = git(dir, &["rev-parse", "HEAD"]);

    // c3: pure rename old.txt -> new.txt (100% similarity, reliably detected).
    git(dir, &["mv", "old.txt", "new.txt"]);
    commit_as(dir, "Carol", "carol@example.com", "c3: rename to new.txt");
    let c3 = git(dir, &["rev-parse", "HEAD"]);

    // c4: edit new.txt
    write(dir, "new.txt", "a\nb\nc\n");
    commit_as(dir, "Dave", "dave@example.com", "c4: edit new.txt");
    let c4 = git(dir, &["rev-parse", "HEAD"]);

    let got: Vec<String> = file_history(dir, "new.txt", 0)
        .expect("history ok")
        .into_iter()
        .map(|e| e.oid)
        .collect();
    let oracle = oracle_follow_log(dir, "new.txt");

    // Best-effort follow: assert parity with `git log --follow` (OPEN #10). Our
    // single-rename follow reproduces git here (pure rename == 100% similarity).
    assert_eq!(got, oracle, "history oids match git log --follow");
    assert_eq!(got, vec![c4.clone(), c3.clone(), c2.clone(), c1.clone()]);

    // The entries also carry the right author/summary for the newest commit.
    let entries = file_history(dir, "new.txt", 0).expect("history ok");
    assert_eq!(entries[0].oid, c4);
    assert_eq!(entries[0].author_name, "Dave");
    assert_eq!(entries[0].summary, "c4: edit new.txt");
}

/// (5) `limit` caps the result; `0` yields up to `MAX_HISTORY`; an unknown path
/// yields an empty history (not an error).
#[test]
fn file_history_limit_and_unknown_path() {
    require_git!();
    let repo = init_repo();
    let dir = repo.path();

    write(dir, "f.txt", "1\n");
    commit_as(dir, "Alice", "alice@example.com", "c1");
    write(dir, "f.txt", "1\n2\n");
    commit_as(dir, "Alice", "alice@example.com", "c2");
    write(dir, "f.txt", "1\n2\n3\n");
    commit_as(dir, "Alice", "alice@example.com", "c3");

    let all = file_history(dir, "f.txt", 0).expect("all history");
    assert_eq!(all.len(), 3, "three commits touched f.txt");
    assert!(all.len() <= MAX_HISTORY);

    let capped = file_history(dir, "f.txt", 2).expect("capped history");
    assert_eq!(capped.len(), 2, "limit=2 caps the result");
    assert_eq!(capped[0].oid, all[0].oid, "newest-first preserved under cap");
    assert_eq!(capped[1].oid, all[1].oid);

    let empty = file_history(dir, "never-existed.txt", 100).expect("unknown path ok");
    assert!(empty.is_empty(), "unknown path -> empty history");
}
