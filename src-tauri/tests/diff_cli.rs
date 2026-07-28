//! M4 CLI-oracle diff tests (contract §6.1–§6.2).
//!
//! The oracle is the `git` CLI (`git diff` / `git diff --cached` /
//! `git diff <oid>^1 <oid>` / `git show` / `git diff --numstat`), compared as
//! PARSED STRUCTURES, never raw text: `parse_cli_diff` reduces CLI output to
//! files/hunks/lines under the same normalization our engine applies
//! (contract §2.4: strip one `\n` then one `\r`; function-context tail
//! dropped; `\ No newline` folded into a flag on the preceding line).
//!
//! HARD RULE: all scratch repos live on D: via `common::scratch_dir()`
//! (through `init_repo`). Fixture repos pin `core.autocrlf=false` so CLI and
//! git2 see identical bytes.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_lib::error::AppError;
use bonsai_lib::git::diff::{
    commit_diff, commit_file_diff, workdir_file_diff, FileDiff, LineKind, MAX_FILE_DIFF_LINES,
};
use bonsai_lib::git::status::FileStatus;
use common::{commit_fixed, git, git_raw, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// Oracle parser (contract §6.1)
// ---------------------------------------------------------------------------

/// One parsed hunk: header numbers + (kind char, content, no_newline) lines.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHunk {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    lines: Vec<(char, String, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedFile {
    path: String,
    orig_path: Option<String>,
    binary: bool,
    hunks: Vec<ParsedHunk>,
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
fn parse_cli_diff(output: &str) -> Vec<ParsedFile> {
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
fn assert_line_numbers(fd: &FileDiff) {
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
fn assert_matches_oracle(fd: &FileDiff, dir: &Path, args: &[&str]) {
    let out = git_raw(dir, args, &[]);
    let parsed = parse_cli_diff(&String::from_utf8_lossy(&out));
    assert_eq!(parsed.len(), 1, "oracle `git {args:?}` yielded: {parsed:?}");
    assert_line_numbers(fd);
    assert_eq!(ours_parsed(fd), parsed[0], "vs oracle `git {args:?}`");
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn numbered_lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// Repo with a committed 40-line `f.txt` (multi-hunk playground).
fn repo_with_f40() -> tempfile::TempDir {
    let dir = init_repo();
    std::fs::write(dir.path().join("f.txt"), numbered_lines(40)).expect("write f.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    dir
}

/// Replaces line `n` (1-based) of `file` with `content` (keeps the rest).
fn edit_line(dir: &Path, file: &str, n: usize, content: &str) {
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

// ---------------------------------------------------------------------------
// §6.2 scenarios
// ---------------------------------------------------------------------------

// Scenario 1: two separated edits -> two hunks, all numbers/kinds/contents
// match the CLI.
#[test]
fn unstaged_modified_multi_hunk() {
    require_git!();
    let dir = repo_with_f40();
    edit_line(dir.path(), "f.txt", 3, "line 3 CHANGED");
    edit_line(dir.path(), "f.txt", 30, "line 30 CHANGED");

    let fd = workdir_file_diff(dir.path(), "f.txt", None, false).expect("unstaged diff");
    assert_eq!(fd.status, FileStatus::Modified);
    assert_eq!(fd.hunks.len(), 2, "edits at lines 3 and 30 must be 2 hunks");
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
}

// Scenario 2: staged edit matches `git diff --cached`; the unstaged diff of
// the same file is the benign-race empty FileDiff.
#[test]
fn staged_modified() {
    require_git!();
    let dir = repo_with_f40();
    edit_line(dir.path(), "f.txt", 3, "line 3 STAGED");
    git(dir.path(), &["add", "--", "f.txt"]);

    let staged = workdir_file_diff(dir.path(), "f.txt", None, true).expect("staged diff");
    assert_eq!(staged.status, FileStatus::Modified);
    assert_matches_oracle(
        &staged,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "f.txt"],
    );

    let unstaged = workdir_file_diff(dir.path(), "f.txt", None, false).expect("unstaged diff");
    assert!(unstaged.hunks.is_empty(), "workdir == index -> no hunks");
    assert!(!unstaged.binary && !unstaged.too_large);
}

// Scenario 3: staged edit + further workdir edit -> the two diffs differ and
// each matches its own oracle.
#[test]
fn staged_vs_unstaged_split() {
    require_git!();
    let dir = repo_with_f40();
    edit_line(dir.path(), "f.txt", 3, "line 3 STAGED");
    git(dir.path(), &["add", "--", "f.txt"]);
    edit_line(dir.path(), "f.txt", 30, "line 30 WORKDIR");

    let staged = workdir_file_diff(dir.path(), "f.txt", None, true).expect("staged diff");
    let unstaged = workdir_file_diff(dir.path(), "f.txt", None, false).expect("unstaged diff");
    assert_ne!(staged, unstaged);
    assert_matches_oracle(
        &staged,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
    assert_matches_oracle(
        &unstaged,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
}

// Scenario 4: untracked file -> structural all-Add assertion (the CLI has no
// direct oracle; contract §6.1 sanctions the structural check).
#[test]
fn untracked_file() {
    require_git!();
    let dir = repo_with_f40();
    let content = ["alpha", "beta", "gamma"];
    std::fs::write(
        dir.path().join("u.txt"),
        format!("{}\n", content.join("\n")),
    )
    .expect("write u.txt");

    let fd = workdir_file_diff(dir.path(), "u.txt", None, false).expect("untracked diff");
    assert_eq!(fd.status, FileStatus::Untracked);
    assert!(!fd.binary && !fd.too_large);
    assert_eq!(fd.hunks.len(), 1);
    let h = &fd.hunks[0];
    assert_eq!((h.old_start, h.old_lines), (0, 0));
    assert_eq!((h.new_start, h.new_lines), (1, content.len() as u32));
    for (i, line) in h.lines.iter().enumerate() {
        assert_eq!(line.kind, LineKind::Add);
        assert_eq!(line.content, content[i]);
        assert_eq!(line.new_no, Some(i as u32 + 1));
        assert_eq!(line.old_no, None);
    }
    assert_line_numbers(&fd);
}

// Scenario 5: fs-deleted tracked file -> unstaged all-Del; staged after
// `git add -A` -> staged all-Del. Both vs their oracles.
#[test]
fn deleted_file() {
    require_git!();
    let dir = repo_with_f40();
    std::fs::remove_file(dir.path().join("f.txt")).expect("delete f.txt");

    let fd = workdir_file_diff(dir.path(), "f.txt", None, false).expect("unstaged diff");
    assert_eq!(fd.status, FileStatus::Deleted);
    assert!(fd.hunks[0].lines.iter().all(|l| l.kind == LineKind::Del));
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "f.txt"],
    );

    git(dir.path(), &["add", "-A", "--", "f.txt"]);
    let staged = workdir_file_diff(dir.path(), "f.txt", None, true).expect("staged diff");
    assert_eq!(staged.status, FileStatus::Deleted);
    assert_matches_oracle(
        &staged,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
}

// Scenario 6: `git mv` + edit + stage -> one Renamed delta with orig_path,
// hunks matching `git diff --cached -M -- old new`.
#[test]
fn renamed_modified_staged() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("old.txt"), numbered_lines(20)).expect("write old.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");

    git(dir.path(), &["mv", "old.txt", "new.txt"]);
    edit_line(dir.path(), "new.txt", 10, "line 10 TWEAKED");
    git(dir.path(), &["add", "--", "new.txt"]);

    let fd = workdir_file_diff(dir.path(), "new.txt", Some("old.txt"), true)
        .expect("staged rename diff");
    assert_eq!(fd.status, FileStatus::Renamed);
    assert_eq!(fd.orig_path.as_deref(), Some("old.txt"));
    assert_eq!(fd.path, "new.txt");
    assert_matches_oracle(
        &fd,
        dir.path(),
        &[
            "diff", "--cached", "--no-color", "-U3", "-M", "--", "old.txt", "new.txt",
        ],
    );
}

// Scenario 7: missing trailing newline -> `no_newline` flags on exactly the
// lines where the CLI prints its marker (del AND add side).
#[test]
fn no_trailing_newline() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("n.txt"), "alpha\nbeta\ngamma").expect("write n.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    std::fs::write(dir.path().join("n.txt"), "alpha\nbeta\ndelta").expect("modify n.txt");

    let fd = workdir_file_diff(dir.path(), "n.txt", None, false).expect("diff");
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "n.txt"],
    );
    // Both the removed and the added final line lack the newline.
    let flagged: Vec<&str> = fd.hunks[0]
        .lines
        .iter()
        .filter(|l| l.no_newline)
        .map(|l| l.content.as_str())
        .collect();
    assert_eq!(flagged, vec!["gamma", "delta"]);
}

// Scenario 8: NUL-bearing blob -> binary: true, hunks: []; CLI agrees.
#[test]
fn binary_file() {
    require_git!();
    let dir = init_repo();
    let blob: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
    std::fs::write(dir.path().join("blob.bin"), &blob).expect("write blob.bin");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    let mut modified = blob;
    modified[10] = 0xAA;
    modified.extend_from_slice(&[0, 1, 2, 3]);
    std::fs::write(dir.path().join("blob.bin"), &modified).expect("modify blob.bin");

    let fd = workdir_file_diff(dir.path(), "blob.bin", None, false).expect("binary diff");
    assert!(fd.binary);
    assert!(!fd.too_large);
    assert!(fd.hunks.is_empty());

    let out = git_raw(
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "blob.bin"],
        &[],
    );
    let parsed = parse_cli_diff(&String::from_utf8_lossy(&out));
    assert_eq!(parsed.len(), 1);
    assert!(parsed[0].binary, "CLI must print Binary files ... differ");
    assert!(parsed[0].hunks.is_empty());
}

// Scenario 9: 6000-line deletion busts the 5000-line cap (too_large,
// all-or-nothing); a 100-line file stays under it.
#[test]
fn too_large_cap() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("big.txt"), numbered_lines(6_000)).expect("write big.txt");
    std::fs::write(dir.path().join("small.txt"), numbered_lines(100)).expect("write small.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    std::fs::remove_file(dir.path().join("big.txt")).expect("delete big.txt");
    std::fs::remove_file(dir.path().join("small.txt")).expect("delete small.txt");

    let big = workdir_file_diff(dir.path(), "big.txt", None, false).expect("big diff");
    assert!(big.too_large, "6000 del lines > {MAX_FILE_DIFF_LINES}");
    assert!(!big.binary);
    assert!(big.hunks.is_empty(), "all-or-nothing: no partial hunks");

    let small = workdir_file_diff(dir.path(), "small.txt", None, false).expect("small diff");
    assert!(!small.too_large);
    assert_eq!(
        small.hunks.iter().map(|h| h.lines.len()).sum::<usize>(),
        100
    );
    assert_matches_oracle(
        &small,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "small.txt"],
    );
}

// Scenario 10: CRLF content (autocrlf=false) -> contents match the CLI after
// §2.4 stripping; no phantom whole-file churn.
#[test]
fn crlf_content() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("c.txt"), "one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n")
        .expect("write c.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    std::fs::write(
        dir.path().join("c.txt"),
        "one\r\ntwo CHANGED\r\nthree\r\nfour\r\nfive\r\n",
    )
    .expect("modify c.txt");

    let fd = workdir_file_diff(dir.path(), "c.txt", None, false).expect("crlf diff");
    assert_eq!(fd.hunks.len(), 1, "single edit must stay a single hunk");
    assert!(
        fd.hunks[0]
            .lines
            .iter()
            .all(|l| !l.content.contains('\r') && !l.content.contains('\n')),
        "line endings must be stripped from content"
    );
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "c.txt"],
    );
}

/// Fixture for the commit-diff scenarios: base commit (a.txt, b.txt, z.txt),
/// tip commit with a multi-line message modifying a.txt + z.txt and adding
/// m.txt. Returns (dir, tip_oid).
fn commit_fixture() -> (tempfile::TempDir, String) {
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
fn numstat(dir: &Path, old: &str, new: &str) -> Vec<(String, u32, u32)> {
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

// Scenario 11: commit details + headers vs `git log` / `rev-parse` /
// `--numstat` oracles; files sorted by path.
#[test]
fn commit_diff_simple() {
    require_git!();
    let (dir, tip) = commit_fixture();
    let p = dir.path();

    let cd = commit_diff(p, &tip).expect("commit_diff");
    assert_eq!(cd.details.oid, tip);
    assert_eq!(cd.details.summary, "feat: subject line");

    let full_message = git(p, &["log", "-1", "--format=%B", &tip]);
    assert_eq!(cd.details.message, full_message.trim_end());
    assert!(cd.details.message.contains("body second line"));

    assert_eq!(cd.details.author_name, "Test User");
    assert_eq!(cd.details.author_email, "test@example.com");
    let at: i64 = git(p, &["log", "-1", "--format=%at", &tip])
        .parse()
        .expect("author ts");
    assert_eq!(cd.details.author_ts, at);
    let ct: i64 = git(p, &["log", "-1", "--format=%ct", &tip])
        .parse()
        .expect("committer ts");
    assert_eq!(cd.details.committer_ts, ct);

    let parents: Vec<String> = git(p, &["rev-parse", &format!("{tip}^@")])
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(cd.details.parents, parents);
    assert_eq!(cd.details.parents.len(), 1);

    // Headers vs numstat (the counts oracle), sorted by path.
    let ours: Vec<(String, u32, u32)> = cd
        .files
        .iter()
        .map(|f| (f.path.clone(), f.additions, f.deletions))
        .collect();
    assert_eq!(ours, numstat(p, &format!("{tip}^1"), &tip));
    let paths: Vec<&str> = cd.files.iter().map(|f| f.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "files must be sorted by path");
    assert!(cd.files.iter().all(|f| !f.binary));
    let m = cd.files.iter().find(|f| f.path == "m.txt").expect("m.txt");
    assert_eq!(m.status, FileStatus::Added);
}

// Scenario 12: per-file commit hunks vs parsed `git diff tip^1 tip -- path`.
#[test]
fn commit_file_diff_matches_show() {
    require_git!();
    let (dir, tip) = commit_fixture();

    for path in ["a.txt", "m.txt", "z.txt"] {
        let fd = commit_file_diff(dir.path(), &tip, path, None)
            .unwrap_or_else(|e| panic!("commit_file_diff({path}): {e:?}"));
        assert_matches_oracle(
            &fd,
            dir.path(),
            &[
                "diff",
                "--no-color",
                "-U3",
                "-M",
                &format!("{tip}^1"),
                &tip,
                "--",
                path,
            ],
        );
    }
}

// Scenario 13: root commit -> parents [], all files Added, file diff matches
// parsed `git show --format= root`.
#[test]
fn root_commit() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("first.txt"), numbered_lines(4)).expect("write first.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "root");
    let root = git(dir.path(), &["rev-parse", "HEAD"]);

    let cd = commit_diff(dir.path(), &root).expect("commit_diff(root)");
    assert!(cd.details.parents.is_empty());
    assert!(cd.files.iter().all(|f| f.status == FileStatus::Added));
    assert_eq!(cd.files.len(), 1);
    assert_eq!(cd.files[0].additions, 4);
    assert_eq!(cd.files[0].deletions, 0);

    let fd = commit_file_diff(dir.path(), &root, "first.txt", None).expect("root file diff");
    assert_eq!(fd.status, FileStatus::Added);
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["show", "--format=", "--no-color", "-U3", "-M", &root],
    );
}

// Scenario 14: merge commit diffs against the FIRST parent only (never --cc);
// details.parents carries both oids in order.
#[test]
fn merge_commit_first_parent() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("main.txt"), numbered_lines(10)).expect("write main.txt");
    std::fs::write(p.join("feat.txt"), numbered_lines(10)).expect("write feat.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");

    git(p, &["checkout", "-b", "feat"]);
    edit_line(p, "feat.txt", 2, "line 2 FEATURE");
    git(p, &["add", "-A"]);
    commit_fixed(p, "feat work");
    let feat_tip = git(p, &["rev-parse", "HEAD"]);

    git(p, &["checkout", "main"]);
    edit_line(p, "main.txt", 7, "line 7 MAINLINE");
    git(p, &["add", "-A"]);
    commit_fixed(p, "main work");
    let main_tip = git(p, &["rev-parse", "HEAD"]);

    common::git_env(
        p,
        &["merge", "--no-ff", "-m", "merge feat", "feat"],
        &[
            ("GIT_AUTHOR_DATE", common::FIXED_DATE),
            ("GIT_COMMITTER_DATE", common::FIXED_DATE),
        ],
    );
    let merge = git(p, &["rev-parse", "HEAD"]);

    let cd = commit_diff(p, &merge).expect("commit_diff(merge)");
    assert_eq!(cd.details.parents, vec![main_tip, feat_tip]);

    // vs first parent: only the feature-branch file changed.
    let ours: Vec<(String, u32, u32)> = cd
        .files
        .iter()
        .map(|f| (f.path.clone(), f.additions, f.deletions))
        .collect();
    assert_eq!(ours, numstat(p, &format!("{merge}^1"), &merge));
    assert_eq!(cd.files.len(), 1);
    assert_eq!(cd.files[0].path, "feat.txt");

    let fd = commit_file_diff(p, &merge, "feat.txt", None).expect("merge file diff");
    assert_matches_oracle(
        &fd,
        p,
        &[
            "diff",
            "--no-color",
            "-U3",
            "-M",
            &format!("{merge}^1"),
            &merge,
            "--",
            "feat.txt",
        ],
    );
}

// Scenario 15: unborn HEAD -> staged diff against the empty tree (all Add).
#[test]
fn unborn_staged() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("seed.txt"), numbered_lines(3)).expect("write seed.txt");
    git(dir.path(), &["add", "--", "seed.txt"]);

    let fd = workdir_file_diff(dir.path(), "seed.txt", None, true).expect("unborn staged diff");
    assert_eq!(fd.status, FileStatus::Added);
    assert!(fd.hunks[0].lines.iter().all(|l| l.kind == LineKind::Add));
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "seed.txt"],
    );
}

// Scenario 16: bad oids -> AppError::Git; escaping paths -> AppError::Other;
// commit_file_diff on an untouched path -> AppError::Git.
#[test]
fn bad_oid_and_path_validation() {
    require_git!();
    let (dir, tip) = commit_fixture();
    let p = dir.path();

    // Garbage oid: not hex.
    let err = commit_diff(p, "not-a-hex-oid").expect_err("garbage oid");
    assert!(matches!(err, AppError::Git(_)), "got: {err:?}");
    // Well-formed but unknown oid.
    let err = commit_diff(p, "0123456789abcdef0123456789abcdef01234567")
        .expect_err("unknown oid");
    assert!(matches!(err, AppError::Git(_)), "got: {err:?}");

    // Path validation (reused validate_rel_path).
    let err = workdir_file_diff(p, "../escape", None, false).expect_err("escaping path");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("invalid path")),
        "got: {err:?}"
    );
    let err = commit_file_diff(p, &tip, "../escape", None).expect_err("escaping path (commit)");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("invalid path")),
        "got: {err:?}"
    );

    // Untouched path in an immutable commit: an error, not an empty diff.
    let err = commit_file_diff(p, &tip, "b.txt", None).expect_err("untouched path");
    match err {
        AppError::Git(m) => assert!(m.contains("path not changed in commit"), "got: {m}"),
        other => panic!("expected AppError::Git, got: {other:?}"),
    }
}
