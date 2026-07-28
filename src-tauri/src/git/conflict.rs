//! Index-conflict listing, read-only marker view, and file-level resolution.
//! Operation-agnostic: works identically during merge (P3c) and rebase (P3d).
//! Pure git2, no Tauri types (P3c contract §3).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{open_workdir_repo, validate_rel_path};

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
    let entry = find_conflict(&repo.index()?, path)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?;
    let file = wd.join(path);

    let make = |binary, too_large, missing, text| ConflictFile {
        path: entry.path.clone(),
        kind: entry.kind,
        binary,
        too_large,
        missing,
        text,
    };

    let meta = match std::fs::symlink_metadata(&file) {
        Ok(m) => m,
        Err(_) => return Ok(make(false, false, true, String::new())),
    };
    if meta.len() > MAX_CONFLICT_BYTES {
        return Ok(make(false, true, false, String::new()));
    }
    let bytes = std::fs::read(&file)?;
    let probe = &bytes[..bytes.len().min(8000)];
    if probe.contains(&0) {
        return Ok(make(true, false, false, String::new()));
    }
    Ok(make(
        false,
        false,
        false,
        String::from_utf8_lossy(&bytes).into_owned(),
    ))
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
    let file = wd.join(path);
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
                "text": ""
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
}
