//! The renderer-writable surface, pinned (P112 AC5, AC6, AC7): what a stored
//! setting can and cannot name.
//!
//! Everything here goes through the explicit-parameter half `picked_from`, so
//! nothing populates the process scan cache or spawns `reg.exe` (the
//! `scan_tests` rule).

use std::path::PathBuf;

use crate::external::TargetOs;

use super::catalog::{self, Rung, CUSTOM_ID};
use super::fake::FakeToolEnv;
use super::{
    coerce_tool_id, legacy_tool_id, legacy_tool_stem, picked_from, Recipe, Resolution, ToolEntry,
    ToolKind, ToolSource,
};

const OSES: [TargetOs; 3] = [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux];
const KINDS: [ToolKind; 2] = [ToolKind::Terminal, ToolKind::Editor];

/// The AC5 table: every shape a compromised renderer might write into
/// `terminalTool` / `editorTool`, including one path that genuinely exists on
/// this machine — existence must never matter.
fn hostile_values() -> Vec<String> {
    let existing = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        // A miss here would weaken the table, so fail the test rather than
        // silently drop the row.
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
        r"\\server\share\x.exe",
        // The browse path now ACCEPTS this shape (AMEND-6) — but only from the
        // dialog, into `custom_*_path`. As a renderer-written *selection* it is
        // still nothing.
        "//host/share/x.exe",
        "Custom",
        "CUSTOM",
        " custom",
        "vscode ",
        "",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    values.push(existing);
    values.push("a".repeat(2000));
    values.push("payl\u{202e}exe.txt".to_string());
    values
}

/// A stored browsed path this host's rules accept, so the hostile table is
/// asserted in the state that is *most* favourable to an attacker: a browsed
/// tool already exists, and the selection still cannot be redirected.
fn stored_browsed_program(dir: &std::path::Path) -> PathBuf {
    let path = if cfg!(windows) {
        dir.join("Portable.exe")
    } else {
        dir.join("portable")
    };
    std::fs::write(&path, b"stub").expect("write stub program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("set the execute bit");
    }
    path
}

// ---- AC5: the hostile-selection table ----------------------------------------

#[test]
fn no_hostile_selection_coerces_to_anything() {
    for value in hostile_values() {
        for kind in KINDS {
            // With and without a stored browsed path: having browsed once must
            // not turn some *other* string into a selection.
            for has_custom in [false, true] {
                assert_eq!(
                    coerce_tool_id(&value, kind, has_custom),
                    "",
                    "{value:?} ({kind:?}, has_custom={has_custom}) must coerce to nothing"
                );
            }
        }
    }
}

#[test]
fn no_hostile_selection_picks_a_tool() {
    let scratch = crate::testutil::scratch_dir();
    let stored = stored_browsed_program(scratch.path());
    let stored = stored.to_string_lossy().into_owned();
    // A populated row set, so a miss cannot be an artefact of "nothing was
    // detected": `cmd` is BuiltIn and `vscode` resolved to a real-looking path.
    let probe = r"C:\Program Files\Microsoft VS Code\Code.exe";
    let env = FakeToolEnv::new().file(probe);
    let rows = vec![
        row(ToolKind::Terminal, "cmd", TargetOs::Windows, built_in("cmd")),
        row(
            ToolKind::Editor,
            "vscode",
            TargetOs::Windows,
            exe(probe, ToolSource::WellKnown),
        ),
    ];
    for value in hostile_values() {
        for kind in KINDS {
            assert_eq!(
                picked_from(&env, TargetOs::Windows, &rows, &value, kind, &stored),
                None,
                "{value:?} ({kind:?}) must select nothing"
            );
        }
    }
}

#[test]
fn custom_selects_only_when_a_path_was_browsed() {
    assert_eq!(coerce_tool_id(CUSTOM_ID, ToolKind::Editor, false), "");
    assert_eq!(coerce_tool_id(CUSTOM_ID, ToolKind::Editor, true), CUSTOM_ID);
    assert_eq!(coerce_tool_id(CUSTOM_ID, ToolKind::Terminal, true), CUSTOM_ID);

    let env = FakeToolEnv::new();
    let os = TargetOs::host();
    assert_eq!(
        picked_from(&env, os, &[], CUSTOM_ID, ToolKind::Editor, ""),
        None,
        "nothing browsed ⇒ nothing selected"
    );
    let scratch = crate::testutil::scratch_dir();
    let stored = stored_browsed_program(scratch.path());
    let stored = stored.to_string_lossy().into_owned();
    let picked = picked_from(&env, os, &[], CUSTOM_ID, ToolKind::Editor, &stored)
        .expect("a browsed program is selectable");
    assert_eq!(picked.program, stored);
    assert_eq!(picked.source, ToolSource::Custom);
}

#[test]
fn a_catalog_id_survives_coercion_on_every_os_and_is_kind_scoped() {
    // Kept even when this host cannot detect it (the OQ1 ruling): a settings
    // file synced from another machine still names its tool.
    for (kind, id) in [
        (ToolKind::Editor, "vscode"),
        (ToolKind::Editor, "notepadpp"),
        (ToolKind::Editor, "kate"),
        (ToolKind::Terminal, "cmd"),
        (ToolKind::Terminal, "iterm2"),
        (ToolKind::Terminal, "gnome-terminal"),
    ] {
        assert_eq!(coerce_tool_id(id, kind, false), id);
        // …but only for its own kind.
        let other = match kind {
            ToolKind::Editor => ToolKind::Terminal,
            ToolKind::Terminal => ToolKind::Editor,
        };
        assert_eq!(coerce_tool_id(id, other, false), "", "{id} is kind-scoped");
    }
}

// ---- AC6: provenance ---------------------------------------------------------

/// Every program a *selection* can produce is a catalog literal, `"open"`, the
/// path a probe resolved, or the stored browsed path — asserted across the
/// whole catalog on all three OSes.
///
/// This is the selection half of AC6. The `LaunchSpec` half (every argv token
/// is a catalog `&'static str`, a catalog prefix + the target dir, or the
/// target dir) lands with the launch rewrite, which is where `spec_from`
/// appears.
#[test]
fn a_picked_program_always_came_from_the_catalog_a_probe_or_the_browsed_path() {
    for kind in KINDS {
        for os in OSES {
            for entry in catalog::entries_for(kind, os) {
                let (env, res, probe_path) = fixture_for(entry, os);
                let picked = picked_from(&env, os, &[(entry, res)], entry.id, kind, "")
                    .unwrap_or_else(|| panic!("{} must resolve for {os:?}", entry.id));
                let allowed = [entry.program.to_string(), "open".to_string(), probe_path];
                assert!(
                    allowed.contains(&picked.program),
                    "{} produced an unaccounted program {:?}",
                    entry.id,
                    picked.program
                );
                if picked.program == "open" {
                    assert_eq!(picked.recipe, Recipe::MacOpen);
                    assert!(picked.open_arg.is_some(), "{} has no open arg", entry.id);
                }
            }
        }
    }
}

// ---- AC7: migration of the legacy free-text commands -------------------------

#[test]
fn a_legacy_command_maps_to_a_catalog_id_or_to_nothing() {
    for (legacy, kind, want) in [
        ("code {path}", ToolKind::Editor, "vscode"),
        ("code", ToolKind::Editor, "vscode"),
        ("wt -d {path}", ToolKind::Terminal, "windows-terminal"),
        ("cmd /K", ToolKind::Terminal, "cmd"),
        // The user asked for PowerShell; dropping `-c calc` is the point.
        ("powershell -c calc", ToolKind::Terminal, "powershell"),
        ("subl", ToolKind::Editor, "sublime"),
        // A full path still normalises to its stem, so a KNOWN program is not
        // lost just because it was written as a path.
        (r"C:\Tools\npp\notepad++.exe", ToolKind::Editor, "notepadpp"),
        ("/usr/bin/konsole", ToolKind::Terminal, "konsole"),
        // Misses: accepted, not preserved.
        (r"C:\Tools\payload.exe", ToolKind::Editor, ""),
        (r"C:\Program Files\X\x.exe", ToolKind::Editor, ""),
        ("make", ToolKind::Terminal, ""),
        ("node", ToolKind::Editor, ""),
        ("/opt/custom/bin/myed", ToolKind::Editor, ""),
        ("", ToolKind::Editor, ""),
        ("   ", ToolKind::Editor, ""),
        ("{path}", ToolKind::Editor, ""),
        // Kind-scoped: a terminal command is not an editor.
        ("wt -d {path}", ToolKind::Editor, ""),
        ("code {path}", ToolKind::Terminal, ""),
    ] {
        assert_eq!(
            legacy_tool_id(legacy, kind),
            want,
            "{legacy:?} ({kind:?}) must migrate to {want:?}"
        );
    }
}

/// Invariant 1, the most important one: migration cannot manufacture the human
/// dialog click the browsed path requires, on ANY input.
#[test]
fn migration_can_never_produce_the_custom_pseudo_id_or_a_path() {
    let mut inputs = hostile_values();
    inputs.push(CUSTOM_ID.to_string());
    inputs.push(r"custom C:\payload.exe".to_string());
    for legacy in inputs {
        for kind in KINDS {
            let id = legacy_tool_id(&legacy, kind);
            assert_ne!(id, CUSTOM_ID, "{legacy:?} migrated to the custom pseudo-id");
            assert!(
                id.is_empty() || catalog::find(kind, &id).is_some(),
                "{legacy:?} migrated to {id:?}, which is not a catalog id"
            );
            // A stored string never becomes a program string.
            assert!(!id.contains(['/', '\\', ' ', ':']), "{id:?} looks like a path");
        }
    }
}

/// Every alias target is a catalog id of its own kind, and none is the reserved
/// pseudo-id — so no legacy value can migrate into the browsed slot.
/// (`catalog_tests` pins existence; this pins the `CUSTOM_ID` half.)
#[test]
fn no_legacy_alias_targets_the_custom_pseudo_id() {
    for ((kind, alias), id) in catalog::LEGACY_ALIASES {
        assert_ne!(*id, CUSTOM_ID, "alias {alias:?} targets the pseudo-id");
        assert!(
            catalog::find(*kind, id).is_some(),
            "alias {alias:?} targets the unknown id {id:?}"
        );
        assert_eq!(
            *alias,
            alias.to_lowercase(),
            "alias {alias:?} can never match: lookup lowercases"
        );
    }
}

/// Migration is idempotent at the function level: re-running it on its own
/// output yields `""`, so a second pass can only ever clear — it can never
/// promote an id into something else.
#[test]
fn re_migrating_a_migrated_id_yields_nothing() {
    for ((kind, _), id) in catalog::LEGACY_ALIASES {
        let again = legacy_tool_id(id, *kind);
        assert!(
            again.is_empty() || again == *id,
            "{id:?} re-migrated to {again:?}"
        );
    }
}

/// [`legacy_tool_stem`] exists so the §5.3 migration can LOG what it dropped,
/// which makes exactly one property load-bearing: the stem carries no directory
/// and no argument, so a diagnostic cannot leak a user's install path. Pinned on
/// both separators, because the stem must answer the same way on every host.
#[test]
fn the_logged_stem_keeps_no_directory_and_no_argument() {
    for (legacy, want) in [
        (r"C:\Tools\npp\notepad++.exe -multiInst", "notepad++"),
        ("/opt/custom/bin/MyEd {path}", "myed"),
        ("powershell -c calc", "powershell"),
        ("myed", "myed"),
        ("   ", ""),
        ("{path}", ""),
    ] {
        let stem = legacy_tool_stem(legacy);
        assert_eq!(stem, want, "{legacy:?}");
        assert!(
            !stem.contains(['/', '\\', ' ']),
            "{stem:?} would put a user path in a log"
        );
    }
    // …and it is genuinely the key the lookup consults, not a parallel
    // normalisation that could drift away from it.
    assert_eq!(legacy_tool_stem("code {path}"), "code");
    assert_eq!(legacy_tool_id("code {path}", ToolKind::Editor), "vscode");
}

// ---- helpers -----------------------------------------------------------------

fn row(kind: ToolKind, id: &str, os: TargetOs, res: Resolution) -> (&'static ToolEntry, Resolution) {
    let entry = catalog::find_for(kind, id, os).expect("catalog row (AC8 pins totality)");
    (entry, res)
}

fn built_in(program: &str) -> Resolution {
    Resolution {
        program: program.to_string(),
        bundle: None,
        source: ToolSource::BuiltIn,
    }
}

fn exe(path: &str, source: ToolSource) -> Resolution {
    Resolution {
        program: path.to_string(),
        bundle: None,
        source,
    }
}

/// A resolution + env that make `entry` resolve, shaped like the rung that
/// would really have produced it. Returns the probe-derived path so the
/// provenance assertion can name it.
fn fixture_for(entry: &'static ToolEntry, os: TargetOs) -> (FakeToolEnv, Resolution, String) {
    if entry.rungs.iter().any(|r| matches!(r, Rung::BuiltIn)) {
        return (
            FakeToolEnv::new(),
            built_in(entry.program),
            entry.program.to_string(),
        );
    }
    if entry.recipe == Recipe::MacOpen {
        let bundle = format!("/Applications/{}.app", entry.app_name.unwrap_or(entry.id));
        let env = FakeToolEnv::new().bundle(&bundle);
        let res = Resolution {
            program: "open".to_string(),
            bundle: Some(bundle.clone()),
            source: ToolSource::AppBundle,
        };
        return (env, res, bundle);
    }
    let path = match os {
        TargetOs::Windows => format!(r"C:\Program Files\{}\{}", entry.id, entry.program),
        TargetOs::MacOs | TargetOs::Linux => format!("/usr/bin/{}", entry.program),
    };
    let env = FakeToolEnv::new().file(&path);
    let res = exe(&path, ToolSource::Path);
    (env, res, path)
}
