//! Preview + activate: diffing a profile's targets against the worktree and
//! writing them out atomically.

use std::path::Path;

use crate::error::AppError;

use super::store::{
    atomic_write, load_store, persist, resolve_single_file_target, resolve_store_root,
};
use super::worktree::{calling_worktree_key, ensure_targets_clean, resolve_worktree_root};
use super::{
    ProfileActivation, ProfilePreviewEntry, TargetWriteAction, TargetWriteResult, MAIN_WORKTREE_KEY,
};

/// Blocking. Compute, WITHOUT WRITING, the per-target before/after for the named
/// profile's activation against the CALLING worktree's files. Thin wrapper over
/// `preview_profile_for_worktree` (P31 D5); non-repo dirs keep pure-P24
/// behavior on `workdir` directly.
pub fn preview_profile(workdir: &Path, name: &str) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let key = calling_worktree_key(workdir)?;
    preview_profile_for_worktree(workdir, &key, name)
}

/// Blocking (P31 §4). Preview `name` against WORKTREE `worktree_key`'s files.
/// The store is read from the shared root; D6 eligibility is enforced BEFORE
/// reading (locked/invalid/prunable → `Git`). Writes nothing. `current` =
/// existing mapped-file content (None if absent); `changed` = byte inequality
/// (a missing file differs). Paths are worktree-relative.
pub fn preview_profile_for_worktree(
    workdir: &Path,
    worktree_key: &str,
    name: &str,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let root = resolve_store_root(workdir);
    let store = load_store(&root)?;
    let profile = store
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| AppError::Other(format!("profile '{name}' not found")))?;
    let wt_root = resolve_worktree_root(workdir, worktree_key)?; // D6

    let mut entries = Vec::with_capacity(profile.targets.len());
    for t in &profile.targets {
        let rel = resolve_single_file_target(&t.asset_id)?;
        let full = wt_root.join(rel);
        let raw = if full.is_file() {
            Some(std::fs::read(&full)?)
        } else {
            None
        };
        let changed = raw.as_deref() != Some(t.content.as_bytes());
        let current = raw.map(|b| String::from_utf8_lossy(&b).into_owned());
        entries.push(ProfilePreviewEntry {
            asset_id: t.asset_id.clone(),
            path: rel.to_string(),
            current,
            proposed: t.content.clone(),
            changed,
        });
    }
    Ok(entries)
}

/// Blocking. Activate `name` onto the CALLING worktree. Thin wrapper over
/// `activate_profile_for_worktree` (P31 D5) so a worktree opened as its own
/// tab records its activation in the shared map automatically; non-repo dirs
/// keep pure-P24 behavior on `workdir` directly.
pub fn activate_profile(workdir: &Path, name: &str) -> Result<ProfileActivation, AppError> {
    let key = calling_worktree_key(workdir)?;
    activate_profile_for_worktree(workdir, &key, name)
}

/// Blocking (P31 §4). THE one write path. Order: resolve store root → find
/// profile → resolve target worktree root (D6 eligibility) → validate ALL
/// targets (SingleFile + `validate_rel_path`) → D7 dirty-target guard over ALL
/// targets → write each (atomic temp+rename, parent dirs created) → update
/// `worktree_activations[key]` (+ legacy mirror when key == "@main") → persist.
/// The UI gates it behind confirm + preview (§8.3).
///
/// On an I/O error mid-loop the targets already written are left as-is (atomic
/// per file, not transactional across files).
pub fn activate_profile_for_worktree(
    workdir: &Path,
    worktree_key: &str,
    name: &str,
) -> Result<ProfileActivation, AppError> {
    let root = resolve_store_root(workdir);
    let mut store = load_store(&root)?;
    let profile = store
        .profiles
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| AppError::Other(format!("profile '{name}' not found")))?;
    let wt_root = resolve_worktree_root(workdir, worktree_key)?; // D6

    // Validate ALL targets first (descriptor is SingleFile + path in-worktree).
    let mut mapped = Vec::with_capacity(profile.targets.len());
    for t in &profile.targets {
        let rel = resolve_single_file_target(&t.asset_id)?;
        mapped.push((t, rel));
    }
    // D7: refuse ANY tracked+modified target before ANY write.
    let guard_targets: Vec<(&str, &[u8])> = mapped
        .iter()
        .map(|(t, rel)| (*rel, t.content.as_bytes()))
        .collect();
    ensure_targets_clean(&wt_root, &guard_targets)?;

    let mut results = Vec::with_capacity(mapped.len());
    for (t, rel) in mapped {
        let full = wt_root.join(rel);
        let existed = full.is_file();
        let current = if existed {
            Some(std::fs::read(&full)?)
        } else {
            None
        };
        let action = if current.as_deref() == Some(t.content.as_bytes()) {
            TargetWriteAction::Unchanged
        } else {
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            atomic_write(&full, t.content.as_bytes())?;
            if existed {
                TargetWriteAction::Written
            } else {
                TargetWriteAction::Created
            }
        };
        results.push(TargetWriteResult {
            asset_id: t.asset_id.clone(),
            path: rel.to_string(),
            action,
        });
    }

    store
        .worktree_activations
        .insert(worktree_key.to_string(), name.to_string());
    if worktree_key == MAIN_WORKTREE_KEY {
        store.active_profile = Some(name.to_string()); // legacy mirror (D4)
    }
    persist(&root, &mut store)?;
    Ok(ProfileActivation {
        profile: name.to_string(),
        results,
        store,
    })
}
