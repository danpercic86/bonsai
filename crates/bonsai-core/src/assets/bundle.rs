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

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::stage::validate_rel_path;

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
    pub validation: Validation,
}

/// Full managed inventory of the three kinds, returned in one round-trip. Flat
/// list (UI groups by `kind`), sorted by (kind order skill<agent<command, name).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssetInventory {
    pub assets: Vec<AgentAsset>,
}

// ---------------------------------------------------------------------------
// Per-kind spec (§3.1) — path layout + known/required frontmatter keys.
// ---------------------------------------------------------------------------

/// Kind ordinal for the deterministic inventory sort (skill < agent < command).
fn kind_ord(kind: AgentAssetKind) -> u8 {
    match kind {
        AgentAssetKind::Skill => 0,
        AgentAssetKind::Agent => 1,
        AgentAssetKind::Command => 2,
    }
}

/// Lowercase label used in validation messages (`"skill" | "agent" | "command"`).
fn kind_label(kind: AgentAssetKind) -> &'static str {
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
fn full_path(workdir: &Path, kind: AgentAssetKind, name: &str) -> PathBuf {
    let claude = workdir.join(".claude");
    match kind {
        AgentAssetKind::Skill => claude.join("skills").join(name).join("SKILL.md"),
        AgentAssetKind::Agent => claude.join("agents").join(format!("{name}.md")),
        AgentAssetKind::Command => claude.join("commands").join(format!("{name}.md")),
    }
}

// ---------------------------------------------------------------------------
// Frontmatter parse / serialize (§4).
// ---------------------------------------------------------------------------

/// Parse a `---`-delimited frontmatter fence at the very top of `raw` (§4.1).
///
/// Returns `(fields, body, complex)`:
/// - `fields`: flat `key: value` entries in file order (unknown keys preserved).
/// - `body`: everything after the closing fence's newline, verbatim; the whole
///   file when there is no (well-formed) fence.
/// - `complex`: `true` if any fence line is not a flat `key: scalar` (sequences,
///   nested maps, block scalars, indented lines) — the asset becomes read-only
///   (§4.3). Blank lines and `#` comments inside the fence are dropped.
pub fn parse_frontmatter(raw: &str) -> (Vec<FrontmatterField>, String, bool) {
    // Strip a leading UTF-8 BOM; normalize fence checks on `\n` (accept `\r\n`).
    let content = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);

    let first_end = content.find('\n');
    let first_line_raw = match first_end {
        Some(i) => &content[..i],
        None => content,
    };
    let first_line = first_line_raw.strip_suffix('\r').unwrap_or(first_line_raw);
    if first_line != "---" {
        // No opening fence (common for slash commands).
        return (Vec::new(), content.to_string(), false);
    }
    let Some(first_nl) = first_end else {
        // Bare `---` with no newline: no closing fence -> treat as no frontmatter.
        return (Vec::new(), content.to_string(), false);
    };

    let after_open = &content[first_nl + 1..];
    let mut fields = Vec::new();
    let mut complex = false;
    let mut body_start = None;
    let mut pos = 0usize;
    for seg in after_open.split_inclusive('\n') {
        let line = seg.strip_suffix('\n').unwrap_or(seg);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == "---" {
            body_start = Some(pos + seg.len());
            break;
        }
        parse_fm_line(line, &mut fields, &mut complex);
        pos += seg.len();
    }

    match body_start {
        Some(bs) => (fields, after_open[bs..].to_string(), complex),
        // No closing fence -> not frontmatter; the whole file is the body.
        None => (Vec::new(), content.to_string(), false),
    }
}

/// Parse a single fence line into a field, or flag `complex` (§4.1 step 4).
fn parse_fm_line(line: &str, fields: &mut Vec<FrontmatterField>, complex: &mut bool) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        // Blank line or comment -> dropped (documented loss).
        return;
    }
    match parse_flat_kv(line) {
        Some((key, value)) if !is_block_scalar_indicator(&value) => {
            fields.push(FrontmatterField { key, value });
        }
        // A block-scalar indicator (`key: |`, `key: >`) OR any non-flat line
        // (`- item`, ` nested: x`, `key:value`) -> multi-line YAML we cannot
        // round-trip. Flag and skip.
        _ => *complex = true,
    }
}

/// Match `^([A-Za-z0-9_.-]+):(?: (.*))?$`: a flat `key:` or `key: value` line.
/// Returns `(key, value)`; `value` is verbatim text after `": "` (empty for a
/// bare `key:`). `None` for anything else (indented, sequence, `key:value`).
fn parse_flat_kv(line: &str) -> Option<(String, String)> {
    let colon = line.find(':')?;
    let key = &line[..colon];
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return None;
    }
    let rest = &line[colon + 1..];
    let value = if rest.is_empty() {
        String::new()
    } else {
        // `key:value` (no space after the colon) is not the flat inline form
        // -> `strip_prefix` returns None -> `?` bails to a complex/non-field line.
        rest.strip_prefix(' ')?.to_string()
    };
    Some((key.to_string(), value))
}

/// A YAML block-scalar indicator value (`|`, `>`, `|-`, `>+`, `|2`, …) signals
/// a multi-line scalar this editor cannot round-trip -> treat as complex.
fn is_block_scalar_indicator(value: &str) -> bool {
    let v = value.trim();
    let mut chars = v.chars();
    match chars.next() {
        Some('|') | Some('>') => chars.all(|c| matches!(c, '+' | '-') || c.is_ascii_digit()),
        _ => false,
    }
}

/// Serialize flat `frontmatter` + `body` back to file bytes (§4.2). Empty
/// frontmatter -> body only, no fence. Values are written verbatim (opaque
/// scalars, no auto-quoting). The ONLY normalization is ensuring the output ends
/// with exactly one trailing `\n` (appended if missing; interior whitespace is
/// untouched).
pub fn serialize_asset(frontmatter: &[FrontmatterField], body: &str) -> String {
    let mut out = String::new();
    if !frontmatter.is_empty() {
        out.push_str("---\n");
        for f in frontmatter {
            if f.value.is_empty() {
                out.push_str(&f.key);
                out.push_str(":\n");
            } else {
                out.push_str(&f.key);
                out.push_str(": ");
                out.push_str(&f.value);
                out.push('\n');
            }
        }
        out.push_str("---\n");
    }
    out.push_str(body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Validation (§4.4, §4.5).
// ---------------------------------------------------------------------------

/// Validate an asset `name` for filesystem safety (§4.4). Rejects blank/`.`/`..`,
/// a leading `-`, any of `/ \ :`, control chars, or any char outside
/// `[A-Za-z0-9._-]` -> `InvalidName`. (This charset makes a path separator or a
/// `..` component impossible.) "Not lowercase-hyphen" is NOT rejected here — it
/// is only a Warning in `validate` (§4.5).
pub fn validate_asset_name(name: &str) -> Result<(), AppError> {
    let bad = name.trim().is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('-')
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.chars().any(char::is_control)
        || name
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')));
    if bad {
        return Err(AppError::InvalidName(format!("invalid asset name: '{name}'")));
    }
    Ok(())
}

/// `^[a-z0-9][a-z0-9-]*$` — the recommended lowercase-hyphen id charset (§4.5).
fn is_lowercase_hyphen(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn error_issue(message: impl Into<String>) -> AssetIssue {
    AssetIssue {
        severity: IssueSeverity::Error,
        message: message.into(),
    }
}

fn warning_issue(message: impl Into<String>) -> AssetIssue {
    AssetIssue {
        severity: IssueSeverity::Warning,
        message: message.into(),
    }
}

/// Validate one asset's content (§4.5). `complex` propagates from
/// `parse_frontmatter`. `valid == no Error-severity issue`.
fn validate(
    kind: AgentAssetKind,
    name: &str,
    fields: &[FrontmatterField],
    body: &str,
    complex: bool,
) -> Validation {
    let mut issues = Vec::new();

    if complex {
        issues.push(error_issue(
            "frontmatter uses multi-line YAML this editor can't safely round-trip — edit the file directly",
        ));
    }

    for key in required_keys(kind) {
        let present = fields
            .iter()
            .any(|f| f.key == *key && !f.value.trim().is_empty());
        if !present {
            issues.push(error_issue(format!(
                "{} requires frontmatter field '{}'",
                kind_label(kind),
                key
            )));
        }
    }

    if !is_lowercase_hyphen(name) {
        issues.push(warning_issue(
            "name should be lowercase letters, digits, and hyphens",
        ));
    }

    if matches!(kind, AgentAssetKind::Skill | AgentAssetKind::Agent) {
        if let Some(f) = fields.iter().find(|f| f.key == "name") {
            if !f.value.is_empty() && f.value != name {
                issues.push(warning_issue(format!(
                    "frontmatter name '{}' differs from the file name '{}'",
                    f.value, name
                )));
            }
        }
    }

    if matches!(kind, AgentAssetKind::Skill | AgentAssetKind::Command) && body.trim().is_empty() {
        issues.push(warning_issue("body is empty — nothing will run"));
    }

    let valid = !issues.iter().any(|i| i.severity == IssueSeverity::Error);
    Validation { valid, issues }
}

// ---------------------------------------------------------------------------
// Read / scan (§3).
// ---------------------------------------------------------------------------

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
            validation: Validation {
                valid: false,
                issues: vec![error_issue("file does not exist")],
            },
        });
    }
    load_asset(kind, name, rel, &full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, bytes).unwrap();
    }

    // §11 row 1 — empty / absent `.claude/` -> empty inventory (not an error).
    #[test]
    fn scan_empty_returns_no_assets() {
        let tmp = TempDir::new().unwrap();
        let inv = scan_agent_assets(tmp.path()).unwrap();
        assert!(inv.assets.is_empty());
        // A `.claude/` with no agent-asset sub-dirs is still empty.
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        assert!(scan_agent_assets(tmp.path()).unwrap().assets.is_empty());
    }

    // §11 row 2 — scan all kinds, sorted + filtered.
    #[test]
    fn scan_all_kinds_sorted_and_filtered() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write(
            root,
            ".claude/skills/code-review/SKILL.md",
            b"---\nname: code-review\ndescription: Reviews code\n---\n\n# Code review\n",
        );
        write(
            root,
            ".claude/agents/test-runner.md",
            b"---\nname: test-runner\ndescription: Runs tests\ntools: Bash\nmodel: inherit\n---\n\nYou run tests.\n",
        );
        write(
            root,
            ".claude/commands/changelog.md",
            b"---\ndescription: Update changelog\nargument-hint: <version>\n---\n\nUpdate for $ARGUMENTS.\n",
        );
        // A skill dir WITHOUT SKILL.md is skipped.
        std::fs::create_dir_all(root.join(".claude/skills/empty-skill")).unwrap();
        // A stray non-.md file in commands is ignored.
        write(root, ".claude/commands/notes.txt", b"ignore me\n");

        let inv = scan_agent_assets(root).unwrap();
        assert_eq!(inv.assets.len(), 3, "3 assets, empty-skill + notes.txt skipped");

        // Sort order: skill < agent < command, then name.
        assert_eq!(inv.assets[0].kind, AgentAssetKind::Skill);
        assert_eq!(inv.assets[0].name, "code-review");
        assert_eq!(inv.assets[0].path, ".claude/skills/code-review/SKILL.md");
        assert_eq!(inv.assets[1].kind, AgentAssetKind::Agent);
        assert_eq!(inv.assets[1].name, "test-runner");
        assert_eq!(inv.assets[1].path, ".claude/agents/test-runner.md");
        assert_eq!(inv.assets[2].kind, AgentAssetKind::Command);
        assert_eq!(inv.assets[2].name, "changelog");
        assert_eq!(inv.assets[2].path, ".claude/commands/changelog.md");

        // Parsed frontmatter + body of the agent.
        let agent = &inv.assets[1];
        assert_eq!(
            agent
                .frontmatter
                .iter()
                .map(|f| (f.key.as_str(), f.value.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("name", "test-runner"),
                ("description", "Runs tests"),
                ("tools", "Bash"),
                ("model", "inherit"),
            ]
        );
        assert_eq!(agent.body, "\nYou run tests.\n");
        assert!(agent.validation.valid, "complete agent is valid");
        assert!(inv.assets.iter().all(|a| a.exists));
    }

    // §11 row 3 — parse: order + unknown keys preserved; no-fence + unterminated.
    #[test]
    fn parse_preserves_order_and_unknown_keys() {
        let raw = "---\nname: foo\ncolor: blue\ndescription: hi\n---\nbody line\n";
        let (fields, body, complex) = parse_frontmatter(raw);
        assert!(!complex);
        assert_eq!(
            fields
                .iter()
                .map(|f| (f.key.as_str(), f.value.as_str()))
                .collect::<Vec<_>>(),
            vec![("name", "foo"), ("color", "blue"), ("description", "hi")]
        );
        assert_eq!(body, "body line\n", "body is verbatim after the fence");

        // No fence at all (common for commands) -> body is the whole file.
        let (f2, b2, c2) = parse_frontmatter("Just a prompt body.\n");
        assert!(f2.is_empty() && !c2);
        assert_eq!(b2, "Just a prompt body.\n");

        // Opening fence with no closing `---` -> not frontmatter.
        let (f3, b3, c3) = parse_frontmatter("---\nname: foo\nno close here\n");
        assert!(f3.is_empty() && !c3);
        assert_eq!(b3, "---\nname: foo\nno close here\n");

        // A bare `key:` line yields an empty value; `key: value` keeps the value.
        let (f4, _, _) = parse_frontmatter("---\nmodel:\ntools: Read, Write\n---\n");
        assert_eq!(f4[0], FrontmatterField { key: "model".into(), value: String::new() });
        assert_eq!(f4[1].value, "Read, Write");
    }

    // §11 row 4 — round-trip is a fixed point for flat frontmatter.
    #[test]
    fn round_trip_flat_frontmatter_is_fixed_point() {
        let raw = "---\nname: foo\ncolor: blue\ndescription: hi\n---\n\nBody text.\n";
        let (fields, body, complex) = parse_frontmatter(raw);
        assert!(!complex);
        let serialized = serialize_asset(&fields, &body);
        // The frontmatter block is byte-stable (canonical `key: value` lines).
        assert_eq!(serialized, raw);
        // Re-parse yields identical fields + body.
        let (fields2, body2, _) = parse_frontmatter(&serialized);
        assert_eq!(fields, fields2);
        assert_eq!(body, body2);

        // Empty frontmatter -> body only, no fence, one trailing newline.
        assert_eq!(serialize_asset(&[], "prompt"), "prompt\n");
        assert_eq!(serialize_asset(&[], "prompt\n"), "prompt\n");
        // A bare `key:` (empty value) serializes without a trailing space.
        let empty_val = vec![FrontmatterField { key: "model".into(), value: String::new() }];
        assert_eq!(serialize_asset(&empty_val, "b\n"), "---\nmodel:\n---\nb\n");
    }

    // §11 row 5 — complex-frontmatter detection.
    #[test]
    fn complex_frontmatter_is_detected_and_errors() {
        // A sequence line under `tools:`.
        let raw = "---\ntools:\n  - Read\n  - Write\n---\nbody\n";
        let (_fields, _body, complex) = parse_frontmatter(raw);
        assert!(complex, "indented sequence items are complex");

        // A top-level `- item` line.
        assert!(parse_frontmatter("---\n- item\n---\n").2);
        // A block-scalar indicator value.
        assert!(parse_frontmatter("---\ndescription: |\n  multi\n---\n").2);
        // But a literal pipe inside a value is NOT a block scalar.
        assert!(!parse_frontmatter("---\ncmd: a | b\n---\n").2);

        // Validation surfaces the complex Error -> invalid.
        let v = validate(AgentAssetKind::Agent, "x", &[], "body", true);
        assert!(!v.valid);
        assert!(v
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error && i.message.contains("multi-line YAML")));
    }

    // §11 row 5 — required/recommended/lowercase-hyphen/name-mismatch validation.
    #[test]
    fn validate_required_and_warning_rules() {
        // Agent missing `description` -> Error, invalid.
        let fields = vec![FrontmatterField { key: "name".into(), value: "test-runner".into() }];
        let v = validate(AgentAssetKind::Agent, "test-runner", &fields, "body", false);
        assert!(!v.valid);
        assert!(v.issues.iter().any(
            |i| i.severity == IssueSeverity::Error && i.message.contains("requires frontmatter field 'description'")
        ));

        // Agent with both required -> valid.
        let full = vec![
            FrontmatterField { key: "name".into(), value: "test-runner".into() },
            FrontmatterField { key: "description".into(), value: "runs".into() },
        ];
        assert!(validate(AgentAssetKind::Agent, "test-runner", &full, "b", false).valid);

        // Skill missing description -> valid (recommended, not required).
        let sv = validate(AgentAssetKind::Skill, "code-review", &[], "body", false);
        assert!(sv.valid);

        // Command with no frontmatter + body -> valid.
        assert!(validate(AgentAssetKind::Command, "changelog", &[], "run it", false).valid);

        // `name: Foo_Bar` -> lowercase-hyphen Warning, still valid.
        let warn = validate(AgentAssetKind::Command, "Foo_Bar", &[], "b", false);
        assert!(warn.valid);
        assert!(warn.issues.iter().any(
            |i| i.severity == IssueSeverity::Warning && i.message.contains("lowercase")
        ));

        // Frontmatter `name` differing from the on-disk name -> Warning.
        let mism = vec![
            FrontmatterField { key: "name".into(), value: "other".into() },
            FrontmatterField { key: "description".into(), value: "d".into() },
        ];
        let mv = validate(AgentAssetKind::Agent, "test-runner", &mism, "b", false);
        assert!(mv.valid, "mismatch is only a Warning");
        assert!(mv.issues.iter().any(|i| i.message.contains("differs from the file name")));

        // Empty body for a command -> Warning.
        let eb = validate(AgentAssetKind::Command, "changelog", &[], "   \n", false);
        assert!(eb.issues.iter().any(|i| i.message.contains("body is empty")));
    }

    // §11 row 4.4 / row 11 — name safety.
    #[test]
    fn validate_asset_name_rejects_unsafe() {
        for bad in ["", "   ", ".", "..", "-x", "a/b", "a\\b", "a:b", "../x", "a\tb", "café"] {
            assert!(
                matches!(validate_asset_name(bad), Err(AppError::InvalidName(_))),
                "name {bad:?} should be InvalidName"
            );
        }
        for good in ["code-review", "test-runner", "foo.bar", "a_b", "x1"] {
            validate_asset_name(good).unwrap_or_else(|e| panic!("must accept {good:?}: {e:?}"));
        }
        // The name check fires before any path is built, so the belt-and-suspenders
        // `validate_rel_path` guard is never reached for `..` via read_agent_asset.
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            read_agent_asset(tmp.path(), AgentAssetKind::Agent, ".."),
            Err(AppError::InvalidName(_))
        ));
        assert!(matches!(
            read_agent_asset(tmp.path(), AgentAssetKind::Agent, "a/b"),
            Err(AppError::InvalidName(_))
        ));
    }

    // read_agent_asset: existing parsed; missing -> exists:false shell.
    #[test]
    fn read_agent_asset_existing_and_missing() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write(
            root,
            ".claude/agents/test-runner.md",
            b"---\nname: test-runner\ndescription: Runs tests\n---\n\nYou run tests.\n",
        );
        let a = read_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
        assert!(a.exists && a.validation.valid);
        assert_eq!(a.name, "test-runner");
        assert_eq!(a.frontmatter[0].value, "test-runner");
        assert_eq!(a.body, "\nYou run tests.\n");

        // Missing -> exists:false shell with a "file does not exist" Error.
        let m = read_agent_asset(root, AgentAssetKind::Skill, "nope").unwrap();
        assert!(!m.exists);
        assert!(m.frontmatter.is_empty() && m.body.is_empty());
        assert!(!m.validation.valid);
        assert!(m
            .validation
            .issues
            .iter()
            .any(|i| i.message.contains("does not exist")));
    }

    // §11 row 6 — wire shapes: camelCase keys + bare-string enums.
    #[test]
    fn wire_shapes_are_camel_case() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            ".claude/agents/broken.md",
            b"---\nname: broken\n---\n\nno description here\n",
        );
        let inv = scan_agent_assets(tmp.path()).unwrap();
        let v = serde_json::to_value(&inv).unwrap();

        let asset = &v["assets"][0];
        // Bare-string kind.
        assert_eq!(asset["kind"], "agent");
        assert!(asset.get("name").is_some());
        assert!(asset.get("path").is_some());
        assert!(asset.get("exists").is_some());
        assert!(asset.get("frontmatter").is_some());
        assert!(asset.get("body").is_some());

        let validation = &asset["validation"];
        assert_eq!(validation["valid"], false);
        let issue = &validation["issues"][0];
        // Bare-string severity.
        assert_eq!(issue["severity"], "error");
        assert!(issue.get("message").is_some());

        // FrontmatterField camelCase (key/value are already lowercase).
        let field = &asset["frontmatter"][0];
        assert_eq!(field["key"], "name");
        assert_eq!(field["value"], "broken");

        // Skill/command kinds also serialize bare.
        assert_eq!(serde_json::to_value(AgentAssetKind::Skill).unwrap(), "skill");
        assert_eq!(serde_json::to_value(AgentAssetKind::Command).unwrap(), "command");
        assert_eq!(serde_json::to_value(IssueSeverity::Warning).unwrap(), "warning");
    }
}
