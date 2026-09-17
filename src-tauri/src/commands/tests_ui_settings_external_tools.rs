//! P112 §5.2 / §5.4 — the external-tool keys on the settings patch: what the
//! renderer can write, what it cannot, and what happens to garbage.
//!
//! Split out of `tests_ui_settings_patch_flags.rs` (which held the P49
//! free-text-command cases) because P112 replaced those two keys with coerced
//! ids and a browsed path the patch type cannot carry at all.

use super::*;

/// Every shape a compromised renderer might write into `terminalTool` /
/// `editorTool` (AC5), including a path that genuinely EXISTS on this machine —
/// existence must never matter, because the value is a lookup key and nothing
/// else.
fn hostile_values() -> Vec<String> {
    let existing = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .expect("the test binary's own path");
    let mut values: Vec<String> = [
        r"C:\hostile\payload.exe",
        "/tmp/payload",
        "node",
        "make",
        "nmake",
        "just",
        "msbuild",
        "python",
        "powershell -c calc",
        "code {path}",
        "../../evil",
        // Accepted by the BROWSE path since AMEND-6, but only from the dialog:
        // as a renderer-written selection it is still nothing.
        r"\\server\share\x.exe",
        "//host/share/x.exe",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    values.push(existing);
    values.push("a".repeat(2000));
    values.push("payl\u{202e}exe".to_string());
    values
}

/// §5.1: the two id keys patch independently — a `Some` overwrites, an absent
/// key leaves the stored value untouched, and `Some("")` resets to the auto
/// ladder.
#[test]
fn set_ui_settings_patch_external_tools_is_partial() {
    let mut s = settings::Settings::default();
    assert_eq!(s.terminal_tool, "");
    assert_eq!(s.editor_tool, "");

    // Only `terminal_tool` changes; the editor + unrelated fields stay.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            terminal_tool: Some("windows-terminal".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_tool, "windows-terminal");
    assert_eq!(s.editor_tool, "");
    assert_eq!(s.theme, ThemeChoice::default());

    // Only `editor_tool` changes; the terminal selection is preserved.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_tool: Some("vscode".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_tool, "windows-terminal");
    assert_eq!(s.editor_tool, "vscode");

    // An unrelated patch, and an empty one, clear neither.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        },
    );
    apply_patch(&mut s, UiSettingsPatch::default());
    assert_eq!(s.terminal_tool, "windows-terminal");
    assert_eq!(s.editor_tool, "vscode");

    // `Some("")` is the documented reset to Auto-detect (the row's reset arrow).
    apply_patch(
        &mut s,
        UiSettingsPatch {
            terminal_tool: Some(String::new()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_tool, "");
    assert_eq!(s.editor_tool, "vscode");
}

/// AC5 / AC15(c): a renderer-written program or path — even one that exists —
/// is COERCED to `""` (⇒ the auto ladder), never stored. Coercion never errors,
/// so it cannot wedge the settings writer the way a rejection would.
#[test]
fn a_renderer_written_program_is_coerced_to_nothing() {
    for value in hostile_values() {
        // Start from a real selection, so the assertion proves the garbage
        // write actively resets to "nothing selected" rather than merely
        // failing to apply.
        let mut s = settings::Settings {
            terminal_tool: "cmd".to_string(),
            editor_tool: "vscode".to_string(),
            // A browsed path is already stored: having browsed once must not
            // make some OTHER string selectable.
            custom_editor_path: r"C:\Portable\Editor.exe".to_string(),
            ..Default::default()
        };
        apply_patch(
            &mut s,
            UiSettingsPatch {
                terminal_tool: Some(value.clone()),
                editor_tool: Some(value.clone()),
                ..Default::default()
            },
        );
        assert_eq!(s.terminal_tool, "", "{value:?} must not select a terminal");
        assert_eq!(s.editor_tool, "", "{value:?} must not select an editor");
        // And the write never touched the browsed path.
        assert_eq!(s.custom_editor_path, r"C:\Portable\Editor.exe");
    }
}

/// §5.2 + the AMEND-2 reversibility: `"custom"` selects only a path the user
/// already browsed to, and reverting to Auto-detect keeps that path, so
/// re-selecting restores the tool (which is why the reset needs no
/// confirmation).
#[test]
fn custom_selects_only_a_browsed_path_and_the_reset_is_reversible() {
    let mut s = settings::Settings::default();
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_tool: Some("custom".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.editor_tool, "", "nothing browsed ⇒ nothing to select");

    // Only `pick_external_tool` writes this field; a test stands in for it.
    s.custom_editor_path = r"C:\Portable\Editor.exe".to_string();
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_tool: Some("custom".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.editor_tool, "custom");

    // Revert to Auto-detect: the PATH survives.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_tool: Some(String::new()),
            ..Default::default()
        },
    );
    assert_eq!(s.editor_tool, "");
    assert_eq!(s.custom_editor_path, r"C:\Portable\Editor.exe");

    // …so re-selecting restores it.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_tool: Some("custom".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.editor_tool, "custom");
}

/// AC15(b): a patch JSON that carries a browsed path (or a legacy free-text
/// command) parses fine — `UiSettingsPatch` has no `deny_unknown_fields` — and
/// changes NOTHING. The valid key in the same patch still applies, which is the
/// point: an injected key cannot even spoil the write it rides along with.
#[test]
fn a_patch_json_carrying_a_browsed_path_is_silently_ignored() {
    let raw = r#"{
        "customEditorPath": "C:\\hostile\\payload.exe",
        "customTerminalPath": "/tmp/payload",
        "terminalCommand": "powershell -c calc",
        "editorCommand": "code {path}",
        "editorTool": "vscode"
    }"#;
    let patch: UiSettingsPatch = serde_json::from_str(raw).expect("unknown keys are ignored");
    // Every field the injected keys name is seeded with a DIFFERENT value, so
    // "ignored" is distinguishable from "coincidentally already equal" — the
    // legacy pair included, which `apply_patch` (unlike `load_from`) must leave
    // exactly as it found them.
    let mut s = settings::Settings {
        custom_editor_path: r"C:\Portable\Editor.exe".to_string(),
        custom_terminal_path: r"C:\Portable\Term.exe".to_string(),
        terminal_command: "wt".to_string(),
        editor_command: "code".to_string(),
        ..Default::default()
    };
    apply_patch(&mut s, patch);
    assert_eq!(
        s.custom_editor_path, r"C:\Portable\Editor.exe",
        "an injected key wrote a browsed path"
    );
    assert_eq!(s.custom_terminal_path, r"C:\Portable\Term.exe");
    assert_eq!(s.terminal_command, "wt", "a legacy key was writable again");
    assert_eq!(s.editor_command, "code", "a legacy key was writable again");
    assert_eq!(s.editor_tool, "vscode", "the legitimate key still applied");
}

/// AC15(a) — STRUCTURAL, and deliberately so: the destructure below is
/// exhaustive (no `..`), so adding any field to `UiSettingsPatch` breaks this
/// test at COMPILE time. P112's property is that a renderer-written program
/// path is *unrepresentable*, not merely rejected, and this field list is the
/// only thing that keeps that true. A new field here needs the scrutiny
/// `customEditorPath` would fail.
#[test]
fn the_patch_type_has_no_field_that_can_carry_a_program_path() {
    let UiSettingsPatch {
        theme: _,
        pane_widths: _,
        list_view: _,
        panel_density: _,
        primary_commit_action: _,
        graph_style: _,
        graph_season: _,
        graph_first_parent: _,
        graph_fold_linear: _,
        graph_minimap_always_show: _,
        graph_color_mode: _,
        graph_ref_filter: _,
        auto_fetch: _,
        health_refresh: _,
        graph: _,
        ai_enabled: _,
        ai_conflict_autonomy: _,
        ai_consented: _,
        mcp_consented: _,
        mcp_write_consented: _,
        onboarding_seen: _,
        auto_check_updates: _,
        profiles: _,
        terminal_tool,
        editor_tool,
        ai_idle_timeout_secs: _,
        ai_hard_cap_secs: _,
        ai_max_turns: _,
        ai_stream_log: _,
        ai_include_partial_messages: _,
        ai_conflict_tools: _,
        ai_bulk_max_bytes: _,
        ai_max_budget_usd: _,
        ai_dock_height: _,
        ai_dock_collapsed: _,
        dev: _,
    } = UiSettingsPatch::default();
    // The only two renderer-writable strings on this type are LOOKUP KEYS, and
    // both are coerced on write
    // (`a_renderer_written_program_is_coerced_to_nothing`).
    assert!(terminal_tool.is_none());
    assert!(editor_tool.is_none());
}

/// AC15(d): the READ DTO cannot carry one either — no settings echo, full
/// resend or failed-patch requeue can round-trip a program path through the
/// renderer. The browsed path travels outbound only once, as
/// `DetectedTool.detail`, and that is a different command.
///
/// STRUCTURAL, exactly as AC15(a) is: the destructure below is exhaustive (no
/// `..`), so adding any field to `UiSettings` breaks this test at COMPILE time
/// (`E0027`) and the new field has to face the scrutiny `customEditorPath`
/// would fail. The serialized-key scan that follows is the WEAKER half and is
/// kept only as a second net: it matches on the spellings `path` / `command` /
/// `program`, so a field named `editorTarget` would pass it — the field list is
/// what actually holds.
///
/// Honest scope: like AC15(a), the destructure is TOP-LEVEL. A path smuggled
/// into a nested struct (`profiles`, `graph`, `dev`) is caught only by the
/// whole-JSON check at the end, and only for the paths this test seeds.
#[test]
fn no_settings_echo_can_carry_a_program_path() {
    let s = settings::Settings {
        editor_tool: "custom".to_string(),
        custom_editor_path: r"C:\Portable\Editor.exe".to_string(),
        custom_terminal_path: r"C:\Portable\Term.exe".to_string(),
        ..Default::default()
    };
    let ui = ui_settings_of(&s);

    let UiSettings {
        theme: _,
        pane_widths: _,
        list_view: _,
        panel_density: _,
        primary_commit_action: _,
        graph_style: _,
        graph_season: _,
        graph_first_parent: _,
        graph_fold_linear: _,
        graph_minimap_always_show: _,
        graph_color_mode: _,
        graph_ref_filter: _,
        auto_fetch: _,
        health_refresh: _,
        graph: _,
        ai_enabled: _,
        ai_conflict_autonomy: _,
        ai_consented: _,
        mcp_consented: _,
        mcp_write_consented: _,
        onboarding_seen: _,
        auto_check_updates: _,
        profiles: _,
        terminal_tool,
        editor_tool,
        ai_idle_timeout_secs: _,
        ai_hard_cap_secs: _,
        ai_max_turns: _,
        ai_stream_log: _,
        ai_include_partial_messages: _,
        ai_conflict_tools: _,
        ai_bulk_max_bytes: _,
        ai_max_budget_usd: _,
        ai_dock_height: _,
        ai_dock_collapsed: _,
        dev: _,
    } = &ui;
    // These two are the only `String` fields at THIS level — every other field
    // above is an enum, a number, a bool, or a nested struct (`profiles`,
    // `graph`, `dev`, `auto_fetch`, `health_refresh`, `graph_ref_filter`, whose
    // own fields include strings and are covered only by the whole-JSON check
    // below). Both are LOOKUP KEYS — a catalog id, `"custom"`, or `""` — so
    // neither can carry a directory separator.
    for key in [terminal_tool, editor_tool] {
        assert!(
            !key.contains(['/', '\\']),
            "{key:?} looks like a path, not a catalog id"
        );
    }

    let json = serde_json::to_value(&ui).expect("serialize the read DTO");
    let obj = json.as_object().expect("a JSON object");
    for key in obj.keys() {
        let k = key.to_lowercase();
        assert!(
            !k.contains("path") && !k.contains("command") && !k.contains("program"),
            "{key} looks like a program/path key on the read DTO"
        );
    }
    let text = serde_json::to_string(&ui).expect("serialize the read DTO");
    assert!(
        !text.contains("Portable"),
        "the echo carried a browsed path"
    );
    // The SELECTION is echoed, which is what the picker renders.
    assert_eq!(
        obj.get("editorTool").and_then(|v| v.as_str()),
        Some("custom")
    );
}
