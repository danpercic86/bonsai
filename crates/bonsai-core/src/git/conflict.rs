//! Index-conflict listing, read-only marker view, and file-level resolution.
//! Operation-agnostic: works identically during merge (P3c) and rebase (P3d).
//! Pure git2, no Tauri types (P3c contract §3).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{ensure_within_workdir, open_workdir_repo, validate_rel_path};

/// Byte cap for the marker view. Above it: too_large=true, text="".
/// Same all-or-nothing spirit as diff.rs MAX_FILE_DIFF_LINES.
pub const MAX_CONFLICT_BYTES: u64 = 1_048_576; // 1 MiB

/// Derived from which index stages exist (contract §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    BothModified,
    BothAdded,
    DeletedByUs,
    DeletedByThem,
    AddedByUs,
    AddedByThem,
    BothDeleted,
}

/// One conflicted path. `path` is repo-relative, forward slashes: prefer the
/// OURS side's path, else THEIRS, else ANCESTOR (rename conflicts can differ;
/// v1 surfaces one row per index conflict record under that preferred path).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntry {
    pub path: String,
    pub kind: ConflictKind,
    pub has_base: bool,
    pub has_ours: bool,
    pub has_theirs: bool,
}

/// Read-only working-tree view of one conflicted file, markers included.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub kind: ConflictKind,
    /// NUL byte within the first 8000 bytes -> binary, text="".
    pub binary: bool,
    /// File size > MAX_CONFLICT_BYTES -> too_large, text="".
    pub too_large: bool,
    /// Worktree file missing (e.g. deletedBy* kinds) -> true, text="".
    pub missing: bool,
    /// Lossy UTF-8 of the worktree file WITH the <<<<<<< ======= >>>>>>> markers.
    pub text: String,
    /// Lossy UTF-8 of the stage-2 (OURS) blob content. "" when the ours side is
    /// absent (deletedByUs / addedByThem / bothDeleted) OR when binary/too_large/
    /// missing suppressed `text`.
    pub ours: String,
    /// Lossy UTF-8 of the stage-3 (THEIRS) blob content. "" when the theirs side
    /// is absent (deletedByThem / addedByUs / bothDeleted) OR when suppressed.
    pub theirs: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictResolution {
    Ours,
    Theirs,
    MarkResolved,
}

/// Pure kind derivation from stage presence (contract §3.1 truth table).
fn derive_kind(has_base: bool, has_ours: bool, has_theirs: bool) -> ConflictKind {
    match (has_base, has_ours, has_theirs) {
        (true, true, true) => ConflictKind::BothModified,
        (false, true, true) => ConflictKind::BothAdded,
        (true, false, true) => ConflictKind::DeletedByUs,
        (true, true, false) => ConflictKind::DeletedByThem,
        (false, true, false) => ConflictKind::AddedByUs,
        (false, false, true) => ConflictKind::AddedByThem,
        (true, false, false) => ConflictKind::BothDeleted,
        (false, false, false) => {
            // Impossible per libgit2 — a conflict record has at least one stage.
            debug_assert!(false, "conflict record with no stages");
            ConflictKind::BothDeleted
        }
    }
}

/// Repo-relative path of a conflict record: ours, else theirs, else ancestor.
fn entry_path(c: &git2::IndexConflict) -> Option<String> {
    c.our
        .as_ref()
        .or(c.their.as_ref())
        .or(c.ancestor.as_ref())
        .map(|e| String::from_utf8_lossy(&e.path).into_owned())
}

fn conflict_to_entry(c: &git2::IndexConflict) -> Option<ConflictEntry> {
    let path = entry_path(c)?;
    let (has_base, has_ours, has_theirs) =
        (c.ancestor.is_some(), c.our.is_some(), c.their.is_some());
    Some(ConflictEntry {
        path,
        kind: derive_kind(has_base, has_ours, has_theirs),
        has_base,
        has_ours,
        has_theirs,
    })
}

/// Blocking. All current index conflicts via `Index::conflicts()`, sorted by
/// path ascending (byte-wise). Empty vec when none / when state is Clean.
pub fn list_conflicts(workdir: &Path) -> Result<Vec<ConflictEntry>, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let index = repo.index()?;
    let mut entries: Vec<ConflictEntry> = Vec::new();
    for c in index.conflicts()? {
        if let Some(entry) = conflict_to_entry(&c?) {
            entries.push(entry);
        }
    }
    entries.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    Ok(entries)
}

/// Finds the conflict record for `path` (against the preferred-path rule) or
/// errors with the contract's "has no conflict" message.
fn find_conflict(index: &git2::Index, path: &str) -> Result<ConflictEntry, AppError> {
    for c in index.conflicts()? {
        if let Some(entry) = conflict_to_entry(&c?) {
            if entry.path == path {
                return Ok(entry);
            }
        }
    }
    Err(AppError::Git(format!("path '{path}' has no conflict")))
}

/// Blocking. Marker view of one CURRENTLY CONFLICTED path. Non-conflicted
/// path -> `AppError::Git("path '<p>' has no conflict")`.
pub fn get_conflict(workdir: &Path, path: &str) -> Result<ConflictFile, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let index = repo.index()?;
    let entry = find_conflict(&index, path)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?;
    // Symlink-escape guard (see `ensure_within_workdir`): refuse a conflict path
    // escaping via a symlinked ancestor before the fs read below (external content
    // would otherwise leak into this view). Escape -> invalidName; IO surfaced as-is.
    let file = ensure_within_workdir(wd, path).map_err(|e| match e {
        io @ AppError::Io(_) => io,
        _ => AppError::InvalidName(format!("invalid path: {path}")),
    })?;

    // When text is suppressed (binary/too_large/missing) all three strings stay ""
    // (§1.1): keep the payload bounded and the frontend mode-selection simple.
    let make_suppressed = |binary, too_large, missing| ConflictFile {
        path: entry.path.clone(),
        kind: entry.kind,
        binary,
        too_large,
        missing,
        text: String::new(),
        ours: String::new(),
        theirs: String::new(),
    };

    let meta = match std::fs::symlink_metadata(&file) {
        Ok(m) => m,
        Err(_) => return Ok(make_suppressed(false, false, true)),
    };
    if meta.len() > MAX_CONFLICT_BYTES {
        return Ok(make_suppressed(false, true, false));
    }
    let bytes = std::fs::read(&file)?;
    let probe = &bytes[..bytes.len().min(8000)];
    if probe.contains(&0) {
        return Ok(make_suppressed(true, false, false));
    }

    // Text file under the cap: read each side blob from the index stages.
    let rel = Path::new(path);
    let read_side = |stage: i32| -> Result<String, AppError> {
        match index.get_path(rel, stage) {
            Some(e) => {
                let blob = repo.find_blob(e.id)?;
                Ok(String::from_utf8_lossy(blob.content()).into_owned())
            }
            None => Ok(String::new()),
        }
    };
    let ours = read_side(2)?;
    let theirs = read_side(3)?;

    Ok(ConflictFile {
        path: entry.path.clone(),
        kind: entry.kind,
        binary: false,
        too_large: false,
        missing: false,
        text: String::from_utf8_lossy(&bytes).into_owned(),
        ours,
        theirs,
    })
}

/// Which conflict side to materialize for a write(side) cell.
#[derive(Debug, Clone, Copy)]
enum Side {
    Ours,
    Theirs,
}

/// What the §3.2 matrix says to do for one (kind, resolution) cell.
#[derive(Debug, Clone, Copy)]
enum Action {
    Write(Side),
    Delete,
    /// MarkResolved: add worktree file if present, remove_path otherwise.
    AddOrDelete,
}

/// The full locked resolution matrix (contract §3.2), every cell.
fn matrix_action(kind: ConflictKind, resolution: ConflictResolution) -> Action {
    use ConflictKind as K;
    use ConflictResolution as R;
    match (kind, resolution) {
        (_, R::MarkResolved) => Action::AddOrDelete,
        (K::BothModified | K::BothAdded, R::Ours) => Action::Write(Side::Ours),
        (K::BothModified | K::BothAdded, R::Theirs) => Action::Write(Side::Theirs),
        (K::DeletedByUs, R::Ours) => Action::Delete, // keep our deletion
        (K::DeletedByUs, R::Theirs) => Action::Write(Side::Theirs),
        (K::DeletedByThem, R::Ours) => Action::Write(Side::Ours),
        (K::DeletedByThem, R::Theirs) => Action::Delete, // accept their deletion
        (K::AddedByUs, R::Ours) => Action::Write(Side::Ours),
        (K::AddedByUs, R::Theirs) => Action::Delete, // theirs has no file
        (K::AddedByThem, R::Ours) => Action::Delete,
        (K::AddedByThem, R::Theirs) => Action::Write(Side::Theirs),
        (K::BothDeleted, R::Ours | R::Theirs) => Action::Delete,
    }
}

/// Blocking. Resolves ONE path per the §3.2 matrix, leaving the index entry
/// at stage 0 (or removed) and the worktree consistent with it.
/// Non-conflicted path -> `AppError::Git("path '<p>' has no conflict")`.
///
/// write(side) = read the side's blob from the ODB, write it to the worktree
/// (creating parent dirs; on Windows the mode is recorded in the index by
/// `add_path` per core.filemode — no chmod call), then `index.add_path`
/// (clears all conflict stages, records stage 0) + `index.write()`.
/// delete() = remove the worktree file if present (missing is fine), then
/// `index.remove_path` (also clears conflict stages) + `index.write()`.
pub fn resolve_conflict(
    workdir: &Path,
    path: &str,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    // Same guard as stage/unstage — no absolute/.. escapes. Surfaced as
    // `invalidName` per the P3c contract §6 error list.
    validate_rel_path(path).map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?;
    let repo = open_workdir_repo(workdir)?;
    let mut index = repo.index()?;
    let entry = find_conflict(&index, path)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?;
    // Symlink-escape guard (see `ensure_within_workdir`): refuse a conflict path
    // escaping via a symlinked ancestor before the raw fs mutation below. Escape
    // -> invalidName (as the lexical guard above); IO surfaced as-is.
    let file = ensure_within_workdir(wd, path).map_err(|e| match e {
        io @ AppError::Io(_) => io,
        _ => AppError::InvalidName(format!("invalid path: {path}")),
    })?;
    let rel = Path::new(path);

    let side_entry = |side: Side| -> Result<git2::IndexEntry, AppError> {
        // The matrix only writes sides that exist by construction (§3.2).
        let stage = match side {
            Side::Ours => 2,
            Side::Theirs => 3,
        };
        let found = index.get_path(rel, stage);
        debug_assert!(found.is_some(), "matrix wrote a non-existent side");
        found.ok_or_else(|| {
            AppError::Git(format!("conflict side missing for '{path}'"))
        })
    };

    match matrix_action(entry.kind, resolution) {
        Action::Write(side) => {
            let side_entry = side_entry(side)?;
            let blob = repo.find_blob(side_entry.id)?;
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file, blob.content())?;
            index.add_path(rel)?;
            index.write()?;
        }
        Action::Delete => {
            if file.symlink_metadata().is_ok() {
                std::fs::remove_file(&file)?;
            }
            index.remove_path(rel)?;
            index.write()?;
        }
        Action::AddOrDelete => {
            // MarkResolved (locked): the user is trusted — leftover <<<<<<<
            // markers are NOT rejected, same as `git add`. Missing worktree
            // file means the user resolved by deleting it by hand.
            if file.symlink_metadata().is_ok() {
                index.add_path(rel)?;
            } else {
                index.remove_path(rel)?;
            }
            index.write()?;
        }
    }
    Ok(())
}

/// Blocking. Stages a user-authored resolution for one CURRENTLY CONFLICTED
/// path: writes `content` verbatim to the worktree file (creating parent dirs)
/// then `index.add_path(rel)` (clears all conflict stages → stage 0) +
/// `index.write()`. This is the single primitive behind BOTH editor view modes
/// (unified + side-by-side); per-region accept / combination happen in the
/// frontend before calling this. Same trust model as `MarkResolved` / `git add`:
/// leftover `<<<<<<<` markers are NOT rejected — the frontend gates its Save
/// button on `hasUnresolvedMarkers`, so an unresolved doc never reaches this fn
/// through the UI; a backend rejection would be a redundant second gate with a
/// worse error surface. Trust the caller.
/// Non-conflicted path -> `AppError::Git("path '<p>' has no conflict")`.
pub fn resolve_conflict_text(workdir: &Path, path: &str, content: &str) -> Result<(), AppError> {
    // Same guard as stage/unstage — no absolute/.. escapes. Surfaced as
    // `invalidName`, identical to `resolve_conflict`.
    validate_rel_path(path).map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?;
    let repo = open_workdir_repo(workdir)?;
    let mut index = repo.index()?;
    // Require the path is currently conflicted; the entry itself is unused.
    let _entry = find_conflict(&index, path)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?;
    // Symlink-escape guard (see `ensure_within_workdir`): refuse a conflict path
    // escaping via a symlinked ancestor before the raw fs mutation below. Escape
    // -> invalidName (as the lexical guard above); IO surfaced as-is.
    let file = ensure_within_workdir(wd, path).map_err(|e| match e {
        io @ AppError::Io(_) => io,
        _ => AppError::InvalidName(format!("invalid path: {path}")),
    })?;
    let rel = Path::new(path);

    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&file, content.as_bytes())?;
    index.add_path(rel)?;
    index.write()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------- wire shape (TS mirrors)

    /// The serde casing must match the TS ConflictEntry / ConflictFile /
    /// ConflictKind / ConflictResolution types exactly.
    #[test]
    fn wire_shapes_are_camel_case() {
        let v = serde_json::to_value(ConflictEntry {
            path: "src/auth.ts".to_string(),
            kind: ConflictKind::BothModified,
            has_base: true,
            has_ours: true,
            has_theirs: true,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "path": "src/auth.ts",
                "kind": "bothModified",
                "hasBase": true,
                "hasOurs": true,
                "hasTheirs": true
            })
        );

        let v = serde_json::to_value(ConflictFile {
            path: "README.md".to_string(),
            kind: ConflictKind::DeletedByThem,
            binary: false,
            too_large: false,
            missing: true,
            text: String::new(),
            ours: String::new(),
            theirs: String::new(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "path": "README.md",
                "kind": "deletedByThem",
                "binary": false,
                "tooLarge": false,
                "missing": true,
                "text": "",
                "ours": "",
                "theirs": ""
            })
        );

        // A text-mergeable (bothModified) sample carries non-empty ours/theirs.
        let v = serde_json::to_value(ConflictFile {
            path: "src/auth.ts".to_string(),
            kind: ConflictKind::BothModified,
            binary: false,
            too_large: false,
            missing: false,
            text: "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n".to_string(),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "path": "src/auth.ts",
                "kind": "bothModified",
                "binary": false,
                "tooLarge": false,
                "missing": false,
                "text": "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n",
                "ours": "ours\n",
                "theirs": "theirs\n"
            })
        );

        for (kind, name) in [
            (ConflictKind::BothModified, "bothModified"),
            (ConflictKind::BothAdded, "bothAdded"),
            (ConflictKind::DeletedByUs, "deletedByUs"),
            (ConflictKind::DeletedByThem, "deletedByThem"),
            (ConflictKind::AddedByUs, "addedByUs"),
            (ConflictKind::AddedByThem, "addedByThem"),
            (ConflictKind::BothDeleted, "bothDeleted"),
        ] {
            let v = serde_json::to_value(kind).expect("json");
            assert_eq!(v, serde_json::json!(name));
        }

        for (json, expect) in [
            ("\"ours\"", ConflictResolution::Ours),
            ("\"theirs\"", ConflictResolution::Theirs),
            ("\"markResolved\"", ConflictResolution::MarkResolved),
        ] {
            let r: ConflictResolution = serde_json::from_str(json).expect("deserialize");
            assert_eq!(r, expect);
        }
    }

    // ------------------------------------------- §3.1 kind derivation table

    #[test]
    fn kind_derivation_truth_table() {
        assert_eq!(derive_kind(true, true, true), ConflictKind::BothModified);
        assert_eq!(derive_kind(false, true, true), ConflictKind::BothAdded);
        assert_eq!(derive_kind(true, false, true), ConflictKind::DeletedByUs);
        assert_eq!(derive_kind(true, true, false), ConflictKind::DeletedByThem);
        assert_eq!(derive_kind(false, true, false), ConflictKind::AddedByUs);
        assert_eq!(derive_kind(false, false, true), ConflictKind::AddedByThem);
        assert_eq!(derive_kind(true, false, false), ConflictKind::BothDeleted);
    }

    /// Clean repo: empty conflict list; get/resolve of any path -> "has no
    /// conflict"; escape paths -> invalid path.
    #[test]
    fn clean_repo_has_no_conflicts() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        assert!(list_conflicts(dir.path()).expect("list").is_empty());

        let err = get_conflict(dir.path(), "a.txt").expect_err("no conflict");
        match err {
            AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        let err =
            resolve_conflict(dir.path(), "a.txt", ConflictResolution::Ours).expect_err("none");
        assert!(matches!(err, AppError::Git(_)));

        let err = resolve_conflict(dir.path(), "../escape", ConflictResolution::Ours)
            .expect_err("escape path");
        match err {
            AppError::InvalidName(m) => assert!(m.contains("invalid path"), "got: {m}"),
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    // ------------------------------------- git2 bothModified fixture builders

    /// Commits everything in the worktree (`add_all("*")`) on `HEAD` with the
    /// given parents; returns the new commit oid.
    fn commit_all(
        repo: &git2::Repository,
        msg: &str,
        parents: &[&git2::Commit],
    ) -> git2::Oid {
        let mut index = repo.index().expect("index");
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("add_all");
        index.write().expect("write index");
        let tree_id = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_id).expect("find tree");
        let sig = git2::Signature::now("Test", "test@example.com").expect("sig");
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, parents)
            .expect("commit")
    }

    /// Builds a scratch repo with an in-progress `bothModified` merge conflict on
    /// `a.txt` (ours = "main" line, theirs = "topic" line) plus a non-conflicted
    /// tracked `keep.txt`. Returns the scratch dir (HEAD = default branch, mid-merge).
    fn both_modified_conflict() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");

        // base commit on the default branch
        std::fs::write(dir.path().join("a.txt"), "line1\nbase\nline3\n").expect("write a");
        std::fs::write(dir.path().join("keep.txt"), "keep\n").expect("write keep");
        let base = commit_all(&repo, "base", &[]);
        let base_commit = repo.find_commit(base).expect("base commit");
        let default_branch = repo
            .head()
            .expect("head")
            .shorthand()
            .expect("branch name")
            .to_string();

        // topic branch (theirs): change the middle line
        repo.branch("topic", &base_commit, false).expect("branch topic");
        repo.set_head("refs/heads/topic").expect("set head topic");
        std::fs::write(dir.path().join("a.txt"), "line1\ntopic\nline3\n").expect("write topic");
        commit_all(&repo, "topic change", &[&base_commit]);

        // back to the default branch (ours): change the same middle line differently
        repo.set_head(&format!("refs/heads/{default_branch}"))
            .expect("set head default");
        std::fs::write(dir.path().join("a.txt"), "line1\nmain\nline3\n").expect("write main");
        commit_all(&repo, "main change", &[&base_commit]);

        // merge topic -> produces an index conflict + worktree markers
        let outcome = crate::git::merge::merge_branch(dir.path(), "topic", false).expect("merge");
        assert!(
            matches!(outcome, crate::git::merge::MergeOutcome::Conflicts { .. }),
            "expected conflicts, got {outcome:?}"
        );
        dir
    }

    /// Reads the stage-`n` blob content for `path` from the repo's index.
    fn stage_blob(dir: &Path, path: &str, stage: i32) -> Vec<u8> {
        let repo = git2::Repository::open(dir).expect("open");
        let index = repo.index().expect("index");
        let entry = index
            .get_path(Path::new(path), stage)
            .unwrap_or_else(|| panic!("no stage {stage} entry for {path}"));
        let blob = repo.find_blob(entry.id).expect("blob");
        blob.content().to_vec()
    }

    #[test]
    fn ours_theirs_equal_stage_blobs_and_text_keeps_markers() {
        let dir = both_modified_conflict();
        let view = get_conflict(dir.path(), "a.txt").expect("get_conflict");

        assert_eq!(view.kind, ConflictKind::BothModified);
        assert!(!view.binary && !view.too_large && !view.missing);

        let stage2 = String::from_utf8_lossy(&stage_blob(dir.path(), "a.txt", 2)).into_owned();
        let stage3 = String::from_utf8_lossy(&stage_blob(dir.path(), "a.txt", 3)).into_owned();
        assert_eq!(view.ours, stage2, "ours must equal the stage-2 blob");
        assert_eq!(view.theirs, stage3, "theirs must equal the stage-3 blob");
        assert!(!view.ours.is_empty() && !view.theirs.is_empty());

        assert!(
            view.text.contains("<<<<<<<")
                && view.text.contains("=======")
                && view.text.contains(">>>>>>>"),
            "text must still carry the conflict markers"
        );
    }

    #[test]
    fn too_large_suppresses_all_three_strings() {
        let dir = both_modified_conflict();
        std::fs::write(
            dir.path().join("a.txt"),
            vec![b'a'; MAX_CONFLICT_BYTES as usize + 1],
        )
        .expect("write huge");
        let view = get_conflict(dir.path(), "a.txt").expect("get_conflict");
        assert!(view.too_large && !view.binary && !view.missing);
        assert_eq!(view.text, "");
        assert_eq!(view.ours, "");
        assert_eq!(view.theirs, "");
    }

    #[test]
    fn binary_suppresses_all_three_strings() {
        let dir = both_modified_conflict();
        std::fs::write(dir.path().join("a.txt"), b"\x00\x01binary blob").expect("write binary");
        let view = get_conflict(dir.path(), "a.txt").expect("get_conflict");
        assert!(view.binary && !view.too_large && !view.missing);
        assert_eq!(view.text, "");
        assert_eq!(view.ours, "");
        assert_eq!(view.theirs, "");
    }

    #[test]
    fn resolve_conflict_text_round_trip() {
        let dir = both_modified_conflict();
        let merged = "line1\nmerged by hand\nline3\n";
        resolve_conflict_text(dir.path(), "a.txt", merged).expect("resolve text");

        // no longer conflicted
        assert!(
            !list_conflicts(dir.path())
                .expect("list")
                .iter()
                .any(|e| e.path == "a.txt"),
            "a.txt still conflicted after resolve_conflict_text"
        );

        // index has a stage-0 entry and no conflict stages for a.txt
        let repo = git2::Repository::open(dir.path()).expect("open");
        let index = repo.index().expect("index");
        assert!(
            index.get_path(Path::new("a.txt"), 0).is_some(),
            "expected a stage-0 index entry for a.txt"
        );
        for stage in [1, 2, 3] {
            assert!(
                index.get_path(Path::new("a.txt"), stage).is_none(),
                "unexpected stage-{stage} entry after resolve"
            );
        }

        // worktree bytes equal the resolved content verbatim
        let bytes = std::fs::read(dir.path().join("a.txt")).expect("read a");
        assert_eq!(bytes, merged.as_bytes());
    }

    #[test]
    fn resolve_conflict_text_non_conflicted_path_errors() {
        let dir = both_modified_conflict();
        let err = resolve_conflict_text(dir.path(), "keep.txt", "x").expect_err("no conflict");
        match err {
            AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn resolve_conflict_text_escape_path_errors() {
        let dir = both_modified_conflict();
        for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
            let err = resolve_conflict_text(dir.path(), bad, "x").expect_err("escape");
            assert!(
                matches!(err, AppError::InvalidName(_)),
                "path {bad:?}: expected InvalidName, got {err:?}"
            );
        }
    }

    #[test]
    fn resolve_conflict_text_accepts_leftover_markers() {
        let dir = both_modified_conflict();
        // Trust model: leftover <<<<<<< markers are NOT rejected (same as git add).
        let content = "<<<<<<< HEAD\nline1\nmain\n=======\ntopic\n>>>>>>> topic\n";
        resolve_conflict_text(dir.path(), "a.txt", content).expect("accept markers");

        let repo = git2::Repository::open(dir.path()).expect("open");
        let index = repo.index().expect("index");
        assert!(
            index.get_path(Path::new("a.txt"), 0).is_some(),
            "leftover-marker content must still stage at stage 0"
        );
        assert!(index.get_path(Path::new("a.txt"), 2).is_none());
        let bytes = std::fs::read(dir.path().join("a.txt")).expect("read a");
        assert_eq!(bytes, content.as_bytes());
    }
}
