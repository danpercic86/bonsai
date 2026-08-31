//! P73 increment 2 — best-effort rollback of a FAILED FRESH-CLONE
//! `update_submodule` (contract `docs/contracts/P73-submodule-reconnect.md` §4-§5),
//! split from `submodule_reconnect.rs` to keep both files under the ~500-line
//! limit (CLAUDE.md). Modelled on `rollback_partial_add`: each step
//! independent, its own failure ignored, the caller returns the ORIGINAL error.

use std::path::Path;

use crate::git::stage::validate_rel_path;
use crate::git::submodule::{remove_cached_git_dir, validate_modules_name};
use crate::git::submodule_reconnect::{module_gitdir, workdir_is_empty};

/// What existed on disk BEFORE a fresh-clone `update_submodule` attempt, so a
/// failure can be rolled back to exactly that state. Cheap: three stat/config
/// probes. Never errors — unknowable ⇒ the conservative value (`true` =
/// "existed", i.e. do not delete).
pub(super) struct PreUpdateState {
    /// The submodule was NOT checked out before the attempt (`WD_UNINITIALIZED`).
    /// Rollback is a FRESH-CLONE-only concept: for an already-checked-out
    /// submodule a failed `update` (e.g. the SAFE checkout refusing to clobber a
    /// dirty file) must leave the worktree completely alone. Unknowable ⇒ `false`
    /// (⇒ no rollback at all), the conservative value.
    uninitialized: bool,
    /// `<commondir>/modules/<key>` already existed (⇒ never delete it).
    module_dir_existed: bool,
    /// `<super_workdir>/<path>` already existed (⇒ never remove the dir itself).
    workdir_existed: bool,
    /// `<super_workdir>/<path>` was absent or held no file/symlink/`.git` at any
    /// depth. `WD_UNINITIALIZED` alone does NOT imply this — it only means
    /// `<path>/.git` is absent, so a registered-but-never-cloned submodule whose
    /// folder holds the user's own untracked files is `uninitialized == true`.
    /// Deleting that content would be data loss, so the WHOLE rollback stands
    /// down unless this holds. Unknowable ⇒ `false` (⇒ never delete).
    workdir_was_empty: bool,
    /// `submodule.<name>.url` was already present in the LOCAL config
    /// (⇒ never clear the registration).
    registered: bool,
}

/// Snapshot the pre-update disk state (see [`PreUpdateState`]).
pub(super) fn snapshot_pre_update(
    repo: &git2::Repository,
    name: &str,
    path: &str,
) -> PreUpdateState {
    PreUpdateState {
        uninitialized: repo
            .submodule_status(name, git2::SubmoduleIgnore::None)
            .map(|f| f.contains(git2::SubmoduleStatus::WD_UNINITIALIZED))
            .unwrap_or(false),
        module_dir_existed: module_gitdir(repo, name, path).ok().flatten().is_some(),
        workdir_existed: repo
            .workdir()
            .map(|wd| wd.join(path).exists())
            .unwrap_or(true),
        workdir_was_empty: repo
            .workdir()
            .map(|wd| workdir_is_empty(&wd.join(path)))
            .unwrap_or(false),
        registered: repo
            .config()
            .and_then(|c| c.open_level(git2::ConfigLevel::Local))
            .ok()
            .and_then(|c| c.get_string(&format!("submodule.{name}.url")).ok())
            .is_some(),
    }
}

/// `remove_dir_all` when `symlink_metadata` says dir, else `remove_file` (never
/// follows symlinks).
fn remove_dir_all_or_file(p: &Path) {
    match std::fs::symlink_metadata(p) {
        Ok(md) if md.is_dir() => {
            let _ = std::fs::remove_dir_all(p);
        }
        Ok(_) => {
            let _ = std::fs::remove_file(p);
        }
        Err(_) => {}
    }
}

/// Best-effort rollback of a failed FRESH-CLONE `update_submodule`, modelled on
/// `rollback_partial_add`: each step independent, its own failure ignored; the
/// caller returns the ORIGINAL error. Called ONLY when
/// `salvage == Salvage::NotApplicable` (contract OPEN-7). Never touches
/// `.gitmodules`, the superproject index, HEAD, or any path outside
/// `<workdir>/<path>` and `<commondir>/modules/<key>`.
pub(super) fn rollback_partial_update(
    repo: &git2::Repository,
    name: &str,
    path: &str,
    pre: &PreUpdateState,
) {
    // 0. Rollback applies to a FRESH CLONE INTO AN EMPTY/ABSENT FOLDER only —
    //    the one case where everything on disk was created by this attempt.
    //
    //    `uninitialized`: if the submodule was already checked out, the failure
    //    came from the fetch or from the SAFE checkout refusing to clobber local
    //    content — nothing here was created by us, and touching the worktree
    //    would DESTROY user data.
    //
    //    `workdir_was_empty`: `WD_UNINITIALIZED` only means `<path>/.git` is
    //    absent, so a registered-but-never-cloned submodule whose folder holds
    //    the user's OWN untracked files is uninitialized too. There, libgit2
    //    clones fine and its SAFE checkout refuses — deleting that content would
    //    be exactly the data loss the refusal prevented, and deleting the module
    //    gitdir would leave a DANGLING gitlink (`git submodule status` then dies
    //    with "not a git repository"). Both are worse than doing nothing, so the
    //    whole rollback stands down and the coherent clone-succeeded /
    //    checkout-refused state is left in place for the user to resolve.
    if !pre.uninitialized || !pre.workdir_was_empty {
        return;
    }

    // 1. Worktree FIRST (only reached when it was empty/absent before the
    //    attempt — see step 0): if we created the dir, remove it; if it
    //    pre-existed empty, remove only its CONTENTS so the pre-state is restored
    //    exactly (the `-` row stays a `-` row, so the UI does not appear to lose
    //    the submodule).
    //
    //    ORDER MATTERS. This is best-effort code, so what counts is the shape of
    //    a PARTIAL failure. Clearing the worktree (and with it the gitlink)
    //    before the module gitdir means a failure of the second step degrades to
    //    "orphan gitdir, no gitlink" — the recoverable wedge that P73's reattach
    //    repairs. The reverse order would leave a DANGLING gitlink, which makes
    //    even `git submodule status` fail.
    if let Some(wd) = repo.workdir() {
        if validate_rel_path(path).is_ok() {
            let target = wd.join(path);
            if !pre.workdir_existed {
                let _ = std::fs::remove_dir_all(&target);
            } else if let Ok(entries) = std::fs::read_dir(&target) {
                for e in entries.flatten() {
                    remove_dir_all_or_file(&e.path());
                }
            }
        }
    }

    // 2. Module gitdir: remove ONLY if this attempt's clone created it.
    if !pre.module_dir_existed {
        if validate_modules_name(name).is_ok() {
            remove_cached_git_dir(repo, name);
        }
        // libgit2 keys the clone repodir on `path`, not `name`.
        if path != name && validate_modules_name(path).is_ok() {
            remove_cached_git_dir(repo, path);
        }
    }

    // 3. Registration: clear ONLY if `update(init = true)` created it.
    if !pre.registered {
        if let Ok(mut cfg) = repo.config() {
            let _ = cfg.remove(&format!("submodule.{name}.url"));
            let _ = cfg.remove(&format!("submodule.{name}.update"));
            let _ = cfg.remove(&format!("submodule.{name}.active"));
        }
    }
}
