//! Partial (line-level) discard core (P28 contract §2).
//!
//! Same blob-reconstruction engine as `stage_partial` with the SIDES
//! substituted (§2.3): `reconstruct(Direction::Unstage, ...)` computes a
//! NEW-side base with selected `Add` lines dropped and selected `Del` lines
//! restored from the OLD side. With `old = index stage-0 blob` and
//! `new = worktree file`, hunks from a fresh `diff_index_to_workdir`, that is
//! exactly "worktree with the selected changes reverted to the index". The
//! result is written to the WORKTREE with `fs::write`; the index is NEVER
//! touched. Destructive — the UI confirms first. No Tauri types here; no
//! `repo-changed` emit.

use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::error::AppError;
use crate::git::diff::{
    apply_find_similar, build_diff_options, collect_file_diff, pathspecs, LineKind,
};
use crate::git::stage::{open_workdir_repo, validate_rel_path};
use crate::git::stage_partial::{
    index_blob_bytes, reconstruct, split_keep_terminator, stale, Direction, LineSelection,
};
use crate::git::status::FileStatus;

/// Blocking. Discards the selected changed lines of ONE tracked working-dir
/// file: the WORKTREE moves toward the INDEX for the selected lines only.
/// Selected `Add` (new-side) lines are removed from the worktree file; selected
/// `Del` (old-side) lines are restored from the index blob. The index is never
/// modified. Destructive — the UI must confirm first.
/// Empty selection -> `Ok(())` no-op. Untracked path -> Err (tracked-only).
pub fn discard_partial(
    workdir: &Path,
    path: &str,
    orig_path: Option<&str>,
    selection: &[LineSelection],
) -> Result<(), AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    // No-op before any repo work (§2.2).
    if selection.is_empty() {
        return Ok(());
    }

    let repo = open_workdir_repo(workdir)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?
        .to_path_buf();
    let index = repo.index()?;
    let rel = Path::new(path);

    // Tracked-only guard (mirrors discard.rs; BEFORE the diff — cheap and
    // decisive). An untracked file has no index blob; a full discard would
    // DELETE user content the index cannot restore.
    if index.get_path(rel, 0).is_none() {
        return Err(AppError::Git(format!(
            "cannot discard '{path}': not a tracked file"
        )));
    }

    // Freshly recompute the unstaged diff for this file (P17 §2.3 pattern;
    // DEFAULT 3-context — line numbering is context-independent).
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths, false);
    let mut diff = repo.diff_index_to_workdir(None, Some(&mut opts))?; // old=index, new=worktree
    apply_find_similar(&mut diff)?;
    let fd = collect_file_diff(&diff)?.ok_or_else(stale)?; // matched nothing -> stale

    // Guards (P17 §2.5, "discard" wording).
    if fd.binary {
        return Err(AppError::Other(
            "partial discard is not supported for binary files".to_string(),
        ));
    }
    if fd.too_large {
        return Err(AppError::Other(
            "partial discard is not supported for a too-large diff".to_string(),
        ));
    }
    if fd.orig_path.is_some() || fd.status == FileStatus::Renamed {
        return Err(AppError::Other(
            "partial discard is not supported for renamed files".to_string(),
        ));
    }

    // Selected sets + stale validation (identical to P17 §2.3). Stray
    // `Context` elements are ignored.
    let mut sel_add: HashSet<u32> = HashSet::new();
    let mut sel_del: HashSet<u32> = HashSet::new();
    for s in selection {
        match s.kind {
            LineKind::Add => {
                if let Some(n) = s.new_no {
                    sel_add.insert(n);
                }
            }
            LineKind::Del => {
                if let Some(n) = s.old_no {
                    sel_del.insert(n);
                }
            }
            LineKind::Context => {}
        }
    }
    let mut valid_add: HashSet<u32> = HashSet::new();
    let mut valid_del: HashSet<u32> = HashSet::new();
    for h in &fd.hunks {
        for l in &h.lines {
            match l.kind {
                LineKind::Add => {
                    if let Some(n) = l.new_no {
                        valid_add.insert(n);
                    }
                }
                LineKind::Del => {
                    if let Some(n) = l.old_no {
                        valid_del.insert(n);
                    }
                }
                LineKind::Context => {}
            }
        }
    }
    if !sel_add.is_subset(&valid_add) || !sel_del.is_subset(&valid_del) {
        return Err(stale());
    }

    // Raw bytes of both sides (NEVER `DiffLine.content`). SIDE SUBSTITUTION
    // (§0.1): old = index stage-0 blob, new = worktree file.
    let old_bytes = index_blob_bytes(&repo, &index, rel)?;
    let new_bytes = match fs::read(wd.join(rel)) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(), // deleted -> recreate
        Err(e) => return Err(e.into()),
    };

    let old_lines = split_keep_terminator(&old_bytes);
    let new_lines = split_keep_terminator(&new_bytes);

    // Side-substituted reconstruct (§2.3): Direction::Unstage semantics on the
    // (index, worktree) pair — base = NEW (worktree), selected changes undone
    // toward OLD (index).
    let result = reconstruct(
        Direction::Unstage,
        &fd.hunks,
        &old_lines,
        &new_lines,
        &sel_add,
        &sel_del,
    )?;
    let result = normalize_terminators(result, &new_lines, autocrlf(&repo));
    let content = assemble_cow(&result);

    // No-op: nothing to write (preserves mtime, avoids a watcher storm).
    if content == new_bytes {
        return Ok(());
    }

    // Write the WORKTREE. Recreates a worktree-deleted file (new_bytes == b"").
    // The index is never touched — `git diff --cached` is invariant here.
    fs::write(wd.join(rel), &content)?;
    Ok(())
}

/// Whether `core.autocrlf` is `true` for this repo (§2.4). Absent / unreadable
/// / `input` -> `false` (byte-exact mode).
fn autocrlf(repo: &git2::Repository) -> bool {
    repo.config()
        .and_then(|c| c.get_bool("core.autocrlf"))
        .unwrap_or(false)
}

/// Terminator normalization for the discard direction only (§2.4). With
/// `core.autocrlf=true` the index blob is LF-normalized while the worktree is
/// CRLF; a restored `Del` slice spliced verbatim would produce mixed endings
/// git reports as perpetually modified. When autocrlf is on AND the CURRENT
/// worktree file is CRLF-majority, any slice ending in bare `\n` is rewritten
/// to end `\r\n`. Slices already CRLF or with no terminator (EOF) are
/// untouched; empty/deleted worktree (recreate case) falls back to the index
/// blob's own terminators.
fn normalize_terminators<'a>(
    result: Vec<&'a [u8]>,
    worktree_lines: &[&[u8]],
    autocrlf: bool,
) -> Vec<Cow<'a, [u8]>> {
    let as_cow = |v: Vec<&'a [u8]>| v.into_iter().map(Cow::Borrowed).collect::<Vec<_>>();
    if !autocrlf {
        return as_cow(result); // byte-exact mode (P17 semantics)
    }
    let crlf = worktree_lines
        .iter()
        .filter(|l| l.ends_with(b"\r\n"))
        .count();
    let lf = worktree_lines
        .iter()
        .filter(|l| l.ends_with(b"\n") && !l.ends_with(b"\r\n"))
        .count();
    if worktree_lines.is_empty() || crlf <= lf {
        return as_cow(result);
    }
    // CRLF-majority worktree: bare-LF slices (from the LF index blob) get CRLF.
    result
        .into_iter()
        .map(|s| {
            if s.ends_with(b"\n") && !s.ends_with(b"\r\n") {
                let mut owned = s[..s.len() - 1].to_vec();
                owned.extend_from_slice(b"\r\n");
                Cow::Owned(owned)
            } else {
                Cow::Borrowed(s)
            }
        })
        .collect()
}

/// `assemble` (P17 §2.4) over `Cow` slices — same semantics: an interior slice
/// lacking a terminator gets a single `\n`; the final slice keeps its own
/// terminator state. Local variant so `assemble`'s signature is unchanged.
fn assemble_cow(lines: &[Cow<'_, [u8]>]) -> Vec<u8> {
    let mut out = Vec::new();
    let last = lines.len().wrapping_sub(1);
    for (i, s) in lines.iter().enumerate() {
        out.extend_from_slice(s);
        if i != last && s.last() != Some(&b'\n') {
            out.push(b'\n');
        }
    }
    out
}

#[cfg(test)]
mod tests;
