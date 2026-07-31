//! AI-asset management core (P24 contract §1). Pure filesystem + git-blob
//! hashing — no Tauri, no git repo needed. Inventories the instruction files
//! every agent reads (`CLAUDE.md`, `AGENTS.md`, Cursor rules, Copilot
//! instructions, …), hashes them raw + normalized, and reports drift between
//! the comparable single-file instruction docs. All functions are blocking; the
//! command layer wraps them in `spawn_blocking`.
//!
//! Sub-increment P24a covers taxonomy + inventory + drift + read. Profiles
//! (P24b, `profiles.rs`) and the optional AI helper (P24e, `generate.rs` — the
//! only part that touches the `claude` CLI) land in later passes.

pub mod drift;
pub mod generate;
pub mod inventory;
pub mod profiles;
pub mod taxonomy;

pub use drift::{compute_drift, DriftEntry, DriftReport};
pub use generate::{generate_asset, AiGeneratedAsset};
pub use inventory::{
    normalize, read_asset, scan_inventory, AiAsset, AiAssetInventory, AssetContent, AssetFile,
};
pub use profiles::{
    activate_profile, delete_profile, list_profiles, preview_profile, save_profile,
    validate_profile_name, ContextProfile, ProfileActivation, ProfilePreviewEntry, ProfileStore,
    ProfileTarget, TargetWriteAction, TargetWriteResult,
};
pub use taxonomy::{descriptor, descriptors, AssetDescriptor, AssetKind};
