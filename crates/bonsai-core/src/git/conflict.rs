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
mod tests;
