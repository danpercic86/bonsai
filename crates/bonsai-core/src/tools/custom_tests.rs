//! Browsed-path rules (P112 §5.4): what `validate_custom_program` refuses, the
//! label it derives, and the recipe a browsed tool launches with.
//!
//! These support AC13 (the custom row's `present` flag is this function's
//! verdict) and pre-cover the AC16 table. Two rules are host-bound and say so
//! at their test: the unix execute bit cannot be read on Windows, and a path
//! that must be *absolute for the target OS* can only exist on the host whose
//! absolute form that is — so the accept-cases are `cfg`-split while every
//! refusal is asserted on every host.

use std::fs;
use std::path::{Path, PathBuf};

use crate::external::TargetOs;

use super::catalog::Recipe;
use super::custom::{display_label, synthesize_recipe, validate_custom_program, CustomKindShape};
use super::ToolKind;

const OSES: [TargetOs; 3] = [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux];

/// An absolute-looking path for `os` that does not exist.
fn absent(os: TargetOs, tail: &str) -> PathBuf {
    match os {
        TargetOs::Windows => PathBuf::from(format!(r"C:\tools\{tail}")),
        TargetOs::MacOs | TargetOs::Linux => PathBuf::from(format!("/opt/tools/{tail}")),
    }
}

fn refused(path: &Path, os: TargetOs) -> bool {
    validate_custom_program(path, os).is_err()
}

#[test]
fn a_path_that_does_not_exist_is_refused_on_every_os() {
    for os in OSES {
        assert!(refused(&absent(os, "payload.exe"), os));
    }
}

#[test]
fn a_relative_path_or_a_bare_name_is_refused_on_every_os() {
    for os in OSES {
        for value in ["code", "code.exe", "../../evil", "tools/code.exe", ""] {
            assert!(
                refused(Path::new(value), os),
                "{value:?} must be refused for {os:?}"
            );
        }
    }
}

#[test]
fn an_absolute_path_for_the_wrong_os_is_refused() {
    // `Path::is_absolute` is host-relative, so this is asserted against the
    // explicit `os` parameter: a `/Applications/…` string is not a Windows
    // program path and `C:\…` is not a unix one.
    assert!(refused(Path::new("/Applications/Cursor.app"), TargetOs::Windows));
    assert!(refused(Path::new(r"C:\tools\code.exe"), TargetOs::MacOs));
    assert!(refused(Path::new(r"C:\tools\code.exe"), TargetOs::Linux));
}

#[test]
fn unc_and_extended_length_roots_are_refused() {
    for value in [
        r"\\server\share\payload.exe",
        "//host/share/payload",
        r"\\?\C:\tools\payload.exe",
    ] {
        for os in OSES {
            assert!(refused(Path::new(value), os), "{value:?} must be refused");
        }
    }
}

#[test]
fn control_and_bidi_characters_are_refused_before_the_filesystem_is_touched() {
    for os in OSES {
        for tail in ["pa\u{202e}gpj.exe", "co\nde.exe", "co\u{200f}de.exe"] {
            assert!(
                refused(&absent(os, tail), os),
                "{tail:?} must be refused for {os:?}"
            );
        }
    }
}

#[test]
fn an_over_long_path_is_refused() {
    for os in OSES {
        let long = absent(os, &"a".repeat(600));
        assert!(refused(&long, os));
        assert!(long.to_string_lossy().len() > 512);
    }
}

#[test]
fn a_plain_directory_is_refused() {
    let scratch = crate::testutil::scratch_dir();
    let dir = scratch.path().join("not-a-program");
    fs::create_dir_all(&dir).expect("create dir");
    assert!(refused(&dir, TargetOs::host()));
}

#[test]
fn the_refusal_is_category_only_and_never_echoes_the_path() {
    let secret = absent(TargetOs::Windows, "S3cret-Payload.exe");
    let err = validate_custom_program(&secret, TargetOs::Windows).expect_err("must be refused");
    let text = err.to_string();
    assert!(!text.contains("S3cret"), "the error echoed the path: {text}");
    assert!(!text.contains("C:\\"), "the error echoed the path: {text}");
    assert!(!text.is_empty());
}

// ---- accept cases (host-split, see the module doc) ---------------------------

#[cfg(windows)]
#[test]
fn on_windows_only_dot_exe_is_accepted_even_though_the_others_exist() {
    // DEC-1's ENFORCEMENT half: the dialog filter is not the gate. `.cmd` /
    // `.bat` / `.ps1` are re-interpreted by `cmd.exe`, which performs `%VAR%`
    // expansion on the argv it receives.
    let scratch = crate::testutil::scratch_dir();
    for name in ["payload.cmd", "payload.bat", "payload.ps1", "payload"] {
        let p = scratch.path().join(name);
        fs::write(&p, b"stub").expect("write stub");
        assert!(p.is_file());
        assert!(refused(&p, TargetOs::Windows), "{name} must be refused");
    }
    let exe = scratch.path().join("Code.exe");
    fs::write(&exe, b"stub").expect("write stub");
    assert_eq!(
        validate_custom_program(&exe, TargetOs::Windows).expect("a .exe is accepted"),
        CustomKindShape::Executable
    );
    // Case-insensitively.
    let upper = scratch.path().join("Code.EXE");
    fs::write(&upper, b"stub").expect("write stub");
    assert_eq!(
        validate_custom_program(&upper, TargetOs::Windows).expect("accepted"),
        CustomKindShape::Executable
    );
}

#[cfg(windows)]
#[test]
fn on_a_windows_host_the_unix_execute_bit_rule_cannot_be_evaluated() {
    // Documenting the one host-bound rule rather than pretending to cover it:
    // there is no mode() to read here, so an existing unix-absolute file would
    // be accepted. The refusal below comes from the ABSOLUTENESS rule, which is
    // os-explicit and therefore does hold on this host.
    let scratch = crate::testutil::scratch_dir();
    let p = scratch.path().join("tool");
    fs::write(&p, b"stub").expect("write stub");
    assert!(refused(&p, TargetOs::Linux));
}

#[cfg(unix)]
#[test]
fn on_unix_the_execute_bit_is_the_gate() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = crate::testutil::scratch_dir();
    let p = scratch.path().join("tool");
    fs::write(&p, b"stub").expect("write stub");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).expect("clear the execute bit");
    assert!(refused(&p, TargetOs::host()));
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("set the execute bit");
    assert_eq!(
        validate_custom_program(&p, TargetOs::host()).expect("executable accepted"),
        CustomKindShape::Executable
    );
}

#[cfg(unix)]
#[test]
fn a_dot_app_directory_is_a_bundle_only_with_an_info_plist() {
    // The `.app` case is precisely what an `is_file()`-only model got wrong: a
    // bundle is a DIRECTORY. Unix-only because the path must be absolute for
    // `os = MacOs` to be considered at all.
    let scratch = crate::testutil::scratch_dir();
    let app = scratch.path().join("Zed.app");
    fs::create_dir_all(app.join("Contents")).expect("create bundle dir");
    assert!(refused(&app, TargetOs::MacOs), "no Info.plist yet");
    fs::write(app.join("Contents").join("Info.plist"), b"<plist/>").expect("write plist");
    assert_eq!(
        validate_custom_program(&app, TargetOs::MacOs).expect("a real bundle is accepted"),
        CustomKindShape::MacBundle
    );
    // Not on Linux: there is no `open -a` there, so a directory stays refused.
    assert!(refused(&app, TargetOs::Linux));
}

// ---- derived label + recipe --------------------------------------------------

#[test]
fn the_label_is_the_file_stem_with_hostile_characters_stripped() {
    assert_eq!(
        display_label(Path::new("/Applications/Visual Studio Code.app")),
        "Visual Studio Code"
    );
    assert_eq!(display_label(Path::new(r"C:\tools\Code.exe")), "Code");
    assert_eq!(display_label(Path::new("/opt/tools/subl")), "subl");
    assert_eq!(display_label(Path::new("/opt/pa\u{202e}gpj")), "pagpj");
    // Truncated, never unbounded.
    let long = format!("/opt/{}", "a".repeat(200));
    assert_eq!(display_label(Path::new(&long)).len(), 48);
}

#[test]
fn the_synthesized_recipe_matches_the_contract_table() {
    assert_eq!(
        synthesize_recipe(ToolKind::Editor, CustomKindShape::Executable),
        Recipe::DirLastArg(&[])
    );
    assert_eq!(
        synthesize_recipe(ToolKind::Editor, CustomKindShape::MacBundle),
        Recipe::MacOpen
    );
    // The one row that hands a program NO arguments and the target as its cwd —
    // reachable only for a program a human browsed to and chose as their
    // terminal.
    assert_eq!(
        synthesize_recipe(ToolKind::Terminal, CustomKindShape::Executable),
        Recipe::DirCwd(&[])
    );
    assert_eq!(
        synthesize_recipe(ToolKind::Terminal, CustomKindShape::MacBundle),
        Recipe::MacOpen
    );
}
