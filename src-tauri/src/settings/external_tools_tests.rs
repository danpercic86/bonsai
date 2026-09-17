//! P112 AC7 — the §5.3 migration, at the settings level.
//!
//! The pure mapping table (`legacy_tool_id`) is pinned in
//! `bonsai-core/src/tools/settings_ids_tests.rs`; what is asserted here is the
//! part that involves [`Settings`]: which key wins, what is cleared, and that
//! the legacy keys do not survive a save.

use super::migrate_external_tools;
use crate::settings::{load_from, save_to, Settings};

/// Every migration case must leave the browsed-path fields untouched and must
/// never select the `"custom"` pseudo-id — invariant 1, the most important one.
fn assert_no_browsed_path_was_invented(s: &Settings) {
    assert_eq!(s.custom_terminal_path, "", "migration wrote a browsed path");
    assert_eq!(s.custom_editor_path, "", "migration wrote a browsed path");
    assert_ne!(
        s.terminal_tool, "custom",
        "migration selected the browsed slot"
    );
    assert_ne!(
        s.editor_tool, "custom",
        "migration selected the browsed slot"
    );
    // And the legacy input is never carried further.
    assert_eq!(s.terminal_command, "");
    assert_eq!(s.editor_command, "");
}

#[test]
fn a_legacy_command_becomes_a_catalog_id_or_nothing() {
    for (terminal, editor, want_terminal, want_editor) in [
        ("wt -d {path}", "code {path}", "windows-terminal", "vscode"),
        ("cmd /K", "subl", "cmd", "sublime"),
        ("powershell -c calc", "codium", "powershell", "vscodium"),
        // Misses are accepted, not preserved.
        (r"C:\Tools\payload.exe", r"C:\Program Files\X\x.exe", "", ""),
        ("make", "node", "", ""),
        ("", "", "", ""),
        ("   ", "   ", "", ""),
    ] {
        let mut s = Settings {
            terminal_command: terminal.to_string(),
            editor_command: editor.to_string(),
            ..Default::default()
        };
        migrate_external_tools(&mut s);
        assert_eq!(s.terminal_tool, want_terminal, "terminal {terminal:?}");
        assert_eq!(s.editor_tool, want_editor, "editor {editor:?}");
        assert_no_browsed_path_was_invented(&s);
    }
}

#[test]
fn a_pre_existing_selection_always_wins_over_the_legacy_key() {
    let mut s = Settings {
        terminal_tool: "pwsh".to_string(),
        editor_tool: "custom".to_string(),
        custom_editor_path: r"C:\Portable\Editor.exe".to_string(),
        terminal_command: "wt -d {path}".to_string(),
        editor_command: "code {path}".to_string(),
        ..Default::default()
    };
    migrate_external_tools(&mut s);
    assert_eq!(s.terminal_tool, "pwsh");
    assert_eq!(s.editor_tool, "custom");
    // A browsed path that was already stored is preserved — migration only ever
    // reads the legacy keys.
    assert_eq!(s.custom_editor_path, r"C:\Portable\Editor.exe");
    assert_eq!(s.terminal_command, "");
    assert_eq!(s.editor_command, "");
}

/// Renamed from `running_the_migration_again_changes_nothing`, which overstated
/// what it checked: comparing run-1 to run-2 does NOT discriminate the
/// `.clear()`, because the `is_empty()` guard blocks the second pass whether or
/// not the legacy fields were cleared. The assertion that bites is that the
/// legacy fields are EMPTY after one run — on the hit branch AND on the miss
/// branch, where nothing was selected and the legacy value would otherwise be
/// left sitting there for a future launch site to find.
///
/// Idempotence across a real reload (the property the module doc claims) is
/// proven by [`the_legacy_keys_are_gone_from_the_file_after_one_save`]'s closing
/// `assert_eq!(again, loaded)`, not here.
#[test]
fn the_migration_clears_the_legacy_keys_and_a_second_run_is_a_no_op() {
    let mut s = Settings {
        terminal_command: "wt -d {path}".to_string(),
        editor_command: "code {path}".to_string(),
        ..Default::default()
    };
    migrate_external_tools(&mut s);
    assert_eq!(s.terminal_tool, "windows-terminal");
    assert_eq!(s.editor_tool, "vscode");
    // THE discriminating assertion (it asserts both legacy fields are `""`).
    assert_no_browsed_path_was_invented(&s);
    let once = s.clone();
    migrate_external_tools(&mut s);
    assert_eq!(s, once, "the migration is not idempotent");

    // The MISS branch clears too: the selection stays empty, so an
    // implementation that only cleared on a hit would leave `myterm`/`myed`
    // behind here.
    let mut miss = Settings {
        terminal_command: "myterm --here".to_string(),
        editor_command: "myed {path}".to_string(),
        ..Default::default()
    };
    migrate_external_tools(&mut miss);
    assert_eq!(miss.terminal_tool, "");
    assert_eq!(miss.editor_tool, "");
    assert_no_browsed_path_was_invented(&miss);
    let once_miss = miss.clone();
    migrate_external_tools(&mut miss);
    assert_eq!(miss, once_miss, "the miss branch is not idempotent");
}

#[test]
fn the_legacy_keys_are_gone_from_the_file_after_one_save() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("settings.json");
    let legacy = r#"{
            "version": 1,
            "theme": "light",
            "terminalCommand": "wt -d {path}",
            "editorCommand": "code {path}"
        }"#;
    std::fs::write(&file, legacy).expect("write a pre-P112 settings.json");

    // Load migrates in memory…
    let loaded = load_from(&file);
    assert_eq!(loaded.terminal_tool, "windows-terminal");
    assert_eq!(loaded.editor_tool, "vscode");
    assert_no_browsed_path_was_invented(&loaded);

    // …and the next ordinary save drops the keys (`skip_serializing`).
    save_to(&file, &loaded).expect("save settings");
    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(
        !raw.contains("terminalCommand"),
        "legacy key survived: {raw}"
    );
    assert!(!raw.contains("editorCommand"), "legacy key survived: {raw}");
    assert!(raw.contains("\"terminalTool\": \"windows-terminal\""));
    assert!(raw.contains("\"editorTool\": \"vscode\""));
    assert!(raw.contains("\"customTerminalPath\""));
    assert!(raw.contains("\"customEditorPath\""));

    // A second load is a no-op.
    let again = load_from(&file);
    assert_eq!(again, loaded);
}

#[test]
fn a_file_with_neither_key_loads_empty_selections() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("settings.json");
    std::fs::write(&file, r#"{ "version": 1, "theme": "light" }"#).expect("write settings.json");
    let loaded = load_from(&file);
    assert_eq!(loaded.terminal_tool, "");
    assert_eq!(loaded.editor_tool, "");
    assert_no_browsed_path_was_invented(&loaded);
}

/// A hand-edited `settings.json` can still name a browsed path — the type-level
/// property is about the RENDERER, not about the user's own file. What matters
/// is that it round-trips untouched by migration and is only ever consumed
/// through `BrowsedProgram`/`validate_custom_program`.
#[test]
fn a_hand_written_browsed_path_round_trips_but_is_never_produced_by_migration() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("settings.json");
    let hand_edited = r#"{
            "version": 1,
            "customEditorPath": "C:\\hostile\\payload.exe",
            "editorCommand": "code {path}"
        }"#;
    std::fs::write(&file, hand_edited).expect("write settings.json");
    let loaded = load_from(&file);
    assert_eq!(loaded.custom_editor_path, r"C:\hostile\payload.exe");
    // Migration neither read it nor selected it.
    assert_eq!(loaded.editor_tool, "vscode");
    assert_eq!(loaded.editor_command, "");
}
