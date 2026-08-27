//! Identity-profile data types for [`crate::settings`] (P44/P82).
//!
//! Split out of `settings.rs` (spec-006 pass) to keep that module under the
//! ~500-line limit. Re-exported from the `settings` module so external call
//! sites are unchanged.

/// Curated identity-profile color (P82). Closed named palette — maps to a
/// theme-aware CSS token in the frontend (see P82-ui.md); no raw hex on the wire.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ProfileColor {
    #[default]
    Neutral,
    Slate,
    Blue,
    Teal,
    Green,
    Amber,
    Orange,
    Purple,
    Pink,
}

/// One named identity profile (P44). Global app setting; applied to a repo's
/// Local git config on demand. `id` is a stable frontend-generated UUID.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProfile {
    /// Stable id (frontend-generated `crypto.randomUUID()`); never reused.
    pub id: String,
    /// Display label, e.g. "Work". Empty/duplicate allowed but discouraged
    /// (frontend soft-validates non-empty).
    pub label: String,
    pub user_name: String,
    pub user_email: String,
    /// Optional `user.signingkey`. None/empty ⇒ not written on apply.
    pub signing_key: Option<String>,
    /// P82: display color. Additive field-level `#[serde(default)]` — a pre-P82
    /// profile (no `color` key) deserializes as `ProfileColor::Neutral`. The
    /// container-level `default` on `Settings` does NOT cover a missing field on a
    /// `Vec` element, so this must be field-level. Display-only; never written to
    /// git config on apply.
    #[serde(default)]
    pub color: ProfileColor,
}
