//! Post-apply verification of the carried UNTRACKED blobs.
//!
//! `git_stash_apply`'s untracked-restore phase is a separate checkout that can
//! silently fail to write a blob back — the target ref already tracks that
//! path (text: markers are written instead; binary: the branch's bytes simply
//! win), or a directory now occupies it — and it records NO conflict entry in
//! the index. The historical rule "Ok(()) && !index.has_conflicts() => drop the
//! stash" therefore destroyed the only remaining copy of a brand-new file.
//!
//! So before any drop we re-read the stash's untracked tree (`stash@{N}^3`) and
//! compare every leaf blob against the worktree. Anything that is missing, is
//! no longer a file, or differs byte-for-byte counts as UNRESTORED: the caller
//! keeps the stash and reports the paths.

use std::path::Path;

use crate::error::AppError;

/// Leaf paths of the stash's untracked tree whose worktree content does NOT
/// match what was stashed. Empty when the stash carried no untracked files
/// (`parent_count() < 3`) or when every blob landed byte-identically.
///
/// Comparison is by blob oid via `blob_path` (libgit2 `create_fromdisk`), which
/// runs the CHECK-IN filter chain — a raw byte compare would flag every file as
/// differing under `core.autocrlf=true`. The blobs it writes are unreferenced
/// and collected by the next `gc`.
///
/// `skip` (the Windows-reserved allowlist) names paths the caller deliberately
/// did not restore; they are never reported as a surprise.
pub(crate) fn unrestored_untracked(
    repo: &git2::Repository,
    workdir: &Path,
    index: usize,
    skip: &[String],
) -> Result<Vec<String>, AppError> {
    let oid = super::apply::stash_commit_oid(repo, index)?;
    let commit = repo.find_commit(oid)?;
    if commit.parent_count() < 3 {
        return Ok(Vec::new()); // created without INCLUDE_UNTRACKED
    }
    let untracked_tree = commit.parent(2)?.tree()?;

    let mut out: Vec<String> = Vec::new();
    // Diff empty -> tree: the deltas are exactly the leaf blob paths (same
    // technique as `stash_path_sets`, and `path_bytes()` survives non-UTF-8
    // names that `path()` would panic on under Windows).
    let diff = repo.diff_tree_to_tree(None, Some(&untracked_tree), None)?;
    for delta in diff.deltas() {
        let Some(bytes) = delta.new_file().path_bytes() else {
            continue;
        };
        let Ok(path) = std::str::from_utf8(bytes) else {
            // Unrepresentable here, but a non-UTF-8 carried path is exactly the
            // kind of thing we must not silently drop the stash over.
            out.push(String::from_utf8_lossy(bytes).into_owned());
            continue;
        };
        if skip.iter().any(|s| s == path) {
            continue;
        }
        let expected = delta.new_file().id();
        let abs = workdir.join(path);
        // A path that is gone, or is no longer a regular file (a directory from
        // the target ref took its place), was not restored.
        if !abs.is_file() {
            out.push(path.to_string());
            continue;
        }
        match repo.blob_path(&abs) {
            Ok(actual) if actual == expected => {}
            // Different content (markers, the branch's version) or unreadable:
            // treat as unrestored — the stash is the safety net either way.
            _ => out.push(path.to_string()),
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}
