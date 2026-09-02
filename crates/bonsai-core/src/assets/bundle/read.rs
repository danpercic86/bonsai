//! Read / scan (§3).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::validate_rel_path;

use super::spec::{full_path, kind_ord, rel_path};
use super::validate::{error_issue, validate, validate_asset_name};
use super::{parse_frontmatter, AgentAsset, AgentAssetInventory, AgentAssetKind, Validation};

/// Read + parse + validate the file at `full` into an `AgentAsset` (existing).
fn load_asset(
    kind: AgentAssetKind,
    name: &str,
    rel: String,
    full: &Path,
) -> Result<AgentAsset, AppError> {
    let bytes = std::fs::read(full)?;
    let raw = String::from_utf8_lossy(&bytes);
    let (frontmatter, body, complex) = parse_frontmatter(&raw);
    let validation = validate(kind, name, &frontmatter, &body, complex);
    Ok(AgentAsset {
        kind,
        name: name.to_string(),
        path: rel,
        exists: true,
        frontmatter,
        body,
        complex,
        validation,
    })
}

/// Scan the skill dirs (`.claude/skills/<name>/SKILL.md`, direct children only).
/// A skill dir without `SKILL.md` is skipped.
fn scan_skills(workdir: &Path) -> Result<Vec<AgentAsset>, AppError> {
    let dir = workdir.join(".claude").join("skills");
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.is_empty() {
            continue;
        }
        let full = entry.path().join("SKILL.md");
        if !full.is_file() {
            continue;
        }
        let rel = rel_path(AgentAssetKind::Skill, &name);
        out.push(load_asset(AgentAssetKind::Skill, &name, rel, &full)?);
    }
    Ok(out)
}

/// Scan a `.claude/<subdir>` of `<name>.md` files (direct children only).
/// Non-`.md` files are ignored.
fn scan_md_dir(
    workdir: &Path,
    kind: AgentAssetKind,
    subdir: &str,
) -> Result<Vec<AgentAsset>, AppError> {
    let dir = workdir.join(".claude").join(subdir);
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        let Some(name) = fname.strip_suffix(".md") else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let rel = rel_path(kind, name);
        out.push(load_asset(kind, name, rel, &entry.path())?);
    }
    Ok(out)
}

/// Blocking. Scan `.claude/{skills,agents,commands}` under `workdir`, parse +
/// validate each, sorted by (kind order skill<agent<command, then name). Only
/// direct children are considered; a missing `.claude/` (or any sub-dir) yields
/// an empty group, not an error. Never touches anything outside `.claude/`.
pub fn scan_agent_assets(workdir: &Path) -> Result<AgentAssetInventory, AppError> {
    let mut assets = scan_skills(workdir)?;
    assets.extend(scan_md_dir(workdir, AgentAssetKind::Agent, "agents")?);
    assets.extend(scan_md_dir(workdir, AgentAssetKind::Command, "commands")?);
    assets.sort_by(|a, b| {
        kind_ord(a.kind)
            .cmp(&kind_ord(b.kind))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(AgentAssetInventory { assets })
}

/// Blocking. Read + parse + validate one asset by `(kind, name)`. The name is
/// validated (§4.4) first. A missing file yields an `exists:false` shell with
/// empty frontmatter/body and validation `valid:false` (issue "file does not
/// exist") — NOT an error (§3).
pub fn read_agent_asset(
    workdir: &Path,
    kind: AgentAssetKind,
    name: &str,
) -> Result<AgentAsset, AppError> {
    validate_asset_name(name)?;
    let rel = rel_path(kind, name);
    // Belt-and-suspenders: the static prefix + name check already guarantee this.
    validate_rel_path(&rel)?;
    let full = full_path(workdir, kind, name);
    if !full.is_file() {
        return Ok(AgentAsset {
            kind,
            name: name.to_string(),
            path: rel,
            exists: false,
            frontmatter: Vec::new(),
            body: String::new(),
            complex: false,
            validation: Validation {
                valid: false,
                issues: vec![error_issue("file does not exist")],
            },
        });
    }
    load_asset(kind, name, rel, &full)
}
