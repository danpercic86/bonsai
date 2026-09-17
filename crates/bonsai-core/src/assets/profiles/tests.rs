//! Store + preview + activate tests for context profiles (§11.1).

use tempfile::TempDir;

use crate::error::AppError;
use crate::git::stage::validate_rel_path;

use super::test_support::{profile, target};
use super::*;

// §11.1 row 6 — lazy default + persist + corrupt.
#[test]
fn list_profiles_lazy_default_creates_no_file() {
    let tmp = TempDir::new().unwrap();
    let store = list_profiles(tmp.path()).unwrap();
    assert_eq!(store.version, 2);
    assert!(store.profiles.is_empty());
    assert_eq!(store.active_profile, None);
    // No file / dir written by a read.
    assert!(!tmp.path().join(".bonsai").exists());
}

#[test]
fn save_profile_creates_store_and_round_trips() {
    let tmp = TempDir::new().unwrap();
    let p = profile("opus", vec![target("claude", "# rich\n")]);
    let store = save_profile(tmp.path(), p.clone()).unwrap();
    assert_eq!(store.profiles.len(), 1);
    assert!(tmp.path().join(".bonsai").join("profiles.json").is_file());

    // Re-load round-trips the persisted profile.
    let reloaded = list_profiles(tmp.path()).unwrap();
    assert_eq!(reloaded.version, 2);
    assert_eq!(reloaded.profiles, vec![p]);

    // Upsert by name replaces in place (no duplicate).
    let updated = save_profile(
        tmp.path(),
        profile("opus", vec![target("claude", "# richer\n")]),
    )
    .unwrap();
    assert_eq!(updated.profiles.len(), 1);
    assert_eq!(updated.profiles[0].targets[0].content, "# richer\n");
}

#[test]
fn corrupt_store_is_other_error() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".bonsai")).unwrap();
    std::fs::write(
        tmp.path().join(".bonsai").join("profiles.json"),
        b"{ not json",
    )
    .unwrap();
    let err = list_profiles(tmp.path()).unwrap_err();
    assert!(matches!(err, AppError::Other(m) if m.contains("corrupt")));
}

// §11.1 row 7 — save validation.
#[test]
fn blank_or_separator_name_rejected() {
    let tmp = TempDir::new().unwrap();
    for bad in ["", "   ", "-lead", "a/b", "a\\b", "a\tb"] {
        let err = save_profile(tmp.path(), profile(bad, vec![])).unwrap_err();
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "name {bad:?} should be InvalidName"
        );
    }
}

#[test]
fn non_single_file_target_rejected() {
    let tmp = TempDir::new().unwrap();
    // A rules-dir id and a config id are both invalid targets.
    for bad_id in ["cursorRules", "mcp", "claudeDir", "does-not-exist"] {
        let err = save_profile(tmp.path(), profile("p", vec![target(bad_id, "x")])).unwrap_err();
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "target {bad_id:?} should be InvalidName"
        );
    }
}

// §11.1 row 8 — preview writes nothing.
#[test]
fn preview_reports_state_and_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), b"# old claude\n").unwrap();
    let claude_before = std::fs::read(tmp.path().join("CLAUDE.md")).unwrap();

    save_profile(
        tmp.path(),
        profile(
            "p",
            vec![
                target("claude", "# new claude\n"),
                target("agents", "# new agents\n"),
            ],
        ),
    )
    .unwrap();

    let preview = preview_profile(tmp.path(), "p").unwrap();
    assert_eq!(preview.len(), 2);

    let claude = &preview[0];
    assert_eq!(claude.asset_id, "claude");
    assert_eq!(claude.path, "CLAUDE.md");
    assert_eq!(claude.current.as_deref(), Some("# old claude\n"));
    assert_eq!(claude.proposed, "# new claude\n");
    assert!(claude.changed);

    let agents = &preview[1];
    assert_eq!(agents.path, "AGENTS.md");
    assert_eq!(agents.current, None, "missing file has no current");
    assert!(agents.changed, "missing file differs");

    // Nothing was written: existing file byte-identical, missing file absent.
    assert_eq!(
        std::fs::read(tmp.path().join("CLAUDE.md")).unwrap(),
        claude_before
    );
    assert!(!tmp.path().join("AGENTS.md").exists());
}

#[test]
fn preview_unchanged_when_content_matches() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), b"# same\n").unwrap();
    save_profile(tmp.path(), profile("p", vec![target("claude", "# same\n")])).unwrap();
    let preview = preview_profile(tmp.path(), "p").unwrap();
    assert!(!preview[0].changed);
}

// §11.1 row 9 — activate: created / written / unchanged + atomicity + set active.
#[test]
fn activate_creates_writes_and_skips() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    // AGENTS.md missing -> Created; CLAUDE.md differs -> Written;
    // GEMINI.md already equal -> Unchanged.
    std::fs::write(root.join("CLAUDE.md"), b"# old\n").unwrap();
    std::fs::write(root.join("GEMINI.md"), b"# gem\n").unwrap();

    save_profile(
        root,
        profile(
            "p",
            vec![
                target("agents", "# agents body\n"),
                target("claude", "# new claude\n"),
                target("gemini", "# gem\n"),
            ],
        ),
    )
    .unwrap();

    let act = activate_profile(root, "p").unwrap();
    assert_eq!(act.profile, "p");
    let by_id = |id: &str| {
        act.results
            .iter()
            .find(|r| r.asset_id == id)
            .unwrap()
            .action
    };
    assert_eq!(by_id("agents"), TargetWriteAction::Created);
    assert_eq!(by_id("claude"), TargetWriteAction::Written);
    assert_eq!(by_id("gemini"), TargetWriteAction::Unchanged);

    // Files hold byte-exact content afterward.
    assert_eq!(
        std::fs::read(root.join("AGENTS.md")).unwrap(),
        b"# agents body\n"
    );
    assert_eq!(
        std::fs::read(root.join("CLAUDE.md")).unwrap(),
        b"# new claude\n"
    );

    // active_profile set + persisted.
    assert_eq!(act.store.active_profile.as_deref(), Some("p"));
    assert_eq!(
        list_profiles(root).unwrap().active_profile.as_deref(),
        Some("p")
    );

    // No .bonsai-tmp remnant beside any written file.
    assert!(!root.join("AGENTS.md.bonsai-tmp").exists());
    assert!(!root.join("CLAUDE.md.bonsai-tmp").exists());

    // A second identical activation is all Unchanged.
    let again = activate_profile(root, "p").unwrap();
    assert!(again
        .results
        .iter()
        .all(|r| r.action == TargetWriteAction::Unchanged));
}

#[test]
fn activate_missing_profile_is_other() {
    let tmp = TempDir::new().unwrap();
    let err = activate_profile(tmp.path(), "nope").unwrap_err();
    assert!(matches!(err, AppError::Other(m) if m.contains("not found")));
}

#[test]
fn delete_clears_active_profile() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), b"# a\n").unwrap();
    save_profile(tmp.path(), profile("p", vec![target("claude", "# b\n")])).unwrap();
    activate_profile(tmp.path(), "p").unwrap();
    let store = delete_profile(tmp.path(), "p").unwrap();
    assert!(store.profiles.is_empty());
    assert_eq!(store.active_profile, None);
    // Deleting an absent profile is a no-op Ok.
    let store2 = delete_profile(tmp.path(), "gone").unwrap();
    assert!(store2.profiles.is_empty());
}

// Path-escape defense: the static table is safe, but assert the guard rejects
// `..` / absolute paths directly (belt-and-suspenders, §11.1 row 9).
#[test]
fn validate_rel_path_rejects_escapes() {
    assert!(validate_rel_path("../escape.md").is_err());
    assert!(validate_rel_path("/etc/passwd").is_err());
    assert!(validate_rel_path("C:/Windows/system32").is_err());
    assert!(validate_rel_path("a\\b").is_err());
    assert!(validate_rel_path("CLAUDE.md").is_ok());
}

// Wire-shape: camelCase keys + bare-string TargetWriteAction.
#[test]
fn wire_shapes_are_camel_case() {
    let tmp = TempDir::new().unwrap();
    save_profile(
        tmp.path(),
        ContextProfile {
            name: "opus".to_string(),
            description: Some("rich".to_string()),
            model: Some("opus".to_string()),
            targets: vec![target("claude", "# c\n")],
        },
    )
    .unwrap();
    let act = activate_profile(tmp.path(), "opus").unwrap();

    let store_v = serde_json::to_value(&act.store).unwrap();
    assert!(store_v.get("version").is_some());
    assert!(store_v.get("activeProfile").is_some());
    assert_eq!(store_v["profiles"][0]["name"], "opus");

    let act_v = serde_json::to_value(&act).unwrap();
    assert!(act_v.get("profile").is_some());
    let result = &act_v["results"][0];
    assert!(result.get("assetId").is_some());
    assert!(result.get("path").is_some());
    // Field-less enum → bare string.
    assert_eq!(result["action"], "created");
}
