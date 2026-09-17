//! Shared CLI-oracle parser, comparison helpers, and fixtures for the M4
//! diff CLI-oracle tests (contract §6.1).
//!
//! Moved verbatim out of `diff_cli.rs` when that file was split by concern,
//! so `diff_cli`, `diff_commit_cli` and `diff_adversarial_cli` share one
//! copy. See `diff_cli` for the oracle rules and the HARD RULE on scratch
//! repo location.

use std::path::Path;

use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo};
use bonsai_core::git::diff::{FileDiff, LineKind};

// ---------------------------------------------------------------------------
// Oracle parser (contract §6.1)
// ---------------------------------------------------------------------------

/// One parsed hunk: header numbers + (kind char, content, no_newline) lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedHunk {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    lines: Vec<(char, String, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedFile {
    path: String,
    orig_path: Option<String>,
    pub(crate) binary: bool,
    pub(crate) hunks: Vec<ParsedHunk>,
}

/// `-a,b` / `+c,d` range token -> (start, count); omitted count = 1.
fn parse_range(tok: &str) -> (u32, u32) {
    let t = &tok[1..];
    match t.split_once(',') {
        Some((a, b)) => (
            a.parse().expect("range start"),
            b.parse().expect("range count"),
        ),
        None => (t.parse().expect("range start"), 1),
    }
}

/// `@@ -a,b +c,d @@ tail` -> hunk numbers (tail ignored by design).
fn parse_hunk_header(line: &str) -> ParsedHunk {
    let mut toks = line.split_whitespace();
    let at = toks.next().expect("@@ token");
    assert_eq!(at, "@@", "not a hunk header: {line}");
    let old = parse_range(toks.next().expect("old range"));
    let new = parse_range(toks.next().expect("new range"));
    ParsedHunk {
        old_start: old.0,
        old_lines: old.1,
        new_start: new.0,
        new_lines: new.1,
        lines: Vec::new(),
    }
}

/// Strips one trailing `\r` (the `\n` was consumed by the line split) —
/// mirrors the engine's §2.4 policy.
fn strip_cr(s: &str) -> String {
    s.strip_suffix('\r').unwrap_or(s).to_string()
}

/// Per-file parse state while walking CLI output.
#[derive(Debug, Default)]
struct CurFile {
    git_b: Option<String>, // b/ side of the `diff --git` line (fallback path)
    rename_from: Option<String>,
    rename_to: Option<String>,
    minus: Option<String>, // `--- a/...` (or /dev/null)
    plus: Option<String>,  // `+++ b/...` (or /dev/null)
    binary: bool,
    hunks: Vec<ParsedHunk>,
    in_hunk: bool,
}

fn finish(cur: CurFile, files: &mut Vec<ParsedFile>) {
    let not_devnull = |p: String| if p == "/dev/null" { None } else { Some(p) };
    let path = cur
        .rename_to
        .clone()
        .or_else(|| cur.plus.clone().and_then(not_devnull))
        .or_else(|| cur.minus.clone().and_then(not_devnull))
        .or(cur.git_b)
        .expect("parsed file has no path");
    files.push(ParsedFile {
        path,
        orig_path: cur.rename_from,
        binary: cur.binary,
        hunks: cur.hunks,
    });
}

/// Strips the CLI's `a/` / `b/` prefix from a `---`/`+++` path.
fn strip_ab(p: &str) -> String {
    if p == "/dev/null" {
        p.to_string()
    } else {
        p.strip_prefix("a/")
            .or_else(|| p.strip_prefix("b/"))
            .unwrap_or(p)
            .to_string()
    }
}

/// Parses `git diff --no-color -U3 -M` (or `git show --format=`) output into
/// structures comparable with our `FileDiff` (contract §6.1). Paths in the
/// fixtures never contain spaces or quoting-triggering bytes.
pub(crate) fn parse_cli_diff(output: &str) -> Vec<ParsedFile> {
    let mut files: Vec<ParsedFile> = Vec::new();
    let mut cur: Option<CurFile> = None;

    let mut lines = output.split('\n').peekable();
    while let Some(raw) = lines.next() {
        // The final split fragment after a trailing \n is empty.
        if raw.is_empty() && lines.peek().is_none() {
            break;
        }
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some(prev) = cur.take() {
                finish(prev, &mut files);
            }
            let b = rest.split_once(" b/").map(|(_, b)| b.to_string());
            cur = Some(CurFile {
                git_b: b,
                ..CurFile::default()
            });
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };

        if c.in_hunk {
            if line.starts_with("@@") {
                c.hunks.push(parse_hunk_header(line));
            } else if let Some(rest) = raw.strip_prefix(' ') {
                c.hunks
                    .last_mut()
                    .expect("content before hunk header")
                    .lines
                    .push((' ', strip_cr(rest), false));
            } else if let Some(rest) = raw.strip_prefix('+') {
                c.hunks
                    .last_mut()
                    .expect("content before hunk header")
                    .lines
                    .push(('+', strip_cr(rest), false));
            } else if let Some(rest) = raw.strip_prefix('-') {
                c.hunks
                    .last_mut()
                    .expect("content before hunk header")
                    .lines
                    .push(('-', strip_cr(rest), false));
            } else if line.starts_with('\\') {
                // "\ No newline at end of file" -> flag the previous line.
                let last = c
                    .hunks
                    .last_mut()
                    .and_then(|h| h.lines.last_mut())
                    .expect("no-newline marker without a preceding line");
                last.2 = true;
            }
            continue;
        }

        if line.starts_with("@@") {
            c.in_hunk = true;
            c.hunks.push(parse_hunk_header(line));
        } else if let Some(p) = line.strip_prefix("rename from ") {
            c.rename_from = Some(p.to_string());
        } else if let Some(p) = line.strip_prefix("rename to ") {
            c.rename_to = Some(p.to_string());
        } else if let Some(p) = line.strip_prefix("--- ") {
            c.minus = Some(strip_ab(p));
        } else if let Some(p) = line.strip_prefix("+++ ") {
            c.plus = Some(strip_ab(p));
        } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            c.binary = true;
        }
        // index/mode/similarity/copy headers: skipped.
    }
    if let Some(prev) = cur.take() {
        finish(prev, &mut files);
    }
    files
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

fn kind_char(kind: LineKind) -> char {
    match kind {
        LineKind::Context => ' ',
        LineKind::Add => '+',
        LineKind::Del => '-',
    }
}

/// Our FileDiff reduced to the oracle's shape.
fn ours_parsed(fd: &FileDiff) -> ParsedFile {
    ParsedFile {
        path: fd.path.clone(),
        orig_path: fd.orig_path.clone(),
        binary: fd.binary,
        hunks: fd
            .hunks
            .iter()
            .map(|h| ParsedHunk {
                old_start: h.old_start,
                old_lines: h.old_lines,
                new_start: h.new_start,
                new_lines: h.new_lines,
                lines: h
                    .lines
                    .iter()
                    .map(|l| (kind_char(l.kind), l.content.clone(), l.no_newline))
                    .collect(),
            })
            .collect(),
    }
}

/// §6.1: DiffLine old/new numbers recomputed from hunk starts and asserted
/// consistent (Add -> old None; Del -> new None; Context -> both).
pub(crate) fn assert_line_numbers(fd: &FileDiff) {
    for hunk in &fd.hunks {
        let mut old = hunk.old_start;
        let mut new = hunk.new_start;
        for line in &hunk.lines {
            match line.kind {
                LineKind::Context => {
                    assert_eq!(line.old_no, Some(old), "context old_no in {}", fd.path);
                    assert_eq!(line.new_no, Some(new), "context new_no in {}", fd.path);
                    old += 1;
                    new += 1;
                }
                LineKind::Add => {
                    assert_eq!(line.old_no, None, "add old_no in {}", fd.path);
                    assert_eq!(line.new_no, Some(new), "add new_no in {}", fd.path);
                    new += 1;
                }
                LineKind::Del => {
                    assert_eq!(line.old_no, Some(old), "del old_no in {}", fd.path);
                    assert_eq!(line.new_no, None, "del new_no in {}", fd.path);
                    old += 1;
                }
            }
        }
    }
}

/// Runs the CLI oracle and asserts our FileDiff matches its single parsed file.
pub(crate) fn assert_matches_oracle(fd: &FileDiff, dir: &Path, args: &[&str]) {
    let out = git_raw(dir, args, &[]);
    let parsed = parse_cli_diff(&String::from_utf8_lossy(&out));
    assert_eq!(parsed.len(), 1, "oracle `git {args:?}` yielded: {parsed:?}");
    assert_line_numbers(fd);
    assert_eq!(ours_parsed(fd), parsed[0], "vs oracle `git {args:?}`");
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

pub(crate) fn numbered_lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// Repo with a committed 40-line `f.txt` (multi-hunk playground).
pub(crate) fn repo_with_f40() -> tempfile::TempDir {
    let dir = init_repo();
    std::fs::write(dir.path().join("f.txt"), numbered_lines(40)).expect("write f.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    dir
}

/// Replaces line `n` (1-based) of `file` with `content` (keeps the rest).
pub(crate) fn edit_line(dir: &Path, file: &str, n: usize, content: &str) {
    let full = dir.join(file);
    let text = std::fs::read_to_string(&full).expect("read file");
    let lines: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i + 1 == n {
                content.to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    std::fs::write(&full, format!("{}\n", lines.join("\n"))).expect("write file");
}

/// Fixture for the commit-diff scenarios: base commit (a.txt, b.txt, z.txt),
/// tip commit with a multi-line message modifying a.txt + z.txt and adding
/// m.txt. Returns (dir, tip_oid).
pub(crate) fn commit_fixture() -> (tempfile::TempDir, String) {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), numbered_lines(10)).expect("write a.txt");
    std::fs::write(p.join("b.txt"), "b content\n").expect("write b.txt");
    std::fs::write(p.join("z.txt"), numbered_lines(5)).expect("write z.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");

    edit_line(p, "a.txt", 5, "line 5 EDITED");
    edit_line(p, "z.txt", 1, "line 1 EDITED");
    std::fs::write(p.join("m.txt"), "brand new\n").expect("write m.txt");
    git(p, &["add", "-A"]);
    common::git_env(
        p,
        &[
            "commit",
            "-m",
            "feat: subject line\n\nbody first line\nbody second line",
        ],
        &[
            ("GIT_AUTHOR_DATE", common::FIXED_DATE),
            ("GIT_COMMITTER_DATE", common::FIXED_DATE),
        ],
    );
    let tip = git(p, &["rev-parse", "HEAD"]);
    (dir, tip)
}

/// `git diff --numstat -M old new` -> sorted (path, additions, deletions);
/// binary files report `-\t-` and are asserted separately.
pub(crate) fn numstat(dir: &Path, old: &str, new: &str) -> Vec<(String, u32, u32)> {
    let out = git_raw(dir, &["diff", "--numstat", "-M", old, new], &[]);
    let text = String::from_utf8_lossy(&out);
    let mut rows: Vec<(String, u32, u32)> = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut cols = l.split('\t');
            let adds: u32 = cols.next().expect("adds").parse().unwrap_or(0);
            let dels: u32 = cols.next().expect("dels").parse().unwrap_or(0);
            // Rename rows may use the "old => new" or NUL-free "old\tnew"? With
            // plain --numstat the third column is the path ("old => new" form
            // for renames); our fixtures avoid renamed rows here.
            let path = cols.next().expect("path").to_string();
            (path, adds, dels)
        })
        .collect();
    rows.sort();
    rows
}
