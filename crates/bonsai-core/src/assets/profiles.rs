//! Context-profile store: CRUD + preview + activate (P24 contract §5).
//!
//! A profile is a named bundle of `(assetId, content)` targets. Activating one
//! writes each target's verbatim content to its mapped single-file instruction
//! doc (atomic temp + rename, parent dirs created), gated behind the UI's
//! confirm + diff preview. The store lives at `<workdir>/.bonsai/profiles.json`
//! and is created lazily on first save. Pure filesystem + serde — no Tauri, no
//! git repo needed; the command layer wraps every function in `spawn_blocking`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::assets::taxonomy::{descriptor, AssetKind};
use crate::error::AppError;
use crate::git::stage::validate_rel_path;
use crate::git::worktree::{canonical, main_workdir};

/// Reserved worktree-activation key for the MAIN worktree (P31 D3). `@` is
/// rejected by `sanitize_slug`, so no linked worktree can collide with it.
pub const MAIN_WORKTREE_KEY: &str = "@main";

/// One profile target: which taxonomy asset to write, and the verbatim content.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTarget {
    /// A descriptor id; MUST be a SingleFile descriptor (§OPEN #4). Rules-dir /
    /// Config ids are rejected by `save_profile`/`activate_profile` with
    /// `InvalidName`.
    pub asset_id: String,
    /// Verbatim content written to the mapped file on activation.
    pub content: String,
}

/// A named bundle of targets (a "context profile" for one model/agent flavor).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextProfile {
    /// Unique key within the store; also the display name. Validated (§5.3).
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Informational label (e.g. "opus", "haiku", "gpt-5"); not enforced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub targets: Vec<ProfileTarget>,
}

/// The on-disk store (`.bonsai/profiles.json`) AND the wire shape of
/// list/save/delete/activate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStore {
    /// Schema version; current = 2 (P31 D4). v1 files load unchanged (the map
    /// defaults to empty); `persist()` stamps 2 on the next save — reads never
    /// rewrite the file. Forward-compatible: unknown-higher versions still load
    /// (serde ignores unknown fields) but the UI may warn.
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<ContextProfile>,
    /// LEGACY mirror of `worktree_activations["@main"]` (kept so P24-era
    /// UI/tests stay correct; `persist()` enforces the invariant both ways).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    /// P31 D3/D4: worktree key (`"@main"` | linked worktree name) → the profile
    /// last activated INTO that worktree.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub worktree_activations: BTreeMap<String, String>,
}

impl ProfileStore {
    /// The lazy default returned when no store file exists.
    fn empty() -> Self {
        ProfileStore {
            version: 2,
            profiles: Vec::new(),
            active_profile: None,
            worktree_activations: BTreeMap::new(),
        }
    }

    /// The `"@main"` activation with the v1 legacy `active_profile` folded in
    /// at READ time (migration rule §3: in-memory only, no write on read).
    pub fn effective_activation(&self, key: &str) -> Option<&str> {
        match self.worktree_activations.get(key) {
            Some(p) => Some(p.as_str()),
            None if key == MAIN_WORKTREE_KEY => self.active_profile.as_deref(),
            None => None,
        }
    }
}

/// Per-target before/after for an activation preview (writes nothing).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePreviewEntry {
    pub asset_id: String,
    /// Resolved repo-relative mapped file.
    pub path: String,
    pub current: Option<String>,
    pub proposed: String,
    /// true iff `current` differs from `proposed` (byte-exact; a missing file differs).
    pub changed: bool,
}

/// What an activation did to one target's file. A field-less serde enum with
/// `rename_all = "camelCase"` and NO `tag` → serializes to the bare string
/// `"created"` / `"written"` / `"unchanged"` (P24 §6.2 correction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetWriteAction {
    /// The file was absent and was created.
    Created,
    /// The file existed with different bytes and was overwritten.
    Written,
    /// The file already matched the target content; skipped.
    Unchanged,
}

/// Per-target outcome of an activation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetWriteResult {
    pub asset_id: String,
    pub path: String,
    pub action: TargetWriteAction,
}

/// Result of `activate_profile`: per-target summary + the persisted store.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileActivation {
    pub profile: String,
    pub results: Vec<TargetWriteResult>,
    /// The store after `active_profile` was updated (frontend refreshes from this).
    pub store: ProfileStore,
}

/// Repo-relative store location (§5.1) — `root` is the RESOLVED store root.
fn store_path(root: &Path) -> PathBuf {
    root.join(".bonsai").join("profiles.json")
}

/// Open the repo at `workdir` exactly (no upward search), like
/// `stage::open_workdir_repo` but without the bare check (read-only helper).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, git2::Error> {
    git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
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

/// The worktree key the D5 wrappers operate under. `"@main"` ONLY when the
/// workdir is not an openable git repo (pure-P24 fallback on a plain folder);
/// for real repos, identity-resolution failures PROPAGATE — a linked worktree
/// whose identity cannot be established must never be silently retargeted to
/// the main worktree's activation slot.
fn calling_worktree_key(workdir: &Path) -> Result<String, AppError> {
    if open_repo_at(workdir).is_err() {
        return Ok(MAIN_WORKTREE_KEY.to_string());
    }
    worktree_key_for(workdir)
}

/// P31 D3. `"@main"` for the main worktree, else the linked worktree's NAME.
/// `Err(Git)` when `workdir` is not a git worktree at all or its identity
/// cannot be established.
pub fn worktree_key_for(workdir: &Path) -> Result<String, AppError> {
    let repo = open_repo_at(workdir)
        .map_err(|e| AppError::Git(format!("not a git repository: {}", e.message())))?;
    if !repo.is_worktree() {
        return Ok(MAIN_WORKTREE_KEY.to_string());
    }
    // A linked worktree's gitdir is `<common>/worktrees/<name>` — the basename
    // is the worktree name. Validate it resolves via find_worktree.
    let name = repo
        .path()
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Git("cannot determine worktree name".to_string()))?;
    if repo.find_worktree(&name).is_ok() {
        return Ok(name);
    }
    // Fallback: match by canonical workdir path against the worktree list.
    let cur = canonical(workdir);
    for n in repo.worktrees()?.iter().flatten() {
        if let Ok(wt) = repo.find_worktree(n) {
            if canonical(wt.path()) == cur {
                return Ok(n.to_string());
            }
        }
    }
    Err(AppError::Git(format!(
        "'{}' matches no worktree of the repository",
        workdir.display()
    )))
}

/// P31 D6 eligibility: a linked worktree must be valid, not prunable, and not
/// locked before we read from or write into its working directory.
fn ensure_eligible(wt: &git2::Worktree, key: &str) -> Result<(), AppError> {
    if wt.validate().is_err() {
        return Err(AppError::Git(format!(
            "worktree '{key}' is invalid (its working directory is missing or broken)"
        )));
    }
    if wt.is_prunable(None).unwrap_or(false) {
        return Err(AppError::Git(format!(
            "worktree '{key}' is stale (prunable); prune or repair it first"
        )));
    }
    if matches!(wt.is_locked()?, git2::WorktreeLockStatus::Locked(_)) {
        return Err(AppError::Git(format!(
            "worktree '{key}' is locked; unlock it first"
        )));
    }
    Ok(())
}

/// Resolve `worktree_key` to the target worktree's ROOT directory, enforcing
/// D6 eligibility for linked worktrees. `"@main"` on a non-repo dir keeps the
/// pure-P24 behavior (`workdir` itself).
fn resolve_worktree_root(workdir: &Path, worktree_key: &str) -> Result<PathBuf, AppError> {
    let repo = match open_repo_at(workdir) {
        Ok(r) => r,
        Err(e) if worktree_key == MAIN_WORKTREE_KEY => {
            // Pure-P24 fallback: not a repo → operate on the dir itself.
            let _ = e;
            return Ok(workdir.to_path_buf());
        }
        Err(e) => {
            return Err(AppError::Git(format!(
                "not a git repository: {}",
                e.message()
            )))
        }
    };
    if worktree_key == MAIN_WORKTREE_KEY {
        return main_workdir(&repo);
    }
    let wt = repo.find_worktree(worktree_key).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => {
            AppError::Git(format!("worktree '{worktree_key}' not found"))
        }
        _ => e.into(),
    })?;
    ensure_eligible(&wt, worktree_key)?;
    Ok(wt.path().to_path_buf())
}

/// P31 D7 dirty-target guard: for each mapped `(rel, proposed)` target in the
/// TARGET worktree, refuse when the file is TRACKED and modified (index or
/// worktree status ≠ CURRENT). Untracked (`WT_NEW`) and clean/missing pass, as
/// does a tracked-modified file whose bytes ALREADY equal the proposed content
/// (the write would be a no-op `Unchanged` — nothing can be lost; this keeps
/// re-activating the same profile idempotent, acceptance §9.3). Checked for
/// ALL targets before ANY write. Pathspec-limited (`status_file`) — never a
/// full-repo scan. Non-repo target dirs (pure-P24) pass trivially.
fn ensure_targets_clean(wt_root: &Path, targets: &[(&str, &[u8])]) -> Result<(), AppError> {
    let repo = match open_repo_at(wt_root) {
        Ok(r) => r,
        Err(_) => return Ok(()), // pure-P24 dir: no git safety net to protect
    };
    for (rel, proposed) in targets {
        let status = match repo.status_file(Path::new(rel)) {
            Ok(s) => s,
            // Not found anywhere (HEAD/index/worktree) → nothing to clobber.
            Err(e) if e.code() == git2::ErrorCode::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        // Block only when the file is TRACKED and modified: intersect against
        // the tracked-modified flags rather than exact-equality checks, so
        // untracked files (WT_NEW) and gitignored files (IGNORED) never block —
        // both mean "git has no committed version to protect" — and combined
        // flag sets (e.g. WT_NEW | IGNORED) are handled correctly.
        let tracked_dirty = status.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE
                // A conflicted target (worktree mid-merge) holds unmerged
                // content with no committed safety net — most losable of all.
                | git2::Status::CONFLICTED,
        );
        if !tracked_dirty {
            continue; // clean, untracked, or ignored — nothing of git's to lose
        }
        // Tracked + dirty: allow only the no-op case (bytes already match).
        let full = wt_root.join(rel);
        if full.is_file() && std::fs::read(&full)?.as_slice() == *proposed {
            continue;
        }
        return Err(AppError::Git(format!(
            "worktree has uncommitted changes to {rel}; commit or stash first"
        )));
    }
    Ok(())
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
fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), AppError> {
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
fn load_store(root: &Path) -> Result<ProfileStore, AppError> {
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
fn persist(root: &Path, store: &mut ProfileStore) -> Result<(), AppError> {
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
fn resolve_single_file_target(asset_id: &str) -> Result<&'static str, AppError> {
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

/// Blocking. Compute, WITHOUT WRITING, the per-target before/after for the named
/// profile's activation against the CALLING worktree's files. Thin wrapper over
/// `preview_profile_for_worktree` (P31 D5); non-repo dirs keep pure-P24
/// behavior on `workdir` directly.
pub fn preview_profile(
    workdir: &Path,
    name: &str,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn target(asset_id: &str, content: &str) -> ProfileTarget {
        ProfileTarget {
            asset_id: asset_id.to_string(),
            content: content.to_string(),
        }
    }

    fn profile(name: &str, targets: Vec<ProfileTarget>) -> ContextProfile {
        ContextProfile {
            name: name.to_string(),
            description: None,
            model: None,
            targets,
        }
    }

    // §11.1 row 6 — lazy default + persist + corrupt.
    #[test]
    fn list_profiles_lazy_default_creates_no_file() {
        let tmp = TempDir::new().unwrap();
        let store = list_profiles(tmp.path()).unwrap();
        assert_eq!(store.version, 2);
        assert!(store.profiles.is_empty());
        assert_eq!(store.active_profile, None);
        // No file / dir written by a read.
        assert!(!tmp.path().join(".bonsai").exists());
    }

    #[test]
    fn save_profile_creates_store_and_round_trips() {
        let tmp = TempDir::new().unwrap();
        let p = profile("opus", vec![target("claude", "# rich\n")]);
        let store = save_profile(tmp.path(), p.clone()).unwrap();
        assert_eq!(store.profiles.len(), 1);
        assert!(tmp.path().join(".bonsai").join("profiles.json").is_file());

        // Re-load round-trips the persisted profile.
        let reloaded = list_profiles(tmp.path()).unwrap();
        assert_eq!(reloaded.version, 2);
        assert_eq!(reloaded.profiles, vec![p]);

        // Upsert by name replaces in place (no duplicate).
        let updated = save_profile(
            tmp.path(),
            profile("opus", vec![target("claude", "# richer\n")]),
        )
        .unwrap();
        assert_eq!(updated.profiles.len(), 1);
        assert_eq!(updated.profiles[0].targets[0].content, "# richer\n");
    }

    #[test]
    fn corrupt_store_is_other_error() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".bonsai")).unwrap();
        std::fs::write(
            tmp.path().join(".bonsai").join("profiles.json"),
            b"{ not json",
        )
        .unwrap();
        let err = list_profiles(tmp.path()).unwrap_err();
        assert!(matches!(err, AppError::Other(m) if m.contains("corrupt")));
    }

    // §11.1 row 7 — save validation.
    #[test]
    fn blank_or_separator_name_rejected() {
        let tmp = TempDir::new().unwrap();
        for bad in ["", "   ", "-lead", "a/b", "a\\b", "a\tb"] {
            let err = save_profile(tmp.path(), profile(bad, vec![])).unwrap_err();
            assert!(
                matches!(err, AppError::InvalidName(_)),
                "name {bad:?} should be InvalidName"
            );
        }
    }

    #[test]
    fn non_single_file_target_rejected() {
        let tmp = TempDir::new().unwrap();
        // A rules-dir id and a config id are both invalid targets.
        for bad_id in ["cursorRules", "mcp", "claudeDir", "does-not-exist"] {
            let err =
                save_profile(tmp.path(), profile("p", vec![target(bad_id, "x")])).unwrap_err();
            assert!(
                matches!(err, AppError::InvalidName(_)),
                "target {bad_id:?} should be InvalidName"
            );
        }
    }

    // §11.1 row 8 — preview writes nothing.
    #[test]
    fn preview_reports_state_and_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), b"# old claude\n").unwrap();
        let claude_before = std::fs::read(tmp.path().join("CLAUDE.md")).unwrap();

        save_profile(
            tmp.path(),
            profile(
                "p",
                vec![
                    target("claude", "# new claude\n"),
                    target("agents", "# new agents\n"),
                ],
            ),
        )
        .unwrap();

        let preview = preview_profile(tmp.path(), "p").unwrap();
        assert_eq!(preview.len(), 2);

        let claude = &preview[0];
        assert_eq!(claude.asset_id, "claude");
        assert_eq!(claude.path, "CLAUDE.md");
        assert_eq!(claude.current.as_deref(), Some("# old claude\n"));
        assert_eq!(claude.proposed, "# new claude\n");
        assert!(claude.changed);

        let agents = &preview[1];
        assert_eq!(agents.path, "AGENTS.md");
        assert_eq!(agents.current, None, "missing file has no current");
        assert!(agents.changed, "missing file differs");

        // Nothing was written: existing file byte-identical, missing file absent.
        assert_eq!(
            std::fs::read(tmp.path().join("CLAUDE.md")).unwrap(),
            claude_before
        );
        assert!(!tmp.path().join("AGENTS.md").exists());
    }

    #[test]
    fn preview_unchanged_when_content_matches() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), b"# same\n").unwrap();
        save_profile(tmp.path(), profile("p", vec![target("claude", "# same\n")])).unwrap();
        let preview = preview_profile(tmp.path(), "p").unwrap();
        assert!(!preview[0].changed);
    }

    // §11.1 row 9 — activate: created / written / unchanged + atomicity + set active.
    #[test]
    fn activate_creates_writes_and_skips() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // AGENTS.md missing -> Created; CLAUDE.md differs -> Written;
        // GEMINI.md already equal -> Unchanged.
        std::fs::write(root.join("CLAUDE.md"), b"# old\n").unwrap();
        std::fs::write(root.join("GEMINI.md"), b"# gem\n").unwrap();

        save_profile(
            root,
            profile(
                "p",
                vec![
                    target("agents", "# agents body\n"),
                    target("claude", "# new claude\n"),
                    target("gemini", "# gem\n"),
                ],
            ),
        )
        .unwrap();

        let act = activate_profile(root, "p").unwrap();
        assert_eq!(act.profile, "p");
        let by_id = |id: &str| {
            act.results
                .iter()
                .find(|r| r.asset_id == id)
                .unwrap()
                .action
        };
        assert_eq!(by_id("agents"), TargetWriteAction::Created);
        assert_eq!(by_id("claude"), TargetWriteAction::Written);
        assert_eq!(by_id("gemini"), TargetWriteAction::Unchanged);

        // Files hold byte-exact content afterward.
        assert_eq!(std::fs::read(root.join("AGENTS.md")).unwrap(), b"# agents body\n");
        assert_eq!(std::fs::read(root.join("CLAUDE.md")).unwrap(), b"# new claude\n");

        // active_profile set + persisted.
        assert_eq!(act.store.active_profile.as_deref(), Some("p"));
        assert_eq!(
            list_profiles(root).unwrap().active_profile.as_deref(),
            Some("p")
        );

        // No .bonsai-tmp remnant beside any written file.
        assert!(!root.join("AGENTS.md.bonsai-tmp").exists());
        assert!(!root.join("CLAUDE.md.bonsai-tmp").exists());

        // A second identical activation is all Unchanged.
        let again = activate_profile(root, "p").unwrap();
        assert!(again
            .results
            .iter()
            .all(|r| r.action == TargetWriteAction::Unchanged));
    }

    #[test]
    fn activate_missing_profile_is_other() {
        let tmp = TempDir::new().unwrap();
        let err = activate_profile(tmp.path(), "nope").unwrap_err();
        assert!(matches!(err, AppError::Other(m) if m.contains("not found")));
    }

    #[test]
    fn delete_clears_active_profile() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), b"# a\n").unwrap();
        save_profile(tmp.path(), profile("p", vec![target("claude", "# b\n")])).unwrap();
        activate_profile(tmp.path(), "p").unwrap();
        let store = delete_profile(tmp.path(), "p").unwrap();
        assert!(store.profiles.is_empty());
        assert_eq!(store.active_profile, None);
        // Deleting an absent profile is a no-op Ok.
        let store2 = delete_profile(tmp.path(), "gone").unwrap();
        assert!(store2.profiles.is_empty());
    }

    // Path-escape defense: the static table is safe, but assert the guard rejects
    // `..` / absolute paths directly (belt-and-suspenders, §11.1 row 9).
    #[test]
    fn validate_rel_path_rejects_escapes() {
        assert!(validate_rel_path("../escape.md").is_err());
        assert!(validate_rel_path("/etc/passwd").is_err());
        assert!(validate_rel_path("C:/Windows/system32").is_err());
        assert!(validate_rel_path("a\\b").is_err());
        assert!(validate_rel_path("CLAUDE.md").is_ok());
    }

    // Wire-shape: camelCase keys + bare-string TargetWriteAction.
    #[test]
    fn wire_shapes_are_camel_case() {
        let tmp = TempDir::new().unwrap();
        save_profile(
            tmp.path(),
            ContextProfile {
                name: "opus".to_string(),
                description: Some("rich".to_string()),
                model: Some("opus".to_string()),
                targets: vec![target("claude", "# c\n")],
            },
        )
        .unwrap();
        let act = activate_profile(tmp.path(), "opus").unwrap();

        let store_v = serde_json::to_value(&act.store).unwrap();
        assert!(store_v.get("version").is_some());
        assert!(store_v.get("activeProfile").is_some());
        assert_eq!(store_v["profiles"][0]["name"], "opus");

        let act_v = serde_json::to_value(&act).unwrap();
        assert!(act_v.get("profile").is_some());
        let result = &act_v["results"][0];
        assert!(result.get("assetId").is_some());
        assert!(result.get("path").is_some());
        // Field-less enum → bare string.
        assert_eq!(result["action"], "created");
    }

    // ---------- P31: schema v2 migration + per-worktree activation ----------

    /// Scratch git fixture under D:\Temp\bonsai-scratch: main repo with a
    /// committed CLAUDE.md + two branches and two linked worktrees
    /// ("feature-x", "feature-y").
    fn git_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let dir = crate::testutil::scratch_dir();
        let repo_dir = dir.path().join("repo");
        let repo = git2::Repository::init(&repo_dir).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        std::fs::write(repo_dir.join("CLAUDE.md"), b"# base\n").unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(Path::new("CLAUDE.md")).unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        let head = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        let commit = repo.find_commit(head).unwrap();
        repo.branch("feature/x", &commit, false).unwrap();
        repo.branch("feature/y", &commit, false).unwrap();
        let wx = crate::git::worktree::add_worktree(&repo_dir, "feature/x", "feature/x").unwrap();
        let wy = crate::git::worktree::add_worktree(&repo_dir, "feature/y", "feature/y").unwrap();
        assert_eq!(wx.name, "feature-x");
        assert_eq!(wy.name, "feature-y");
        (
            dir,
            repo_dir,
            PathBuf::from(wx.abs_path),
            PathBuf::from(wy.abs_path),
        )
    }

    const V1_FIXTURE: &str = r##"{
  "version": 1,
  "profiles": [
    { "name": "opus", "targets": [ { "assetId": "claude", "content": "# opus\n" } ] }
  ],
  "activeProfile": "opus"
}"##;

    // §9.1 — migration: v1 loads unchanged, read is byte-safe, save stamps v2.
    #[test]
    fn v1_store_loads_byte_safe_and_migrates_on_save() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".bonsai").join("profiles.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, V1_FIXTURE.as_bytes()).unwrap();

        // Read: v1 parses, empty map, legacy active honored as "@main".
        let store = list_profiles(tmp.path()).unwrap();
        assert_eq!(store.version, 1);
        assert!(store.worktree_activations.is_empty());
        assert_eq!(store.active_profile.as_deref(), Some("opus"));
        assert_eq!(store.effective_activation(MAIN_WORKTREE_KEY), Some("opus"));
        // A pure read leaves the file BYTE-identical.
        assert_eq!(std::fs::read(&path).unwrap(), V1_FIXTURE.as_bytes());

        // First save stamps version 2 + materializes the "@main" mirror.
        let saved = save_profile(tmp.path(), profile("haiku", vec![])).unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(
            saved.worktree_activations.get(MAIN_WORKTREE_KEY).map(String::as_str),
            Some("opus")
        );
        assert_eq!(saved.active_profile.as_deref(), Some("opus"));

        // v2 round-trips.
        let reloaded = list_profiles(tmp.path()).unwrap();
        assert_eq!(reloaded, saved);
    }

    // §3 — delete_profile clears matching worktree-activation entries.
    #[test]
    fn delete_profile_clears_matching_map_entries() {
        let (_dir, main, _wx, _wy) = git_fixture();
        save_profile(&main, profile("p", vec![target("claude", "# p\n")])).unwrap();
        activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
        activate_profile_for_worktree(&main, MAIN_WORKTREE_KEY, "p").unwrap();
        let store = delete_profile(&main, "p").unwrap();
        assert!(store.worktree_activations.is_empty());
        assert_eq!(store.active_profile, None);
    }

    // §3 key hygiene — stale keys are GC'd on the next persist.
    #[test]
    fn persist_garbage_collects_stale_worktree_keys() {
        let (_dir, main, _wx, _wy) = git_fixture();
        save_profile(&main, profile("p", vec![])).unwrap();
        // Inject a stale key directly into the store file.
        let path = main.join(".bonsai").join("profiles.json");
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        v["worktreeActivations"] = serde_json::json!({ "ghost": "p", "feature-x": "p" });
        std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

        let store = save_profile(&main, profile("q", vec![])).unwrap();
        assert!(!store.worktree_activations.contains_key("ghost"), "stale key GC'd");
        assert_eq!(
            store.worktree_activations.get("feature-x").map(String::as_str),
            Some("p"),
            "live worktree key kept"
        );
    }

    // §9.2 — shared-store resolution from a linked worktree.
    #[test]
    fn linked_worktree_reads_and_writes_the_main_store() {
        let (_dir, main, wx, _wy) = git_fixture();
        assert_eq!(worktree_key_for(&main).unwrap(), MAIN_WORKTREE_KEY);
        assert_eq!(worktree_key_for(&wx).unwrap(), "feature-x");
        assert_eq!(resolve_store_root(&wx), resolve_store_root(&main));

        // save via the LINKED worktree lands in the MAIN store.
        save_profile(&wx, profile("p", vec![target("claude", "# p\n")])).unwrap();
        assert!(main.join(".bonsai").join("profiles.json").is_file());
        assert!(!wx.join(".bonsai").exists(), "no .bonsai in the linked worktree");
        assert_eq!(list_profiles(&wx).unwrap().profiles.len(), 1);
        assert_eq!(list_profiles(&main).unwrap().profiles.len(), 1);
    }

    // §9.3 — activation writes into THAT worktree only; map persisted;
    // second run idempotent.
    #[test]
    fn activate_into_linked_worktree_writes_only_there() {
        let (_dir, main, wx, wy) = git_fixture();
        save_profile(
            &main,
            profile(
                "p",
                vec![
                    target("claude", "# wt claude\n"),
                    target("agents", "# wt agents\n"),
                ],
            ),
        )
        .unwrap();
        let main_claude_before = std::fs::read(main.join("CLAUDE.md")).unwrap();
        let wy_claude_before = std::fs::read(wy.join("CLAUDE.md")).unwrap();

        let act = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
        assert_eq!(act.results.len(), 2);

        // Byte-exact writes INSIDE the linked worktree.
        assert_eq!(std::fs::read(wx.join("CLAUDE.md")).unwrap(), b"# wt claude\n");
        assert_eq!(std::fs::read(wx.join("AGENTS.md")).unwrap(), b"# wt agents\n");
        assert!(!wx.join("CLAUDE.md.bonsai-tmp").exists());
        assert!(!wx.join("AGENTS.md.bonsai-tmp").exists());
        // Main + sibling worktree untouched (byte-compare).
        assert_eq!(std::fs::read(main.join("CLAUDE.md")).unwrap(), main_claude_before);
        assert!(!main.join("AGENTS.md").exists());
        assert_eq!(std::fs::read(wy.join("CLAUDE.md")).unwrap(), wy_claude_before);
        assert!(!wy.join("AGENTS.md").exists());

        // Map persisted; legacy mirror NOT set (key != "@main").
        let store = list_profiles(&main).unwrap();
        assert_eq!(
            store.worktree_activations.get("feature-x").map(String::as_str),
            Some("p")
        );
        assert_eq!(store.active_profile, None);

        // Second identical run: all unchanged, nothing blocked.
        let again = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
        assert!(again
            .results
            .iter()
            .all(|r| r.action == TargetWriteAction::Unchanged));
    }

    // "@main" keying + legacy mirror via the D5 wrapper.
    #[test]
    fn legacy_activate_records_main_key_and_mirror() {
        let (_dir, main, wx, _wy) = git_fixture();
        save_profile(&main, profile("p", vec![target("agents", "# a\n")])).unwrap();
        // Wrapper from the MAIN worktree → "@main" + legacy mirror.
        let act = activate_profile(&main, "p").unwrap();
        assert_eq!(
            act.store.worktree_activations.get(MAIN_WORKTREE_KEY).map(String::as_str),
            Some("p")
        );
        assert_eq!(act.store.active_profile.as_deref(), Some("p"));
        // Wrapper from the LINKED worktree tab → records under its own key (D5).
        save_profile(&main, profile("q", vec![target("gemini", "# g\n")])).unwrap();
        let act2 = activate_profile(&wx, "q").unwrap();
        assert_eq!(
            act2.store.worktree_activations.get("feature-x").map(String::as_str),
            Some("q")
        );
        assert!(wx.join("GEMINI.md").is_file());
        assert!(!main.join("GEMINI.md").exists());
    }

    // §9.4 — D7 dirty-target guard: tracked+modified blocks BEFORE any write;
    // untracked does not block.
    #[test]
    fn dirty_tracked_target_blocks_all_writes() {
        let (_dir, main, wx, _wy) = git_fixture();
        // Target #1 (agents) is missing/clean; target #2 (claude) is tracked +
        // human-modified in the target worktree.
        std::fs::write(wx.join("CLAUDE.md"), b"# human edit\n").unwrap();
        save_profile(
            &main,
            profile(
                "p",
                vec![
                    target("agents", "# a\n"),
                    target("claude", "# machine\n"),
                ],
            ),
        )
        .unwrap();

        let err = activate_profile_for_worktree(&main, "feature-x", "p").unwrap_err();
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("uncommitted changes")),
            "expected dirty-target Git error, got {err:?}"
        );
        // ZERO files written: target #1 not created, target #2 byte-preserved.
        assert!(!wx.join("AGENTS.md").exists(), "all targets checked before any write");
        assert_eq!(std::fs::read(wx.join("CLAUDE.md")).unwrap(), b"# human edit\n");

        // UNTRACKED target file does NOT block (prior uncommitted activation).
        std::fs::write(wx.join("GEMINI.md"), b"# old untracked\n").unwrap();
        save_profile(&main, profile("q", vec![target("gemini", "# new\n")])).unwrap();
        let act = activate_profile_for_worktree(&main, "feature-x", "q").unwrap();
        assert_eq!(act.results[0].action, TargetWriteAction::Written);
        assert_eq!(std::fs::read(wx.join("GEMINI.md")).unwrap(), b"# new\n");
    }

    // Carry-forward (a): a GITIGNORED target file never blocks activation —
    // like untracked, git holds no committed version of it to protect.
    #[test]
    fn gitignored_target_does_not_block_activation() {
        let (_dir, main, wx, _wy) = git_fixture();
        // GEMINI.md is ignored in the target worktree and holds stale content.
        std::fs::write(wx.join(".gitignore"), b"GEMINI.md\n").unwrap();
        std::fs::write(wx.join("GEMINI.md"), b"# old ignored\n").unwrap();
        save_profile(&main, profile("p", vec![target("gemini", "# fresh\n")])).unwrap();

        let act = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
        assert_eq!(act.results[0].action, TargetWriteAction::Written);
        assert_eq!(std::fs::read(wx.join("GEMINI.md")).unwrap(), b"# fresh\n");
    }

    // Carry-forward (b): the D5 wrappers fall back to "@main" ONLY for
    // non-repo dirs. A real linked worktree whose identity cannot be resolved
    // propagates the error instead of silently retargeting MAIN.
    #[test]
    fn wrapper_propagates_identity_errors_for_real_repos() {
        let (_dir, main, wx, _wy) = git_fixture();
        save_profile(&main, profile("p", vec![target("gemini", "# g\n")])).unwrap();

        // Break feature-x's identity while keeping its repo openable: move the
        // admin dir OUT of `.git/worktrees/` and repoint the worktree's `.git`
        // file at it. The repo still opens (the gitdir layout is intact), but
        // `find_worktree(<basename>)` fails (not registered under worktrees/)
        // and the canonical-path fallback scan finds no registered worktrees.
        let admin_old = main.join(".git").join("worktrees").join("feature-x");
        let admin_new = main.join(".git").join("ghost");
        std::fs::rename(&admin_old, &admin_new).unwrap();
        std::fs::write(
            wx.join(".git"),
            format!("gitdir: {}\n", admin_new.display()),
        )
        .unwrap();

        // Precondition: the worktree still opens as a repo, but its identity
        // cannot be established.
        assert!(open_repo_at(&wx).is_ok(), "worktree repo must still open");
        assert!(worktree_key_for(&wx).is_err());

        let err = activate_profile(&wx, "p").unwrap_err();
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
        // Nothing was written anywhere, and no activation was recorded.
        assert!(!main.join("GEMINI.md").exists());
        assert!(!wx.join("GEMINI.md").exists());
        assert!(list_profiles(&main).unwrap().worktree_activations.is_empty());
        assert!(matches!(
            preview_profile(&wx, "p").unwrap_err(),
            AppError::Git(_)
        ));
    }

    // §9.5 — D6 eligibility: locked / invalid worktrees refuse preview AND activate.
    #[test]
    fn locked_and_invalid_worktrees_are_refused() {
        let (_dir, main, wx, _wy) = git_fixture();
        save_profile(&main, profile("p", vec![target("claude", "# p\n")])).unwrap();

        crate::git::worktree::lock_worktree(&main, "feature-y", Some("pinned")).unwrap();
        for res in [
            preview_profile_for_worktree(&main, "feature-y", "p").map(|_| ()),
            activate_profile_for_worktree(&main, "feature-y", "p").map(|_| ()),
        ] {
            match res {
                Err(AppError::Git(m)) => assert!(m.contains("locked"), "got: {m}"),
                other => panic!("expected locked refusal, got {other:?}"),
            }
        }

        // Invalid: delete the linked worktree's working directory.
        std::fs::remove_dir_all(&wx).unwrap();
        for res in [
            preview_profile_for_worktree(&main, "feature-x", "p").map(|_| ()),
            activate_profile_for_worktree(&main, "feature-x", "p").map(|_| ()),
        ] {
            match res {
                Err(AppError::Git(_)) => {}
                other => panic!("expected invalid/prunable refusal, got {other:?}"),
            }
        }

        // Unknown worktree key → precise Git error.
        match activate_profile_for_worktree(&main, "nope", "p") {
            Err(AppError::Git(m)) => assert!(m.contains("not found")),
            other => panic!("expected not-found, got {other:?}"),
        }
    }

    // §9.7 — every written path stays under the target worktree root.
    #[test]
    fn written_paths_are_contained_in_the_target_worktree() {
        let (_dir, main, wx, _wy) = git_fixture();
        save_profile(
            &main,
            profile("p", vec![target("agents", "# a\n"), target("gemini", "# g\n")]),
        )
        .unwrap();
        let act = activate_profile_for_worktree(&main, "feature-x", "p").unwrap();
        for r in &act.results {
            validate_rel_path(&r.path).unwrap();
            let full = wx.join(&r.path);
            assert!(full.starts_with(&wx), "{} escapes the worktree", r.path);
            assert!(full.is_file());
        }
    }
}
