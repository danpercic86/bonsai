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
