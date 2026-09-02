//! Shared helpers for the P17 CLI-oracle partial-staging tests.
//!
//! Moved verbatim out of `stage_partial_cli.rs` when that file was split by
//! concern, so every `stage_partial_*_cli` module shares one copy. See
//! `stage_partial_cli` for the oracle rules and the HARD RULE on scratch
//! repo location.

use std::path::Path;

use bonsai_core::git::diff::{FileDiff, LineKind};
use bonsai_core::git::stage_partial::LineSelection;
use crate::common::{commit_fixed, git, git_raw, init_repo};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn write(dir: &Path, name: &str, content: &[u8]) {
    std::fs::write(dir.join(name), content).unwrap_or_else(|e| panic!("write {name}: {e}"));
}

/// Raw staged (stage-0 index) blob bytes for `path` via `git show :path`.
/// With `core.autocrlf=false` this is byte-exact.
pub(crate) fn staged_bytes(dir: &Path, path: &str) -> Vec<u8> {
    git_raw(dir, &["show", &format!(":{path}")], &[])
}

/// `git status --porcelain=v1` XY code for `path`, or `None` if not listed.
/// Reads RAW (untrimmed) output: a leading X-column space (e.g. " M") must not
/// be stripped, so the trimming `git()` helper cannot be used here.
pub(crate) fn xy(dir: &Path, path: &str) -> Option<String> {
    let raw = git_raw(dir, &["status", "--porcelain=v1", "--", path], &[]);
    let out = String::from_utf8_lossy(&raw);
    out.split('\n')
        .find(|l| l.get(3..).map(|p| p == path).unwrap_or(false))
        .map(|l| l[..2].to_string())
}

/// All Add/Del lines of a FileDiff as a selection (whole-file stage).
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

/// Add/Del lines of ONE hunk as a selection.
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

pub(crate) fn numbered(n: usize) -> Vec<u8> {
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

/// Base repo with a committed multi-line `f.txt` (n lines).
pub(crate) fn repo_with(n: usize) -> tempfile::TempDir {
    let dir = init_repo();
    write(dir.path(), "f.txt", &numbered(n));
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    dir
}