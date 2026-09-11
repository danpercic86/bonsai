//! T2 Area 9 (in-crate half) — external launcher **argv assembly is
//! injection-safe**, and no ladder rung ever waits when it must not.
//!
//! These cases lived in `tests/misc/external_spawn.rs` until 2026-09-11, when
//! [`program_spec`], [`terminal_ladder`] and [`editor_ladder`] became
//! `pub(crate)`: handed an unvalidated string they build a spec that launches
//! it, so — by the same rule that keeps `external_url::url_ladder` private — the
//! crate boundary is not the right place to expose them. The cases that only
//! need the public surface (`launch_first`, `reveal_spec`, `resolve_program`,
//! `validate_command_setting`) stayed in the integration test.
//!
//! The safety property under test: a repo path containing shell
//! metacharacters, quotes, a newline, a leading dash, or unicode is substituted
//! into a SINGLE argv token — never split, never shell-interpreted. Nothing here
//! spawns; every case inspects the built [`LaunchSpec`].

use super::{editor_ladder, program_spec, reveal_spec, terminal_ladder, PathDelivery, TargetOs};
use crate::external_cmd::{safe_cwd, validate_command_setting};
use std::path::{Path, PathBuf};

/// Every hostile path substitutes into ONE argv token, verbatim — the program
/// stays `editor`, and the metacharacters never become extra args or shell ops.
#[test]
fn hostile_path_becomes_one_argv_token() {
    let hostile = [
        r#"C:\proj\a & b"#,
        r#"C:\proj\"quoted""#,
        r#"C:\proj\a^b%PATH%!x"#,
        r#"/home/me/a;rm -rf ~"#,
        r#"/home/me/$(reboot)"#,
        "/home/me/line\nbreak",
        "-rf --no-preserve-root",       // leading dash
        "/tmp/café/Ünïcode/日本語/Ж",   // unicode
    ];
    for raw in hostile {
        let path = PathBuf::from(raw);
        let spec = program_spec("editor", &path, false, PathDelivery::Argument)
            .unwrap_or_else(|| panic!("spec must build for {raw:?}"));
        assert_eq!(spec.program, "editor", "program never becomes the path");
        assert_eq!(spec.args.len(), 1, "path is exactly ONE arg for {raw:?}: {:?}", spec.args);
        assert_eq!(spec.args[0], path.display().to_string(), "arg is the path verbatim");
        // LOW-1: the child does not launch from the (hostile) repo directory.
        assert_eq!(spec.cwd, safe_cwd(), "argument delivery keeps the neutral cwd");
    }
}

/// A hostile path delivered as the WORKING DIRECTORY (the terminal rungs that
/// have no directory argument) never turns into an argument, so it cannot be
/// read as a flag or a script name however it is spelled.
#[test]
fn hostile_path_as_working_dir_never_becomes_an_argument() {
    for raw in [r#"C:\a b & c\repo"#, "/tmp/-rf", "/tmp/x;y", "/tmp/café/日本語"] {
        let path = PathBuf::from(raw);
        let spec = program_spec("pwsh", &path, true, PathDelivery::WorkingDir)
            .unwrap_or_else(|| panic!("spec must build for {raw:?}"));
        assert_eq!(spec.program, "pwsh");
        assert!(spec.args.is_empty(), "no argv token for {raw:?}: {:?}", spec.args);
        assert_eq!(spec.cwd, path, "the directory IS the delivery here");
        assert!(spec.hide_console, "hide_console threaded through");
    }
}

/// MEDIUM-2 end-to-end: the renderer-settable string is a program, so every
/// command line — quoted, unbalanced, argument-bearing or metacharacter-bearing
/// — is REFUSED before a spec exists, while the two legitimate shapes (empty ⇒
/// auto-detect, bare name) survive.
#[test]
fn configured_program_shape_rules_hold_before_any_spec_is_built() {
    for bad in [
        r#"editor "/tmp/x"#,
        r#""""#,
        "editor /tmp/x",
        "powershell -c calc",
        "code&calc",
        "./code",
        r"\server\share\evil.exe",
    ] {
        assert!(
            validate_command_setting(bad, "Editor command").is_err(),
            "must be refused: {bad:?}"
        );
    }
    for ok in ["", "   ", "code", "code-insiders", "notepad++.exe"] {
        assert!(
            validate_command_setting(ok, "Editor command").is_ok(),
            "must be accepted: {ok:?}"
        );
    }
    // Whitespace-only still collapses to "no configured program" downstream.
    assert!(program_spec("   ", Path::new("/tmp/x"), false, PathDelivery::Argument).is_none());
}

// ---------------------------------------------------------------- F-MAC-1
// The macOS editor ladder used to no-op silently: `/usr/bin/open` ALWAYS
// spawns successfully and reports "Unable to find application" through its
// EXIT CODE, so a spawn-only runner made rung #1 win forever and a Mac without
// VS Code got nothing. `LaunchSpec::wait_for_exit` marks exactly the macOS
// `open` rungs as "wait and judge the exit status"; everything else keeps the
// detached-spawn semantics (Windows `explorer` exits non-zero on success).

/// (a) Both macOS `open -a` editor rungs carry `wait_for_exit`; the `code` CLI
/// fallback does not.
#[test]
fn macos_editor_open_rungs_wait_for_exit() {
    let path = PathBuf::from("/tmp/work");
    let ladder = editor_ladder(TargetOs::MacOs, "", &path);
    assert_eq!(ladder.len(), 3, "open -a VS Code, open -a Insiders, code");
    assert_eq!(ladder[0].program, "open");
    assert!(ladder[0].wait_for_exit, "rung 1 must judge open's exit code");
    assert_eq!(ladder[1].program, "open");
    assert!(ladder[1].wait_for_exit, "rung 2 must judge open's exit code");
    assert_eq!(ladder[2].program, "code");
    assert!(!ladder[2].wait_for_exit, "the plain CLI rung stays detached");
}

/// (b) No Windows or Linux spec ever waits — editor, terminal, or reveal.
#[test]
fn windows_linux_specs_never_wait_for_exit() {
    let path = PathBuf::from("/tmp/work");
    for os in [TargetOs::Windows, TargetOs::Linux] {
        for spec in editor_ladder(os, "", &path)
            .into_iter()
            .chain(terminal_ladder(os, "", &path))
            .chain(std::iter::once(reveal_spec(os, &path)))
        {
            assert!(
                !spec.wait_for_exit,
                "{os:?} `{}` must stay a detached spawn",
                spec.program
            );
        }
    }
}
