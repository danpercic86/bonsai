//! Discard core (P20 contract §4).
//!
//! Pure git2 logic, no Tauri types. Restores tracked worktree files to their
//! INDEX version (`git checkout -- <paths>` / `git restore --worktree`),
//! discarding unstaged edits and recreating unstaged deletions; staged content
//! is untouched. Destructive — the UI confirms first. The per-file `discard_paths`
//! is tracked-only (errors defensively on an untracked path); the bulk
//! `discard_paths_force` (P36) additionally DELETES untracked/new files from disk,
//! so a folder/section "Discard all" can fully clean the working tree.

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{ensure_within_workdir, open_workdir_repo, validate_rel_path};

/// Blocking. Restores each tracked path's WORKTREE content to the INDEX version
/// (`git checkout -- <paths>`), discarding unstaged edits and recreating
/// unstaged deletions. Staged content is untouched. All-or-nothing validation
/// (like `stage_paths`): validate every path first. An empty `paths` vec is a
/// no-op `Ok(())` (P20 contract §4.1).
pub fn discard_paths(workdir: &Path, paths: &[String]) -> Result<(), AppError> {
    if paths.is_empty() {
        return Ok(());
    }
    for p in paths {
        validate_rel_path(p)?;
    }

    let repo = open_workdir_repo(workdir)?;
    let index = repo.index()?;

    // Defensive tracked-only guard (OPEN #3): the UI never offers Discard on
    // untracked rows, so this is belt-and-suspenders.
    for p in paths {
        if index.get_path(Path::new(p), 0).is_none() {
            return Err(AppError::Git(format!(
                "cannot discard '{p}': not a tracked file"
            )));
        }
    }

    // Force-checkout exactly those paths from the current index.
    //
    // CRITICAL (same lesson as `abort_merge`): a CheckoutBuilder with ZERO
    // .path() calls matches ALL paths. The `paths.is_empty()` early return
    // above guarantees at least one .path() is set here, so a whole-worktree
    // clobber is impossible.
    let mut cb = git2::build::CheckoutBuilder::new();
    cb.force().remove_untracked(false);
    for p in paths {
        cb.path(p.as_str());
    }
    // None target == the repo's current index.
    repo.checkout_index(None, Some(&mut cb))?;
    Ok(())
}

/// Blocking. Force-discard a mixed set of paths:
///   - TRACKED paths (present in the index) are restored to their INDEX content
///     via `checkout_index` + per-path `CheckoutBuilder` (identical mechanism to
///     `discard_paths`) — reverts unstaged edits, recreates unstaged deletions.
///     Staged content is untouched.
///   - UNTRACKED paths (absent from the index) are DELETED from disk
///     (`std::fs::remove_file`).
///
/// All-or-nothing validation up-front (like `discard_paths`): every path is
/// `validate_rel_path`-checked before any mutation. Empty `paths` is a no-op
/// `Ok(())` — `checkout_index` is NEVER reached with a zero-`.path()`
/// (match-all) pathspec. Destructive — the UI confirms first.
pub fn discard_paths_force(workdir: &Path, paths: &[String]) -> Result<(), AppError> {
    // 1. Empty guard FIRST (same match-all-clobber guarantee as `discard_paths`).
    if paths.is_empty() {
        return Ok(());
    }
    // 2. Validate all paths before touching the repo or filesystem.
    for p in paths {
        validate_rel_path(p)?;
    }

    let repo = open_workdir_repo(workdir)?;
    let index = repo.index()?;

    // 3. Partition by index membership.
    let mut tracked: Vec<&String> = Vec::new();
    let mut untracked: Vec<&String> = Vec::new();
    for p in paths {
        if index.get_path(Path::new(p), 0).is_some() {
            tracked.push(p);
        } else {
            untracked.push(p);
        }
    }

    // 4. Validate every untracked entry is a regular file BEFORE deleting any of
    //    them. A directory (or other non-file) would make `remove_file` in 4a
    //    fail AFTER earlier siblings were already deleted — a partial, non-atomic
    //    result that also skips the tracked-restore branch. Tolerate a not-found
    //    entry (that IS the desired end state); every other IO error rejects the
    //    whole batch before the first deletion, preserving all-or-nothing.
    for p in &untracked {
        // Symlink-escape guard (defense in depth on top of the lexical
        // `validate_rel_path` above): refuse an untracked path that would resolve
        // OUTSIDE the repository through a symlinked ANCESTOR directory before any
        // metadata read or `remove_file` touches it. It runs inside this up-front
        // validation loop — before loop 4a deletes anything — so one escaping
        // entry aborts the whole batch with nothing deleted (all-or-nothing). A
        // genuine IO failure is surfaced as-is; the escape is the discard-flavored
        // `Git` error, matching the sibling "not a regular file" message.
        ensure_within_workdir(workdir, p.as_str()).map_err(|e| match e {
            io @ AppError::Io(_) => io,
            _ => AppError::Git(format!(
                "cannot discard '{p}': path resolves outside the repository"
            )),
        })?;
        match std::fs::symlink_metadata(workdir.join(p)) {
            Ok(md) if !md.is_file() => {
                return Err(AppError::Git(format!(
                    "cannot discard '{p}': not a regular file"
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::Io(e.to_string())),
        }
    }

    // 4a. Delete untracked files first. Tolerate already-gone (NotFound); a
    //     missing untracked file is the desired end state.
    for p in &untracked {
        if let Err(e) = std::fs::remove_file(workdir.join(p)) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(AppError::Io(e.to_string()));
            }
        }
    }

    // 4b. Restore tracked files. The `!tracked.is_empty()` guard preserves the
    //     "at least one .path()" invariant so this branch can never match-all-clobber.
    if !tracked.is_empty() {
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force().remove_untracked(false);
        for p in &tracked {
            cb.path(p.as_str());
        }
        repo.checkout_index(None, Some(&mut cb))?;
    }
    Ok(())
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

    /// An empty paths vec is a no-op Ok (never clobbers the whole worktree).
    #[test]
    fn discard_empty_is_noop() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

        discard_paths(d, &[]).expect("empty discard is Ok");
        // The unstaged edit is UNTOUCHED (proves no whole-worktree clobber).
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt")).expect("read"),
            "edited\n"
        );
    }

    /// Discarding an untracked path errors defensively.
    #[test]
    fn discard_untracked_errors() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("untracked.txt"), "new\n").expect("write untracked");

        let err = discard_paths(d, &["untracked.txt".to_string()]).expect_err("untracked");
        match err {
            AppError::Git(m) => assert!(m.contains("not a tracked file"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    /// An invalid (escaping) path is rejected before any repo access.
    #[test]
    fn discard_invalid_path_errors() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        let err = discard_paths(d, &["../evil".to_string()]).expect_err("escape");
        assert!(matches!(err, AppError::Other(_)), "got: {err:?}");
    }

    // -------------------------------------- P36 §2.1: discard_paths_force

    /// A modified TRACKED file is reverted to its index content.
    #[test]
    fn force_modified_tracked_reverts_to_index() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

        discard_paths_force(d, &["a.txt".to_string()]).expect("force discard");
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt")).expect("read"),
            "base\n",
            "tracked file restored to index content"
        );
    }

    /// An UNTRACKED file is deleted from disk.
    #[test]
    fn force_untracked_file_deleted() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("untracked.txt"), "new\n").expect("write untracked");

        discard_paths_force(d, &["untracked.txt".to_string()]).expect("force discard");
        assert!(
            !d.join("untracked.txt").exists(),
            "untracked file deleted from disk"
        );
    }

    /// A mixed set (one modified tracked + one untracked) handled in one call.
    #[test]
    fn force_mixed_set() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit tracked");
        std::fs::write(d.join("new.txt"), "new\n").expect("write untracked");

        discard_paths_force(d, &["a.txt".to_string(), "new.txt".to_string()])
            .expect("force discard");
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt")).expect("read"),
            "base\n",
            "tracked file reverted"
        );
        assert!(!d.join("new.txt").exists(), "untracked file deleted");
    }

    /// Empty `paths` is a no-op `Ok(())` and does NOT clobber the whole worktree —
    /// a pre-existing unstaged edit survives (mirror of `discard_empty_is_noop`).
    #[test]
    fn force_empty_is_noop() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

        discard_paths_force(d, &[]).expect("empty force discard is Ok");
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt")).expect("read"),
            "edited\n",
            "empty batch must not clobber the worktree"
        );
    }

    /// An already-gone untracked path in the list is tolerated (`Ok`); other
    /// listed paths are still processed.
    #[test]
    fn force_already_gone_untracked_tolerated() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("real.txt"), "new\n").expect("write untracked");

        // "ghost.txt" is absent from index AND disk → NotFound is tolerated.
        discard_paths_force(d, &["ghost.txt".to_string(), "real.txt".to_string()])
            .expect("missing untracked tolerated");
        assert!(!d.join("ghost.txt").exists(), "ghost still absent");
        assert!(!d.join("real.txt").exists(), "real.txt still deleted");
    }

    /// A directory in the untracked set is rejected up-front, and a sibling
    /// untracked FILE also in the batch is NOT deleted — the all-or-nothing
    /// guarantee holds even though the directory sorts after the file.
    #[test]
    fn force_untracked_directory_rejected_atomically() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);

        // One untracked regular file + one untracked directory, both in the batch.
        std::fs::write(d.join("keep.txt"), "new\n").expect("write file");
        std::fs::create_dir(d.join("subdir")).expect("mkdir");
        std::fs::write(d.join("subdir").join("inner.txt"), "x\n").expect("write inner");

        let err = discard_paths_force(d, &["keep.txt".to_string(), "subdir".to_string()])
            .expect_err("directory must be rejected");
        match err {
            AppError::Git(m) => assert!(m.contains("not a regular file"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
        // The sibling untracked file survived — nothing was deleted before the
        // up-front validation rejected the batch.
        assert!(
            d.join("keep.txt").exists(),
            "sibling untracked file must NOT be deleted (atomic)"
        );
        assert!(
            d.join("subdir").join("inner.txt").exists(),
            "the directory and its contents are left untouched"
        );
    }

    /// An invalid/escaping path rejects the whole batch before any mutation.
    #[test]
    fn force_invalid_path_rejected() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "base", &[("a.txt", "base\n")]);
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

        let err = discard_paths_force(d, &["../evil".to_string(), "a.txt".to_string()])
            .expect_err("escape rejected");
        assert!(matches!(err, AppError::Other(_)), "got: {err:?}");
        // Nothing mutated: the dirty tracked file is untouched.
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt")).expect("read"),
            "edited\n",
            "invalid batch must not mutate anything"
        );
    }
}
