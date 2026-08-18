//! Stage / unstage core (M3 contract §2.1–§2.3).
//!
//! Pure git2 logic, no Tauri types. All-or-nothing batch semantics: validate
//! every path first, apply all index operations in memory, then a single
//! `index.write()` (stage) / single libgit2 reset (unstage). Any error before
//! the write aborts the whole call with no index change.

use std::path::Path;

use crate::error::AppError;

/// Opens the repo at `workdir` (`NO_SEARCH`, like `read_status`) and rejects
/// bare repositories.
pub(crate) fn open_workdir_repo(workdir: &Path) -> Result<git2::Repository, AppError> {
    let repo = git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?;
    if repo.is_bare() {
        return Err(AppError::Git(
            "cannot modify index: repository is bare".to_string(),
        ));
    }
    Ok(repo)
}

/// Validates a wire path (worktree-relative, forward slashes). Rejects empty
/// strings, absolute paths (leading `/` or a drive letter `X:`), any path
/// containing `\` (the wire format is forward-slash only — backslashes would
/// hit libgit2's opaque error path), and any `..` component (M3 contract
/// §2.1). Shared by staging (M3) and per-file diffs (M4).
pub(crate) fn validate_rel_path(p: &str) -> Result<(), AppError> {
    let bytes = p.as_bytes();
    let absolute = p.starts_with('/')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':');
    let escapes = p.split('/').any(|component| component == "..");
    if p.is_empty() || absolute || p.contains('\\') || escapes {
        return Err(AppError::Other(format!("invalid path: {p}")));
    }
    Ok(())
}

/// Blocking. Stages each path into the index (`git add` / `git rm --cached`
/// semantics combined):
/// - path exists in the worktree (`symlink_metadata().is_ok()`, so symlinks
///   count) -> `index.add_path` (covers untracked, modified, typechange,
///   rename NEW side);
/// - path missing from the worktree -> `index.remove_path` (covers deleted,
///   rename OLD side).
///
/// Then `index.write()` once. Note: `add_path` has `git add -f` semantics
/// (adds even ignored files); acceptable — the UI only offers paths already
/// present in `StatusSnapshot`. An empty `paths` vec is a no-op `Ok(())`.
pub fn stage_paths(workdir: &Path, paths: &[String]) -> Result<(), AppError> {
    if paths.is_empty() {
        return Ok(());
    }
    for p in paths {
        validate_rel_path(p)?;
    }

    let repo = open_workdir_repo(workdir)?;
    let wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no workdir".to_string()))?
        .to_path_buf();

    let mut index = repo.index()?;
    for p in paths {
        let rel = Path::new(p);
        if wd.join(rel).symlink_metadata().is_ok() {
            index.add_path(rel)?;
        } else {
            index.remove_path(rel)?;
        }
    }
    index.write()?;
    Ok(())
}

/// Blocking. Unstages each path (index entry reset to HEAD's version, the
/// worktree is never touched):
/// - HEAD resolvable -> `repo.reset_default(head_commit, paths)` (libgit2
///   `git_reset_default` == `git restore --staged -- <paths>`);
/// - HEAD unborn -> `index.remove_path` per path + one `index.write()`
///   (removing from the index == unstaging when there is no HEAD to restore
///   from).
///
/// Unborn detection: `repo.head()` error with code `UnbornBranch` or
/// `NotFound`. An empty `paths` vec is a no-op `Ok(())`.
pub fn unstage_paths(workdir: &Path, paths: &[String]) -> Result<(), AppError> {
    if paths.is_empty() {
        return Ok(());
    }
    for p in paths {
        validate_rel_path(p)?;
    }

    let repo = open_workdir_repo(workdir)?;
    match repo.head() {
        Ok(head) => {
            let commit = head.peel_to_commit()?;
            repo.reset_default(Some(commit.as_object()), paths.iter().map(String::as_str))?;
        }
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            let mut index = repo.index()?;
            for p in paths {
                index.remove_path(Path::new(p))?;
            }
            index.write()?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Data-loss guard shared by the interactive-rebase and bisect engines. Both
/// bring the worktree to a target tree with a `.force()` checkout (needed to
/// update tracked files across divergent trees), but a forced checkout silently
/// OVERWRITES an untracked working-tree file whose path collides with a file in
/// the target tree. This refuses that case, restoring git's own "would be
/// overwritten by checkout" protection. Call it immediately BEFORE the force
/// checkout.
///
/// Only UNTRACKED, non-ignored files are guarded: tracked files are the force
/// checkout's job to update, and git itself overwrites IGNORED collisions, so
/// those are intentionally left to `.force()`. Runtime-free (git2 + std only).
pub(crate) fn ensure_no_untracked_collision(
    repo: &git2::Repository,
    target_tree: &git2::Tree,
) -> Result<(), AppError> {
    let mut sopts = git2::StatusOptions::new();
    sopts
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    // On a case-insensitive filesystem the worktree path `README.md` and the
    // tree path `readme.md` are the same physical file, so an exact-case tree
    // lookup would MISS the collision and let `.force()` clobber the untracked
    // file (finding F-A3-6). Key off git's own `core.ignorecase` (shared helper
    // so every case-folding decision in the app uses one rule).
    let ignorecase = super::repo::repo_ignorecase(repo);

    let statuses = repo.statuses(Some(&mut sopts))?;
    for entry in statuses.iter() {
        if !entry.status().contains(git2::Status::WT_NEW) {
            continue;
        }
        // Untracked rows are on the index→workdir side; fall back to the raw
        // entry path. git stores tree paths with '/' separators, which
        // `Tree::get_path` matches directly.
        let path = entry
            .index_to_workdir()
            .and_then(|d| d.new_file().path().map(|p| p.to_string_lossy().into_owned()))
            .unwrap_or_else(|| String::from_utf8_lossy(entry.path_bytes()).into_owned());
        if target_path_collides(repo, target_tree, &path, ignorecase) {
            return Err(AppError::Git(format!(
                "untracked working-tree file '{path}' would be overwritten by checkout; \
                 remove or stash it first"
            )));
        }
    }
    Ok(())
}

/// True iff checking out `target_tree` would overwrite/delete the untracked
/// worktree file at `path`. Two ways this happens:
/// 1. **Direct** — `path` exists in the target tree (blob, or a tree that
///    replaces the untracked file with a directory).
/// 2. **Type-swap** — an ANCESTOR prefix of `path` (e.g. `foo` for
///    `foo/bar.txt`) is a BLOB in the target tree. The checkout replaces that
///    directory with a file, deleting everything under it, including this
///    untracked file. An exact `get_path(path)` alone misses this: traversing
///    a blob as an interior component returns Err, a false negative.
///
/// When `ignorecase` is true (case-insensitive FS per `core.ignorecase`) the
/// name comparison is ASCII case-folded at every path component — git itself
/// uses simple ASCII case-folding, not full Unicode — so `README.md` in the
/// worktree collides with `readme.md` in the tree. When false, the original
/// exact-case behavior is preserved.
fn target_path_collides(
    repo: &git2::Repository,
    target_tree: &git2::Tree,
    path: &str,
    ignorecase: bool,
) -> bool {
    if !ignorecase {
        if target_tree.get_path(Path::new(path)).is_ok() {
            return true;
        }
        let components: Vec<&str> = path.split('/').collect();
        let mut prefix = String::new();
        // Proper ancestor prefixes only (exclude the full path, handled above).
        for comp in &components[..components.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(comp);
            if let Ok(entry) = target_tree.get_path(Path::new(&prefix)) {
                if entry.kind() == Some(git2::ObjectType::Blob) {
                    return true;
                }
            }
        }
        return false;
    }
    let components: Vec<&str> = path.split('/').collect();
    ci_path_collides(repo, target_tree, &components)
}

/// Case-insensitive (ASCII case-fold) walk of `components` through `tree`,
/// implementing the same Direct + Type-swap collision logic as the exact-case
/// branch. Descends one tree level per component; a BLOB found at an interior
/// component is a type-swap collision, and any entry matching the final
/// component is a direct collision.
fn ci_path_collides(repo: &git2::Repository, tree: &git2::Tree, components: &[&str]) -> bool {
    let (first, rest) = match components.split_first() {
        Some(split) => split,
        None => return false,
    };
    for entry in tree.iter() {
        if !entry.name_bytes().eq_ignore_ascii_case(first.as_bytes()) {
            continue;
        }
        if rest.is_empty() {
            // Final component present (blob, tree, or submodule) -> collision.
            return true;
        }
        return match entry.kind() {
            Some(git2::ObjectType::Blob) => true, // type-swap: dir replaced by file
            Some(git2::ObjectType::Tree) => match repo.find_tree(entry.id()) {
                Ok(subtree) => ci_path_collides(repo, &subtree, rest),
                Err(_) => false,
            },
            _ => false, // submodule/other interior component: nothing under it
        };
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_validation_rejects_bad_paths() {
        for bad in [
            "",
            "/abs/path",
            "\\abs\\path",
            "C:/abs/path",
            "c:relative",
            "..",
            "../escape",
            "a/../escape",
            "a/..",
            "..\\escape",
        ] {
            let err = validate_rel_path(bad).expect_err(&format!("must reject {bad:?}"));
            match err {
                AppError::Other(m) => assert!(m.contains("invalid path"), "got: {m}"),
                other => panic!("expected AppError::Other, got: {other:?}"),
            }
        }
    }

    #[test]
    fn path_validation_rejects_interior_backslashes() {
        for bad in ["dir\\file.txt", "a\\b\\c.rs", "trailing\\", "mid\\..end"] {
            let err = validate_rel_path(bad).expect_err(&format!("must reject {bad:?}"));
            match err {
                AppError::Other(m) => assert!(m.contains("invalid path"), "got: {m}"),
                other => panic!("expected AppError::Other, got: {other:?}"),
            }
        }
    }

    #[test]
    fn path_validation_accepts_normal_paths() {
        for good in [
            "file.txt",
            "dir/file.txt",
            "deeply/nested/dir/file.rs",
            "..dots/file..txt",
            "with space.txt",
            "über-café.txt",
        ] {
            validate_rel_path(good).unwrap_or_else(|e| panic!("must accept {good:?}: {e:?}"));
        }
    }

    #[test]
    fn empty_paths_vec_is_a_noop() {
        // No repo needed: the empty-vec early return fires before any repo open.
        let dir = crate::testutil::scratch_dir();
        let missing = dir.path().join("not-a-repo");
        assert!(stage_paths(&missing, &[]).is_ok());
        assert!(unstage_paths(&missing, &[]).is_ok());
    }

    #[test]
    fn bare_repo_is_an_error() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init_bare(dir.path()).expect("init bare repo");
        let paths = vec!["file.txt".to_string()];
        for result in [
            stage_paths(dir.path(), &paths),
            unstage_paths(dir.path(), &paths),
        ] {
            match result.expect_err("bare repo must be an error") {
                AppError::Git(m) => assert!(m.contains("bare"), "got: {m}"),
                other => panic!("expected AppError::Git, got: {other:?}"),
            }
        }
    }

    /// Builds a repo whose `core.ignorecase` is forced to `ignorecase` (so the
    /// test is deterministic regardless of the host filesystem), a one-entry
    /// in-memory target tree containing `tree_entry` (a blob), and an untracked
    /// worktree file `worktree_file`. Returns the guard result of checking out
    /// that tree. The tree entry is built in memory (not written to the
    /// worktree) so the two file names never collide physically on a real
    /// case-insensitive host.
    fn collision_guard_result(
        ignorecase: bool,
        tree_entry: &str,
        worktree_file: &str,
    ) -> Result<(), AppError> {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        repo.config()
            .expect("config")
            .set_bool("core.ignorecase", ignorecase)
            .expect("set ignorecase");

        let blob = repo.blob(b"tree side").expect("blob");
        let mut tb = repo.treebuilder(None).expect("treebuilder");
        tb.insert(tree_entry, blob, 0o100644).expect("insert");
        let tree_oid = tb.write().expect("write tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");

        std::fs::write(dir.path().join(worktree_file), b"worktree side")
            .expect("write untracked file");

        // `dir`/`repo` stay in scope through the guard call and drop naturally
        // at end of function (repo holds the worktree path).
        ensure_no_untracked_collision(&repo, &tree)
    }

    /// F-A3-6 (T2.3): on a case-insensitive FS an untracked `README.md` must
    /// collide with the tree entry `readme.md`, so the force-checkout refuses.
    #[test]
    fn ignorecase_untracked_case_variant_is_detected() {
        let err = collision_guard_result(true, "readme.md", "README.md")
            .expect_err("case-insensitive collision must be refused");
        match err {
            AppError::Git(m) => assert!(m.contains("would be overwritten"), "got: {m}"),
            other => panic!("expected AppError::Git, got: {other:?}"),
        }
    }

    /// F-A3-6 (T2.3): with `core.ignorecase=false` the two names are distinct
    /// paths, so the guard must NOT flag a collision (no regression of the
    /// case-sensitive path).
    #[test]
    fn case_sensitive_case_variant_is_not_a_collision() {
        collision_guard_result(false, "readme.md", "README.md")
            .expect("case-sensitive: distinct names must not collide");
    }

    /// Regression: an exact-case match is still a collision under both modes.
    #[test]
    fn exact_case_match_is_a_collision_both_modes() {
        for ignorecase in [true, false] {
            let err = collision_guard_result(ignorecase, "readme.md", "readme.md")
                .expect_err("exact-case collision must be refused");
            assert!(matches!(err, AppError::Git(_)), "ignorecase={ignorecase}");
        }
    }
}
