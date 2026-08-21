//! Submodule support (P19 contract §2). Read + common ops: list with
//! classified status, init, update (fetch + checkout the pinned commit), sync.
//!
//! Pure git2 logic, no Tauri types (runtime-free core → unit/CLI-testable
//! without the Tauri "test" feature, same rule as stash/remote). `update`
//! fetches, so it reuses the M6 credential chain (`remote::acquire_cred`)
//! verbatim — never prompts, never stores passwords.
//!
//! `status.rs` stays AS-IS (`.exclude_submodules(true)`, §7): submodule state
//! surfaces ONLY here, never mixed into the working-dir file-status lists.

use std::cell::RefCell;
use std::path::Path;

use crate::error::AppError;
use crate::git::remote::{acquire_cred, map_remote_err, CredAttempts};
use crate::git::search::GitRunner;
use crate::git::stage::{open_workdir_repo, validate_rel_path};
use crate::git::submodule_reconnect::{
    is_reinitialize_error, msg_unusable_module_dir, reattach_module_gitdir, Salvage,
};
use crate::git::submodule_rollback::{rollback_partial_update, snapshot_pre_update};

/// Consolidated state of one submodule. Wire: a camelCase string enum (no
/// data). Derived from git2's `Repository::submodule_status` bitflags (§2.4),
/// evaluated in PRIORITY order (first match wins):
/// Uninitialized > OutOfSync > ModifiedWorkdir > UpToDate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleStatus {
    /// Registered in .gitmodules/index but not checked out (`WD_UNINITIALIZED`).
    /// Maps to `git submodule status` leading `-`.
    Uninitialized,
    /// Checked out and matching the recorded commit, clean workdir.
    /// Maps to `git submodule status` leading ` ` (space).
    UpToDate,
    /// The checked-out commit differs from the commit recorded in the
    /// superproject (index or HEAD). Maps to `git submodule status` leading `+`.
    OutOfSync,
    /// Checked-out commit matches, but the submodule's OWN worktree/index is
    /// dirty (staged, unstaged, or untracked changes inside it).
    ModifiedWorkdir,
}

/// One submodule row. Wire: camelCase. All oids are full 40-hex or null.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInfo {
    /// Submodule NAME (stable key for init/update/sync). `Submodule::name()`.
    pub name: String,
    /// Repo-relative path, forward slashes on the wire. `Submodule::path()`.
    pub path: String,
    /// ABSOLUTE workdir path for open-in-tab (§OPEN-1): superproject workdir
    /// joined with `path`. Fed verbatim to the existing open-repo/tab flow.
    pub abs_path: String,
    /// Configured URL from .gitmodules/.git config. `Submodule::url()`.
    pub url: Option<String>,
    /// Commit recorded in the superproject HEAD tree. `Submodule::head_id()`.
    pub head_oid: Option<String>,
    /// Commit recorded in the superproject index. `Submodule::index_id()`.
    pub index_oid: Option<String>,
    /// Commit currently checked out in the submodule worktree.
    /// `Submodule::workdir_id()`. None when uninitialized.
    pub wt_oid: Option<String>,
    pub status: SubmoduleStatus,
}

// P82 (F-A7-7): the deinit/remove FORCE machinery (outcome enums, the dirty
// check, and the conditional-`-f` argv builders) lives in `submodule_teardown`
// to keep this file under the ~500-line limit. Re-exported here so the wire
// types keep their contracted path (`bonsai_core::git::submodule::*`) and the
// argv builders / dirty check stay reachable from the ops + the test module.
pub use super::submodule_teardown::{SubmoduleDeinitOutcome, SubmoduleRemoveOutcome};
pub(crate) use super::submodule_teardown::{deinit_args, is_submodule_dirty, rm_args};

/// Maps git2's `SubmoduleStatus` bitflags to our single enum in PRIORITY order
/// (first match wins). A submodule that is simultaneously out-of-sync AND dirty
/// classifies as `OutOfSync` (higher priority) — so the UI badge is
/// deterministic (§2.4).
fn classify_status(f: git2::SubmoduleStatus) -> SubmoduleStatus {
    use git2::SubmoduleStatus as S;
    // 1. Not checked out at all.
    if f.contains(S::WD_UNINITIALIZED) {
        return SubmoduleStatus::Uninitialized;
    }
    // 2. Recorded-commit mismatch: superproject index/HEAD pointer changed, OR
    //    the checked-out commit differs from the index pointer.
    if f.intersects(S::INDEX_ADDED | S::INDEX_DELETED | S::INDEX_MODIFIED | S::WD_MODIFIED) {
        return SubmoduleStatus::OutOfSync;
    }
    // 3. Submodule's own index/worktree is dirty (but the pinned commit matches).
    if f.intersects(S::WD_INDEX_MODIFIED | S::WD_WD_MODIFIED | S::WD_UNTRACKED) {
        return SubmoduleStatus::ModifiedWorkdir;
    }
    // 4. Checked out, clean, matching.
    SubmoduleStatus::UpToDate
}

/// Build one [`SubmoduleInfo`] row from an opened submodule handle and its
/// already-resolved (UTF-8) `name`. `sm_workdir` is the superproject workdir
/// (used for the absolute `abs_path`). Shared by [`list_submodules`] and
/// [`add_submodule`] so the wire shape is produced in exactly one place.
fn submodule_info(
    repo: &git2::Repository,
    sm: &git2::Submodule,
    name: String,
    sm_workdir: &Path,
) -> Result<SubmoduleInfo, AppError> {
    let flags = repo.submodule_status(&name, git2::SubmoduleIgnore::None)?; // §OPEN-2
    Ok(SubmoduleInfo {
        name,
        path: sm.path().to_string_lossy().replace('\\', "/"), // forward slashes on the wire
        abs_path: sm_workdir.join(sm.path()).to_string_lossy().into_owned(),
        url: sm.url().ok().flatten().map(str::to_string),
        head_oid: sm.head_id().map(|o| o.to_string()),
        index_oid: sm.index_id().map(|o| o.to_string()),
        wt_oid: sm.workdir_id().map(|o| o.to_string()),
        status: classify_status(flags),
    })
}

/// Blocking. List every submodule with its classified status. No submodules →
/// Ok(vec![]). Order: `Repository::submodules()` order (stable).
pub fn list_submodules(workdir: &Path) -> Result<Vec<SubmoduleInfo>, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let sm_workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?
        .to_path_buf();

    let mut out = Vec::new();
    for sm in repo.submodules()? {
        // Skip non-UTF-8 names (cannot key status; log + skip, like fetch_all).
        let name = match sm.name().ok() {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping submodule with non-UTF-8 name");
                continue;
            }
        };
        out.push(submodule_info(&repo, &sm, name, &sm_workdir)?);
    }
    Ok(out)
}

/// Shared open + name-validate + `find_submodule` prologue. NotFound →
/// `AppError::Git` (§OPEN-3); blank name → `AppError::InvalidName`.
fn open_submodule<'r>(
    repo: &'r git2::Repository,
    name: &str,
) -> Result<git2::Submodule<'r>, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::InvalidName("submodule name is empty".to_string()));
    }
    repo.find_submodule(name).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => AppError::Git(format!("submodule '{name}' not found")),
        _ => e.into(),
    })
}

/// Blocking. Register submodule `name` into .git/config (copies .gitmodules
/// url/config). git2: `Submodule::init(false)` (no overwrite). No worktree change.
pub fn init_submodule(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let mut sm = open_submodule(&repo, name)?;
    sm.init(false)?;
    Ok(())
}

/// Blocking. Init-if-needed + fetch (shared M6 credential chain) + checkout the
/// pinned commit. git2: `Submodule::update(true, Some(&mut opts))` with the
/// fetch callbacks wired to the credential chain (§2.5). MODIFIES the submodule
/// worktree (safe checkout default; never force).
///
/// P73: before handing over to libgit2, try to reconnect an orphaned
/// `.git/modules/<key>` gitdir to an empty, gitlink-less worktree
/// ([`reattach_module_gitdir`]) — libgit2 would otherwise take its `NO_REINIT`
/// clone branch and fail with `attempt to reinitialize '<...>'`. On that salvage
/// path the checkout additionally gets `recreate_missing` (NOT `force`: dirty
/// content is still protected) because the module index already matches the
/// target, so a plain SAFE checkout would write nothing over the empty dir. A
/// failed FRESH clone rolls back to the pre-attempt disk state.
pub fn update_submodule(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let sm = open_submodule(&repo, name)?;
    let path = sm.path().to_string_lossy().replace('\\', "/");

    // Snapshot before anything can be created (used only on the clone path).
    let pre = snapshot_pre_update(&repo, name, &path);

    // P73 salvage. Any refusal propagates immediately — falling through to
    // libgit2's clone branch would only fail with "attempt to reinitialize".
    let salvage = reattach_module_gitdir(&repo, &sm, name)?;

    // Re-open the handle: the reattach changed on-disk state and
    // `Submodule::reload` ignores `force`. Drop the old handle first (borrowck).
    drop(sm);
    let mut sm = open_submodule(&repo, name)?;

    let attempts = RefCell::new(CredAttempts::default());
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed| {
        acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);
    let mut opts = git2::SubmoduleUpdateOptions::new();
    opts.fetch(fo);

    // On the salvage path ONLY: let the checkout recreate files that are ABSENT
    // from the (verified empty) worktree. `recreate_missing` is not `force` —
    // existing content is never overwritten, so the SAFE-checkout invariant
    // asserted by `update_refuses_to_clobber_dirty_submodule` still holds.
    if salvage == Salvage::Reattached {
        let mut co = git2::build::CheckoutBuilder::new();
        co.recreate_missing(true);
        opts.checkout(co);
    }

    // init=true → init-then-update in one call (§OPEN-4). SAFE checkout default.
    match sm.update(true, Some(&mut opts)) {
        Ok(()) => Ok(()),
        Err(raw) => {
            // BACKSTOP (P73): the salvage stood down (`NotApplicable`) — e.g. the
            // cached `.git/modules/<key>` exists but is not an openable repo, so
            // step 4 bailed — and libgit2 then took its clone branch and refused
            // with "attempt to reinitialize '<abs path>'". That raw sentence must
            // never reach the toast; the rollback also stands down here (the
            // module dir pre-existed), so tell the user the one remedy instead.
            let mapped = if salvage == Salvage::NotApplicable && is_reinitialize_error(&raw) {
                // `path`, not `name`: libgit2 keys the dir it failed to init on
                // `sm->path`, so that is the folder the user must delete.
                AppError::Git(msg_unusable_module_dir(&path))
            } else {
                map_remote_err(raw, name)
            };
            // Rollback ONLY the fresh-clone path: on the salvage path the module
            // gitdir pre-existed and MUST NOT be deleted, and the freshly
            // written gitlink is correct (it makes a retry work).
            if salvage == Salvage::NotApplicable {
                rollback_partial_update(&repo, name, &path, &pre);
            }
            Err(mapped)
        }
    }
}

/// Blocking. Copy the URL from .gitmodules into .git/config and the submodule's
/// remote. git2: `Submodule::sync()`. No worktree change, no fetch/credentials.
pub fn sync_submodule(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let mut sm = open_submodule(&repo, name)?;
    sm.sync()?;
    Ok(())
}

/// Blocking. Adds a submodule at repo-relative `path` from `url` (D4): git2
/// `Repository::submodule(url, Path::new(path), /*use_gitlink*/ true)` →
/// clone the subrepo with the shared M6 credential callback (`acquire_cred`,
/// exactly as [`update_submodule`]) → `Submodule::init(false)` (write
/// `submodule.<name>.*` into .git/config so it is "initialized", matching
/// `git submodule add`) → `Submodule::add_finalize()` (stage .gitmodules + the
/// new gitlink). `path` is validated with `validate_rel_path`; a blank url/path
/// → InvalidName. Errors: `invalidName` | `git` (incl. network/auth via
/// `map_remote_err`) | `noRepo`.
pub fn add_submodule(workdir: &Path, url: &str, path: &str) -> Result<SubmoduleInfo, AppError> {
    if url.trim().is_empty() {
        return Err(AppError::InvalidName("submodule url is empty".to_string()));
    }
    if path.trim().is_empty() {
        return Err(AppError::InvalidName("submodule path is empty".to_string()));
    }
    validate_rel_path(path)?; // reject absolute / `..` / backslash traversal

    let repo = open_workdir_repo(workdir)?;
    let sm_workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?
        .to_path_buf();

    // add-setup: write .gitmodules + register the submodule (use_gitlink = true).
    let mut sm = repo
        .submodule(url, Path::new(path), true)
        .map_err(|e| map_remote_err(e, path))?;

    // The name is path-derived at add-setup; grab it up front so a failure
    // below can be rolled back by name (F-A7-10).
    let name = match sm.name().ok() {
        Some(n) => n.to_string(),
        None => {
            rollback_partial_add(&repo, path, path);
            return Err(AppError::Git("submodule has a non-UTF-8 name".to_string()));
        }
    };

    // Clone + register + finalize. On ANY failure, best-effort rollback of the
    // add-setup residue (.gitmodules entries, .git/config registration, the
    // partial checkout dir, the cached .git/modules dir) so a retry does not
    // hit "submodule already exists" (F-A7-10); the ORIGINAL error is returned.
    let finalize = (|| -> Result<(), AppError> {
        // Clone the subrepo with the shared M6 credential chain (uniform with
        // `update_submodule`; never prompts, never stores passwords).
        {
            let attempts = RefCell::new(CredAttempts::default());
            let mut callbacks = git2::RemoteCallbacks::new();
            callbacks.credentials(|url, username_from_url, allowed| {
                acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
            });
            let mut fo = git2::FetchOptions::new();
            fo.remote_callbacks(callbacks);
            let mut opts = git2::SubmoduleUpdateOptions::new();
            opts.fetch(fo);
            sm.clone(Some(&mut opts))
                .map_err(|e| map_remote_err(e, url))?;
        }

        // Register in .git/config (like `git submodule add`) then stage
        // .gitmodules + the gitlink for the next commit.
        sm.init(false)?;
        sm.add_finalize()?;
        Ok(())
    })();
    if let Err(e) = finalize {
        rollback_partial_add(&repo, &name, path);
        return Err(e);
    }

    submodule_info(&repo, &sm, name, &sm_workdir)
}

/// Best-effort rollback of a failed [`add_submodule`] (F-A7-10): removes the
/// `.gitmodules` entries written by add-setup, the `submodule.<name>.*`
/// registration `init` may have written to .git/config, the (partial) checkout
/// dir at `path` (already contained in the workdir via `validate_rel_path`),
/// and the cached `.git/modules/<name>` dir the clone may have created. Every
/// step is independent and its own failure ignored — the caller reports the
/// ORIGINAL error. An empty `.gitmodules` file may remain (harmless to git),
/// and anything `add_finalize` staged before failing stays staged; both
/// self-heal on the next successful add or a manual `git checkout .gitmodules`.
fn rollback_partial_add(repo: &git2::Repository, name: &str, path: &str) {
    if let Some(wd) = repo.workdir() {
        // .gitmodules entries (written by add-setup).
        if let Ok(mut cfg) = git2::Config::open(&wd.join(".gitmodules")) {
            let _ = cfg.remove(&format!("submodule.{name}.path"));
            let _ = cfg.remove(&format!("submodule.{name}.url"));
        }
        // The partial checkout dir at `path` (validated repo-relative).
        let _ = std::fs::remove_dir_all(wd.join(path));
    }
    // .git/config registration (written by `init`, when reached).
    if let Ok(mut cfg) = repo.config() {
        let _ = cfg.remove(&format!("submodule.{name}.url"));
        let _ = cfg.remove(&format!("submodule.{name}.update"));
        let _ = cfg.remove(&format!("submodule.{name}.active"));
    }
    // Cached git dir under .git/modules (created by the clone), with the same
    // traversal guard as `remove_submodule` (F-A7-2).
    if validate_modules_name(name).is_ok() {
        remove_cached_git_dir(repo, name);
    }
}

/// Resolve submodule `name` → its repo-relative path with forward slashes,
/// validating the name via [`open_submodule`] (blank → InvalidName, unknown →
/// Git). Shared by the deinit/remove shell-out ops so the pathspec fed to `git`
/// is always the tracked submodule path.
fn submodule_path(repo: &git2::Repository, name: &str) -> Result<String, AppError> {
    let sm = open_submodule(repo, name)?;
    Ok(sm.path().to_string_lossy().replace('\\', "/"))
}

/// Blocking. `git submodule deinit [-f] -- <path>` via `runner` (no libgit2
/// primitive; D4). Clears `submodule.<name>` from .git/config and empties the
/// submodule worktree; KEEPS the .gitmodules entry (re-init-able). `name` is
/// resolved to its path via `find_submodule`.
///
/// P82 (F-A7-7): with `force == false`, if the submodule worktree is dirty this
/// returns [`SubmoduleDeinitOutcome::DirtyNeedsForce`] mutating NOTHING (no
/// `-f`); clean → proceeds without `-f`. `force == true` runs with `-f` and
/// discards. Errors: `invalidName` | `git` (stderr tail) | `noRepo`.
pub fn deinit_submodule(
    workdir: &Path,
    runner: &dyn GitRunner,
    name: &str,
    force: bool,
) -> Result<SubmoduleDeinitOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let path = submodule_path(&repo, name)?; // validates name
    if !force && is_submodule_dirty(&repo, name)? {
        return Ok(SubmoduleDeinitOutcome::DirtyNeedsForce); // zero mutation
    }
    runner.run(&deinit_args(&path, force), workdir)?;
    Ok(SubmoduleDeinitOutcome::Deinitialized)
}

/// Blocking. Full removal (git's documented sequence): `git submodule deinit
/// [-f] -- <path>` → `git rm [-f] -- <path>` → best-effort `remove_dir_all(.git/
/// modules/<name>)`. DESTRUCTIVE (deletes the worktree, edits the index, drops
/// the .gitmodules entry + gitlink).
///
/// P82 (F-A7-7): with `force == false`, a dirty submodule worktree returns
/// [`SubmoduleRemoveOutcome::DirtyNeedsForce`] mutating NOTHING; clean → proceeds
/// without `-f`. `force == true` runs both shell-outs with `-f` and discards.
/// Errors: `invalidName` | `git` | `noRepo`.
pub fn remove_submodule(
    workdir: &Path,
    runner: &dyn GitRunner,
    name: &str,
    force: bool,
) -> Result<SubmoduleRemoveOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    // F-A7-2: `name` comes from .gitmodules (attacker-controllable) and is
    // joined under .git/modules — refuse traversal BEFORE any destructive step.
    validate_modules_name(name)?;
    let path = submodule_path(&repo, name)?;
    if !force && is_submodule_dirty(&repo, name)? {
        return Ok(SubmoduleRemoveOutcome::DirtyNeedsForce); // zero mutation
    }
    runner.run(&deinit_args(&path, force), workdir)?;
    runner.run(&rm_args(&path, force), workdir)?;
    // Best-effort: drop the cached git dir (`git rm` leaves it; may be absent).
    remove_cached_git_dir(&repo, name);
    Ok(SubmoduleRemoveOutcome::Removed)
}

/// F-A7-2: reject submodule names that could escape `.git/modules` when joined
/// (CVE-2018-11235 vector). Names MAY contain `/` (nested defaults such as
/// `vendor/libcore`), but never `..`/`.`/empty components (across `/` AND `\`)
/// or absolute paths.
pub(super) fn validate_modules_name(name: &str) -> Result<(), AppError> {
    let bytes = name.as_bytes();
    let absolute = name.starts_with('/')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':');
    let bad_component = name
        .split(['/', '\\'])
        .any(|c| c.is_empty() || c == "." || c == "..");
    if name.trim().is_empty() || absolute || bad_component {
        return Err(AppError::Git(format!(
            "refusing to touch .git/modules for submodule '{name}': unsafe name"
        )));
    }
    Ok(())
}

/// Best-effort removal of the cached `.git/modules/<name>` dir with
/// belt-and-braces containment (F-A7-2): even after name validation, delete
/// only when the canonicalized dir is strictly inside the canonicalized
/// modules root. An absent dir is a silent no-op (canonicalize fails).
pub(super) fn remove_cached_git_dir(repo: &git2::Repository, name: &str) {
    // P73 §6: commondir, NOT path() — inside a linked worktree `path()` is
    // `.git/worktrees/<wt>/`, whose `modules/` does not exist, so the cleanup
    // silently no-opped and left a stale gitdir behind (the P73 wedge). In a
    // plain repo `commondir() == path()`, so behaviour there is unchanged.
    let root = repo.commondir().join("modules");
    let dir = root.join(name);
    if let (Ok(canon_root), Ok(canon_dir)) = (root.canonicalize(), dir.canonicalize()) {
        if canon_dir.starts_with(&canon_root) && canon_dir != canon_root {
            let _ = std::fs::remove_dir_all(&canon_dir);
        }
    }
}

#[cfg(test)]
#[path = "submodule_tests.rs"]
mod tests;
