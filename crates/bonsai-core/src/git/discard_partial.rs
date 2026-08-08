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
mod tests {
    use super::*;

    fn init(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    fn commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
        for (name, content) in files {
            std::fs::write(dir.join(name), content).expect("write");
        }
        crate::git::stage::stage_paths(
            dir,
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
    }

    fn sel(kind: LineKind, old_no: Option<u32>, new_no: Option<u32>) -> LineSelection {
        LineSelection {
            kind,
            old_no,
            new_no,
        }
    }

    /// Stage-0 index entry oid for `path` (the index-invariant probe).
    fn index_oid(dir: &Path, path: &str) -> git2::Oid {
        let repo = git2::Repository::open(dir).expect("open");
        let index = repo.index().expect("index");
        index.get_path(Path::new(path), 0).expect("entry").id
    }

    /// Empty selection is a no-op before any repo work (§6.1.1).
    #[test]
    fn empty_selection_noop() {
        let dir = crate::testutil::scratch_dir();
        let missing = dir.path().join("not-a-repo");
        assert!(discard_partial(&missing, "file.txt", None, &[]).is_ok());
    }

    /// Invalid paths (and orig_path) are rejected before repo work (§6.1.2).
    #[test]
    fn invalid_path_rejected() {
        let dir = crate::testutil::scratch_dir();
        let s = vec![sel(LineKind::Add, None, Some(1))];
        for bad in ["", "../escape", "/abs", "a\\b"] {
            let err = discard_partial(dir.path(), bad, None, &s)
                .expect_err(&format!("must reject {bad:?}"));
            assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
        }
        let err = discard_partial(dir.path(), "ok.txt", Some("../escape"), &s)
            .expect_err("bad orig_path");
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }

    /// Untracked file -> tracked-only Git error; worktree bytes untouched (§6.1.3).
    #[test]
    fn untracked_rejected() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("new.txt"), "precious\n").expect("write");

        let s = vec![sel(LineKind::Add, None, Some(1))];
        let err = discard_partial(d, "new.txt", None, &s).expect_err("untracked");
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("not a tracked file")),
            "got: {err:?}"
        );
        assert_eq!(std::fs::read(d.join("new.txt")).expect("read"), b"precious\n");
    }

    /// Binary diff -> unsupported (§6.1.4).
    #[test]
    fn binary_rejected() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        std::fs::write(d.join("bin.dat"), b"a\0b\n").expect("write");
        crate::git::stage::stage_paths(d, &["bin.dat".to_string()]).expect("stage");
        crate::git::commit::create_commit(d, "bin", None, false).expect("commit");
        std::fs::write(d.join("bin.dat"), b"c\0d\n").expect("edit");

        let s = vec![sel(LineKind::Add, None, Some(1))];
        let err = discard_partial(d, "bin.dat", None, &s).expect_err("binary");
        assert!(matches!(err, AppError::Other(m) if m.contains("binary")));
    }

    /// > MAX_FILE_DIFF_LINES emitted lines -> too_large -> unsupported (§6.1.4).
    #[test]
    fn too_large_rejected() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("big.txt", "base\n")]);
        let mut big = String::new();
        for i in 0..6000 {
            big.push_str(&format!("line {i}\n"));
        }
        std::fs::write(d.join("big.txt"), big).expect("edit");

        let s = vec![sel(LineKind::Add, None, Some(1))];
        let err = discard_partial(d, "big.txt", None, &s).expect_err("too large");
        assert!(matches!(err, AppError::Other(m) if m.contains("too-large")));
    }

    /// A rename-shaped unstaged state (worktree rename, orig_path passed like
    /// the frontend does) is rejected with `Other`; neither file is touched
    /// (§6.1.4). NOTE: `apply_find_similar` does not set FIND_FOR_UNTRACKED,
    /// so `diff_index_to_workdir` never actually pairs an untracked rename
    /// target into one `Renamed` delta — the explicit renamed guard is purely
    /// defensive and this state is rejected as a stale selection instead
    /// (still `AppError::Other`, still zero writes).
    #[test]
    fn renamed_rejected() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        let body = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\n";
        commit(d, "base", &[("old.txt", body)]);
        // Unstaged worktree rename: old.txt (tracked) gone, new.txt appears.
        std::fs::rename(d.join("old.txt"), d.join("new.txt")).expect("rename");

        // Called with the TRACKED side (passes the tracked-only guard) and the
        // rename partner as orig_path — the diff pairs them into one Renamed
        // delta and the guard rejects it.
        let s = vec![sel(LineKind::Del, Some(1), None)];
        let err =
            discard_partial(d, "old.txt", Some("new.txt"), &s).expect_err("renamed diff");
        assert!(
            matches!(&err, AppError::Other(m) if m.contains("renamed") || m.contains("stale")),
            "got: {err:?}"
        );
        // Nothing changed on disk.
        assert!(!d.join("old.txt").exists());
        assert_eq!(std::fs::read(d.join("new.txt")).expect("read"), body.as_bytes());
    }

    /// Stale coordinates and clean-file pathspec both -> stale (§6.1.5).
    #[test]
    fn stale_selection() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "one\ntwo\nthree\n")]);
        std::fs::write(d.join("a.txt"), "one\nTWO\nthree\n").expect("edit");

        // Coordinate not in the fresh diff (add at new line 99).
        let s = vec![sel(LineKind::Add, None, Some(99))];
        let err = discard_partial(d, "a.txt", None, &s).expect_err("stale coord");
        assert!(matches!(err, AppError::Other(m) if m.contains("stale")));

        // Clean file: pathspec matches nothing -> stale.
        commit(d, "clean", &[("b.txt", "x\n")]);
        let s = vec![sel(LineKind::Add, None, Some(1))];
        let err = discard_partial(d, "b.txt", None, &s).expect_err("clean file");
        assert!(matches!(err, AppError::Other(m) if m.contains("stale")));
    }

    /// Discarding the middle of three hunks reverts it; hunks 1 & 3 survive;
    /// the index blob oid is unchanged (§6.1.6, §6.1.9).
    #[test]
    fn one_of_three_hunks() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        // 15 lines; edits at 2, 8, 14 -> three separated hunks (3-context).
        let base: String = (1..=15).map(|i| format!("l{i}\n")).collect();
        commit(d, "base", &[("f.txt", &base)]);
        let edited = base
            .replace("l2\n", "L2\n")
            .replace("l8\n", "L8\n")
            .replace("l14\n", "L14\n");
        std::fs::write(d.join("f.txt"), &edited).expect("edit");
        let oid_before = index_oid(d, "f.txt");

        // Middle hunk: del old line 8, add new line 8.
        let s = vec![
            sel(LineKind::Del, Some(8), None),
            sel(LineKind::Add, None, Some(8)),
        ];
        discard_partial(d, "f.txt", None, &s).expect("discard middle hunk");

        let expect = base.replace("l2\n", "L2\n").replace("l14\n", "L14\n");
        assert_eq!(
            std::fs::read(d.join("f.txt")).expect("read"),
            expect.as_bytes()
        );
        assert_eq!(index_oid(d, "f.txt"), oid_before, "index must be untouched");
    }

    /// P45: a ONE-element `Add` selection discards exactly that inserted line
    /// while a second inserted line (same diff) survives; the index blob oid is
    /// untouched. Confirms single-line (not whole-hunk) discard granularity.
    #[test]
    fn single_added_line_discarded() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        // Index/HEAD = 3 lines; worktree inserts X (new 2) and Y (new 4).
        commit(d, "base", &[("f.txt", "a\nb\nc\n")]);
        std::fs::write(d.join("f.txt"), "a\nX\nb\nY\nc\n").expect("edit");
        let oid_before = index_oid(d, "f.txt");

        // Discard ONLY the first inserted line (one-element selection).
        let s = vec![sel(LineKind::Add, None, Some(2))];
        discard_partial(d, "f.txt", None, &s).expect("discard one added line");

        // X reverted; Y (the other added line) remains; index untouched.
        assert_eq!(
            std::fs::read(d.join("f.txt")).expect("read"),
            b"a\nb\nY\nc\n",
            "only the selected added line reverts to the index blob"
        );
        assert_eq!(index_oid(d, "f.txt"), oid_before, "index must be untouched");
    }

    /// P45: a ONE-element `Del` selection restores exactly that removed line
    /// from the index blob while a second removed line stays deleted; the index
    /// blob oid is untouched.
    #[test]
    fn single_deleted_line_discarded() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        // Index/HEAD = 4 lines; worktree deletes b (old 2) and d (old 4).
        commit(d, "base", &[("f.txt", "a\nb\nc\nd\n")]);
        std::fs::write(d.join("f.txt"), "a\nc\n").expect("edit");
        let oid_before = index_oid(d, "f.txt");

        // Discard ONLY the deletion of b (one-element selection) -> restore it.
        let s = vec![sel(LineKind::Del, Some(2), None)];
        discard_partial(d, "f.txt", None, &s).expect("discard one deleted line");

        // b restored from the index; d stays deleted; index untouched.
        assert_eq!(
            std::fs::read(d.join("f.txt")).expect("read"),
            b"a\nb\nc\n",
            "only the selected deleted line is restored from the index blob"
        );
        assert_eq!(index_oid(d, "f.txt"), oid_before, "index must be untouched");
    }

    /// Deleting the worktree file and discarding all its Del lines recreates
    /// it with the index bytes (§6.1.7).
    #[test]
    fn deleted_file_recreated() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        let body = "a\nb\nc\n";
        commit(d, "base", &[("gone.txt", body)]);
        std::fs::remove_file(d.join("gone.txt")).expect("delete");
        let oid_before = index_oid(d, "gone.txt");

        let s = vec![
            sel(LineKind::Del, Some(1), None),
            sel(LineKind::Del, Some(2), None),
            sel(LineKind::Del, Some(3), None),
        ];
        discard_partial(d, "gone.txt", None, &s).expect("recreate");
        assert_eq!(
            std::fs::read(d.join("gone.txt")).expect("read"),
            body.as_bytes()
        );
        assert_eq!(index_oid(d, "gone.txt"), oid_before);
    }

    /// A selection whose reconstruction equals the current worktree bytes is a
    /// no-op: bytes AND mtime untouched (§6.1.8).
    #[test]
    fn noop_result() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        // Index deleted a line relative to the worktree? Simplest true no-op:
        // an unstaged Del whose restoration is NOT selected and no Add selected
        // — but empty selections short-circuit. Instead: worktree adds line
        // "X"; select only a stray Context -> sets empty after filtering, and
        // the reconstruction (nothing dropped/restored) equals the worktree.
        commit(d, "base", &[("n.txt", "a\nb\n")]);
        std::fs::write(d.join("n.txt"), "a\nX\nb\n").expect("edit");
        let file = d.join("n.txt");
        let mtime_before = std::fs::metadata(&file)
            .and_then(|m| m.modified())
            .expect("mtime");

        let s = vec![sel(LineKind::Context, Some(1), Some(1))];
        discard_partial(d, "n.txt", None, &s).expect("noop");
        assert_eq!(std::fs::read(&file).expect("read"), b"a\nX\nb\n");
        let mtime_after = std::fs::metadata(&file)
            .and_then(|m| m.modified())
            .expect("mtime");
        assert_eq!(mtime_before, mtime_after, "no-op must not rewrite the file");
    }

    /// autocrlf=true + CRLF worktree: a restored index (LF) line is spliced
    /// with a CRLF terminator — no mixed endings (§2.4).
    #[test]
    fn crlf_restored_del_normalized() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);
        repo.config()
            .expect("config")
            .set_bool("core.autocrlf", true)
            .expect("autocrlf on");
        // CRLF worktree file; autocrlf stores LF in the index blob.
        std::fs::write(d.join("c.txt"), b"one\r\ntwo\r\nthree\r\n").expect("write");
        crate::git::stage::stage_paths(d, &["c.txt".to_string()]).expect("stage");
        crate::git::commit::create_commit(d, "crlf base", None, false).expect("commit");
        // Worktree deletes line "two" (still CRLF).
        std::fs::write(d.join("c.txt"), b"one\r\nthree\r\n").expect("edit");

        // Discard the deletion: restore old line 2 from the LF index blob.
        let s = vec![sel(LineKind::Del, Some(2), None)];
        discard_partial(d, "c.txt", None, &s).expect("discard del");
        assert_eq!(
            std::fs::read(d.join("c.txt")).expect("read"),
            b"one\r\ntwo\r\nthree\r\n",
            "restored line must carry CRLF, not bare LF"
        );
    }

    // --- normalize_terminators unit coverage --------------------------------

    #[test]
    fn normalize_is_identity_when_autocrlf_off_or_lf_majority() {
        let wt: Vec<&[u8]> = vec![b"a\r\n", b"b\n", b"c\n"];
        let r = normalize_terminators(vec![b"x\n"], &wt, false);
        assert_eq!(r[0].as_ref(), b"x\n");
        // autocrlf on but LF-majority -> untouched.
        let r = normalize_terminators(vec![b"x\n"], &wt, true);
        assert_eq!(r[0].as_ref(), b"x\n");
        // Empty worktree (recreate) -> untouched.
        let r = normalize_terminators(vec![b"x\n"], &[], true);
        assert_eq!(r[0].as_ref(), b"x\n");
    }

    #[test]
    fn normalize_rewrites_bare_lf_on_crlf_majority() {
        let wt: Vec<&[u8]> = vec![b"a\r\n", b"b\r\n", b"c\n"];
        let r = normalize_terminators(vec![b"x\n", b"y\r\n", b"z"], &wt, true);
        assert_eq!(r[0].as_ref(), b"x\r\n"); // bare LF -> CRLF
        assert_eq!(r[1].as_ref(), b"y\r\n"); // already CRLF untouched
        assert_eq!(r[2].as_ref(), b"z"); // no terminator (EOF) untouched
    }
}
