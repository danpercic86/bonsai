//! Per-kind spec (§3.1) — path layout + known/required frontmatter keys.

use std::path::{Path, PathBuf};

use super::AgentAssetKind;

/// Kind ordinal for the deterministic inventory sort (skill < agent < command).
pub(super) fn kind_ord(kind: AgentAssetKind) -> u8 {
    match kind {
        AgentAssetKind::Skill => 0,
        AgentAssetKind::Agent => 1,
        AgentAssetKind::Command => 2,
    }
}

/// Lowercase label used in validation messages (`"skill" | "agent" | "command"`).
pub(super) fn kind_label(kind: AgentAssetKind) -> &'static str {
    match kind {
        AgentAssetKind::Skill => "skill",
        AgentAssetKind::Agent => "agent",
        AgentAssetKind::Command => "command",
    }
}

/// Required frontmatter keys per kind (§3.1). Only subagents require fields
/// (`name`, `description`); skills and commands have none.
pub fn required_keys(kind: AgentAssetKind) -> &'static [&'static str] {
    match kind {
        AgentAssetKind::Agent => &["name", "description"],
        AgentAssetKind::Skill | AgentAssetKind::Command => &[],
    }
}

/// Known-optional frontmatter keys per kind (§3.1) — the fields the P26c editor
/// surfaces as first-class inputs. Not enforced here; exposed so the form layer
/// and future validation share one source of truth.
pub fn known_optional_keys(kind: AgentAssetKind) -> &'static [&'static str] {
    match kind {
        AgentAssetKind::Skill => &[
            "name",
            "description",
            "argument-hint",
            "allowed-tools",
            "model",
            "disable-model-invocation",
        ],
        AgentAssetKind::Agent => &["tools", "model"],
        AgentAssetKind::Command => &[
            "description",
            "argument-hint",
            "allowed-tools",
            "model",
            "disable-model-invocation",
        ],
    }
}

/// Repo-relative path (forward slashes) for `(kind, name)` (§3.1).
pub fn rel_path(kind: AgentAssetKind, name: &str) -> String {
    match kind {
        AgentAssetKind::Skill => format!(".claude/skills/{name}/SKILL.md"),
        AgentAssetKind::Agent => format!(".claude/agents/{name}.md"),
        AgentAssetKind::Command => format!(".claude/commands/{name}.md"),
    }
}

/// Absolute on-disk path for `(kind, name)` under `workdir` (joins avoid mixing
/// separators on Windows).
pub(super) fn full_path(workdir: &Path, kind: AgentAssetKind, name: &str) -> PathBuf {
    let claude = workdir.join(".claude");
    match kind {
        AgentAssetKind::Skill => claude.join("skills").join(name).join("SKILL.md"),
        AgentAssetKind::Agent => claude.join("agents").join(format!("{name}.md")),
        AgentAssetKind::Command => claude.join("commands").join(format!("{name}.md")),
    }
}
