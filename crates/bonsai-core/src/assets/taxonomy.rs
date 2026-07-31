//! Static descriptor table of known AI-asset files/dirs (P24 contract §2).
//!
//! Pure data + accessors — no I/O, no git2. `inventory.rs` resolves each
//! descriptor against a workdir; `drift.rs` uses the drift-comparable subset.
//! The table order is the UI display order.

/// Kind of asset a descriptor points at. Wire: a bare camelCase string
/// (`singleFile` / `rulesDir` / `config`) — a field-less serde enum, NOT tagged
/// (P24 §6.2 correction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    SingleFile,
    RulesDir,
    Config,
}

/// One row of the taxonomy: a known AI-asset target and its metadata. Paths are
/// repo-relative, forward-slash. For `RulesDir`, `path` is the directory and
/// `glob` selects members; otherwise `glob` is ignored.
#[derive(Debug, Clone, Copy)]
pub struct AssetDescriptor {
    /// Stable slug used as the wire `id` and as a `ProfileTarget.assetId`.
    pub id: &'static str,
    /// Tool/agent this serves (wire `agent`).
    pub agent: &'static str,
    /// Human label for the UI.
    pub label: &'static str,
    pub kind: AssetKind,
    /// Repo-relative location: file path (SingleFile/Config) or directory
    /// (RulesDir).
    pub path: &'static str,
    /// Member glob for RulesDir (e.g. `"*.mdc"`); ignored otherwise.
    pub glob: Option<&'static str>,
    /// true => the UI groups it under "managed"; false => detect-only.
    pub managed: bool,
    /// true => carries tool-specific frontmatter or is a dir; excluded from the
    /// sync comparison (see `comparable`).
    pub frontmatter: bool,
}

impl AssetDescriptor {
    /// Drift-comparable predicate (§2 / §4.3): a managed single-file instruction
    /// doc without tool-specific frontmatter.
    pub fn comparable(&self) -> bool {
        self.managed && !self.frontmatter && matches!(self.kind, AssetKind::SingleFile)
    }
}

/// The full ordered descriptor table (§2). Order = display order.
static DESCRIPTORS: &[AssetDescriptor] = &[
    AssetDescriptor {
        id: "claude",
        agent: "Claude Code",
        label: "CLAUDE.md",
        kind: AssetKind::SingleFile,
        path: "CLAUDE.md",
        glob: None,
        managed: true,
        frontmatter: false,
    },
    AssetDescriptor {
        id: "agents",
        agent: "Codex/Cursor/Gemini/Zed",
        label: "AGENTS.md",
        kind: AssetKind::SingleFile,
        path: "AGENTS.md",
        glob: None,
        managed: true,
        frontmatter: false,
    },
    AssetDescriptor {
        id: "copilot",
        agent: "GitHub Copilot",
        label: "copilot-instructions.md",
        kind: AssetKind::SingleFile,
        path: ".github/copilot-instructions.md",
        glob: None,
        managed: true,
        frontmatter: false,
    },
    AssetDescriptor {
        id: "gemini",
        agent: "Gemini CLI",
        label: "GEMINI.md",
        kind: AssetKind::SingleFile,
        path: "GEMINI.md",
        glob: None,
        managed: true,
        frontmatter: false,
    },
    AssetDescriptor {
        id: "windsurf",
        agent: "Windsurf (legacy)",
        label: ".windsurfrules",
        kind: AssetKind::SingleFile,
        path: ".windsurfrules",
        glob: None,
        managed: true,
        frontmatter: false,
    },
    AssetDescriptor {
        id: "cursorLegacy",
        agent: "Cursor (legacy)",
        label: ".cursorrules",
        kind: AssetKind::SingleFile,
        path: ".cursorrules",
        glob: None,
        managed: true,
        frontmatter: false,
    },
    AssetDescriptor {
        id: "cursorRules",
        agent: "Cursor",
        label: ".cursor/rules/",
        kind: AssetKind::RulesDir,
        path: ".cursor/rules",
        glob: Some("*.mdc"),
        managed: true,
        frontmatter: true,
    },
    AssetDescriptor {
        id: "windsurfRules",
        agent: "Windsurf",
        label: ".windsurf/rules/",
        kind: AssetKind::RulesDir,
        path: ".windsurf/rules",
        glob: Some("*.md"),
        managed: true,
        frontmatter: true,
    },
    AssetDescriptor {
        id: "copilotInstr",
        agent: "GitHub Copilot",
        label: ".github/instructions/",
        kind: AssetKind::RulesDir,
        path: ".github/instructions",
        glob: Some("*.instructions.md"),
        managed: false,
        frontmatter: true,
    },
    AssetDescriptor {
        id: "copilotPrompts",
        agent: "GitHub Copilot",
        label: ".github/prompts/",
        kind: AssetKind::RulesDir,
        path: ".github/prompts",
        glob: Some("*.prompt.md"),
        managed: false,
        frontmatter: true,
    },
    AssetDescriptor {
        id: "claudeDir",
        agent: "Claude Code",
        label: ".claude/ (skills/agents/commands)",
        kind: AssetKind::Config,
        path: ".claude",
        glob: None,
        managed: false,
        frontmatter: true,
    },
    AssetDescriptor {
        id: "mcp",
        agent: "MCP clients",
        label: ".mcp.json",
        kind: AssetKind::Config,
        path: ".mcp.json",
        glob: None,
        managed: false,
        frontmatter: false,
    },
];

/// The full ordered descriptor table.
pub fn descriptors() -> &'static [AssetDescriptor] {
    DESCRIPTORS
}

/// Look up a descriptor by its stable id.
pub fn descriptor(id: &str) -> Option<&'static AssetDescriptor> {
    DESCRIPTORS.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparable_set_is_the_six_single_files() {
        let ids: Vec<&str> = descriptors()
            .iter()
            .filter(|d| d.comparable())
            .map(|d| d.id)
            .collect();
        assert_eq!(
            ids,
            ["claude", "agents", "copilot", "gemini", "windsurf", "cursorLegacy"]
        );
    }

    #[test]
    fn descriptor_lookup_round_trips() {
        assert_eq!(descriptor("mcp").map(|d| d.kind), Some(AssetKind::Config));
        assert!(descriptor("nope").is_none());
    }
}
