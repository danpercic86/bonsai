//! Probe-ladder tests (P112 AC2, AC3, AC4-detection, AC20).
//!
//! **The central test-design requirement (AC2): all three OS ladders run on ONE
//! machine.** Every test below passes an explicit [`TargetOs`] and a
//! [`FakeToolEnv`], so the Windows, macOS and Linux rungs all execute wherever
//! the suite runs. Nothing here touches the real registry or spawns anything;
//! the two tests that touch the real machine are the scratch-dir bundle test
//! (filesystem only) and the `#[ignore]`d host test.

use std::path::Path;
use std::time::Instant;

use crate::external::TargetOs;

use super::catalog;
use super::detect::{probe_entry, scan_for, HostToolEnv, ToolEnv};
use super::fake::FakeToolEnv;
use super::{ToolKind, ToolSource};

const APP_PATHS_HKCU_CODE: &str =
    r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\Code.exe";
const APP_PATHS_HKLM_CODE: &str =
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\Code.exe";

/// `(id, program)` pairs of a scan, for compact assertions.
fn summary(rows: &[(&'static super::ToolEntry, super::Resolution)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(e, r)| (e.id.to_string(), r.program.clone()))
        .collect()
}

fn probe(env: &FakeToolEnv, kind: ToolKind, id: &str, os: TargetOs) -> Option<super::Resolution> {
    let entry = catalog::find_for(kind, id, os).expect("catalog row (AC8 pins totality)");
    probe_entry(env, entry)
}

// ---- AC2: every rung kind, and rung ORDER, per OS ----------------------------

#[test]
fn windows_on_path_rung_wins_over_the_later_well_known_rung() {
    let env = FakeToolEnv::new()
        .on_path("wt", r"C:\bin\wt.exe")
        .exe(r"C:\bin\wt.exe")
        // Rung 2 would ALSO hit — the assertion is that rung 1 short-circuits.
        .var("LOCALAPPDATA", r"C:\Users\ada\AppData\Local")
        .file(r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe");
    let res = probe(&env, ToolKind::Terminal, "windows-terminal", TargetOs::Windows)
        .expect("wt resolves on PATH");
    assert_eq!(res.source, ToolSource::Path);
    assert_eq!(res.program, r"C:\bin\wt.exe");
    assert_eq!(res.bundle, None);
}

#[test]
fn windows_well_known_folder_rung_joins_the_env_var_host_independently() {
    let env = FakeToolEnv::new()
        .var("LOCALAPPDATA", r"C:\Users\ada\AppData\Local")
        .file(r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe");
    let res = probe(&env, ToolKind::Terminal, "windows-terminal", TargetOs::Windows)
        .expect("wt resolves under LOCALAPPDATA");
    assert_eq!(res.source, ToolSource::WellKnown);
    // Backslash-joined regardless of the host separator, so the Windows ladder
    // behaves identically under a Linux/macOS run.
    assert_eq!(
        res.program,
        r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe"
    );
}

#[test]
fn app_paths_rung_asks_hkcu_then_hklm_for_the_default_value() {
    let env = FakeToolEnv::new().registry(
        APP_PATHS_HKLM_CODE,
        "",
        r#""C:\Program Files\Microsoft VS Code\Code.exe""#,
    );
    let res = probe(&env, ToolKind::Editor, "vscode", TargetOs::Windows);
    // The registry value exists but the file does not, so the rung misses …
    assert_eq!(res, None);
    // … and it asked for exactly these keys, in this order, with the EMPTY
    // value name that means "the key's default value" (the `/ve` convention).
    assert_eq!(
        env.registry_calls(),
        vec![
            (APP_PATHS_HKCU_CODE.to_string(), String::new()),
            (APP_PATHS_HKLM_CODE.to_string(), String::new()),
        ]
    );
}

#[test]
fn app_paths_rung_trims_quotes_and_yields_registry_provenance() {
    let code = r"C:\Program Files\Microsoft VS Code\Code.exe";
    let env = FakeToolEnv::new()
        .registry(APP_PATHS_HKCU_CODE, "", &format!("\"{code}\""))
        .file(code);
    let res = probe(&env, ToolKind::Editor, "vscode", TargetOs::Windows).expect("App Paths hit");
    assert_eq!(res.source, ToolSource::Registry);
    assert_eq!(res.program, code);
}

#[test]
fn a_windows_path_hit_without_an_extension_is_not_launchable_so_the_ladder_falls_through() {
    // Observed on a real Windows box: `resolve_on_path("code")` returns VS
    // Code's extension-LESS POSIX shim `…\bin\code`, which Windows cannot
    // execute. Offering it would list a tool that then fails to launch, so the
    // rung must miss and the App Paths rung must win.
    let shim = r"C:\Users\ada\AppData\Local\Programs\Microsoft VS Code\bin\code";
    let real = r"C:\Users\ada\AppData\Local\Programs\Microsoft VS Code\Code.exe";
    let env = FakeToolEnv::new()
        .on_path("code", shim)
        .file(shim)
        .registry(APP_PATHS_HKCU_CODE, "", real)
        .file(real);
    let res = probe(&env, ToolKind::Editor, "vscode", TargetOs::Windows).expect("App Paths wins");
    assert_eq!(res.source, ToolSource::Registry);
    assert_eq!(res.program, real);
    // The rule is Windows-only: on unix an extension-less program is the norm
    // (`/usr/bin/code`), and the execute bit is the gate instead.
    let unix = FakeToolEnv::new().on_path("code", "/usr/bin/code").exe("/usr/bin/code");
    let res = probe(&unix, ToolKind::Editor, "vscode", TargetOs::Linux).expect("PATH hit");
    assert_eq!(res.program, "/usr/bin/code");
}

#[test]
fn built_in_rungs_touch_nothing() {
    // Deliberately the "nothing exists" env: `cmd` and `powershell` are present
    // by definition on Windows, and an `is_file("cmd")` test would be false and
    // would silently break picking them.
    let env = FakeToolEnv::new();
    for id in ["cmd", "powershell"] {
        let res = probe(&env, ToolKind::Terminal, id, TargetOs::Windows).expect("built in");
        assert_eq!(res.source, ToolSource::BuiltIn);
        assert_eq!(res.program, id);
    }
}

#[test]
fn mac_bundle_rung_hits_the_second_candidate_and_launches_via_open() {
    let env = FakeToolEnv::new().bundle("/Applications/Utilities/Terminal.app");
    let res = probe(&env, ToolKind::Terminal, "apple-terminal", TargetOs::MacOs)
        .expect("Terminal.app resolves");
    assert_eq!(res.source, ToolSource::AppBundle);
    assert_eq!(res.program, "open");
    assert_eq!(
        res.bundle.as_deref(),
        Some("/Applications/Utilities/Terminal.app")
    );
}

#[test]
fn a_home_relative_bundle_rung_joins_with_forward_slashes() {
    let env = FakeToolEnv::new()
        .home("/Users/ada")
        .bundle("/Users/ada/Applications/iTerm.app");
    let res =
        probe(&env, ToolKind::Terminal, "iterm2", TargetOs::MacOs).expect("~/Applications hit");
    // A `Path::join` here would emit `\`-mixed output on a Windows host, so the
    // probed bundle — an argv token AND the picker subtitle — would not be
    // byte-identical to what a Mac produces.
    assert_eq!(
        res.bundle.as_deref(),
        Some("/Users/ada/Applications/iTerm.app")
    );
}

#[test]
fn a_directory_that_is_not_a_real_bundle_is_not_detected() {
    // Modelled by leaving it out of the bundle set: `is_bundle` requires
    // `Contents/Info.plist`, which is what separates a real bundle from a
    // directory merely NAMED `*.app`. The host-level half of this is
    // `host_is_bundle_requires_an_info_plist` below.
    let env = FakeToolEnv::new().file("/Applications/Warp.app");
    assert_eq!(probe(&env, ToolKind::Terminal, "warp", TargetOs::MacOs), None);
}

#[test]
fn linux_unix_file_rung_requires_an_execute_bit() {
    let present_but_not_executable = FakeToolEnv::new().file("/usr/bin/konsole");
    assert_eq!(
        probe(
            &present_but_not_executable,
            ToolKind::Terminal,
            "konsole",
            TargetOs::Linux
        ),
        None
    );
    let executable = FakeToolEnv::new().exe("/usr/bin/konsole");
    let res =
        probe(&executable, ToolKind::Terminal, "konsole", TargetOs::Linux).expect("konsole hit");
    assert_eq!(res.source, ToolSource::WellKnown);
    assert_eq!(res.program, "/usr/bin/konsole");
}

#[test]
fn linux_unix_file_rungs_are_tried_in_table_order() {
    // alacritty: OnPath, /usr/bin, then /snap/bin — only the snap copy exists.
    let env = FakeToolEnv::new().exe("/snap/bin/alacritty");
    let res = probe(&env, ToolKind::Terminal, "alacritty", TargetOs::Linux).expect("snap hit");
    assert_eq!(res.program, "/snap/bin/alacritty");
}

#[test]
fn all_three_os_ladders_scan_on_one_machine() {
    // ONE env, three explicit target OSes: the AC2 headline. Each ladder sees
    // only its own rungs, so the same table answers all three.
    let env = FakeToolEnv::new()
        .var("LOCALAPPDATA", r"C:\Users\ada\AppData\Local")
        .file(r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe")
        .bundle("/Applications/Utilities/Terminal.app")
        .bundle("/Applications/Visual Studio Code.app")
        .on_path("gnome-terminal", "/usr/bin/gnome-terminal")
        .exe("/usr/bin/gnome-terminal")
        .exe("/usr/bin/code");

    assert_eq!(
        summary(&scan_for(&env, TargetOs::Windows)),
        vec![
            (
                "windows-terminal".to_string(),
                r"C:\Users\ada\AppData\Local\Microsoft\WindowsApps\wt.exe".to_string()
            ),
            ("powershell".to_string(), "powershell".to_string()),
            ("cmd".to_string(), "cmd".to_string()),
        ]
    );
    assert_eq!(
        summary(&scan_for(&env, TargetOs::MacOs)),
        vec![
            ("apple-terminal".to_string(), "open".to_string()),
            ("vscode".to_string(), "open".to_string()),
        ]
    );
    assert_eq!(
        summary(&scan_for(&env, TargetOs::Linux)),
        vec![
            (
                "gnome-terminal".to_string(),
                "/usr/bin/gnome-terminal".to_string()
            ),
            ("vscode".to_string(), "/usr/bin/code".to_string()),
        ]
    );
}

// ---- AC3: every rung degrades to `None` --------------------------------------

#[test]
fn nothing_exists_yields_an_empty_scan_apart_from_the_built_ins() {
    let env = FakeToolEnv::new();
    assert!(scan_for(&env, TargetOs::MacOs).is_empty());
    assert!(scan_for(&env, TargetOs::Linux).is_empty());
    // Windows keeps exactly its two `BuiltIn` rows — present by definition, and
    // the only rungs that legitimately answer without looking at anything.
    assert_eq!(
        scan_for(&env, TargetOs::Windows)
            .iter()
            .map(|(e, _)| e.id)
            .collect::<Vec<_>>(),
        vec!["powershell", "cmd"]
    );
}

#[test]
fn every_rung_degrades_under_a_failing_env() {
    // OnPath: PATH resolves, but the file does not exist.
    let ghost_on_path = FakeToolEnv::new().on_path("konsole", "/usr/bin/konsole");
    assert_eq!(
        probe(&ghost_on_path, ToolKind::Terminal, "konsole", TargetOs::Linux),
        None
    );
    // WinFolder: absent var, then an EMPTY var (which would otherwise join into
    // a relative `\Microsoft\WindowsApps\wt.exe`).
    assert_eq!(
        probe(
            &FakeToolEnv::new(),
            ToolKind::Terminal,
            "git-bash",
            TargetOs::Windows
        ),
        None
    );
    let empty_var = FakeToolEnv::new().var("ProgramFiles", "");
    assert_eq!(
        probe(&empty_var, ToolKind::Terminal, "git-bash", TargetOs::Windows),
        None
    );
    // AppPaths: absent key, and unparseable/garbage data.
    let garbage_registry = FakeToolEnv::new().registry(APP_PATHS_HKCU_CODE, "", "   ");
    assert_eq!(
        probe(
            &garbage_registry,
            ToolKind::Editor,
            "vscode",
            TargetOs::Windows
        ),
        None
    );
    // A RELATIVE candidate never resolves, even when the file "exists": a
    // relative program would be searched against the process cwd.
    let relative_registry = FakeToolEnv::new()
        .registry(APP_PATHS_HKCU_CODE, "", "Code.exe")
        .file("Code.exe");
    assert_eq!(
        probe(
            &relative_registry,
            ToolKind::Editor,
            "vscode",
            TargetOs::Windows
        ),
        None
    );
}

#[test]
fn a_unc_or_drive_relative_candidate_is_never_a_hit() {
    for cand in [
        // A remote share is not a local tool (and stat-ing one inside a
        // budgeted scan goes to the network).
        r"\\server\share\Code.exe",
        "//host/share/Code.exe",
        // Drive-RELATIVE on Windows, so it would resolve against the process
        // cwd — the same class of bug as an empty PATH component.
        r"\Windows\Code.exe",
    ] {
        let env = FakeToolEnv::new()
            .registry(APP_PATHS_HKCU_CODE, "", cand)
            .file(cand);
        assert_eq!(
            probe(&env, ToolKind::Editor, "vscode", TargetOs::Windows),
            None,
            "{cand} must not be a hit"
        );
    }
}

#[test]
fn an_exhausted_registry_budget_makes_every_remaining_app_paths_rung_miss() {
    let code = r"C:\Program Files\Microsoft VS Code\Code.exe";
    // Budget 0 ⇒ the HKCU read is refused even though the value is there.
    let spent = FakeToolEnv::new()
        .registry(APP_PATHS_HKCU_CODE, "", code)
        .file(code)
        .registry_budget(0);
    assert_eq!(
        probe(&spent, ToolKind::Editor, "vscode", TargetOs::Windows),
        None
    );
    // Budget 1 ⇒ HKCU consumes it, so the HKLM fallback (where the value
    // actually is) is refused. Degradation only means "not offered".
    let one_read = FakeToolEnv::new()
        .registry(APP_PATHS_HKLM_CODE, "", code)
        .file(code)
        .registry_budget(1);
    assert_eq!(
        probe(&one_read, ToolKind::Editor, "vscode", TargetOs::Windows),
        None
    );
}

#[test]
fn host_registry_reads_stop_at_the_scan_budget_without_spawning() {
    // A deadline in the past ⇒ the pre-spawn cut-off fires. `HKCR\.txt`'s
    // default value exists on every Windows install, so a `Some` here would
    // prove the gate was skipped. (Vacuous on non-Windows, where
    // `registry_string` is inert by design.)
    let spent = HostToolEnv::with_deadline(Instant::now());
    assert_eq!(spent.registry_string(r"HKCR\.txt", ""), None);
}

#[test]
fn detection_never_constructs_a_child_process_for_a_candidate() {
    // `ToolEnv` has no run/spawn method, and the prober builds no `Command`.
    // The ONLY process detection ever starts is `reg.exe`, via `gitbin` — a
    // reader of the registry, never a candidate tool.
    let src = include_str!("detect.rs");
    for forbidden in ["Command::new", "Stdio", "std::process"] {
        assert!(
            !src.contains(forbidden),
            "tools/detect.rs must not spawn: found `{forbidden}`"
        );
    }
}

// ---- host-touching tests -----------------------------------------------------

#[test]
fn host_is_bundle_requires_an_info_plist() {
    // AC4's negative case at the HOST level (filesystem only, no registry, no
    // spawn): a directory named `*.app` is not a bundle until it has one.
    let scratch = crate::testutil::scratch_dir();
    let app = scratch.path().join("Warp.app");
    std::fs::create_dir_all(app.join("Contents")).expect("create fake bundle dir");
    let env = HostToolEnv::new();
    assert!(
        !env.is_bundle(&app),
        "a directory without Contents/Info.plist is not a bundle"
    );
    std::fs::write(app.join("Contents").join("Info.plist"), b"<plist/>")
        .expect("write Info.plist");
    assert!(env.is_bundle(&app));
    // A plain FILE named `*.app` is not a bundle either.
    let file_app = scratch.path().join("Fake.app");
    std::fs::write(&file_app, b"not a bundle").expect("write file");
    assert!(!env.is_bundle(&file_app));
}

#[test]
#[ignore = "AC20: probes the real machine (PATH, registry, filesystem). Run with --ignored."]
fn host_scan_finds_real_tools_and_only_absolute_existing_paths() {
    let env = HostToolEnv::new();
    let rows = scan_for(&env, TargetOs::host());
    // Printed because this test's whole purpose is to report what the real
    // machine has; run it with `--ignored --nocapture`.
    eprintln!(
        "host scan: {} rows {:?}",
        rows.len(),
        rows.iter()
            .map(|(e, r)| (e.id, r.source, r.program.as_str()))
            .collect::<Vec<_>>()
    );
    if cfg!(windows) {
        // Both are `BuiltIn`, so this cannot fail on a supported Windows.
        for id in ["cmd", "powershell"] {
            let found = rows
                .iter()
                .find(|(e, _)| e.id == id)
                .expect("a built-in Windows terminal must be detected");
            assert_eq!(found.1.source, ToolSource::BuiltIn);
        }
        // End-to-end proof of the `/ve` (default value) convention against the
        // real `reg.exe`: `HKCR\.txt` exists on every Windows install.
        assert_eq!(
            env.registry_string(r"HKCR\.txt", "").as_deref(),
            Some("txtfile")
        );
    }
    for (entry, res) in &rows {
        match res.source {
            ToolSource::BuiltIn => assert_eq!(res.program, entry.program),
            ToolSource::Path | ToolSource::Registry | ToolSource::WellKnown => {
                let p = Path::new(&res.program);
                assert!(p.is_absolute(), "{} is not absolute", res.program);
                assert!(p.is_file(), "{} is not an existing file", res.program);
                assert_eq!(res.bundle, None);
            }
            ToolSource::AppBundle => {
                let bundle = res.bundle.as_deref().unwrap_or_default();
                assert!(Path::new(bundle).is_dir(), "{bundle} is not a directory");
            }
            ToolSource::Custom => panic!("a probe never yields a Custom resolution"),
        }
    }
}
