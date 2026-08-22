//! Partial (line-level) staging core (P17 contract §2).
//!
//! git2 0.20 has no line-level apply primitive and the wire `DiffLine.content`
//! is lossy, so we do **blob reconstruction, not patch-text synthesis**
//! (§0.1): recompute the exact diff for the direction, read the RAW BYTES of
//! both blobs, and splice line-slices by their `old_no`/`new_no` to build the
//! new index blob, written with `Index::add_frombuffer`. Each line slice keeps
//! its own terminator, so CRLF and no-newline-at-EOF round-trip byte-exact.
//! The workdir side is read as the CHECK-IN FILTERED blob (what `git add`
//! would stage), never raw disk bytes — see `workdir_filtered_bytes`.
//!
//! Direction is encoded by the command (§0.2): `stage_partial` moves the index
//! toward the workdir for the selected lines; `unstage_partial` moves it toward
//! HEAD. Both are pure re-derivations of the CURRENT index, so repeated partial
//! calls compose. No Tauri types here; no `repo-changed` emit.

use std::collections::HashSet;
use std::path::Path;

use crate::error::AppError;
use crate::git::diff::{
    apply_find_similar, build_diff_options, collect_file_diff, head_tree, pathspecs, Hunk, LineKind,
};
use crate::git::stage::{ensure_within_workdir, open_workdir_repo, validate_rel_path};
use crate::git::status::FileStatus;

/// One selected changed line from the UI (P17 §2.1). `Context` elements are
/// ignored (context is always kept in both directions) and do NOT participate
/// in the stale-selection check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSelection {
    /// `Add` or `Del`; stray `Context` is ignored.
    pub kind: LineKind,
    /// OLD-file line number; the identity of a selected `Del` line.
    pub old_no: Option<u32>,
    /// NEW-file line number; the identity of a selected `Add` line.
    pub new_no: Option<u32>,
}

/// Which way the selected lines move. Encoded by the command, not a wire arg.
#[derive(Clone, Copy)]
pub(crate) enum Direction {
    /// Index moves toward the workdir (old = index, new = workdir).
    Stage,
    /// Index moves toward HEAD (old = HEAD, new = index).
    Unstage,
}

/// Stage the selected changed lines of ONE working-dir file (index moves toward
/// the workdir for the selected lines only). Empty selection -> `Ok` no-op.
pub fn stage_partial(
    workdir: &Path,
    path: &str,
    orig_path: Option<&str>,
    selection: &[LineSelection],
) -> Result<(), AppError> {
    apply_partial(workdir, path, orig_path, selection, Direction::Stage)
}

/// Unstage the selected changed lines of ONE staged file (index moves toward
/// HEAD for the selected lines only). Empty selection -> `Ok` no-op.
pub fn unstage_partial(
    workdir: &Path,
    path: &str,
    orig_path: Option<&str>,
    selection: &[LineSelection],
) -> Result<(), AppError> {
    apply_partial(workdir, path, orig_path, selection, Direction::Unstage)
}

/// The stale-selection error (§2.5): a coordinate absent from the freshly
/// recomputed diff, a pathspec that matched nothing, or a byte range that no
/// longer lines up (a TOCTOU race between the diff and the blob read).
pub(crate) fn stale() -> AppError {
    AppError::Other("selection is stale; refresh the diff".to_string())
}

/// The shared blob-reconstruction path for both directions (§2.3, normative).
fn apply_partial(
    workdir: &Path,
    path: &str,
    orig_path: Option<&str>,
    selection: &[LineSelection],
    dir: Direction,
) -> Result<(), AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    // No-op before any repo work.
    if selection.is_empty() {
        return Ok(());
    }

    let repo = open_workdir_repo(workdir)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?
        .to_path_buf();

    // Symlink-escape guard (see `ensure_within_workdir`): the workdir-side reads
    // below join `path` onto `wd`; refuse a `path` escaping via a symlinked ancestor.
    ensure_within_workdir(&wd, path)?;

    // Recompute the exact diff for this direction. DEFAULT 3-context is fine:
    // the reconstruction fills inter-hunk gaps from the blobs by line number,
    // and context amount never changes add/del line numbers (§2.3, §9.3).
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths, false);
    // HEAD tree is `None` when HEAD is unborn. Used by the Unstage diff, its
    // old-side blob read, and the presence-based removal discriminator (SF-1).
    // Peeled once here for both directions (cheap; the Stage path only reads it
    // for `head_has_path`).
    let head = head_tree(&repo)?;
    let mut diff = match dir {
        Direction::Stage => repo.diff_index_to_workdir(None, Some(&mut opts))?,
        Direction::Unstage => repo.diff_tree_to_index(head.as_ref(), None, Some(&mut opts))?,
    };
    apply_find_similar(&mut diff)?;

    // Pathspec matched nothing -> stale (the file raced back to clean).
    let fd = collect_file_diff(&diff)?.ok_or_else(stale)?;

    // Guards (§2.5): the frontend suppresses these controls, but defend.
    if fd.binary {
        return Err(AppError::Other(
            "partial staging is not supported for binary files".to_string(),
        ));
    }
    if fd.too_large {
        return Err(AppError::Other(
            "partial staging is not supported for a too-large diff".to_string(),
        ));
    }
    if fd.orig_path.is_some() || fd.status == FileStatus::Renamed {
        return Err(AppError::Other(
            "partial staging is not supported for renamed files".to_string(),
        ));
    }

    // Selected sets + stale validation. Stray `Context` elements are ignored;
    // an `Add` selection with no `new_no` (or `Del` with no `old_no`) simply
    // contributes nothing (§2.1).
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

    let mut index = repo.index()?;
    let rel = Path::new(path);

    // Exact bytes of both sides (NEVER `DiffLine.content`). The workdir side
    // is the check-in FILTERED content: the diff above already compares the
    // filtered workdir against the index, so line numbers line up, and the
    // reconstructed blob must match what `git add` would stage.
    let (old_bytes, new_bytes) = match dir {
        Direction::Stage => {
            let old = index_blob_bytes(&repo, &index, rel)?; // b"" if untracked
            let new = workdir_filtered_bytes(&repo, &wd, rel)?; // b"" if deleted
            (old, new)
        }
        Direction::Unstage => {
            let old = head_blob_bytes(&repo, head.as_ref(), rel)?; // b"" if unborn/added
            let new = index_blob_bytes(&repo, &index, rel)?; // b"" if absent
            (old, new)
        }
    };

    let old_lines = split_keep_terminator(&old_bytes);
    let new_lines = split_keep_terminator(&new_bytes);
    let result = reconstruct(dir, &fd.hunks, &old_lines, &new_lines, &sel_add, &sel_del)?;
    let content = assemble(&result);

    // No-op: reconstructed index content == current index content -> write
    // nothing (§2.5). For Stage the current index blob IS `old_bytes`; for
    // Unstage it is `new_bytes`. Re-read to keep the two branches uniform.
    let cur_index_bytes = index_blob_bytes(&repo, &index, rel)?;
    if content == cur_index_bytes {
        return Ok(());
    }

    // Removal cases (§2.5): the file legitimately should not exist in the index.
    // Discriminate by PRESENCE, not byte-emptiness (SF-1): a `b""` result also
    // means "file exists but is empty" (an emptied tracked file staged, or a
    // committed empty file restored) — those write an EMPTY BLOB, they are not
    // removals.
    let head_has_path = head
        .as_ref()
        .and_then(|t| t.get_path(rel).ok())
        .is_some();
    if content.is_empty() && should_remove(dir, fd.status, head_has_path) {
        index.remove_path(rel)?;
        index.write()?;
        return Ok(());
    }

    let entry = index
        .get_path(rel, 0)
        .unwrap_or_else(|| synthesize_entry(path, &wd));
    index.add_frombuffer(&entry, &content)?;
    index.write()?;
    Ok(())
}

/// Stage-0 index blob bytes for `path`, or `b""` when there is no entry.
pub(crate) fn index_blob_bytes(
    repo: &git2::Repository,
    index: &git2::Index,
    path: &Path,
) -> Result<Vec<u8>, AppError> {
    match index.get_path(path, 0) {
        Some(entry) => Ok(repo.find_blob(entry.id)?.content().to_vec()),
        None => Ok(Vec::new()),
    }
}

/// Check-in FILTERED workdir bytes for `path`, or `b""` when the file is
/// absent (an unstaged deletion). `Repository::blob_path` (libgit2
/// `git_blob_create_fromdisk`, absolute path) runs the check-in filter chain
/// (CRLF/ident) for files inside the workdir — exactly what `git add` stages.
/// Raw `fs::read` here would embed CRLF against an LF index under
/// `core.autocrlf=true`, desyncing both the reconstruction input and the
/// no-op comparison.
fn workdir_filtered_bytes(
    repo: &git2::Repository,
    wd: &Path,
    path: &Path,
) -> Result<Vec<u8>, AppError> {
    let abs = wd.join(path);
    // Only a genuinely-missing file maps to the deletion path (`b""`); every
    // other error (EACCES, sharing violation, ...) must propagate — and there
    // is no pre-check, so a racing delete lands on the same NotFound arm.
    let oid = match repo.blob_path(&abs) {
        Ok(oid) => oid,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(repo.find_blob(oid)?.content().to_vec())
}

/// HEAD blob bytes for `path`, or `b""` when HEAD is unborn / the file is
/// absent in HEAD (a never-committed add).
fn head_blob_bytes(
    repo: &git2::Repository,
    head_tree: Option<&git2::Tree>,
    path: &Path,
) -> Result<Vec<u8>, AppError> {
    let Some(tree) = head_tree else {
        return Ok(Vec::new());
    };
    match tree.get_path(path) {
        Ok(entry) => Ok(repo.find_blob(entry.id())?.content().to_vec()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Whether a fully-emptied reconstruction means the file should be REMOVED from
/// the index rather than written as an empty blob (§2.5, SF-1). Presence-based,
/// never byte-emptiness:
/// - **Stage**: only when the workdir file is genuinely absent (status
///   `Deleted`). An emptied-but-present tracked file is `Modified` -> empty blob.
/// - **Unstage**: only when the path is genuinely absent from HEAD (a
///   never-committed add). A committed *empty* file IS present in HEAD ->
///   restore its empty blob.
fn should_remove(dir: Direction, status: FileStatus, head_has_path: bool) -> bool {
    match dir {
        Direction::Stage => status == FileStatus::Deleted,
        Direction::Unstage => !head_has_path,
    }
}

/// Builds a synthesized `IndexEntry` for staging an untracked file (§2.3).
/// `mode` from `wd.join(path).symlink_metadata()`; every other field is zero
/// and `id = Oid::zero()` — `add_frombuffer` recomputes `id`/`file_size` from
/// the buffer.
fn synthesize_entry(path: &str, wd: &Path) -> git2::IndexEntry {
    git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: mode_for(&wd.join(path)),
        uid: 0,
        gid: 0,
        file_size: 0,
        id: git2::Oid::ZERO_SHA1,
        flags: 0,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

/// File mode for a synthesized entry: `0o120000` symlink, `0o100755` when the
/// owner-exec bit is set (unix only), else `0o100644` (Windows default).
fn mode_for(full: &Path) -> u32 {
    let Ok(meta) = full.symlink_metadata() else {
        return 0o100644;
    };
    if meta.file_type().is_symlink() {
        return 0o120000;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o100 != 0 {
            return 0o100755;
        }
    }
    0o100644
}

/// Splits `bytes` into line slices, each KEEPING its trailing `\n`; only the
/// last slice may lack one. `b""` -> `vec![]` (§2.4).
pub(crate) fn split_keep_terminator(bytes: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            out.push(&bytes[start..=i]);
            start = i + 1;
        }
    }
    if start < bytes.len() {
        out.push(&bytes[start..]);
    }
    out
}

/// Indexes a 1-based line-number slice, mapping any out-of-range / missing
/// number to a stale-selection error rather than panicking (defends the rare
/// TOCTOU race between the recomputed diff and the blob read).
pub(crate) fn nth<'a>(lines: &[&'a [u8]], n: Option<u32>) -> Result<&'a [u8], AppError> {
    let n = n.ok_or_else(stale)?;
    let idx = (n as usize).checked_sub(1).ok_or_else(stale)?;
    lines.get(idx).copied().ok_or_else(stale)
}

/// Reconstructs the new index blob's line slices for `dir` (§2.4, normative).
///
/// **Stage** = OLD (index) with unselected changes reverted and selected
/// changes applied. **Unstage** = NEW (index) with selected changes undone
/// (adds removed, dels restored from HEAD) and unselected changes kept. Both
/// fill inter-hunk gaps from the base side by 1-based line number.
pub(crate) fn reconstruct<'a>(
    dir: Direction,
    hunks: &[Hunk],
    old_lines: &[&'a [u8]],
    new_lines: &[&'a [u8]],
    sel_add: &HashSet<u32>,
    sel_del: &HashSet<u32>,
) -> Result<Vec<&'a [u8]>, AppError> {
    let mut result: Vec<&[u8]> = Vec::new();
    match dir {
        Direction::Stage => {
            // base = OLD (index); target = new index.
            let mut cursor: u32 = 1;
            for h in hunks {
                while cursor < h.old_start {
                    result.push(nth(old_lines, Some(cursor))?);
                    cursor += 1;
                }
                for line in &h.lines {
                    match line.kind {
                        LineKind::Context => {
                            let n = line.old_no.ok_or_else(stale)?;
                            result.push(nth(old_lines, Some(n))?);
                            cursor = n + 1;
                        }
                        LineKind::Del => {
                            let n = line.old_no.ok_or_else(stale)?;
                            if !sel_del.contains(&n) {
                                result.push(nth(old_lines, Some(n))?);
                            }
                            cursor = n + 1;
                        }
                        LineKind::Add => {
                            let n = line.new_no.ok_or_else(stale)?;
                            if sel_add.contains(&n) {
                                result.push(nth(new_lines, Some(n))?);
                            }
                            // cursor unchanged — Add has no old line.
                        }
                    }
                }
            }
            while (cursor as usize) <= old_lines.len() {
                result.push(nth(old_lines, Some(cursor))?);
                cursor += 1;
            }
        }
        Direction::Unstage => {
            // base = NEW (index); target = new index.
            let mut cursor: u32 = 1;
            for h in hunks {
                while cursor < h.new_start {
                    result.push(nth(new_lines, Some(cursor))?);
                    cursor += 1;
                }
                for line in &h.lines {
                    match line.kind {
                        LineKind::Context => {
                            let n = line.new_no.ok_or_else(stale)?;
                            result.push(nth(new_lines, Some(n))?);
                            cursor = n + 1;
                        }
                        LineKind::Add => {
                            let n = line.new_no.ok_or_else(stale)?;
                            if !sel_add.contains(&n) {
                                result.push(nth(new_lines, Some(n))?);
                            }
                            cursor = n + 1;
                        }
                        LineKind::Del => {
                            let n = line.old_no.ok_or_else(stale)?;
                            if sel_del.contains(&n) {
                                result.push(nth(old_lines, Some(n))?); // restore HEAD line
                            }
                            // cursor unchanged — Del has no new line.
                        }
                    }
                }
            }
            while (cursor as usize) <= new_lines.len() {
                result.push(nth(new_lines, Some(cursor))?);
                cursor += 1;
            }
        }
    }
    Ok(result)
}

/// Concatenates line slices. An interior slice that lacks a terminator (was
/// EOF in its SOURCE file but is now interior) gets a single `\n`; the final
/// slice keeps its own terminator state (§2.4). This is what makes CRLF +
/// no-EOF-newline byte-exact.
pub(crate) fn assemble(lines: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    let last = lines.len().wrapping_sub(1);
    for (i, s) in lines.iter().enumerate() {
        out.extend_from_slice(s);
        let is_last = i == last;
        if !is_last && s.last() != Some(&b'\n') {
            out.push(b'\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::diff::DiffLine;

    // --- split_keep_terminator ---------------------------------------------

    #[test]
    fn split_empty_is_no_lines() {
        assert!(split_keep_terminator(b"").is_empty());
    }

    #[test]
    fn split_keeps_lf_and_trailing_no_newline() {
        assert_eq!(split_keep_terminator(b"a\nb\nc"), vec![&b"a\n"[..], b"b\n", b"c"]);
        assert_eq!(split_keep_terminator(b"a\nb\n"), vec![&b"a\n"[..], b"b\n"]);
        assert_eq!(split_keep_terminator(b"solo"), vec![&b"solo"[..]]);
        // A lone newline is one line "\n".
        assert_eq!(split_keep_terminator(b"\n"), vec![&b"\n"[..]]);
    }

    #[test]
    fn split_keeps_crlf_inside_the_slice() {
        assert_eq!(
            split_keep_terminator(b"one\r\ntwo\r\n"),
            vec![&b"one\r\n"[..], b"two\r\n"]
        );
    }

    // --- assemble ----------------------------------------------------------

    #[test]
    fn assemble_empty_is_empty() {
        assert_eq!(assemble(&[]), b"");
    }

    #[test]
    fn assemble_interior_missing_terminator_gets_lf() {
        // "b" was EOF-with-no-newline in its source but is now interior.
        assert_eq!(assemble(&[b"a\n", b"b", b"c\n"]), b"a\nb\nc\n");
    }

    #[test]
    fn assemble_final_slice_keeps_its_terminator_state() {
        assert_eq!(assemble(&[b"a\n", b"b"]), b"a\nb"); // no final newline
        assert_eq!(assemble(&[b"a\n", b"b\n"]), b"a\nb\n"); // final newline kept
    }

    // --- reconstruct helpers -----------------------------------------------

    fn dl(kind: LineKind, old_no: Option<u32>, new_no: Option<u32>) -> DiffLine {
        DiffLine {
            kind,
            old_no,
            new_no,
            content: String::new(),
            no_newline: false,
            spans: Vec::new(),
        }
    }

    fn set(v: &[u32]) -> HashSet<u32> {
        v.iter().copied().collect()
    }

    /// Full stage: reconstruct(Stage) with everything selected == new bytes;
    /// nothing selected == old bytes. Byte-exact, CRLF preserved.
    #[test]
    fn stage_crlf_modification_is_byte_exact() {
        let old = b"one\r\ntwo\r\nthree\r\n"; // index
        let new = b"one\r\ntwo CHANGED\r\nthree\r\n"; // workdir
        let old_lines = split_keep_terminator(old);
        let new_lines = split_keep_terminator(new);
        let hunk = Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                dl(LineKind::Context, Some(1), Some(1)),
                dl(LineKind::Del, Some(2), None),
                dl(LineKind::Add, None, Some(2)),
                dl(LineKind::Context, Some(3), Some(3)),
            ],
        };
        // Accept the modification: pick both the del and the add.
        let hunks = std::slice::from_ref(&hunk);
        let got = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[2]), &set(&[2]))
            .expect("reconstruct");
        assert_eq!(assemble(&got), new, "CRLF must survive byte-for-byte");
        // Reject everything: back to the index bytes.
        let none = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[]), &set(&[]))
            .expect("reconstruct");
        assert_eq!(assemble(&none), old);
    }

    /// no-newline-at-EOF: staging the final-line modification keeps the exact
    /// (absent) terminator; staging only the deletion keeps the earlier
    /// terminator.
    #[test]
    fn stage_no_newline_eof_is_byte_exact() {
        let old = b"a\nb\nc"; // no trailing newline
        let new = b"a\nb\nd"; // no trailing newline
        let old_lines = split_keep_terminator(old);
        let new_lines = split_keep_terminator(new);
        let hunk = Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                dl(LineKind::Context, Some(1), Some(1)),
                dl(LineKind::Context, Some(2), Some(2)),
                dl(LineKind::Del, Some(3), None),
                dl(LineKind::Add, None, Some(3)),
            ],
        };
        let hunks = std::slice::from_ref(&hunk);
        let full = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[3]), &set(&[3]))
            .expect("reconstruct");
        assert_eq!(assemble(&full), new); // "a\nb\nd", no trailing newline

        // Stage only the deletion of "c": "b" keeps its own newline.
        let del_only = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[]), &set(&[3]))
            .expect("reconstruct");
        assert_eq!(assemble(&del_only), b"a\nb\n");
    }

    /// Unstage restores the HEAD line for a selected Del and keeps unselected
    /// index changes.
    #[test]
    fn unstage_restores_head_line_for_selected_del() {
        // HEAD (old) has "b"; index (new) deleted it and added "x".
        let head = b"a\nb\nc\n";
        let index = b"a\nx\nc\n";
        let old_lines = split_keep_terminator(head);
        let new_lines = split_keep_terminator(index);
        let hunk = Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                dl(LineKind::Context, Some(1), Some(1)),
                dl(LineKind::Del, Some(2), None),
                dl(LineKind::Add, None, Some(2)),
                dl(LineKind::Context, Some(3), Some(3)),
            ],
        };
        // Unstage BOTH the add and the del -> index reverts to HEAD.
        let hunks = std::slice::from_ref(&hunk);
        let both = reconstruct(Direction::Unstage, hunks, &old_lines, &new_lines, &set(&[2]), &set(&[2]))
            .expect("reconstruct");
        assert_eq!(assemble(&both), head);
        // Unstage only the add -> "x" removed but "b" not restored yet.
        let add_only = reconstruct(Direction::Unstage, hunks, &old_lines, &new_lines, &set(&[2]), &set(&[]))
            .expect("reconstruct");
        assert_eq!(assemble(&add_only), b"a\nc\n");
    }

    /// Inter-hunk gap fill: a two-hunk stage where only the first hunk's change
    /// is selected keeps the untouched middle region and reverts the 2nd hunk.
    #[test]
    fn stage_two_hunks_gap_filled_from_base() {
        // 6-line file; edits at line 2 and line 5.
        let old = b"l1\nl2\nl3\nl4\nl5\nl6\n";
        let new = b"l1\nL2\nl3\nl4\nL5\nl6\n";
        let old_lines = split_keep_terminator(old);
        let new_lines = split_keep_terminator(new);
        let h1 = Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                dl(LineKind::Context, Some(1), Some(1)),
                dl(LineKind::Del, Some(2), None),
                dl(LineKind::Add, None, Some(2)),
                dl(LineKind::Context, Some(3), Some(3)),
            ],
        };
        let h2 = Hunk {
            old_start: 4,
            old_lines: 3,
            new_start: 4,
            new_lines: 3,
            lines: vec![
                dl(LineKind::Context, Some(4), Some(4)),
                dl(LineKind::Del, Some(5), None),
                dl(LineKind::Add, None, Some(5)),
                dl(LineKind::Context, Some(6), Some(6)),
            ],
        };
        // Select only hunk 1's change (new line 2). Hunk 2 reverted; gap (l4)
        // filled from old.
        let got = reconstruct(
            Direction::Stage,
            &[h1, h2],
            &old_lines,
            &new_lines,
            &set(&[2]),
            &set(&[2]),
        )
        .expect("reconstruct");
        assert_eq!(assemble(&got), b"l1\nL2\nl3\nl4\nl5\nl6\n");
    }

    #[test]
    fn stale_line_number_out_of_range_errors() {
        let old_lines = split_keep_terminator(b"a\n");
        let new_lines = split_keep_terminator(b"a\nb\n");
        let hunk = Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 2,
            lines: vec![
                dl(LineKind::Context, Some(1), Some(1)),
                // Add referencing new line 2, which is present.
                dl(LineKind::Add, None, Some(2)),
            ],
        };
        // A hunk that references an old line 5 that does not exist -> stale.
        let bad = Hunk {
            old_start: 5,
            old_lines: 0,
            new_start: 3,
            new_lines: 0,
            lines: vec![dl(LineKind::Context, Some(5), Some(3))],
        };
        let err = reconstruct(
            Direction::Stage,
            &[hunk, bad],
            &old_lines,
            &new_lines,
            &set(&[2]),
            &set(&[]),
        )
        .expect_err("out-of-range context must be stale");
        assert!(matches!(err, AppError::Other(m) if m.contains("stale")));
    }

    /// Empty selection is a no-op before any repo work (no path/repo needed).
    #[test]
    fn empty_selection_is_a_noop() {
        let dir = crate::testutil::scratch_dir();
        let missing = dir.path().join("not-a-repo");
        assert!(stage_partial(&missing, "file.txt", None, &[]).is_ok());
        assert!(unstage_partial(&missing, "file.txt", None, &[]).is_ok());
    }

    /// Audit 2026-08-07 §2.1: partial staging must apply CHECK-IN filters.
    /// Under `core.autocrlf=true` a CRLF workdir file must stage an LF blob,
    /// and a full-selection partial stage must produce the EXACT index blob
    /// `git add` (`Index::add_path`, which filters) would produce.
    #[test]
    fn stage_partial_applies_checkin_filters_under_autocrlf() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        repo.config()
            .expect("config")
            .set_bool("core.autocrlf", true)
            .expect("autocrlf");
        let sig = git2::Signature::now("Test", "t@example.com").expect("sig");

        // Base commit: CRLF on disk -> LF in the ODB via the filtering add_path.
        std::fs::write(dir.path().join("f.txt"), "one\r\ntwo\r\n").expect("write base");
        let mut idx = repo.index().expect("index");
        idx.add_path(Path::new("f.txt")).expect("add base");
        idx.write().expect("write index");
        let tree_oid = idx.write_tree().expect("tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .expect("commit");
        drop(tree);

        // Workdir edit (CRLF): append line 3.
        std::fs::write(dir.path().join("f.txt"), "one\r\ntwo\r\nthree\r\n").expect("edit");

        // Oracle: the blob oid `git add` would stage, then reset the index back
        // to HEAD so the partial stage starts from a clean index.
        idx.add_path(Path::new("f.txt")).expect("oracle add");
        let expected_oid = idx.get_path(Path::new("f.txt"), 0).expect("oracle entry").id;
        let head_tree = repo.head().expect("head").peel_to_tree().expect("head tree");
        idx.read_tree(&head_tree).expect("reset index");
        idx.write().expect("write reset index");
        drop(head_tree);
        drop(idx);

        // Partial-stage the single added line — the FULL selection of this diff.
        let sel = vec![LineSelection {
            kind: LineKind::Add,
            old_no: None,
            new_no: Some(3),
        }];
        stage_partial(dir.path(), "f.txt", None, &sel).expect("stage_partial");

        // Fresh open: stage_partial wrote through its own Repository handle.
        let repo2 = git2::Repository::open(dir.path()).expect("reopen");
        let entry = repo2
            .index()
            .expect("index")
            .get_path(Path::new("f.txt"), 0)
            .expect("staged entry");
        let content = repo2.find_blob(entry.id).expect("blob").content().to_vec();
        assert_eq!(content, b"one\ntwo\nthree\n", "staged blob must be LF-only");
        assert_eq!(
            entry.id, expected_oid,
            "full-selection partial stage must equal `git add` byte-for-byte"
        );
    }

    /// Invalid paths are rejected by the reused validator, before repo work.
    #[test]
    fn invalid_paths_are_rejected() {
        let dir = crate::testutil::scratch_dir();
        let sel = vec![LineSelection {
            kind: LineKind::Add,
            old_no: None,
            new_no: Some(1),
        }];
        for bad in ["", "../escape", "/abs", "a\\b"] {
            let err = stage_partial(dir.path(), bad, None, &sel)
                .expect_err(&format!("must reject {bad:?}"));
            assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
        }
        let err = stage_partial(dir.path(), "ok.txt", Some("../escape"), &sel)
            .expect_err("bad orig_path");
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }
}
