//! Unit tests for [`super`] (`external.rs`) — kept in a sibling file so the
//! module itself stays under the ~500-line soft limit. Declared with `#[path]`
//! as a child module of `external`, so `super::*` reaches the pure builders
//! (`spec`, `open_spec`).
//!
//! Covers the **auto** ladders (`picked = None`): the per-`TargetOs` tables
//! (including the F-MAC-1 `wait_for_exit` flags and the LOW-1 cwd split) and the
//! `launch_first` fallback logic driven by `fake::FakeRunner`, which never
//! spawns. P112 AC9 is this file: every assertion below predates P112 and must
//! pass with **no edit other than the new parameter**, which is what proves the
//! catalog-driven rebuild is byte-identical to the hardcoded ladders.
//!
//! The picked/browsed launch shapes live in `external_picked_tests.rs`; the
//! web-URL half in `external_url_tests.rs`. The program-string shape rules that
//! used to live in `external_cmd_tests.rs` are GONE with the setting — see
//! `tools/custom_tests.rs` for what replaced them.
//!
//! PATH hostility (audit 2026-09-03) is deliberately NOT tested here: a
//! `.gitmodules` path can never reach a `LaunchSpec` unvalidated, because the
//! containment gate is at the PRODUCER, not this launcher. Those cases live
//! with the code that owns them — `git::submodule_abs_path` (pure validator)
//! and `git::submodule` tests (`list_submodules_*`, proving a rooted/UNC path
//! becomes `abs_path: None`). Testing them here would test the wrong layer.

use super::fake::FakeRunner;
use super::*;

fn p() -> PathBuf {
    PathBuf::from("/tmp/work")
}

// ---- builder tables (per TargetOs, `picked = None` = the auto ladder) ----
//
// The `cwd` column is the LOW-1 fix and is asserted here deliberately: a rung
// that passes the directory as an ARGUMENT launches from `safe_cwd()`; only the
// rungs that have no directory argument keep the repo path.

#[test]
fn terminal_ladder_windows_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::Windows, None, &p()),
        vec![
            spec("wt", &["-d", "/tmp/work"], &safe_cwd(), false, false),
            spec("powershell", &[], &p(), false, false),
            spec("cmd", &["/K"], &p(), false, false),
        ]
    );
}

#[test]
fn terminal_ladder_macos_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::MacOs, None, &p()),
        vec![spec(
            "open",
            &["-a", "Terminal", "/tmp/work"],
            &safe_cwd(),
            false,
            true
        )]
    );
}

#[test]
fn terminal_ladder_linux_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::Linux, None, &p()),
        vec![
            spec(
                "gnome-terminal",
                &["--working-directory=/tmp/work"],
                &safe_cwd(),
                false,
                false
            ),
            spec(
                "konsole",
                &["--workdir", "/tmp/work"],
                &safe_cwd(),
                false,
                false
            ),
            spec("x-terminal-emulator", &[], &p(), false, false),
        ]
    );
}

/// LOW-1 stays open, by necessity, on exactly the rungs whose semantics ARE the
/// cwd. Pinned as an assertion so a future "harden everything" pass cannot
/// silently break "open a terminal here".
#[test]
fn only_the_directory_less_terminal_rungs_keep_the_repo_as_cwd() {
    let keeps_repo: Vec<String> = [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux]
        .into_iter()
        .flat_map(|os| terminal_ladder(os, None, &p()))
        .filter(|s| s.cwd == p())
        .map(|s| s.program)
        .collect();
    assert_eq!(keeps_repo, vec!["powershell", "cmd", "x-terminal-emulator"]);
    // …and every one of them passes no DIRECTORY argument, so there is no
    // alternative for it (`cmd /K` has an argument — just not the path).
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        for s in terminal_ladder(os, None, &p()) {
            let carries_dir = s.args.iter().any(|a| a.contains("/tmp/work"));
            assert_eq!(
                s.cwd == p(),
                !carries_dir,
                "`{}` must take the repo as cwd iff it gets no directory argument",
                s.program
            );
        }
    }
}

#[test]
fn reveal_spec_per_os() {
    // LOW-1: all three take the directory as an argument, so none of them
    // launches from the repo.
    assert_eq!(
        reveal_spec(TargetOs::Windows, &p()),
        spec("explorer", &["/tmp/work"], &safe_cwd(), true, false)
    );
    assert_eq!(
        reveal_spec(TargetOs::MacOs, &p()),
        spec("open", &["/tmp/work"], &safe_cwd(), true, true)
    );
    assert_eq!(
        reveal_spec(TargetOs::Linux, &p()),
        spec("xdg-open", &["/tmp/work"], &safe_cwd(), true, false)
    );
}

#[test]
fn editor_ladder_windows_and_linux_auto() {
    let expected = vec![
        spec("code", &["/tmp/work"], &safe_cwd(), true, false),
        spec("code-insiders", &["/tmp/work"], &safe_cwd(), true, false),
    ];
    assert_eq!(editor_ladder(TargetOs::Windows, None, &p()), expected);
    assert_eq!(editor_ladder(TargetOs::Linux, None, &p()), expected);
}

#[test]
fn editor_ladder_macos_auto() {
    assert_eq!(
        editor_ladder(TargetOs::MacOs, None, &p()),
        vec![
            spec(
                "open",
                &["-a", "Visual Studio Code", "/tmp/work"],
                &safe_cwd(),
                true,
                true
            ),
            spec(
                "open",
                &["-a", "Visual Studio Code - Insiders", "/tmp/work"],
                &safe_cwd(),
                true,
                true
            ),
            spec("code", &["/tmp/work"], &safe_cwd(), true, false),
        ]
    );
}

/// F-MAC-1: the macOS editor ladder marks BOTH `open -a` rungs
/// `wait_for_exit` (their exit code is the only "app not found" signal),
/// while the plain `code` fallback stays a detached spawn.
#[test]
fn editor_ladder_macos_marks_open_specs_wait_for_exit() {
    let ladder = editor_ladder(TargetOs::MacOs, None, &p());
    assert_eq!(ladder.len(), 3);
    assert!(
        ladder[0].wait_for_exit,
        "open -a VS Code waits for its exit code"
    );
    assert!(
        ladder[1].wait_for_exit,
        "open -a Insiders waits for its exit code"
    );
    assert_eq!(ladder[2].program, "code");
    assert!(
        !ladder[2].wait_for_exit,
        "the `code` CLI rung stays detached"
    );
}

/// The Windows/Linux ladders NEVER wait — `explorer` exits non-zero after a
/// successful hand-off and an editor would keep us waiting for its session.
#[test]
fn windows_and_linux_specs_never_wait_for_exit() {
    for os in [TargetOs::Windows, TargetOs::Linux] {
        for s in editor_ladder(os, None, &p()) {
            assert!(
                !s.wait_for_exit,
                "{os:?} editor `{}` must not wait",
                s.program
            );
        }
        for s in terminal_ladder(os, None, &p()) {
            assert!(
                !s.wait_for_exit,
                "{os:?} terminal `{}` must not wait",
                s.program
            );
        }
        assert!(
            !reveal_spec(os, &p()).wait_for_exit,
            "{os:?} reveal must not wait"
        );
    }
}

// ---- ladder fallback logic (fake::FakeRunner — NEVER spawns) ----

#[test]
fn first_candidate_fails_second_succeeds_picks_second() {
    // wt unresolvable ⇒ falls through to PowerShell, which succeeds; cmd is
    // never tried.
    let runner = FakeRunner::new(&["powershell"]);
    open_in_terminal(&runner, TargetOs::Windows, None, &p()).expect("second candidate wins");
    assert_eq!(runner.calls(), vec!["wt", "powershell"]);
}

#[test]
fn all_candidates_fail_errors_naming_last_program() {
    // wt → powershell → cmd all fail: ExternalToolFailed names the LAST
    // program (cmd) and the "terminal" label.
    let runner = FakeRunner::new(&[]);
    let err =
        open_in_terminal(&runner, TargetOs::Windows, None, &p()).expect_err("all candidates fail");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    let msg = err.to_string();
    assert!(msg.contains("cmd"), "message names last program: {msg}");
    assert!(msg.contains("terminal"), "message carries the label: {msg}");
    assert_eq!(runner.calls(), vec!["wt", "powershell", "cmd"]);
}

#[test]
fn reveal_single_candidate_success_and_failure() {
    // Success: the one reveal spec runs.
    let ok = FakeRunner::new(&["explorer"]);
    reveal_in_file_manager(&ok, TargetOs::Windows, &p()).expect("reveal spawns");
    assert_eq!(ok.calls(), vec!["explorer"]);

    // Failure: the single candidate fails ⇒ ExternalToolFailed naming it.
    let bad = FakeRunner::new(&[]);
    let err = reveal_in_file_manager(&bad, TargetOs::Windows, &p()).expect_err("reveal fails");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(err.to_string().contains("explorer"));
}

/// `None` (the shipped default for both settings, and the silent fallback for a
/// selection whose tool is gone) reaches the auto ladder.
#[test]
fn no_selection_still_runs_the_auto_ladder() {
    let runner = FakeRunner::new(&["code"]);
    open_in_editor(&runner, TargetOs::Windows, None, &p()).expect("auto ladder runs");
    assert_eq!(runner.calls(), vec!["code"]);
}
