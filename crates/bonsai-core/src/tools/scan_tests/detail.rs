//! Detail sanitisation (bidi overrides) and length caps.

use super::*;

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
