//! Context-profile store: CRUD + preview + activate (P24 contract §5).
//!
//! A profile is a named bundle of `(assetId, content)` targets. Activating one
//! writes each target's verbatim content to its mapped single-file instruction
//! doc (atomic temp + rename, parent dirs created), gated behind the UI's
//! confirm + diff preview. The store lives at `<workdir>/.bonsai/profiles.json`
//! and is created lazily on first save. Pure filesystem + serde — no Tauri, no
//! git repo needed; the command layer wraps every function in `spawn_blocking`.

use std::path::{Path, PathBuf};

use crate::assets::taxonomy::{descriptor, AssetKind};
use crate::error::AppError;
use crate::git::stage::validate_rel_path;

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
    /// Schema version; current = 1. Forward-compatible: unknown-higher versions
    /// still load (serde ignores unknown fields) but the UI may warn.
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<ContextProfile>,
    /// Name of the last activated profile, or None. Informational.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
}

impl ProfileStore {
    /// The lazy default returned when no store file exists.
    fn empty() -> Self {
        ProfileStore {
            version: 1,
            profiles: Vec::new(),
            active_profile: None,
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

/// Repo-relative store location (§5.1).
fn store_path(workdir: &Path) -> PathBuf {
    workdir.join(".bonsai").join("profiles.json")
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
    std::fs::rename(&tmp, target)?;
    Ok(())
}

/// Blocking. Load the store, or the lazy empty default if the file / `.bonsai/`
/// dir is absent. Malformed JSON → `Other("profiles.json is corrupt: …")`.
pub fn list_profiles(workdir: &Path) -> Result<ProfileStore, AppError> {
    let path = store_path(workdir);
    if !path.is_file() {
        return Ok(ProfileStore::empty());
    }
    let bytes = std::fs::read(&path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Other(format!("profiles.json is corrupt: {e}")))
}

/// Persist the store: create `.bonsai/`, stamp `version = 1`, write pretty JSON
/// atomically (temp + rename). Mutates `store.version` to 1 so the returned
/// value matches disk.
fn persist(workdir: &Path, store: &mut ProfileStore) -> Result<(), AppError> {
    store.version = 1;
    let dir = workdir.join(".bonsai");
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(store)
        .map_err(|e| AppError::Other(format!("failed to serialize profiles.json: {e}")))?;
    atomic_write(&store_path(workdir), &json)
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
    let mut store = list_profiles(workdir)?;
    if let Some(existing) = store.profiles.iter_mut().find(|p| p.name == profile.name) {
        *existing = profile;
    } else {
        store.profiles.push(profile);
    }
    persist(workdir, &mut store)?;
    Ok(store)
}

/// Blocking. Remove the profile named `name` (no-op if absent), clear
/// `active_profile` if it pointed there, persist. Returns the updated store.
pub fn delete_profile(workdir: &Path, name: &str) -> Result<ProfileStore, AppError> {
    let mut store = list_profiles(workdir)?;
    store.profiles.retain(|p| p.name != name);
    if store.active_profile.as_deref() == Some(name) {
        store.active_profile = None;
    }
    persist(workdir, &mut store)?;
    Ok(store)
}

/// Blocking. Compute, WITHOUT WRITING, the per-target before/after for the named
/// profile's activation. `current` = existing mapped-file content (None if
/// absent); `changed` = byte inequality (a missing file differs).
pub fn preview_profile(
    workdir: &Path,
    name: &str,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let store = list_profiles(workdir)?;
    let profile = store
        .profiles
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| AppError::Other(format!("profile '{name}' not found")))?;

    let mut entries = Vec::with_capacity(profile.targets.len());
    for t in &profile.targets {
        let rel = resolve_single_file_target(&t.asset_id)?;
        let full = workdir.join(rel);
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

/// Blocking. WRITE each target's content to its mapped file (atomic temp+rename,
/// parent dirs created), set `active_profile = name`, persist the store. Returns
/// a per-target summary. This is the ONLY write-to-instruction-files path; the
/// UI gates it behind confirm + preview (§8.3).
///
/// All targets are validated before any write. On an I/O error mid-loop the
/// targets already written are left as-is (atomic per file, not transactional
/// across files).
pub fn activate_profile(workdir: &Path, name: &str) -> Result<ProfileActivation, AppError> {
    let mut store = list_profiles(workdir)?;
    let profile = store
        .profiles
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| AppError::Other(format!("profile '{name}' not found")))?;

    // Validate ALL targets first (descriptor is SingleFile + path is in-workdir).
    let mut mapped = Vec::with_capacity(profile.targets.len());
    for t in &profile.targets {
        let rel = resolve_single_file_target(&t.asset_id)?;
        mapped.push((t, rel));
    }

    let mut results = Vec::with_capacity(mapped.len());
    for (t, rel) in mapped {
        let full = workdir.join(rel);
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

    store.active_profile = Some(name.to_string());
    persist(workdir, &mut store)?;
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
        assert_eq!(store.version, 1);
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
        assert_eq!(reloaded.version, 1);
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
}
