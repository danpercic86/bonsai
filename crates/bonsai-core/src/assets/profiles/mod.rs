//! Context-profile store: CRUD + preview + activate (P24 contract §5).
//!
//! A profile is a named bundle of `(assetId, content)` targets. Activating one
//! writes each target's verbatim content to its mapped single-file instruction
//! doc (atomic temp + rename, parent dirs created), gated behind the UI's
//! confirm + diff preview. The store lives at `<workdir>/.bonsai/profiles.json`
//! and is created lazily on first save. Pure filesystem + serde — no Tauri, no
//! git repo needed; the command layer wraps every function in `spawn_blocking`.

mod activate;
mod store;
mod worktree;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_worktree;

pub use activate::{
    activate_profile, activate_profile_for_worktree, preview_profile, preview_profile_for_worktree,
};
pub use store::{
    delete_profile, list_profiles, resolve_store_root, save_profile, validate_profile_name,
};
pub use worktree::worktree_key_for;

use std::collections::BTreeMap;

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
