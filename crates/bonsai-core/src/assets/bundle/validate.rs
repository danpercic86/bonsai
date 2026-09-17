//! Validation (§4.4, §4.5).

use crate::error::AppError;

use super::spec::{kind_label, required_keys};
use super::{AgentAssetKind, AssetIssue, FrontmatterField, IssueSeverity, Validation};

/// Windows reserved device names (case-insensitive). Rejected whether bare
/// (`CON`) or with an extension (`CON.md`) — Windows treats `CON.md` as the
/// device `CON`, so a scan would never find a file written under that name.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate an asset `name` for filesystem safety (§4.4). Rejects blank/`.`/`..`,
/// a leading `-`, any of `/ \ :`, control chars, or any char outside
/// `[A-Za-z0-9._-]` -> `InvalidName`. (This charset makes a path separator or a
/// `..` component impossible.) Also rejects Windows reserved device names
/// (`CON`, `NUL`, `COM1`, …, matched on the base name before the first `.`,
/// case-insensitive) and a trailing dot or space (Windows silently strips these,
/// causing a scan/write name mismatch). "Not lowercase-hyphen" is NOT rejected
/// here — it is only a Warning in `validate` (§4.5).
pub fn validate_asset_name(name: &str) -> Result<(), AppError> {
    // Base name = text before the first '.' (so `CON.md` still resolves to `CON`).
    let base = name.split('.').next().unwrap_or(name);
    let reserved = WINDOWS_RESERVED_NAMES
        .iter()
        .any(|r| base.eq_ignore_ascii_case(r));
    let trailing_dot_or_space = name.ends_with('.') || name.ends_with(' ');
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
            .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        || reserved
        || trailing_dot_or_space;
    if bad {
        return Err(AppError::InvalidName(format!(
            "invalid asset name: '{name}'"
        )));
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

pub(super) fn error_issue(message: impl Into<String>) -> AssetIssue {
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
pub(super) fn validate(
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
