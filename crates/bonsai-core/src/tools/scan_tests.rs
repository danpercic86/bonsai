//! Scan-assembly and selection tests (P112 AC13 `present`, AC12's stale
//! selection, and the bundle normalisation half of AC4).
//!
//! **No test here calls `tool_scan` / `refresh_tool_scan` / `picked`**: those
//! populate the process-global scan cache and would spawn `reg.exe` for the
//! `AppPaths` rungs. Everything goes through the explicit-parameter halves
//! (`scan_from_rows`, `picked_from`) with hand-built rows, which is also what
//! makes every OS assertable from one machine.

use std::fs;
use std::path::{Path, PathBuf};

use crate::external::TargetOs;

use super::catalog::{self, Recipe, CUSTOM_ID};
use super::fake::FakeToolEnv;
use super::{
    picked_from, scan_from_rows, DetectedTool, Resolution, ToolEntry, ToolKind, ToolSource,
};

const AT_MS: u64 = 1_700_000_000_000;

fn row(
    kind: ToolKind,
    id: &str,
    os: TargetOs,
    res: Resolution,
) -> (&'static ToolEntry, Resolution) {
    let entry = catalog::find_for(kind, id, os).expect("catalog row (AC8 pins totality)");
    (entry, res)
}

fn exe_res(path: &str, source: ToolSource) -> Resolution {
    Resolution {
        program: path.to_string(),
        bundle: None,
        source,
    }
}

fn built_in_res(program: &str) -> Resolution {
    Resolution {
        program: program.to_string(),
        bundle: None,
        source: ToolSource::BuiltIn,
    }
}

fn bundle_res(bundle: &str) -> Resolution {
    Resolution {
        program: "open".to_string(),
        bundle: Some(bundle.to_string()),
        source: ToolSource::AppBundle,
    }
}

fn ids(rows: &[DetectedTool]) -> Vec<&str> {
    rows.iter().map(|r| r.id.as_str()).collect()
}

/// A file the HOST's browsed-program rules accept (`.exe` on Windows, the
/// execute bit on unix), so the `present: true` case is provable anywhere.
fn browsable_program(dir: &Path, stem: &str) -> PathBuf {
    let path = if cfg!(windows) {
        dir.join(format!("{stem}.exe"))
    } else {
        dir.join(stem)
    };
    fs::write(&path, b"stub").expect("write stub program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("set the execute bit");
    }
    path
}

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

// ---- selection: the launch-time recheck, by source ---------------------------

#[test]
fn a_built_in_selection_resolves_without_any_filesystem_test() {
    // The "nothing exists" env: an `is_file("cmd")` test would be false, and
    // rechecking one would silently break picking `cmd` / `powershell`.
    let env = FakeToolEnv::new();
    let rows = vec![
        row(
            ToolKind::Terminal,
            "cmd",
            TargetOs::Windows,
            built_in_res("cmd"),
        ),
        row(
            ToolKind::Terminal,
            "powershell",
            TargetOs::Windows,
            built_in_res("powershell"),
        ),
    ];
    let picked = picked_from(
        &env,
        TargetOs::Windows,
        &rows,
        "cmd",
        ToolKind::Terminal,
        "",
    )
    .expect("a built-in selection always resolves");
    assert_eq!(picked.program, "cmd");
    assert_eq!(picked.recipe, Recipe::DirCwd(&["/K"]));
    assert_eq!(picked.open_arg, None);
    assert_eq!(picked.source, ToolSource::BuiltIn);
}

#[test]
fn a_well_known_selection_whose_file_was_removed_falls_back_to_auto() {
    let wt = r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe";
    let rows = vec![row(
        ToolKind::Terminal,
        "windows-terminal",
        TargetOs::Windows,
        exe_res(wt, ToolSource::WellKnown),
    )];
    // File still there ⇒ picked, with the catalog recipe.
    let live = FakeToolEnv::new().file(wt);
    let picked = picked_from(
        &live,
        TargetOs::Windows,
        &rows,
        "windows-terminal",
        ToolKind::Terminal,
        "",
    )
    .expect("still installed");
    assert_eq!(picked.program, wt);
    assert_eq!(picked.recipe, Recipe::DirLastArg(&["-d"]));
    // Uninstalled between the scan and the launch ⇒ `None` ⇒ auto ladder,
    // silently (the OQ1 ruling: no toast).
    let gone = FakeToolEnv::new();
    assert_eq!(
        picked_from(
            &gone,
            TargetOs::Windows,
            &rows,
            "windows-terminal",
            ToolKind::Terminal,
            ""
        ),
        None
    );
}

#[test]
fn a_bundle_selection_is_normalised_to_open_a_with_the_bundle_as_one_token() {
    // AC4's normalisation half: `picked` hands the launch side `MacOpen` +
    // `program: "open"` + the resolved bundle PATH, including its spaces, so
    // the spec builder needs no bundle branch.
    let bundle = "/Applications/Visual Studio Code.app";
    let rows = vec![row(
        ToolKind::Editor,
        "vscode",
        TargetOs::MacOs,
        bundle_res(bundle),
    )];
    let installed = FakeToolEnv::new().bundle(bundle);
    let picked = picked_from(
        &installed,
        TargetOs::MacOs,
        &rows,
        "vscode",
        ToolKind::Editor,
        "",
    )
    .expect("bundle still installed");
    assert_eq!(picked.recipe, Recipe::MacOpen);
    assert_eq!(picked.program, "open");
    assert_eq!(picked.open_arg.as_deref(), Some(bundle));
    assert_eq!(picked.source, ToolSource::AppBundle);
    // Bundle deleted ⇒ `None`. (`is_file` would be false for a directory, so
    // the recheck must be `is_bundle` — that is what this pins.)
    let removed = FakeToolEnv::new().file(bundle);
    assert_eq!(
        picked_from(
            &removed,
            TargetOs::MacOs,
            &rows,
            "vscode",
            ToolKind::Editor,
            ""
        ),
        None
    );
}

#[test]
fn an_undetected_unknown_or_wrong_kind_selection_yields_none() {
    let env = FakeToolEnv::new();
    let rows = vec![row(
        ToolKind::Terminal,
        "cmd",
        TargetOs::Windows,
        built_in_res("cmd"),
    )];
    for setting in ["", "no-such-tool", CUSTOM_ID] {
        assert_eq!(
            picked_from(
                &env,
                TargetOs::Windows,
                &rows,
                setting,
                ToolKind::Terminal,
                ""
            ),
            None,
            "setting {setting:?} must not resolve"
        );
    }
    // A catalog id of the OTHER kind.
    assert_eq!(
        picked_from(
            &env,
            TargetOs::Windows,
            &rows,
            "vscode",
            ToolKind::Terminal,
            ""
        ),
        None
    );
    // A real catalog id that simply was not detected on this host.
    assert_eq!(
        picked_from(
            &env,
            TargetOs::Windows,
            &rows,
            "pwsh",
            ToolKind::Terminal,
            ""
        ),
        None
    );
    // An id that exists only on ANOTHER OS is nameable but never launchable.
    assert_eq!(
        picked_from(
            &env,
            TargetOs::Windows,
            &rows,
            "apple-terminal",
            ToolKind::Terminal,
            ""
        ),
        None
    );
}

#[test]
fn a_browsed_selection_is_revalidated_at_launch_and_gets_a_synthesized_recipe() {
    let scratch = crate::testutil::scratch_dir();
    let program = browsable_program(scratch.path(), "Portable");
    let stored = program.to_string_lossy().to_string();
    let env = FakeToolEnv::new();
    let os = TargetOs::host();

    // Editor: the folder is one argv token.
    let editor = picked_from(&env, os, &[], CUSTOM_ID, ToolKind::Editor, &stored)
        .expect("a stored browsed editor resolves");
    assert_eq!(editor.recipe, Recipe::DirLastArg(&[]));
    assert_eq!(editor.program, stored);
    assert_eq!(editor.open_arg, None);
    assert_eq!(editor.source, ToolSource::Custom);
    // Terminal: no arguments at all — the cwd IS the target.
    let terminal = picked_from(&env, os, &[], CUSTOM_ID, ToolKind::Terminal, &stored)
        .expect("a stored browsed terminal resolves");
    assert_eq!(terminal.recipe, Recipe::DirCwd(&[]));

    // Deleted since it was browsed ⇒ `None` ⇒ auto ladder.
    fs::remove_file(&program).expect("remove the stub program");
    assert_eq!(
        picked_from(&env, os, &[], CUSTOM_ID, ToolKind::Editor, &stored),
        None
    );
}

#[test]
fn a_detail_that_cannot_be_validated_cannot_carry_a_bidi_override_into_the_ui() {
    // A stored path is normally dialog-derived and already refuses these
    // characters. This row is the one that is DISPLAYED despite failing
    // validation (a gone or hand-edited path), so its subtitle needs the
    // sanitizing most — a bidi override exists only to make a row read as
    // something other than what it is. Every OTHER row gets the same treatment
    // (see the probe-derived test below); it is unconditional, not a
    // custom-row special case.
    let hostile = "C:\\tools\\pa\u{202e}gpj.exe";
    let scan = scan_from_rows(&[], "", hostile, TargetOs::Windows, AT_MS);
    let custom = &scan.editors[0];
    assert!(!custom.present);
    assert!(!custom.detail.contains('\u{202e}'));
    assert!(!custom.label.contains('\u{202e}'));
}

#[test]
fn a_probe_derived_detail_is_sanitized_too() {
    // A probe-derived path never passes through `validate_custom_program`, and
    // `executable_hit` performs no character check — so a PATH (or App Paths)
    // directory whose NAME carries a bidi override would otherwise produce a
    // row whose label is trustworthy (it comes from the static catalog) but
    // whose subtitle reads as a different path than the one that launches.
    let hostile = "C:\\to\u{202e}ols\\wt.exe";
    let rows = vec![row(
        ToolKind::Terminal,
        "windows-terminal",
        TargetOs::Windows,
        exe_res(hostile, ToolSource::Path),
    )];
    let scan = scan_from_rows(&rows, "", "", TargetOs::Windows, AT_MS);
    let wt = &scan.terminals[0];
    assert!(wt.present);
    assert_eq!(wt.label, "Windows Terminal");
    assert!(!wt.detail.contains('\u{202e}'));
    assert_eq!(wt.detail, "C:\\tools\\wt.exe");

    // A bundle detail takes the same route.
    let bundle = "/Applications/Vis\u{202e}ual Studio Code.app";
    let rows = vec![row(
        ToolKind::Editor,
        "vscode",
        TargetOs::MacOs,
        bundle_res(bundle),
    )];
    let scan = scan_from_rows(&rows, "", "", TargetOs::MacOs, AT_MS);
    assert!(!scan.editors[0].detail.contains('\u{202e}'));
}

#[test]
fn a_hand_written_detail_is_length_capped() {
    // The custom row is displayed DESPITE failing validation, so a hand-edited
    // `settings.json` can put an arbitrarily long value here. 512 is the same
    // cap `validate_custom_program` enforces, counted in chars — so `detail`
    // stays byte-identical for anything validation ACCEPTED (<= 512 bytes,
    // hence <= 512 chars). It is NOT the only row the cap can fire on: a
    // probe-derived `Resolution::program` is length-unbounded too.
    //
    // Truncation is marked: an ellipsis is appended, so a trustworthy prefix
    // cannot be mistaken for the whole path.
    let stored = format!("C:\\long\\{}.exe", "a".repeat(900));
    let scan = scan_from_rows(&[], "", &stored, TargetOs::Windows, AT_MS);
    let custom = &scan.editors[0];
    assert!(!custom.present, "too long to validate");
    assert_eq!(
        custom.detail.chars().count(),
        513,
        "512 chars + the ellipsis"
    );
    let prefix = custom
        .detail
        .strip_suffix('…')
        .expect("a truncated detail is marked with an ellipsis");
    assert_eq!(prefix.chars().count(), 512);
    assert!(stored.starts_with(prefix));
}

#[test]
fn a_probe_derived_detail_is_length_capped_too() {
    // The claim the previous test's comment used to make — "the browsed row is
    // the ONE detail string the cap can fire on" — is false, and this pins why.
    // `Resolution::program` is built from a `PATH` directory plus a program
    // name; it never passes `validate_custom_program`'s `MAX_LEN`, and
    // `is_file()` succeeds on a path this long because `std` applies the
    // `\\?\` prefix internally. So a probe-derived subtitle is length-unbounded
    // too.
    let long = format!("C:\\{}\\wt.exe", "d".repeat(900));
    let rows = vec![row(
        ToolKind::Terminal,
        "windows-terminal",
        TargetOs::Windows,
        exe_res(&long, ToolSource::Path),
    )];
    let scan = scan_from_rows(&rows, "", "", TargetOs::Windows, AT_MS);
    let wt = &scan.terminals[0];
    let prefix = wt
        .detail
        .strip_suffix('…')
        .expect("a truncated detail is marked with an ellipsis");
    assert_eq!(prefix.chars().count(), 512);
    assert!(long.starts_with(prefix));
}
