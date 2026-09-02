//! Agent-asset bundle: the managed CRUD surface over the three `.claude/`
//! agent-asset kinds — **skills** (`.claude/skills/<name>/SKILL.md`),
//! **subagents** (`.claude/agents/<name>.md`), and **slash commands**
//! (`.claude/commands/<name>.md`) — as specified by P26 contract §3/§4.
//!
//! Named `bundle` (the ".claude/ bundle") rather than `agents` to avoid clashing
//! with the "agent" *kind*. Pure filesystem + a minimal hand-rolled frontmatter
//! splitter — no Tauri, no git repo, no `serde_yaml`, no `claude` CLI. Every
//! function is blocking; the command layer wraps them in `spawn_blocking`.
//!
//! Sub-increment P26a covers the read path: types, the per-kind spec, frontmatter
//! parse/serialize, name + content validation, and `scan_agent_assets` /
//! `read_agent_asset`. The write path (`save`/`delete`) lands in P26b.
//!
//! **Frontmatter round-trip:** for an asset whose fence contains only flat
//! `key: value` lines (no comments, blank lines, or multi-line YAML),
//! `parse_frontmatter → serialize_asset → parse_frontmatter` is a fixed point
//! (fields identical incl. unknown keys; body identical modulo one trailing
//! `\n`). Comments and blank lines inside the fence are DROPPED on re-serialize
//! (documented, acceptable loss). Any multi-line / sequence / nested / block
//! frontmatter is DETECTED (`complex = true`) and preserved read-only via a
//! validation Error — never silently rewritten (§4.3).

mod frontmatter;
mod read;
mod spec;
mod validate;
mod write;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_write;

pub use frontmatter::{parse_frontmatter, serialize_asset};
pub use read::{read_agent_asset, scan_agent_assets};
pub use spec::{known_optional_keys, rel_path, required_keys};
pub use validate::validate_asset_name;
pub use write::{delete_agent_asset, save_agent_asset};

/// Which `.claude/` agent-asset kind. Wire: bare camelCase string
/// (`"skill" | "agent" | "command"`) — a field-less enum, NOT tagged. Used both
/// as a serialized field AND as a command argument, so it needs Deserialize too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentAssetKind {
    /// `.claude/skills/<name>/SKILL.md`
    Skill,
    /// `.claude/agents/<name>.md`
    Agent,
    /// `.claude/commands/<name>.md`
    Command,
}

/// One frontmatter entry, preserving insertion order and unknown keys. `value`
/// is the verbatim opaque scalar text after `key: ` (§4).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterField {
    pub key: String,
    pub value: String,
}

/// Severity of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueSeverity {
    Error,
    Warning,
}

/// One validation finding for an asset.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIssue {
    pub severity: IssueSeverity,
    pub message: String,
}

/// Validation verdict for one asset. `valid == issues have NO Error severity`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub valid: bool,
    pub issues: Vec<AssetIssue>,
}

/// One parsed agent asset (read/inventory result). Serialize only — `validation`
/// is server-computed and never sent back on save.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAsset {
    pub kind: AgentAssetKind,
    /// Directory name (skill) or file stem (agent/command).
    pub name: String,
    /// Repo-relative file path, forward slashes (e.g. `.claude/agents/foo.md`).
    pub path: String,
    pub exists: bool,
    /// Parsed flat frontmatter, in file order, unknown keys preserved (§4).
    pub frontmatter: Vec<FrontmatterField>,
    /// Everything after the closing `---` fence (verbatim); whole file if no
    /// fence.
    pub body: String,
    /// `true` when the frontmatter uses multi-line / sequence / nested YAML the
    /// flat parser cannot round-trip (§4.3). The authoritative, structural signal
    /// the editor uses to open the asset read-only — a save that ignored it would
    /// silently drop the un-parsed YAML, so `save_agent_asset` also re-guards on
    /// this against the on-disk file.
    pub complex: bool,
    pub validation: Validation,
}

/// Full managed inventory of the three kinds, returned in one round-trip. Flat
/// list (UI groups by `kind`), sorted by (kind order skill<agent<command, name).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssetInventory {
    pub assets: Vec<AgentAsset>,
}

/// The write payload for `save_agent_asset` (§3). No `path`/`exists`/`validation`
/// — those are derived/computed by the backend. Deserialize only (comes off the
/// wire from the editor). `frontmatter` is a flat ordered list, so a "complex"
/// (multi-line YAML) payload cannot arrive; the editor keeps values single-line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssetInput {
    pub kind: AgentAssetKind,
    pub name: String,
    pub frontmatter: Vec<FrontmatterField>,
    pub body: String,
}
