//! Discard core (P20 contract §4).
//!
//! Pure git2 logic, no Tauri types. Restores tracked worktree files to their
//! INDEX version (`git checkout -- <paths>` / `git restore --worktree`),
//! discarding unstaged edits and recreating unstaged deletions; staged content
//! is untouched. Destructive — the UI confirms first. Untracked files are out
//! of scope (the backend errors defensively on an untracked path).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{open_workdir_repo, validate_rel_path};

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
        crate::git::commit::create_commit(dir, msg).expect("commit");
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
}
