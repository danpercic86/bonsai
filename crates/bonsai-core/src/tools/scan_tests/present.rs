//! AC13 `present` and AC12 stale-selection naming.

use super::*;

// ---- AC13: `present` ---------------------------------------------------------

#[test]
fn probe_derived_rows_are_always_present_with_their_resolved_detail() {
    let wt = r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe";
    let rows = vec![
        row(
            ToolKind::Terminal,
            "windows-terminal",
            TargetOs::Windows,
            exe_res(wt, ToolSource::WellKnown),
        ),
        row(
            ToolKind::Terminal,
            "cmd",
            TargetOs::Windows,
            built_in_res("cmd"),
        ),
        row(
            ToolKind::Editor,
            "vscode",
            TargetOs::MacOs,
            bundle_res("/Applications/Visual Studio Code.app"),
        ),
    ];
    let scan = scan_from_rows(&rows, "", "", TargetOs::Windows, AT_MS);

    assert_eq!(ids(&scan.terminals), vec!["windows-terminal", "cmd"]);
    assert!(scan.terminals.iter().all(|r| r.present));
    assert_eq!(scan.terminals[0].detail, wt);
    assert_eq!(scan.terminals[0].label, "Windows Terminal");
    // A `BuiltIn` row has no path, so `detail` is the literal.
    assert_eq!(scan.terminals[1].detail, "built in");
    assert_eq!(scan.terminals[1].source, ToolSource::BuiltIn);
    // A bundle row's detail is the resolved bundle directory.
    assert_eq!(ids(&scan.editors), vec!["vscode"]);
    assert_eq!(
        scan.editors[0].detail,
        "/Applications/Visual Studio Code.app"
    );
    assert_eq!(scan.editors[0].source, ToolSource::AppBundle);
    assert_eq!(scan.scanned_at_ms, AT_MS);
}

#[test]
fn no_stored_path_means_no_custom_row() {
    let scan = scan_from_rows(&[], "", "", TargetOs::host(), AT_MS);
    assert!(scan.terminals.is_empty());
    assert!(scan.editors.is_empty());
}

#[test]
fn a_stored_custom_path_that_resolves_is_listed_and_present() {
    let scratch = crate::testutil::scratch_dir();
    let program = browsable_program(scratch.path(), "Code");
    let stored = program.to_string_lossy().to_string();

    let scan = scan_from_rows(&[], "", &stored, TargetOs::host(), AT_MS);
    assert_eq!(ids(&scan.editors), vec![CUSTOM_ID]);
    let custom = &scan.editors[0];
    assert!(custom.present);
    assert_eq!(custom.source, ToolSource::Custom);
    assert_eq!(custom.detail, stored);
    // The label is derived by the BACKEND from the path, never supplied.
    assert_eq!(custom.label, "Code");
    assert_eq!(custom.kind, ToolKind::Editor);
    // The custom row belongs to ONE kind: the terminal list stays empty.
    assert!(scan.terminals.is_empty());
}

#[test]
fn a_stored_custom_path_that_is_gone_is_still_listed_but_not_present() {
    // The row must be LISTED even though it no longer resolves, so the UI can
    // render the "custom path gone" state instead of an unexplained empty
    // picker.
    let scratch = crate::testutil::scratch_dir();
    let gone = scratch
        .path()
        .join("Removed.exe")
        .to_string_lossy()
        .to_string();

    let scan = scan_from_rows(&[], &gone, "", TargetOs::host(), AT_MS);
    assert_eq!(ids(&scan.terminals), vec![CUSTOM_ID]);
    let custom = &scan.terminals[0];
    assert!(!custom.present);
    assert_eq!(custom.detail, gone);
    assert_eq!(custom.label, "Removed");
}

#[test]
fn the_custom_row_comes_after_the_detected_rows() {
    let scratch = crate::testutil::scratch_dir();
    let program = browsable_program(scratch.path(), "Portable");
    let rows = vec![row(
        ToolKind::Terminal,
        "cmd",
        TargetOs::Windows,
        built_in_res("cmd"),
    )];
    let scan = scan_from_rows(
        &rows,
        &program.to_string_lossy(),
        "",
        TargetOs::host(),
        AT_MS,
    );
    assert_eq!(ids(&scan.terminals), vec!["cmd", CUSTOM_ID]);
}

// ---- AC12: a stale selection is nameable -------------------------------------

#[test]
fn a_selection_that_is_not_detected_is_still_nameable_from_the_label_map() {
    // This is the assertion that would have caught the picker rendering
    // `notepadpp`: the id is absent from `editors` (nothing detected it) but
    // present in `editorLabels`, so the UI can show "Notepad++ — not
    // installed".
    let scan = scan_from_rows(&[], "", "", TargetOs::Windows, AT_MS);
    assert!(!ids(&scan.editors).contains(&"notepadpp"));
    assert_eq!(
        scan.editor_labels.get("notepadpp").map(String::as_str),
        Some("Notepad++")
    );
    // Cross-OS: a macOS-authored selection renders on Windows.
    assert_eq!(
        scan.terminal_labels.get("iterm2").map(String::as_str),
        Some("iTerm")
    );
    assert_eq!(
        scan.editor_labels.get("kate").map(String::as_str),
        Some("Kate")
    );
}
