//! Selection: the launch-time recheck, by source.

use super::*;

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
