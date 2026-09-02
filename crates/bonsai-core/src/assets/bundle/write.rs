//! Write / delete (§5) — atomic temp+rename, mirroring `profiles.rs`.

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::stage::validate_rel_path;

use super::read::scan_agent_assets;
use super::spec::{full_path, rel_path};
use super::validate::validate_asset_name;
use super::{
    parse_frontmatter, serialize_asset, AgentAssetInput, AgentAssetInventory, AgentAssetKind,
};

/// Sibling temp path `<file>.bonsai-tmp` for an atomic write (mirrors the
/// `profiles.rs` idiom — the temp lands in the SAME dir so `rename` is atomic).
fn tmp_sibling(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".bonsai-tmp");
    target.with_file_name(name)
}

/// Atomically replace `target` with `bytes`: write a sibling temp file, then
/// rename over the target (rename is atomic + replaces on both platforms). On a
/// rename failure the temp is best-effort removed so no `.bonsai-tmp` remnant is
/// left. The caller must ensure `target`'s parent dir exists.
fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let tmp = tmp_sibling(target);
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Blocking. Create or overwrite the asset described by `input` (§5). Validates
/// the name (§4.4 → `InvalidName`) and the computed rel path stays in-workdir
/// (`validate_rel_path` → `Other`, belt-and-suspenders); creates parent dirs
/// (incl. the skill's `<name>/` dir); serializes (§4) + writes atomically
/// (temp+rename). Returns the FRESH full inventory so the frontend re-selects the
/// saved asset by (kind, name).
///
/// Validation warnings/required-field errors do NOT block the write (§5, §11
/// row 9): a save with a missing required field still writes and the returned
/// inventory flags it `valid:false` (recomputed by the re-scan). A "complex"
/// payload cannot arrive — `AgentAssetInput.frontmatter` is a flat list.
pub fn save_agent_asset(
    workdir: &Path,
    input: AgentAssetInput,
) -> Result<AgentAssetInventory, AppError> {
    validate_asset_name(&input.name)?;
    let rel = rel_path(input.kind, &input.name);
    // Belt-and-suspenders: the name charset already precludes escapes.
    validate_rel_path(&rel)?;

    let full = full_path(workdir, input.kind, &input.name);

    // Structural fail-safe (SHOULD-FIX): never overwrite an existing file whose
    // on-disk frontmatter is "complex" (multi-line/sequence/nested YAML the flat
    // parser can't round-trip). Rebuilding from the editor's flat fields would
    // silently drop that YAML. The backend is the authoritative barrier here, so
    // a frontend guard failure (e.g. drifted Error text) can never cause a lossy
    // rewrite. Creating a NEW asset, or overwriting a FLAT one, is unaffected.
    if full.is_file() {
        let existing = std::fs::read(&full)?;
        let raw = String::from_utf8_lossy(&existing);
        let (_, _, complex) = parse_frontmatter(&raw);
        if complex {
            return Err(AppError::Other(
                "cannot overwrite complex YAML frontmatter from the editor — edit this file directly"
                    .to_string(),
            ));
        }
    }

    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serialize_asset(&input.frontmatter, &input.body);
    atomic_write(&full, bytes.as_bytes())?;

    scan_agent_assets(workdir)
}

/// Blocking. Delete one asset by `(kind, name)` (§5). The name is validated
/// (§4.4) + the computed rel path re-checked first. **Skill → remove the whole
/// `.claude/skills/<name>/` directory recursively** (a skill IS its directory:
/// SKILL.md + any supporting files — §8/OPEN-2; the UI confirm spells this out);
/// **agent/command → remove the single `.md` file**. A missing target is a no-op
/// `Ok`. Returns the fresh inventory. Every path is static-prefixed under
/// `.claude/` in `workdir`, so nothing outside it is reachable.
pub fn delete_agent_asset(
    workdir: &Path,
    kind: AgentAssetKind,
    name: &str,
) -> Result<AgentAssetInventory, AppError> {
    validate_asset_name(name)?;
    let rel = rel_path(kind, name);
    validate_rel_path(&rel)?;

    match kind {
        AgentAssetKind::Skill => {
            let dir = workdir.join(".claude").join("skills").join(name);
            if dir.is_dir() {
                std::fs::remove_dir_all(&dir)?;
            }
        }
        AgentAssetKind::Agent | AgentAssetKind::Command => {
            let full = full_path(workdir, kind, name);
            if full.is_file() {
                std::fs::remove_file(&full)?;
            }
        }
    }

    scan_agent_assets(workdir)
}
