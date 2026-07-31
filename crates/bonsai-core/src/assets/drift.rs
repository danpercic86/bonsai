//! Drift report over the comparable single-file instruction docs (P24 §4.1/§4.3).
//!
//! Pure computation over an already-scanned inventory: pick a canonical
//! reference (auto by priority, or a user override), then flag each comparable
//! asset as in-sync iff its normalized hash equals the canonical's.

use crate::assets::inventory::AiAsset;
use crate::assets::taxonomy::descriptors;

/// Auto-pick priority for the canonical reference (§4.3 / OPEN #1).
const PRIORITY: [&str; 6] = [
    "claude",
    "agents",
    "copilot",
    "gemini",
    "windsurf",
    "cursorLegacy",
];

/// Drift verdict for the comparable set, returned inside `AiAssetInventory`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    /// The reference asset id, or None if no comparable single-file exists.
    pub canonical_id: Option<String>,
    /// Normalized hash of the canonical, or None.
    pub canonical_hash: Option<String>,
    /// One entry per drift-comparable descriptor (the §2 set), in table order.
    pub entries: Vec<DriftEntry>,
    /// true iff every EXISTING comparable asset is in sync (0/1 existing => true).
    pub in_sync: bool,
}

/// Per-asset drift status (§4.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftEntry {
    pub asset_id: String,
    pub exists: bool,
    /// false => not compared (missing, or outside the comparable set).
    pub comparable: bool,
    /// Normalized hash when comparable && exists, else None.
    pub normalized_hash: Option<String>,
    /// true iff comparable && exists && normalized_hash == canonical_hash.
    pub in_sync: bool,
}

/// Computes the `DriftReport` from a scanned inventory (§4.3). `canonical`: an
/// optional override asset id, honored only if it is comparable AND exists;
/// otherwise the priority auto-pick applies.
pub fn compute_drift(assets: &[AiAsset], canonical: Option<&str>) -> DriftReport {
    // Comparable descriptor ids, in table order.
    let comparable_ids: Vec<&'static str> = descriptors()
        .iter()
        .filter(|d| d.comparable())
        .map(|d| d.id)
        .collect();

    let lookup = |id: &str| assets.iter().find(|a| a.id == id);
    let exists = |id: &str| lookup(id).map(|a| a.exists).unwrap_or(false);
    let nhash = |id: &str| {
        lookup(id)
            .and_then(|a| a.files.first())
            .map(|f| f.normalized_hash.clone())
    };
    let is_comparable = |id: &str| comparable_ids.contains(&id);

    // Canonical selection: honored override → priority auto-pick → None.
    let canonical_id: Option<String> = match canonical {
        Some(c) if is_comparable(c) && exists(c) => Some(c.to_string()),
        _ => {
            let mut found = None;
            for id in PRIORITY {
                if is_comparable(id) && exists(id) {
                    found = Some(id.to_string());
                    break;
                }
            }
            found
        }
    };
    let canonical_hash = canonical_id.as_deref().and_then(nhash);

    let mut entries = Vec::with_capacity(comparable_ids.len());
    for id in &comparable_ids {
        let present = exists(id);
        let normalized_hash = if present { nhash(id) } else { None };
        let in_sync = present && canonical_hash.is_some() && normalized_hash == canonical_hash;
        entries.push(DriftEntry {
            asset_id: id.to_string(),
            exists: present,
            comparable: true,
            normalized_hash,
            in_sync,
        });
    }

    // Every EXISTING comparable entry must be in sync (0/1 existing => true).
    let in_sync = entries.iter().filter(|e| e.exists).all(|e| e.in_sync);

    DriftReport {
        canonical_id,
        canonical_hash,
        entries,
        in_sync,
    }
}

#[cfg(test)]
mod tests {
    use crate::assets::inventory::scan_inventory;
    use std::path::Path;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, bytes).unwrap();
    }

    // §11.1 row 4 — EOL-only difference is still in sync.
    #[test]
    fn eol_only_difference_is_in_sync() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "CLAUDE.md", b"# Title\nbody\n");
        write(tmp.path(), "AGENTS.md", b"# Title\r\nbody\r\n");
        let d = scan_inventory(tmp.path(), None).unwrap().drift;
        assert_eq!(d.canonical_id.as_deref(), Some("claude"));
        let agents = d.entries.iter().find(|e| e.asset_id == "agents").unwrap();
        assert!(agents.in_sync);
        assert!(d.in_sync);
    }

    // §11.1 row 4 — a one-word change drifts.
    #[test]
    fn one_word_change_drifts() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "CLAUDE.md", b"# Title\nbody\n");
        write(tmp.path(), "AGENTS.md", b"# Title\nBODY\n");
        let d = scan_inventory(tmp.path(), None).unwrap().drift;
        let agents = d.entries.iter().find(|e| e.asset_id == "agents").unwrap();
        let claude = d.entries.iter().find(|e| e.asset_id == "claude").unwrap();
        assert!(!agents.in_sync);
        assert!(claude.in_sync);
        assert!(!d.in_sync);
    }

    // §11.1 row 4 — canonical override flips the reference.
    #[test]
    fn canonical_override_flips_reference() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "CLAUDE.md", b"# Title\nA\n");
        write(tmp.path(), "AGENTS.md", b"# Title\nB\n");

        let auto = scan_inventory(tmp.path(), None).unwrap().drift;
        assert_eq!(auto.canonical_id.as_deref(), Some("claude"));

        let overridden = scan_inventory(tmp.path(), Some("agents")).unwrap().drift;
        assert_eq!(overridden.canonical_id.as_deref(), Some("agents"));
        assert_ne!(auto.canonical_hash, overridden.canonical_hash);

        let agents = overridden
            .entries
            .iter()
            .find(|e| e.asset_id == "agents")
            .unwrap();
        let claude = overridden
            .entries
            .iter()
            .find(|e| e.asset_id == "claude")
            .unwrap();
        assert!(agents.in_sync);
        assert!(!claude.in_sync);
    }

    // §11.1 row 4 — an override that doesn't exist falls back to priority.
    #[test]
    fn override_missing_falls_back_to_priority() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "CLAUDE.md", b"# Title\nA\n");
        let d = scan_inventory(tmp.path(), Some("agents")).unwrap().drift;
        assert_eq!(d.canonical_id.as_deref(), Some("claude"));
    }

    // §11.1 row 4 — no comparable file → canonicalId None, in_sync true.
    #[test]
    fn no_comparable_file_yields_none() {
        let tmp = TempDir::new().unwrap();
        // Only a non-comparable (detect-only) asset present.
        write(tmp.path(), ".mcp.json", b"{}\n");
        let d = scan_inventory(tmp.path(), None).unwrap().drift;
        assert_eq!(d.canonical_id, None);
        assert_eq!(d.canonical_hash, None);
        assert!(d.in_sync);
    }
}
