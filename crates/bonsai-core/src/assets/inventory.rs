//! Inventory scan + content hashing + normalization (P24 contract §3, §4.2).
//!
//! Pure filesystem read + git-blob hashing — no Tauri, no git repo needed. Walks
//! the workdir once per taxonomy descriptor, hashing existing files (raw +
//! normalized), and never touches anything outside `workdir`. Blocking; the
//! command layer wraps it in `spawn_blocking`.

use std::path::Path;

use crate::assets::drift::{compute_drift, DriftReport};
use crate::assets::taxonomy::{descriptors, AssetDescriptor, AssetKind};
use crate::error::AppError;
use crate::git::stage::validate_rel_path;

/// One concrete file inside an asset (the single file for SingleFile/Config, or
/// each matched member for RulesDir). Paths are repo-relative, forward slashes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFile {
    pub path: String,
    /// Byte length of the raw file.
    pub size: u64,
    /// git-blob SHA-1 (40 hex) of the RAW bytes.
    pub content_hash: String,
    /// git-blob SHA-1 (40 hex) of the NORMALIZED content (§4.2).
    pub normalized_hash: String,
    /// mtime, epoch seconds; None if unavailable.
    pub modified: Option<i64>,
}

/// One detected AI-asset target (a taxonomy descriptor resolved against the repo).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAsset {
    pub id: String,
    pub agent: String,
    pub label: String,
    pub kind: AssetKind,
    /// File or dir path, repo-relative, forward slashes.
    pub path: String,
    pub managed: bool,
    pub exists: bool,
    /// SingleFile/Config: 0 or 1 entry. RulesDir: 0..N matched members (sorted by path).
    pub files: Vec<AssetFile>,
}

/// Full inventory + drift, returned by `list_ai_assets` in one round-trip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAssetInventory {
    pub assets: Vec<AiAsset>,
    pub drift: DriftReport,
}

/// Raw content of one asset file (editor/preview). `content` is None only when
/// the file is absent; existing files are lossy-decoded (§3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetContent {
    pub path: String,
    pub exists: bool,
    pub content: Option<String>,
}

/// Blocking. Scan `workdir` for every taxonomy descriptor, hashing existing
/// files. `canonical`: optional override for the drift reference asset id
/// (§4.3); `None` => auto-pick. Never touches anything outside `workdir`.
pub fn scan_inventory(
    workdir: &Path,
    canonical: Option<&str>,
) -> Result<AiAssetInventory, AppError> {
    let mut assets = Vec::with_capacity(descriptors().len());
    for d in descriptors() {
        assets.push(resolve_descriptor(workdir, d)?);
    }
    let drift = compute_drift(&assets, canonical);
    Ok(AiAssetInventory { assets, drift })
}

/// Resolves one descriptor against the workdir into an `AiAsset`.
fn resolve_descriptor(workdir: &Path, d: &AssetDescriptor) -> Result<AiAsset, AppError> {
    let full = workdir.join(d.path);
    let (exists, files) = match d.kind {
        AssetKind::SingleFile => {
            if full.is_file() {
                (true, vec![build_asset_file(&full, d.path)?])
            } else {
                (false, Vec::new())
            }
        }
        AssetKind::Config => {
            // Config may be a file (`.mcp.json`) or a directory (`.claude`).
            if full.is_file() {
                (true, vec![build_asset_file(&full, d.path)?])
            } else if full.exists() {
                (true, Vec::new())
            } else {
                (false, Vec::new())
            }
        }
        AssetKind::RulesDir => {
            if full.is_dir() {
                (true, scan_dir_members(&full, d)?)
            } else {
                (false, Vec::new())
            }
        }
    };
    Ok(AiAsset {
        id: d.id.to_string(),
        agent: d.agent.to_string(),
        label: d.label.to_string(),
        kind: d.kind,
        path: d.path.to_string(),
        managed: d.managed,
        exists,
        files,
    })
}

/// Lists the glob-matched member files (flat, non-recursive) of a RulesDir,
/// sorted by repo-relative path.
fn scan_dir_members(dir_full: &Path, d: &AssetDescriptor) -> Result<Vec<AssetFile>, AppError> {
    let mut members = Vec::new();
    for entry in std::fs::read_dir(dir_full)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(glob) = d.glob {
            if !glob_match(glob, &name) {
                continue;
            }
        }
        let rel = format!("{}/{name}", d.path);
        members.push(build_asset_file(&entry.path(), &rel)?);
    }
    members.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(members)
}

/// Minimal glob matcher for the taxonomy patterns (`*.<ext>` suffix globs). A
/// leading `*` matches any prefix; otherwise the pattern must equal the name.
fn glob_match(pattern: &str, name: &str) -> bool {
    match pattern.strip_prefix('*') {
        Some(suffix) => name.ends_with(suffix),
        None => pattern == name,
    }
}

/// Reads a file and builds its `AssetFile` (raw + normalized git-blob hashes).
fn build_asset_file(full: &Path, rel: &str) -> Result<AssetFile, AppError> {
    let bytes = std::fs::read(full)?;
    let meta = std::fs::metadata(full)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    let content_hash = blob_hash(&bytes)?;
    let normalized_hash = blob_hash(normalize(&bytes).as_bytes())?;
    Ok(AssetFile {
        path: rel.to_string(),
        size: meta.len(),
        content_hash,
        normalized_hash,
        modified,
    })
}

/// git-blob SHA-1 (40-hex) of `bytes`, matching `git hash-object` (§0).
fn blob_hash(bytes: &[u8]) -> Result<String, AppError> {
    Ok(git2::Oid::hash_object(git2::ObjectType::Blob, bytes)?.to_string())
}

/// Normalizes raw bytes of a comparable single-file asset (LOCKED rules, §4.2):
/// lossy-decode, strip a leading BOM, fold CRLF/CR to LF, right-trim each line,
/// trim leading/trailing blank lines, then ensure exactly one trailing `\n`
/// (empty stays empty). No lowercasing, no reflow, no internal blank collapsing.
pub fn normalize(raw: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(raw);
    let mut s: &str = decoded.as_ref();
    if let Some(stripped) = s.strip_prefix('\u{FEFF}') {
        s = stripped;
    }
    let unified = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = unified
        .split('\n')
        .map(|l| l.trim_end_matches([' ', '\t']))
        .collect();
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut body = lines.join("\n");
    body.push('\n');
    body
}

/// Blocking. Read one asset FILE's raw content (a repo-relative path validated
/// inside `workdir`). Returns the lossy-decoded content or `exists:false`.
/// Any in-workdir path is allowed to read (defensive path validation only).
pub fn read_asset(workdir: &Path, path: &str) -> Result<AssetContent, AppError> {
    validate_rel_path(path)?;
    let full = workdir.join(path);
    if !full.is_file() {
        return Ok(AssetContent {
            path: path.to_string(),
            exists: false,
            content: None,
        });
    }
    let bytes = std::fs::read(&full)?;
    Ok(AssetContent {
        path: path.to_string(),
        exists: true,
        content: Some(String::from_utf8_lossy(&bytes).into_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::taxonomy::descriptors;
    use std::path::Path;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, bytes).unwrap();
    }

    // §11.1 row 1 — empty repo.
    #[test]
    fn empty_repo_all_absent_no_drift() {
        let tmp = TempDir::new().unwrap();
        let inv = scan_inventory(tmp.path(), None).unwrap();
        assert_eq!(inv.assets.len(), descriptors().len());
        assert!(inv.assets.iter().all(|a| !a.exists && a.files.is_empty()));
        assert_eq!(inv.drift.canonical_id, None);
        assert!(inv.drift.in_sync);
        assert!(inv
            .drift
            .entries
            .iter()
            .all(|e| e.comparable && !e.exists && !e.in_sync));
    }

    // §11.1 row 2 — inventory + hashing against a git2 oracle.
    #[test]
    fn inventory_hashes_match_git_oracle() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let claude_bytes = b"# Claude\ninstructions\n";
        write(root, "CLAUDE.md", claude_bytes);
        write(root, "AGENTS.md", b"# Agents\n");
        write(root, ".cursor/rules/b.mdc", b"B rule\n");
        write(root, ".cursor/rules/a.mdc", b"A rule\n");
        // A non-matching member must be ignored by the glob.
        write(root, ".cursor/rules/notes.txt", b"ignore me\n");

        let inv = scan_inventory(root, None).unwrap();

        let claude = inv.assets.iter().find(|a| a.id == "claude").unwrap();
        assert!(claude.exists);
        assert_eq!(claude.files.len(), 1);
        assert_eq!(claude.files[0].size, claude_bytes.len() as u64);
        let oracle = git2::Oid::hash_object(git2::ObjectType::Blob, claude_bytes)
            .unwrap()
            .to_string();
        assert_eq!(claude.files[0].content_hash, oracle);

        let cursor = inv.assets.iter().find(|a| a.id == "cursorRules").unwrap();
        assert!(cursor.exists);
        assert_eq!(cursor.files.len(), 2, "only *.mdc members counted");
        assert_eq!(cursor.files[0].path, ".cursor/rules/a.mdc");
        assert_eq!(cursor.files[1].path, ".cursor/rules/b.mdc");

        let gemini = inv.assets.iter().find(|a| a.id == "gemini").unwrap();
        assert!(!gemini.exists);
        assert!(gemini.files.is_empty());
    }

    // §11.1 row 3 — normalization table.
    #[test]
    fn normalize_folds_eol_bom_trailing_and_edge_blanks() {
        let canonical = "line one\nline two\n";
        let variants: &[&[u8]] = &[
            b"line one\r\nline two\r\n",            // CRLF
            b"\xEF\xBB\xBFline one\nline two\n",     // leading BOM
            b"line one   \nline two\t\n",           // trailing whitespace
            b"\n\nline one\nline two\n\n\n",         // edge blank lines
            b"line one\nline two",                   // missing final newline
        ];
        for v in variants {
            assert_eq!(normalize(v), canonical, "variant {v:?}");
        }
        // A genuine content change does NOT collapse.
        assert_ne!(normalize(b"line one\nline THREE\n"), canonical);
        // Byte-exact behavior from the contract.
        assert_eq!(normalize(b"x\r\n\r\n"), "x\n");
        assert_eq!(normalize(b""), "");
        assert_eq!(normalize(b"\n\n\n"), "");
    }

    // §11.1 row 5 — wire shapes: camelCase keys + bare-string AssetKind.
    #[test]
    fn wire_shapes_are_camel_case() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "CLAUDE.md", b"hello\n");
        write(tmp.path(), ".cursor/rules/a.mdc", b"rule\n");
        write(tmp.path(), ".mcp.json", b"{}\n");
        let inv = scan_inventory(tmp.path(), None).unwrap();
        let v = serde_json::to_value(&inv).unwrap();

        assert!(v.get("assets").is_some() && v.get("drift").is_some());
        let assets = v["assets"].as_array().unwrap();
        let by_id = |id: &str| assets.iter().find(|a| a["id"] == id).unwrap();

        let claude = by_id("claude");
        assert_eq!(claude["kind"], "singleFile");
        let file = &claude["files"][0];
        assert!(file.get("contentHash").is_some());
        assert!(file.get("normalizedHash").is_some());
        assert!(file.get("size").is_some());

        assert_eq!(by_id("cursorRules")["kind"], "rulesDir");
        assert_eq!(by_id("mcp")["kind"], "config");

        let drift = &v["drift"];
        assert!(drift.get("canonicalId").is_some());
        assert!(drift.get("canonicalHash").is_some());
        assert!(drift.get("inSync").is_some());
        let entry = &drift["entries"][0];
        assert!(entry.get("assetId").is_some());
        assert!(entry.get("inSync").is_some());
        assert!(entry.get("normalizedHash").is_some());
    }

    #[test]
    fn read_asset_returns_content_and_absence() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "CLAUDE.md", b"# hi\n");
        let present = read_asset(tmp.path(), "CLAUDE.md").unwrap();
        assert!(present.exists);
        assert_eq!(present.content.as_deref(), Some("# hi\n"));

        let absent = read_asset(tmp.path(), "GEMINI.md").unwrap();
        assert!(!absent.exists);
        assert_eq!(absent.content, None);

        // Defensive path validation reuses stage.rs (rejects `..`).
        assert!(read_asset(tmp.path(), "../escape.md").is_err());
    }
}
