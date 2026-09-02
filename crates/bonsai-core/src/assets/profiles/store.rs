//! The on-disk profile store (`<workdir>/.bonsai/profiles.json`): where it
//! lives, how it is loaded/persisted atomically, and CRUD over its profiles.

use std::path::{Path, PathBuf};

use crate::assets::taxonomy::{descriptor, AssetKind};
use crate::error::AppError;
use crate::git::stage::validate_rel_path;
use crate::git::worktree::main_workdir;

use super::worktree::open_repo_at;
use super::{ContextProfile, ProfileStore, MAIN_WORKTREE_KEY};

/// Repo-relative store location (§5.1) — `root` is the RESOLVED store root.
fn store_path(root: &Path) -> PathBuf {
    root.join(".bonsai").join("profiles.json")
}

/// P31 D2. The directory whose `.bonsai/profiles.json` is THE shared store:
/// the MAIN worktree's workdir when `workdir` is any worktree of a repo, and
/// `workdir` itself when it is not a git repo (pure-P24 fallback). Best-effort:
/// never errors.
pub fn resolve_store_root(workdir: &Path) -> PathBuf {
    match open_repo_at(workdir) {
        Ok(repo) => main_workdir(&repo).unwrap_or_else(|_| workdir.to_path_buf()),
        Err(_) => workdir.to_path_buf(),
    }
}

/// Sibling temp path `<file>.bonsai-tmp` for an atomic write.
fn tmp_sibling(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".bonsai-tmp");
    target.with_file_name(name)
}

/// Atomically replace `target` with `bytes`: write a sibling temp file, then
/// rename over the target (rename is atomic and replaces on both platforms).
/// The caller must ensure `target`'s parent dir exists.
pub(super) fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let tmp = tmp_sibling(target);
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, target) {
        // Best-effort cleanup so a failed rename leaves no `.bonsai-tmp` remnant.
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Blocking. Load the store from the RESOLVED shared root (P31 D1/D2: always
/// the main worktree's `.bonsai/profiles.json`; non-repo dirs use `workdir`
/// itself), or the lazy empty default if the file / `.bonsai/` dir is absent.
/// Read-only — never writes. Malformed JSON → `Other("profiles.json is
/// corrupt: …")`.
pub fn list_profiles(workdir: &Path) -> Result<ProfileStore, AppError> {
    load_store(&resolve_store_root(workdir))
}

/// Load the store file under an already-resolved root.
pub(super) fn load_store(root: &Path) -> Result<ProfileStore, AppError> {
    let path = store_path(root);
    if !path.is_file() {
        return Ok(ProfileStore::empty());
    }
    let bytes = std::fs::read(&path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Other(format!("profiles.json is corrupt: {e}")))
}

/// Persist the store under the RESOLVED root: create `.bonsai/`, stamp
/// `version = 2`, enforce the `"@main"` mirror invariant both directions
/// (`active_profile == worktree_activations["@main"]`), garbage-collect stale
/// worktree keys (§3 key hygiene), then write pretty JSON atomically
/// (temp + rename). Mutates `store` so the returned value matches disk.
pub(super) fn persist(root: &Path, store: &mut ProfileStore) -> Result<(), AppError> {
    store.version = 2;
    // Mirror invariant (D4), both directions.
    match store.worktree_activations.get(MAIN_WORKTREE_KEY) {
        Some(p) => store.active_profile = Some(p.clone()),
        None => {
            if let Some(a) = store.active_profile.clone() {
                store
                    .worktree_activations
                    .insert(MAIN_WORKTREE_KEY.to_string(), a);
            }
        }
    }
    // GC stale keys: keep "@main" and names that still resolve as worktrees.
    // Best-effort — non-repo roots (pure P24) keep the map as-is.
    if let Ok(repo) = open_repo_at(root) {
        store
            .worktree_activations
            .retain(|k, _| k == MAIN_WORKTREE_KEY || repo.find_worktree(k).is_ok());
    }
    let dir = root.join(".bonsai");
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(store)
        .map_err(|e| AppError::Other(format!("failed to serialize profiles.json: {e}")))?;
    atomic_write(&store_path(root), &json)
}

/// `validate_profile_name` (§5.3): reject blank / leading `-` / path separators
/// (`/`, `\`) / control chars → `InvalidName`.
pub fn validate_profile_name(name: &str) -> Result<(), AppError> {
    let invalid = name.trim().is_empty()
        || name.starts_with('-')
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control);
    if invalid {
        return Err(AppError::InvalidName(format!(
            "invalid profile name: '{name}'"
        )));
    }
    Ok(())
}

/// Resolve a target's `asset_id` to its mapped repo-relative path, enforcing
/// that it is a known SingleFile descriptor (§OPEN #4) and that the path is
/// in-workdir (belt-and-suspenders; descriptor paths are static + safe).
pub(super) fn resolve_single_file_target(asset_id: &str) -> Result<&'static str, AppError> {
    let d = descriptor(asset_id)
        .filter(|d| matches!(d.kind, AssetKind::SingleFile))
        .ok_or_else(|| {
            AppError::InvalidName(format!("invalid profile target asset: '{asset_id}'"))
        })?;
    validate_rel_path(d.path)?;
    Ok(d.path)
}

/// Blocking. Insert or replace the profile keyed by `profile.name`, then
/// persist. Validates the name (§5.3) and every target (`assetId` is a known
/// SingleFile descriptor; else `InvalidName`). Returns the updated store.
pub fn save_profile(workdir: &Path, profile: ContextProfile) -> Result<ProfileStore, AppError> {
    validate_profile_name(&profile.name)?;
    for t in &profile.targets {
        resolve_single_file_target(&t.asset_id)?;
    }
    let root = resolve_store_root(workdir);
    let mut store = load_store(&root)?;
    if let Some(existing) = store.profiles.iter_mut().find(|p| p.name == profile.name) {
        *existing = profile;
    } else {
        store.profiles.push(profile);
    }
    persist(&root, &mut store)?;
    Ok(store)
}

/// Blocking. Remove the profile named `name` (no-op if absent), clear
/// `active_profile` if it pointed there, drop every worktree-activation entry
/// whose value is `name` (P31 §3), persist. Returns the updated store.
pub fn delete_profile(workdir: &Path, name: &str) -> Result<ProfileStore, AppError> {
    let root = resolve_store_root(workdir);
    let mut store = load_store(&root)?;
    store.profiles.retain(|p| p.name != name);
    if store.active_profile.as_deref() == Some(name) {
        store.active_profile = None;
    }
    store.worktree_activations.retain(|_, v| v != name);
    persist(&root, &mut store)?;
    Ok(store)
}
