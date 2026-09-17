//! P112 §4 — the **picked** launch path: [`super::spec_from`] and the
//! single-spec ladders a selection produces.
//!
//! Its own file so `external_tests.rs` stays the untouched AC9 record of the
//! auto ladders. Nothing here spawns; every case inspects the built
//! [`super::LaunchSpec`].
//!
//! Covers **AC17** (the browsed-tool launch shapes), the launch half of **AC6**
//! (provenance: every `program` and every argv token traces to a catalog
//! `&'static str`, `"open"`, a probe-derived absolute path, or the target
//! directory) and the "a selection short-circuits the ladder" property that
//! `editor_program_is_the_only_candidate_tried` used to assert for the deleted
//! free-text setting.

use super::fake::FakeRunner;
use super::*;
use crate::tools::catalog::{Recipe, CATALOG};
use crate::tools::{synthesize_recipe, CustomKindShape, PickedTool, ToolKind, ToolSource};

fn p() -> PathBuf {
    PathBuf::from("/tmp/work")
}

/// Exactly what `tools::picked` builds for a browsed selection, so the shapes
/// asserted here are the shapes that launch.
fn browsed(kind: ToolKind, shape: CustomKindShape, target: &str) -> PickedTool {
    let recipe = synthesize_recipe(kind, shape);
    match shape {
        CustomKindShape::MacBundle => PickedTool {
            kind,
            recipe,
            program: "open".to_string(),
            open_arg: Some(target.to_string()),
            source: ToolSource::Custom,
        },
        CustomKindShape::Executable => PickedTool {
            kind,
            recipe,
            program: target.to_string(),
            open_arg: None,
            source: ToolSource::Custom,
        },
    }
}

// ---- AC17: browsed-tool launch shapes ----------------------------------

/// Editor + `Executable`: the folder is ONE argv token, the console is hidden
/// (a `.cmd` shim would flash one), and the child launches from the neutral cwd
/// — never from the repo (LOW-1).
#[test]
fn browsed_editor_executable_gets_one_argv_token_and_the_neutral_cwd() {
    let tool = browsed(
        ToolKind::Editor,
        CustomKindShape::Executable,
        r"C:\Portable\Editor.exe",
    );
    let s = spec_from(&tool, &p());
    assert_eq!(s.program, r"C:\Portable\Editor.exe");
    assert_eq!(s.args, vec!["/tmp/work".to_string()]);
    assert_eq!(s.cwd, safe_cwd());
    assert!(s.hide_console);
    assert!(
        !s.wait_for_exit,
        "an editor session must never be waited on"
    );
}

/// A browsed program path **containing a space** stays ONE verbatim
/// `LaunchSpec.program` token.
///
/// Restored 2026-09-15 from the deleted
/// `external_cmd_tests::program_spec_absolute_program_is_used_verbatim`, which
/// pinned `/opt/My Editor/bin/edit`; the replacement above uses a space-free
/// `C:\Portable\Editor.exe` and so no longer covers it. Structurally guaranteed
/// today — there is no program STRING left to split, `PickedTool::program` is
/// moved into `LaunchSpec::program` whole, and `SpawnRunner` hands it to
/// `Command::new` — which is exactly why the line is cheap: it is what fails the
/// day someone reintroduces shell-style splitting or quoting on the program.
#[test]
fn a_browsed_program_path_with_a_space_survives_as_one_verbatim_token() {
    let target = "/opt/My Editor/bin/edit";
    for kind in [ToolKind::Terminal, ToolKind::Editor] {
        let tool = browsed(kind, CustomKindShape::Executable, target);
        let s = spec_from(&tool, &p());
        assert_eq!(s.program, target, "{kind:?}: not split on the space");
        assert!(
            !s.program.contains('"'),
            "{kind:?}: no shell quoting is added"
        );
    }
}

/// Terminal + `Executable`: NO arguments at all and the target IS the cwd —
/// the only mechanism "open a terminal here" has for an unknown program, and
/// audit route 3's shape, reachable only for a program a human browsed to.
#[test]
fn browsed_terminal_executable_takes_no_args_and_the_target_as_cwd() {
    let tool = browsed(
        ToolKind::Terminal,
        CustomKindShape::Executable,
        "/usr/local/bin/myterm",
    );
    let s = spec_from(&tool, &p());
    assert_eq!(s.program, "/usr/local/bin/myterm");
    assert!(s.args.is_empty(), "no argv token: {:?}", s.args);
    assert_eq!(s.cwd, p());
    assert!(!s.hide_console, "a terminal window MUST be visible");
    assert!(!s.wait_for_exit);
}

/// Either kind + `MacBundle`: `open -a <bundle> <dir>`, the bundle as exactly
/// ONE token (so a bundle path with spaces cannot split), and `wait_for_exit` —
/// `open`'s exit code is the only "Unable to find application" signal.
#[test]
fn a_browsed_bundle_launches_through_open_and_waits() {
    for kind in [ToolKind::Terminal, ToolKind::Editor] {
        let bundle = "/Applications/My Portable Editor.app";
        let tool = browsed(kind, CustomKindShape::MacBundle, bundle);
        let s = spec_from(&tool, &p());
        assert_eq!(s.program, "open");
        assert_eq!(
            s.args,
            vec![
                "-a".to_string(),
                bundle.to_string(),
                "/tmp/work".to_string()
            ]
        );
        assert_eq!(s.args.len(), 3, "the bundle is ONE token, spaces and all");
        assert_eq!(s.cwd, safe_cwd());
        assert!(
            s.wait_for_exit,
            "open reports 'not found' through its exit code"
        );
        assert_eq!(s.hide_console, kind == ToolKind::Editor);
    }
}

/// `hide_console` is a function of the KIND and nothing else, on every recipe.
#[test]
fn hide_console_follows_the_kind_for_every_recipe() {
    for recipe in [
        Recipe::DirLastArg(&["-x"]),
        Recipe::DirJoinedArg(&[], "--cd="),
        Recipe::DirCwd(&["/K"]),
        Recipe::MacOpen,
    ] {
        for kind in [ToolKind::Terminal, ToolKind::Editor] {
            let tool = PickedTool {
                kind,
                recipe,
                program: if matches!(recipe, Recipe::MacOpen) {
                    "open"
                } else {
                    "tool"
                }
                .to_string(),
                open_arg: matches!(recipe, Recipe::MacOpen).then(|| "App".to_string()),
                source: ToolSource::BuiltIn,
            };
            assert_eq!(
                spec_from(&tool, &p()).hide_console,
                kind == ToolKind::Editor,
                "{kind:?} / {recipe:?}"
            );
        }
    }
}

/// The two argv recipes differ in ONE way: `DirJoinedArg` fuses its prefix onto
/// the directory (`--working-directory=/tmp/work`), `DirLastArg` keeps them
/// apart (`--workdir` `/tmp/work`). A path with spaces must not split either way.
#[test]
fn the_fixed_prefix_recipes_place_the_directory_exactly_as_the_catalog_says() {
    let spacey = PathBuf::from(r"C:\My Repos\bon sai");
    let joined = PickedTool {
        kind: ToolKind::Terminal,
        recipe: Recipe::DirJoinedArg(&[], "--working-directory="),
        program: "gnome-terminal".to_string(),
        open_arg: None,
        source: ToolSource::Path,
    };
    let s = spec_from(&joined, &spacey);
    assert_eq!(
        s.args,
        vec![format!("--working-directory={}", spacey.display())]
    );

    let last = PickedTool {
        recipe: Recipe::DirLastArg(&["start", "--cwd"]),
        program: "wezterm".to_string(),
        ..joined
    };
    let s = spec_from(&last, &spacey);
    assert_eq!(
        s.args,
        vec![
            "start".to_string(),
            "--cwd".to_string(),
            spacey.display().to_string()
        ]
    );
}

// ---- AC6 (launch half): provenance --------------------------------------

/// For EVERY catalog entry, the spec a `Name`-style selection builds names the
/// entry's own `program` and every argv token is either one of that entry's
/// catalog `&'static str`s or the target directory (possibly fused onto a
/// catalog prefix). No token is ever free text, because there is no free-text
/// constructor left.
#[test]
fn every_catalog_entry_launches_only_catalog_strings_and_the_target_dir() {
    let dir = p();
    let d = dir.display().to_string();
    for e in CATALOG {
        let tool = PickedTool {
            kind: e.kind,
            recipe: e.recipe,
            program: e.program.to_string(),
            open_arg: e.app_name.map(str::to_string),
            source: ToolSource::Path,
        };
        let s = spec_from(&tool, &dir);
        let fixed: &[&str] = match e.recipe {
            Recipe::DirLastArg(f) | Recipe::DirJoinedArg(f, _) | Recipe::DirCwd(f) => f,
            Recipe::MacOpen => &["-a"],
        };
        assert_eq!(
            s.program, e.program,
            "{}: program is the catalog literal",
            e.id
        );
        for arg in &s.args {
            let is_catalog = fixed.contains(&arg.as_str())
                || e.app_name.is_some_and(|a| a == arg)
                || arg == "-a";
            let is_target = arg == &d || arg.ends_with(&d);
            assert!(
                is_catalog || is_target,
                "{}: stray argv token {arg:?}",
                e.id
            );
        }
        // The cwd is either the neutral one or the target — never anything else.
        assert!(
            s.cwd == safe_cwd() || s.cwd == dir,
            "{}: unexpected cwd",
            e.id
        );
    }
}

/// A `MacOpen` selection with no `open_arg` cannot be produced by `tools::picked`
/// or by the auto arm, so [`spec_from`] answers it with a `debug_assert!` plus a
/// spec that merely fails to launch — never an `expect()` on a user's launch.
///
/// **Both halves are now pinned, each in the configuration it exists in.** This
/// test was `#[cfg(not(debug_assertions))]` until 2026-09-15, and nothing in the
/// gate builds `--release`: it therefore compiled in NO configuration anyone
/// runs — not passing, not failing, simply absent. What runs where now:
///   * under `debug_assertions` (every gate run) the `debug_assert!` MUST fire,
///     and `should_panic` — with its message prefix — is the assertion;
///   * under `--release` the body's assertions pin the degradation
///     (`open -a "" <dir>`, which fails at spawn and falls through the ladder).
#[test]
#[cfg_attr(
    debug_assertions,
    should_panic(expected = "a MacOpen selection always carries")
)]
fn a_mac_open_selection_without_an_app_argument_debug_asserts_then_degrades() {
    let tool = PickedTool {
        kind: ToolKind::Editor,
        recipe: Recipe::MacOpen,
        program: "open".to_string(),
        open_arg: None,
        source: ToolSource::Path,
    };
    // Panics HERE under `debug_assertions`; a release build walks on.
    let s = spec_from(&tool, &p());
    #[cfg(not(debug_assertions))]
    {
        assert_eq!(s.program, "open");
        assert_eq!(
            s.args,
            vec!["-a".to_string(), String::new(), "/tmp/work".to_string()]
        );
    }
    // Debug: the line above never returns, but the binding must still be used.
    #[cfg(debug_assertions)]
    let _ = &s;
}

// ---- a selection short-circuits the ladder ------------------------------

/// A selection yields EXACTLY one candidate on every OS, for both kinds — the
/// auto ladder is not appended as a fallback (OQ1: a gone tool is handled by
/// `tools::picked` returning `None`, not by the launcher trying more rungs).
#[test]
fn a_selection_is_the_only_candidate_on_every_os() {
    let ed = browsed(ToolKind::Editor, CustomKindShape::Executable, "/opt/ed");
    let term = browsed(ToolKind::Terminal, CustomKindShape::Executable, "/opt/term");
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        assert_eq!(
            editor_ladder(os, Some(&ed), &p()),
            vec![spec_from(&ed, &p())]
        );
        assert_eq!(
            terminal_ladder(os, Some(&term), &p()),
            vec![spec_from(&term, &p())]
        );
    }
}

/// End to end through `launch_first`: only the selected program is attempted,
/// and on failure it is what the error names.
#[test]
fn the_selected_program_is_the_only_one_spawned() {
    let ed = browsed(ToolKind::Editor, CustomKindShape::Executable, "my-editor");
    let runner = FakeRunner::new(&[]);
    let err = open_in_editor(&runner, TargetOs::Windows, Some(&ed), &p())
        .expect_err("the selected program is missing");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(err.to_string().contains("my-editor"));
    assert_eq!(runner.calls(), vec!["my-editor"]);
}
