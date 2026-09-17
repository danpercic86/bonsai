//! T2 Area 9 (in-crate half) — external launcher **argv assembly is
//! injection-safe**, and no ladder rung ever waits when it must not.
//!
//! These cases lived in `tests/misc/external_spawn.rs` until 2026-09-11, when
//! the ladder builders became `pub(crate)`. P112 made them `pub` again (there is
//! no `program: &str` parameter left to hand an unvalidated string to), but the
//! cases stay here: they assert the argv the PRIVATE auto arm builds, and the
//! `PickedTool` literals below are a test fixture, not an API a caller would
//! reach for.
//!
//! The safety property under test: a repo path containing shell metacharacters,
//! quotes, a newline, a leading dash, or unicode is substituted into a SINGLE
//! argv token — never split, never shell-interpreted. Nothing here spawns; every
//! case inspects the built [`LaunchSpec`].

use super::{editor_ladder, reveal_spec, spec_from, terminal_ladder, TargetOs};
use crate::procutil::safe_cwd;
use crate::tools::catalog::Recipe;
use crate::tools::{PickedTool, ToolKind, ToolSource};
use std::path::PathBuf;

/// The shape a browsed editor gets (`synthesize_recipe(Editor, Executable)`) —
/// the one launch route whose `program` is a path rather than a catalog literal,
/// so the hostile-path cases exercise the riskiest builder arm.
fn browsed_editor(program: &str) -> PickedTool {
    PickedTool {
        kind: ToolKind::Editor,
        recipe: Recipe::DirLastArg(&[]),
        program: program.to_string(),
        open_arg: None,
        source: ToolSource::Custom,
    }
}

/// `synthesize_recipe(Terminal, Executable)` — no argv token at all, the target
/// IS the cwd.
fn browsed_terminal(program: &str) -> PickedTool {
    PickedTool {
        kind: ToolKind::Terminal,
        recipe: Recipe::DirCwd(&[]),
        program: program.to_string(),
        open_arg: None,
        source: ToolSource::Custom,
    }
}

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
        "-rf --no-preserve-root",     // leading dash
        "/tmp/café/Ünïcode/日本語/Ж", // unicode
    ];
    for raw in hostile {
        let path = PathBuf::from(raw);
        let spec = spec_from(&browsed_editor("editor"), &path);
        assert_eq!(spec.program, "editor", "program never becomes the path");
        assert_eq!(
            spec.args.len(),
            1,
            "path is exactly ONE arg for {raw:?}: {:?}",
            spec.args
        );
        assert_eq!(
            spec.args[0],
            path.display().to_string(),
            "arg is the path verbatim"
        );
        // LOW-1: the child does not launch from the (hostile) repo directory.
        assert_eq!(
            spec.cwd,
            safe_cwd(),
            "argument delivery keeps the neutral cwd"
        );
    }
}

/// A hostile path delivered as the WORKING DIRECTORY (the terminal rungs that
/// have no directory argument) never turns into an argument, so it cannot be
/// read as a flag or a script name however it is spelled.
#[test]
fn hostile_path_as_working_dir_never_becomes_an_argument() {
    for raw in [
        r#"C:\a b & c\repo"#,
        "/tmp/-rf",
        "/tmp/x;y",
        "/tmp/café/日本語",
    ] {
        let path = PathBuf::from(raw);
        let spec = spec_from(&browsed_terminal("pwsh"), &path);
        assert_eq!(spec.program, "pwsh");
        assert!(
            spec.args.is_empty(),
            "no argv token for {raw:?}: {:?}",
            spec.args
        );
        assert_eq!(spec.cwd, path, "the directory IS the delivery here");
        assert!(!spec.hide_console, "a terminal window must stay visible");
    }
}

/// P112 §4, replacing `configured_program_shape_rules_hold_before_any_spec_is_built`:
/// the shape rules are gone because the STRING is gone. There is no
/// `program: &str` parameter on any entry point, so a renderer-written command
/// line (`powershell -c calc`, `code&calc`, `./code`, `\\server\share\evil.exe`)
/// has no way in at all — the only inputs are `None` and a [`PickedTool`].
///
/// Asserted the only way a "there is no such parameter" property can be: the
/// auto arm (`None`) builds specs whose `program` is a catalog literal, and every
/// argv token is a catalog literal or the target directory (AC6).
#[test]
fn the_auto_arm_can_only_name_catalog_programs() {
    let path = PathBuf::from("/tmp/work");
    let p = path.display().to_string();
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        for spec in editor_ladder(os, None, &path)
            .into_iter()
            .chain(terminal_ladder(os, None, &path))
        {
            assert!(
                !spec.program.contains(' ') && !spec.program.contains(&p),
                "{os:?}: program is a bare catalog name, never a command line: {:?}",
                spec.program
            );
            for arg in &spec.args {
                assert!(
                    arg == &p || arg.ends_with(&p) || !arg.contains(&p),
                    "{os:?}: the target dir is its own token or a catalog prefix + it: {arg:?}"
                );
            }
        }
    }
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
    let ladder = editor_ladder(TargetOs::MacOs, None, &path);
    assert_eq!(ladder.len(), 3, "open -a VS Code, open -a Insiders, code");
    assert_eq!(ladder[0].program, "open");
    assert!(
        ladder[0].wait_for_exit,
        "rung 1 must judge open's exit code"
    );
    assert_eq!(ladder[1].program, "open");
    assert!(
        ladder[1].wait_for_exit,
        "rung 2 must judge open's exit code"
    );
    assert_eq!(ladder[2].program, "code");
    assert!(
        !ladder[2].wait_for_exit,
        "the plain CLI rung stays detached"
    );
}

/// (b) No Windows or Linux spec ever waits — editor, terminal, or reveal.
#[test]
fn windows_linux_specs_never_wait_for_exit() {
    let path = PathBuf::from("/tmp/work");
    for os in [TargetOs::Windows, TargetOs::Linux] {
        for spec in editor_ladder(os, None, &path)
            .into_iter()
            .chain(terminal_ladder(os, None, &path))
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
