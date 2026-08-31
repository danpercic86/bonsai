//! Making a STASH commit diff like the user expects.
//!
//! A stash created with `INCLUDE_UNTRACKED` is three commits, not one: `w`
//! (the worktree state) with parents `[HEAD, index, untracked]`. The tracked
//! changes live in `w`'s own tree, but every brand-new file lives ONLY in the
//! third parent's tree. So the ordinary commit-vs-first-parent diff — which is
//! what the stash row in the graph renders — shows modified files and silently
//! omits every added (A) one, even though the stash really does contain them.
//!
//! This module overlays the untracked tree onto `w`'s tree so the diff reports
//! those files as Adds. The overlay tree is written to the object database
//! (trees only, unreferenced, collected by the next `gc`) because libgit2 can
//! only diff persisted trees.

use crate::error::AppError;

/// The stash's untracked tree overlaid on `commit`'s own tree, or `None` when
/// `commit` is not a stash commit carrying untracked files.
///
/// A stash `w` commit is identified by BOTH its shape (exactly 3 parents) and
/// its presence in the `refs/stash` reflog — the shape alone would also match
/// a 3-way octopus merge, whose third parent is a real ancestor and must not be
/// overlaid.
pub(crate) fn stash_untracked_overlay<'r>(
    repo: &'r git2::Repository,
    commit: &git2::Commit<'_>,
) -> Result<Option<git2::Tree<'r>>, AppError> {
    if commit.parent_count() != 3 || !is_stash_commit(repo, commit.id()) {
        return Ok(None);
    }
    let untracked_tree = commit.parent(2)?.tree()?;

    // In-memory index seeded with the stash's own (tracked) tree, then every
    // untracked leaf blob added on top. Diffing empty -> tree yields exactly
    // the leaf blobs, with their oid and filemode already resolved.
    let mut combined = git2::Index::new()?;
    combined.read_tree(&commit.tree()?)?;
    let diff = repo.diff_tree_to_tree(None, Some(&untracked_tree), None)?;
    for delta in diff.deltas() {
        let file = delta.new_file();
        let Some(bytes) = file.path_bytes() else {
            continue;
        };
        combined.add(&git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: u32::from(file.mode()),
            uid: 0,
            gid: 0,
            file_size: 0,
            id: file.id(),
            flags: 0,
            flags_extended: 0,
            path: bytes.to_vec(),
        })?;
    }
    drop(diff);

    let oid = combined.write_tree_to(repo)?;
    Ok(Some(repo.find_tree(oid)?))
}

/// True iff `oid` is one of the stash stack's `w` commits. The stack IS the
/// `refs/stash` reflog (the same source `list_stashes` reads). No reflog (no
/// stash was ever created) -> false.
fn is_stash_commit(repo: &git2::Repository, oid: git2::Oid) -> bool {
    let Ok(reflog) = repo.reflog("refs/stash") else {
        return false;
    };
    reflog.iter().any(|entry| entry.id_new() == oid)
}
