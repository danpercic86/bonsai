//! Git config read/edit path (P40 contract §4).
//!
//! Pure git2 logic, no Tauri types — runtime-free (`&Path`/`&str`), CLI-testable
//! against the `git config` oracle like `remote.rs`. All functions BLOCKING
//! (the command layer wraps them in `spawn_blocking`).
//!
//! Reads present the **effective** value (merged system+global+local view) plus
//! which level set it; writes target the chosen level (Local | Global). System
//! is read-only (never a write target). Multi-valued keys read the last/effective
//! value and edit single-valued only (contract §2).
//!
//! This closes the identity-unset gap: setting `user.name`/`user.email` here
//! makes `commit::resolve_signature`'s `ConfigMissing` actionable.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::AppError;

// ---------------------------------------------------------------- curated table

/// Input widget kind for a curated key (contract §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueKind {
    Text,
    Bool,
    Enum,
}

/// One curated key definition (static). `enum_values` non-empty only for Enum.
struct CuratedKey {
    key: &'static str,
    kind: ValueKind,
    enum_values: &'static [&'static str],
}

/// The curated set (contract §4.1). Order = display order (Identity, then
/// Behaviour). All current curated writes use `set_str` — the enum keys are
/// tri-state STRINGS (true/false/input/only/…), never libgit2 bools.
const CURATED_KEYS: &[CuratedKey] = &[
    CuratedKey { key: "user.name", kind: ValueKind::Text, enum_values: &[] },
    CuratedKey { key: "user.email", kind: ValueKind::Text, enum_values: &[] },
    CuratedKey {
        key: "core.autocrlf",
        kind: ValueKind::Enum,
        enum_values: &["true", "false", "input"],
    },
    CuratedKey { key: "init.defaultBranch", kind: ValueKind::Text, enum_values: &[] },
    CuratedKey {
        key: "pull.ff",
        kind: ValueKind::Enum,
        enum_values: &["true", "false", "only"],
    },
    CuratedKey {
        key: "pull.rebase",
        kind: ValueKind::Enum,
        enum_values: &["true", "false", "merges", "interactive"],
    },
];

// ------------------------------------------------------------------ wire types

/// Write-target level requested by the client. System is NOT a valid target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigLevelArg {
    Local,
    Global,
}

/// The level a value's effective/target value actually lives at (read result).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigLevelName {
    Local,
    Global,
    System,
    Other,
}

/// A curated key with its effective value + the value set AT the target level.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedEntry {
    /// e.g. "user.email".
    pub key: String,
    pub kind: ValueKind,
    /// Allowed values for Enum; empty otherwise.
    pub enum_values: Vec<String>,
    /// Effective value from the merged snapshot; None if unset at every level.
    pub effective_value: Option<String>,
    /// Which level the effective value came from; None if unset.
    pub effective_level: Option<ConfigLevelName>,
    /// Value set explicitly AT the target level; None if inherited/unset there.
    pub target_value: Option<String>,
}

/// An arbitrary `section.key = value` entry read AT the target level (Advanced
/// list). Multivar keys collapse to the LAST value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEntry {
    /// Full dotted name, e.g. "alias.co".
    pub name: String,
    pub value: String,
    /// Always == the target level for Advanced entries.
    pub level: ConfigLevelName,
}

/// Response of `read_config` for one target level.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub target_level: ConfigLevelArg,
    /// Curated keys (fixed order = CURATED_KEYS).
    pub curated: Vec<CuratedEntry>,
    /// Arbitrary entries defined at the target level, EXCLUDING the curated
    /// keys. Sorted by name.
    pub advanced: Vec<ConfigEntry>,
}

// --------------------------------------------------------------- open helpers

/// Opens the repo at `workdir` (`NO_SEARCH`, like every git/ module). Maps a
/// "not a repository" open failure to `NoRepo` (contract §4.6); rejects bare
/// repos (no workdir to scope Local config against).
fn open_repo(workdir: &Path) -> Result<git2::Repository, AppError> {
    let repo = match git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    ) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Err(AppError::NoRepo),
        Err(e) => return Err(e.into()),
    };
    if repo.is_bare() {
        return Err(AppError::Git(
            "cannot edit config: repository is bare".to_string(),
        ));
    }
    Ok(repo)
}

/// A WRITABLE single-level config view for `level`. For Global, if the merged
/// config has no Global level yet (no `~/.gitconfig`), fall back to opening the
/// global file by path (created on first write) — contract §4.4 / §11.5.
fn open_target(repo: &git2::Repository, level: ConfigLevelArg) -> Result<git2::Config, AppError> {
    let cfg = repo.config()?;
    match level {
        ConfigLevelArg::Local => Ok(cfg.open_level(git2::ConfigLevel::Local)?),
        ConfigLevelArg::Global => match cfg.open_level(git2::ConfigLevel::Global) {
            Ok(c) => Ok(c),
            Err(_) => {
                let path = git2::Config::find_global()?;
                Ok(git2::Config::open(&path)?)
            }
        },
    }
}

/// Maps a libgit2 config level to the wire enum.
fn map_level(level: git2::ConfigLevel) -> ConfigLevelName {
    match level {
        // Intentional deviation from contract §4.4 (which folds Worktree into
        // Other): a worktree-level value is repo-scoped, so Local is the more
        // useful bucket for the UI's target hinting.
        git2::ConfigLevel::Local | git2::ConfigLevel::Worktree => ConfigLevelName::Local,
        git2::ConfigLevel::Global | git2::ConfigLevel::XDG => ConfigLevelName::Global,
        git2::ConfigLevel::System | git2::ConfigLevel::ProgramData => ConfigLevelName::System,
        _ => ConfigLevelName::Other,
    }
}

/// The wire level for entries read AT a target level (always that level).
fn target_level_name(level: ConfigLevelArg) -> ConfigLevelName {
    match level {
        ConfigLevelArg::Local => ConfigLevelName::Local,
        ConfigLevelArg::Global => ConfigLevelName::Global,
    }
}

// ------------------------------------------------------------------ read path

/// Blocking. Reads the merged config for effective values + the single-level
/// (target) config for `target_value`/advanced entries (contract §4.4).
/// Errors: `NoRepo` (workdir not a repo) | `Git`.
pub fn read_config(workdir: &Path, level: ConfigLevelArg) -> Result<ConfigView, AppError> {
    let repo = open_repo(workdir)?;
    let merged = repo.config()?.snapshot()?;
    let target = open_target(&repo, level)?.snapshot()?;

    let mut curated = Vec::with_capacity(CURATED_KEYS.len());
    for ck in CURATED_KEYS {
        let (effective_value, effective_level) = match merged.get_entry(ck.key) {
            Ok(e) => (e.value().ok().map(str::to_string), Some(map_level(e.level()))),
            Err(_) => (None, None),
        };
        curated.push(CuratedEntry {
            key: ck.key.to_string(),
            kind: ck.kind,
            enum_values: ck.enum_values.iter().map(|s| (*s).to_string()).collect(),
            effective_value,
            effective_level,
            target_value: target.get_string(ck.key).ok(),
        });
    }

    // Iterate the target-level entries only, excluding curated keys; multivar
    // keys collapse to the last value (BTreeMap overwrite keyed by name, which
    // also yields the required name-sorted output).
    let level_name = target_level_name(level);
    let mut advanced: BTreeMap<String, ConfigEntry> = BTreeMap::new();
    target.entries(None)?.for_each(|e| {
        let name = String::from_utf8_lossy(e.name_bytes()).into_owned();
        if CURATED_KEYS.iter().any(|ck| ck.key.eq_ignore_ascii_case(&name)) {
            return;
        }
        let value = String::from_utf8_lossy(e.value_bytes()).into_owned();
        advanced.insert(
            name.clone(),
            ConfigEntry { name, value, level: level_name },
        );
    })?;

    Ok(ConfigView {
        target_level: level,
        curated,
        advanced: advanced.into_values().collect(),
    })
}

// ----------------------------------------------------------------- validation

/// Friendly server-side key-shape pre-check (contract §4.5). git2's `set_str`
/// also rejects truly-invalid names, but this yields a clearer message.
fn validate_key(key: &str) -> Result<(), AppError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidName(
            "config key must not be empty".to_string(),
        ));
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() < 2 {
        return Err(AppError::InvalidName(
            "config key must be section.key".to_string(),
        ));
    }
    let section = parts[0];
    if section.is_empty()
        || !section
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AppError::InvalidName("invalid section name".to_string()));
    }
    let variable = parts[parts.len() - 1];
    let mut vch = variable.chars();
    let first_is_letter = matches!(vch.next(), Some(c) if c.is_ascii_alphabetic());
    let rest_ok = vch.all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !first_is_letter || !rest_ok {
        return Err(AppError::InvalidName("invalid key name".to_string()));
    }
    for sub in &parts[1..parts.len() - 1] {
        if sub.is_empty() || sub.chars().any(char::is_whitespace) {
            return Err(AppError::InvalidName("invalid subsection".to_string()));
        }
    }
    Ok(())
}

/// For a curated Enum key, rejects values not in its `enum_values`. Text keys
/// are unconstrained; email format is a SOFT client-side warning, never blocked
/// here (contract §4.5).
fn validate_curated_value(key: &str, value: &str) -> Result<(), AppError> {
    let trimmed_key = key.trim();
    if let Some(ck) = CURATED_KEYS
        .iter()
        .find(|c| c.key.eq_ignore_ascii_case(trimmed_key))
    {
        if ck.kind == ValueKind::Enum && !ck.enum_values.contains(&value.trim()) {
            return Err(AppError::InvalidName(format!(
                "value for {} must be one of: {}",
                ck.key,
                ck.enum_values.join(", ")
            )));
        }
    }
    Ok(())
}

// ----------------------------------------------------------------- write path

/// Blocking. Validates `key` shape + (for curated Enum keys) the value, then
/// writes `value` at `level` (single-valued — replaces any existing value).
/// Errors: `NoRepo` | `InvalidName` (bad key/value) | `Git`.
pub fn set_config(
    workdir: &Path,
    level: ConfigLevelArg,
    key: &str,
    value: &str,
) -> Result<(), AppError> {
    validate_key(key)?;
    validate_curated_value(key, value)?;
    let repo = open_repo(workdir)?;
    let mut cfg = open_target(&repo, level)?;
    // Every curated key is a string/tri-state enum in v1 → always set_str.
    cfg.set_str(key.trim(), value.trim())?;
    Ok(())
}

/// Blocking. Applies an identity to the repo's LOCAL git config (P44): writes
/// `user.name`, `user.email`, and — only when `signing_key` is Some AND
/// non-empty (after trim) — `user.signingkey`. A None/empty signing key is left
/// UNTOUCHED (never unset), to avoid surprising removals. Overwrites existing
/// Local values. Returns the refreshed Local `ConfigView` (same shape as
/// `read_config(_, Local)`). Errors: `NoRepo` (workdir not a repo) |
/// `InvalidName` | `Git`.
///
/// Reuses the validated `set_config` write path per key (no reinvented config
/// logic); `user.signingkey` is not a curated key, so it surfaces in the
/// returned `ConfigView.advanced` list rather than `curated`.
pub fn apply_identity_profile(
    workdir: &Path,
    user_name: &str,
    user_email: &str,
    signing_key: Option<&str>,
) -> Result<ConfigView, AppError> {
    set_config(workdir, ConfigLevelArg::Local, "user.name", user_name)?;
    set_config(workdir, ConfigLevelArg::Local, "user.email", user_email)?;
    if let Some(key) = signing_key {
        if !key.trim().is_empty() {
            set_config(workdir, ConfigLevelArg::Local, "user.signingkey", key)?;
        }
    }
    read_config(workdir, ConfigLevelArg::Local)
}

/// Blocking. Removes `key` at `level`. Idempotent: a key not present at that
/// level yields `Ok(())` (NotFound swallowed). Errors: `NoRepo` | `InvalidName`
/// | `Git`.
pub fn unset_config(workdir: &Path, level: ConfigLevelArg, key: &str) -> Result<(), AppError> {
    validate_key(key)?;
    let repo = open_repo(workdir)?;
    let mut cfg = open_target(&repo, level)?;
    match cfg.remove(key.trim()) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

// ------------------------------------------------------------------- tests
//
// ISOLATION (contract §2/§9): in-process unit tests write **Local only**
// (repo-scoped, safe). Global-level verification lives in the CLI oracle
// subprocess (`tests/config_cli.rs`) with an isolated `GIT_CONFIG_GLOBAL`/`HOME`
// — these tests MUST NEVER touch the developer's real `~/.gitconfig`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::scratch_dir;

    fn init_repo() -> tempfile::TempDir {
        let dir = scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");
        dir
    }

    fn find_curated<'a>(view: &'a ConfigView, key: &str) -> &'a CuratedEntry {
        view.curated
            .iter()
            .find(|c| c.key == key)
            .unwrap_or_else(|| panic!("curated key {key} missing"))
    }

    /// (1) The wire JSON is exact camelCase — guards the TS mirror types.
    #[test]
    fn config_view_wire_shape_is_camel_case() {
        let view = ConfigView {
            target_level: ConfigLevelArg::Local,
            curated: vec![CuratedEntry {
                key: "user.email".to_string(),
                kind: ValueKind::Text,
                enum_values: vec![],
                effective_value: Some("a@b.co".to_string()),
                effective_level: Some(ConfigLevelName::Global),
                target_value: None,
            }],
            advanced: vec![ConfigEntry {
                name: "alias.co".to_string(),
                value: "checkout".to_string(),
                level: ConfigLevelName::Local,
            }],
        };
        let json = serde_json::to_value(&view).expect("json");
        assert_eq!(
            json,
            serde_json::json!({
                "targetLevel": "local",
                "curated": [{
                    "key": "user.email",
                    "kind": "text",
                    "enumValues": [],
                    "effectiveValue": "a@b.co",
                    "effectiveLevel": "global",
                    "targetValue": null,
                }],
                "advanced": [{
                    "name": "alias.co",
                    "value": "checkout",
                    "level": "local",
                }],
            })
        );
    }

    /// (2) Local identity is reported as effective + target at Local.
    #[test]
    fn read_config_reports_curated_identity() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "user.name", "Ada").expect("name");
        set_config(path, ConfigLevelArg::Local, "user.email", "ada@x.io").expect("email");

        let view = read_config(path, ConfigLevelArg::Local).expect("read");
        let name = find_curated(&view, "user.name");
        assert_eq!(name.effective_value.as_deref(), Some("Ada"));
        assert_eq!(name.effective_level, Some(ConfigLevelName::Local));
        assert_eq!(name.target_value.as_deref(), Some("Ada"));
        let email = find_curated(&view, "user.email");
        assert_eq!(email.effective_value.as_deref(), Some("ada@x.io"));
        assert_eq!(email.target_value.as_deref(), Some("ada@x.io"));
    }

    /// (3) An unset curated key has no value AT the target level. Asserts
    /// `target_value` (Local), NOT `effective_value`: the merged view inherits
    /// the developer's REAL global config, so a globally-set `pull.ff` would
    /// make an effective-None assertion non-hermetic (contract §9 isolation).
    #[test]
    fn read_config_unset_key_is_none() {
        let dir = init_repo();
        let view = read_config(dir.path(), ConfigLevelArg::Local).expect("read");
        let ff = find_curated(&view, "pull.ff");
        assert_eq!(ff.target_value, None);
    }

    /// (4) A Local write is read back as the target value.
    #[test]
    fn set_config_writes_local_then_reads_back() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "user.email", "a@b.co").expect("set");
        let view = read_config(path, ConfigLevelArg::Local).expect("read");
        assert_eq!(
            find_curated(&view, "user.email").target_value.as_deref(),
            Some("a@b.co")
        );
    }

    /// (5) Malformed keys are rejected before any write.
    #[test]
    fn set_config_rejects_bad_key() {
        let dir = init_repo();
        let path = dir.path();
        for bad in ["nodot", "user.", ".email", "user.1bad", "  "] {
            match set_config(path, ConfigLevelArg::Local, bad, "x") {
                Err(AppError::InvalidName(_)) => {}
                other => panic!("expected InvalidName for {bad:?}, got {other:?}"),
            }
        }
    }

    /// (6) A curated Enum key rejects an out-of-set value.
    #[test]
    fn set_config_rejects_bad_enum() {
        let dir = init_repo();
        match set_config(dir.path(), ConfigLevelArg::Local, "core.autocrlf", "maybe") {
            Err(AppError::InvalidName(m)) => assert!(m.contains("core.autocrlf"), "{m}"),
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    /// (7) A valid Enum value writes and reads back.
    #[test]
    fn set_config_accepts_enum_value() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "core.autocrlf", "input").expect("set");
        let view = read_config(path, ConfigLevelArg::Local).expect("read");
        assert_eq!(
            find_curated(&view, "core.autocrlf").target_value.as_deref(),
            Some("input")
        );
    }

    /// (8) Unset removes the key and is idempotent.
    #[test]
    fn unset_config_removes_and_is_idempotent() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "user.name", "Ada").expect("set");
        unset_config(path, ConfigLevelArg::Local, "user.name").expect("unset");
        // Second unset is still Ok (NotFound swallowed).
        unset_config(path, ConfigLevelArg::Local, "user.name").expect("idempotent unset");
        let view = read_config(path, ConfigLevelArg::Local).expect("read");
        // Assert the LOCAL (target) value is gone. NOT `effective_value`: the
        // merged view legitimately inherits the developer's REAL global
        // `user.name`, so an effective-None assertion would be non-hermetic
        // (contract §9 isolation — deviates from §4.7 test 8 for that reason).
        assert_eq!(find_curated(&view, "user.name").target_value, None);
    }

    /// (9) Advanced lists arbitrary keys but excludes curated ones.
    #[test]
    fn advanced_excludes_curated_and_lists_arbitrary() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "alias.co", "checkout").expect("alias");
        set_config(path, ConfigLevelArg::Local, "user.name", "Ada").expect("name");
        let view = read_config(path, ConfigLevelArg::Local).expect("read");
        assert!(
            view.advanced.iter().any(|e| e.name == "alias.co" && e.value == "checkout"),
            "advanced missing alias.co: {:?}",
            view.advanced
        );
        assert!(
            !view.advanced.iter().any(|e| e.name == "user.name"),
            "advanced must exclude curated user.name"
        );
    }

    /// (10) An empty (non-repo) dir errors with NoRepo.
    #[test]
    fn read_config_no_repo_errors() {
        let dir = scratch_dir();
        match read_config(dir.path(), ConfigLevelArg::Local) {
            Err(AppError::NoRepo) => {}
            other => panic!("expected NoRepo, got {other:?}"),
        }
    }

    // ---- P44 apply_identity_profile (Local only; never touch global) --------

    /// (P44.1) Apply ("Ada","ada@x.io", None) → Local user.name/user.email are
    /// the applied values; user.signingkey is NOT written (absent from advanced).
    #[test]
    fn apply_identity_profile_writes_local_identity() {
        let dir = init_repo();
        let view = apply_identity_profile(dir.path(), "Ada", "ada@x.io", None).expect("apply");
        assert_eq!(
            find_curated(&view, "user.name").target_value.as_deref(),
            Some("Ada")
        );
        assert_eq!(
            find_curated(&view, "user.email").target_value.as_deref(),
            Some("ada@x.io")
        );
        assert!(
            !view.advanced.iter().any(|e| e.name == "user.signingkey"),
            "signing key must be absent when None: {:?}",
            view.advanced
        );
    }

    /// (P44.2) A Some non-empty signing key is written and surfaces in advanced
    /// (user.signingkey is not a curated key).
    #[test]
    fn apply_identity_profile_writes_signing_key_when_set() {
        let dir = init_repo();
        let view =
            apply_identity_profile(dir.path(), "Ada", "ada@x.io", Some("KEYID")).expect("apply");
        assert!(
            view.advanced
                .iter()
                .any(|e| e.name == "user.signingkey" && e.value == "KEYID"),
            "advanced must contain user.signingkey=KEYID: {:?}",
            view.advanced
        );
    }

    /// (P44.3) A None signing key leaves a pre-existing Local user.signingkey
    /// UNTOUCHED (never unset).
    #[test]
    fn apply_identity_profile_leaves_existing_signing_key_on_none() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "user.signingkey", "OLD").expect("preset key");
        let view = apply_identity_profile(path, "Ada", "ada@x.io", None).expect("apply");
        assert!(
            view.advanced
                .iter()
                .any(|e| e.name == "user.signingkey" && e.value == "OLD"),
            "pre-existing signing key must be preserved on None: {:?}",
            view.advanced
        );
    }

    /// (P44.4) Apply overwrites a different pre-existing Local identity.
    #[test]
    fn apply_identity_profile_overwrites_existing_identity() {
        let dir = init_repo();
        let path = dir.path();
        set_config(path, ConfigLevelArg::Local, "user.name", "Old Name").expect("preset name");
        set_config(path, ConfigLevelArg::Local, "user.email", "old@x.io").expect("preset email");
        let view = apply_identity_profile(path, "New Name", "new@x.io", None).expect("apply");
        assert_eq!(
            find_curated(&view, "user.name").target_value.as_deref(),
            Some("New Name")
        );
        assert_eq!(
            find_curated(&view, "user.email").target_value.as_deref(),
            Some("new@x.io")
        );
    }
}
