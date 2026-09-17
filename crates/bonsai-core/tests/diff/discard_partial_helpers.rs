//! Shared helpers for the P28 CLI-oracle `discard_partial` tests.
//!
//! Moved verbatim out of `discard_partial_cli.rs` when the helper block was
//! split off that file. See `discard_partial_cli` for the oracles and the
//! HARD RULE on scratch repo location.

use std::path::Path;

use crate::common::{commit_fixed, git, git_raw, init_repo};
use bonsai_core::git::diff::{FileDiff, LineKind};
use bonsai_core::git::stage_partial::LineSelection;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn write(dir: &Path, name: &str, content: &[u8]) {
    std::fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("write {name}: {e}"));
}

pub(crate) fn read(dir: &Path, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// Raw bytes of `git diff --cached` — the index-invariant oracle.
pub(crate) fn cached_diff(dir: &Path) -> Vec<u8> {
    git_raw(dir, &["diff", "--cached", "--no-color"], &[])
}

/// `git status --porcelain=v1` XY code for `path`, or `None` if not listed.
/// Raw (untrimmed) output — a leading X-column space must survive.
pub(crate) fn xy(dir: &Path, path: &str) -> Option<String> {
    let raw = git_raw(dir, &["status", "--porcelain=v1", "--", path], &[]);
    let out = String::from_utf8_lossy(&raw);
    out.split('\n')
        .find(|l| l.get(3..).map(|p| p == path).unwrap_or(false))
        .map(|l| l[..2].to_string())
}

/// All Add/Del lines of a FileDiff as a selection (whole-file discard).
pub(crate) fn all_changed(fd: &FileDiff) -> Vec<LineSelection> {
    let mut sel = Vec::new();
    for h in &fd.hunks {
        for l in &h.lines {
            if matches!(l.kind, LineKind::Add | LineKind::Del) {
                sel.push(LineSelection {
                    kind: l.kind,
                    old_no: l.old_no,
                    new_no: l.new_no,
                });
            }
        }
    }
    sel
}

/// Add/Del lines of ONE hunk as a selection (the UI's hunk button shape).
pub(crate) fn hunk_changed(fd: &FileDiff, hunk_idx: usize) -> Vec<LineSelection> {
    let mut sel = Vec::new();
    for l in &fd.hunks[hunk_idx].lines {
        if matches!(l.kind, LineKind::Add | LineKind::Del) {
            sel.push(LineSelection {
                kind: l.kind,
                old_no: l.old_no,
                new_no: l.new_no,
            });
        }
    }
    sel
}

fn numbered(n: usize) -> Vec<u8> {
    (1..=n)
        .map(|i| format!("line {i}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Numbered file with the given 1-based lines replaced by new text.
pub(crate) fn numbered_edited(n: usize, edits: &[(usize, &str)]) -> Vec<u8> {
    let mut lines: Vec<String> = (1..=n).map(|i| format!("line {i}")).collect();
    for (idx, text) in edits {
        lines[*idx - 1] = (*text).to_string();
    }
    (lines.join("\n") + "\n").into_bytes()
}

/// Base repo with a committed multi-line `f.txt` (n lines), autocrlf=false.
pub(crate) fn repo_with(n: usize) -> tempfile::TempDir {
    let dir = init_repo();
    write(dir.path(), "f.txt", &numbered(n));
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    dir
}

/// Extracts the `diff --git` header + hunk number `idx` (0-based) from a full
/// unified patch as a standalone single-file, single-hunk patch.
pub(crate) fn single_hunk_patch(full: &str, idx: usize) -> String {
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    let mut cur: Option<String> = None;
    for line in full.lines() {
        if line.starts_with("@@") {
            if let Some(h) = cur.take() {
                hunks.push(h);
            }
            cur = Some(format!("{line}\n"));
        } else if let Some(h) = cur.as_mut() {
            h.push_str(line);
            h.push('\n');
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }
    if let Some(h) = cur.take() {
        hunks.push(h);
    }
    format!("{header}{}", hunks[idx])
}

/// Byte-level variant of `single_hunk_patch` for patches whose body contains
/// \r bytes (CRLF files): splits on b'\n' WITHOUT stripping \r.
pub(crate) fn single_hunk_patch_bytes(full: &[u8], idx: usize) -> Vec<u8> {
    let mut header: Vec<u8> = Vec::new();
    let mut hunks: Vec<Vec<u8>> = Vec::new();
    let mut cur: Option<Vec<u8>> = None;
    for line in full.split_inclusive(|&b| b == b'\n') {
        if line.starts_with(b"@@") {
            if let Some(h) = cur.take() {
                hunks.push(h);
            }
            cur = Some(line.to_vec());
        } else if let Some(h) = cur.as_mut() {
            h.extend_from_slice(line);
        } else {
            header.extend_from_slice(line);
        }
    }
    if let Some(h) = cur.take() {
        hunks.push(h);
    }
    header.extend_from_slice(&hunks[idx]);
    header
}

/// Pipes `patch` into `git apply <args>` in `dir`, asserting success.
pub(crate) fn git_apply_stdin(dir: &Path, args: &[&str], patch: &[u8]) {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .arg("apply")
        .args(args)
        .arg("-")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git apply");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(patch)
        .expect("write patch");
    let out = child.wait_with_output().expect("wait git apply");
    assert!(
        out.status.success(),
        "git apply {:?} failed: {}\n--- patch ---\n{}",
        args,
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(patch)
    );
}
