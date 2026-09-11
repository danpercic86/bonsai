//! Unit tests for [`super`] (`external.rs`) — kept in a sibling file so the
//! module itself stays under the ~500-line soft limit. Declared with `#[path]`
//! as a child module of `external`, so `super::*` reaches the pure builders
//! (`spec`, `open_spec`).
//!
//! Covers: how a configured program receives the target directory, the
//! per-`TargetOs` ladder tables (including the F-MAC-1 `wait_for_exit` flags and
//! the LOW-1 cwd split), and the `launch_first` fallback logic driven by
//! `fake::FakeRunner`, which never spawns. The web-URL half lives in
//! `external_url_tests.rs`; the setting-shape rules in `external_cmd_tests.rs`.
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

// ---- program_spec: how a configured program receives the directory ----

#[test]
fn program_spec_argument_delivery_hands_the_path_over_and_leaves_the_repo() {
    // MEDIUM-2 removed `{path}`, so the LAUNCHER appends the directory — as ONE
    // argv token, and LOW-1 keeps the child out of the repo.
    let s = program_spec("code", &p(), true, PathDelivery::Argument).expect("built");
    assert_eq!(s.program, "code");
    assert_eq!(s.args, vec!["/tmp/work".to_string()]);
    assert_eq!(s.cwd, safe_cwd());
    assert!(s.hide_console);
}

#[test]
fn program_spec_working_dir_delivery_passes_no_args_at_all() {
    // A shell opens where it is STARTED; `powershell /tmp/work` would try to run
    // the directory as a script. So the terminal rung keeps the repo cwd.
    let s = program_spec("powershell", &p(), false, PathDelivery::WorkingDir).expect("built");
    assert_eq!(s.program, "powershell");
    assert!(s.args.is_empty());
    assert_eq!(s.cwd, p());
    assert!(!s.hide_console);
}

#[test]
fn program_spec_path_with_spaces_stays_one_argument() {
    let path = PathBuf::from("/tmp/my repo");
    let s = program_spec("code", &path, true, PathDelivery::Argument).expect("built");
    assert_eq!(s.args, vec!["/tmp/my repo".to_string()]);
}

#[test]
fn program_spec_absolute_program_is_used_verbatim() {
    // The portable-editor case: an absolute path with a space survives as the
    // program, never re-split into tokens.
    let prog = "/opt/My Editor/bin/edit";
    let s = program_spec(prog, &p(), true, PathDelivery::Argument).expect("built");
    assert_eq!(s.program, prog);
    assert_eq!(s.args, vec!["/tmp/work".to_string()]);
}

#[test]
fn program_spec_empty_or_whitespace_is_none() {
    for delivery in [PathDelivery::Argument, PathDelivery::WorkingDir] {
        assert!(program_spec("", &p(), false, delivery).is_none());
        assert!(program_spec("   \t  ", &p(), false, delivery).is_none());
    }
}

// ---- builder tables (per TargetOs, empty program = auto ladder) ----
//
// The `cwd` column is the LOW-1 fix and is asserted here deliberately: a rung
// that passes the directory as an ARGUMENT launches from `safe_cwd()`; only the
// rungs that have no directory argument keep the repo path.

#[test]
fn terminal_ladder_windows_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::Windows, "", &p()),
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
        terminal_ladder(TargetOs::MacOs, "", &p()),
        vec![spec("open", &["-a", "Terminal", "/tmp/work"], &safe_cwd(), false, true)]
    );
}

#[test]
fn terminal_ladder_linux_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::Linux, "", &p()),
        vec![
            spec("gnome-terminal", &["--working-directory=/tmp/work"], &safe_cwd(), false, false),
            spec("konsole", &["--workdir", "/tmp/work"], &safe_cwd(), false, false),
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
        .flat_map(|os| terminal_ladder(os, "", &p()))
        .filter(|s| s.cwd == p())
        .map(|s| s.program)
        .collect();
    assert_eq!(keeps_repo, vec!["powershell", "cmd", "x-terminal-emulator"]);
    // …and every one of them passes no DIRECTORY argument, so there is no
    // alternative for it (`cmd /K` has an argument — just not the path).
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        for s in terminal_ladder(os, "", &p()) {
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

/// The trim in `validate_command_setting` and the trim in `program_spec` must
/// agree: if they diverged, a value could validate as one string and launch as
/// another. Pinned rather than assumed.
///
/// Scope, deliberately narrow (corrected 2026-09-11): this proves the SAME TRIM
/// runs on both sides, i.e. both see a byte-identical string. It does NOT prove
/// "what is validated is what executes" — Rust std's Windows `resolve_exe`
/// appends `.exe` to a separator-carrying path with no extension, so the OS can
/// spawn `D:\x\foo.exe` for a validated `D:\x\foo`. That gap is stated as
/// residual route 1 in the `external_cmd` module docs; proving it would require
/// really spawning, which no unit test here does.
#[test]
fn the_same_trim_runs_on_both_sides() {
    let raw = "	code
";
    validate_command_setting(raw, "Editor command").expect("trimmed value is valid");
    let s = program_spec(raw, &p(), true, PathDelivery::Argument).expect("built");
    assert_eq!(s.program, "code");
}

#[test]
fn terminal_ladder_configured_program_overrides_to_single_spec() {
    // A configured program yields exactly one candidate on every OS,
    // hide_console false (visible terminal), no args, repo cwd.
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        assert_eq!(
            terminal_ladder(os, "alacritty", &p()),
            vec![spec(
                "alacritty",
                &[],
                &p(),
                false,
                // A configured program is NEVER waited on, not even on macOS.
                false
            )]
        );
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
    assert_eq!(editor_ladder(TargetOs::Windows, "", &p()), expected);
    assert_eq!(editor_ladder(TargetOs::Linux, "", &p()), expected);
}

#[test]
fn editor_ladder_macos_auto() {
    assert_eq!(
        editor_ladder(TargetOs::MacOs, "", &p()),
        vec![
            spec("open", &["-a", "Visual Studio Code", "/tmp/work"], &safe_cwd(), true, true),
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
    let ladder = editor_ladder(TargetOs::MacOs, "", &p());
    assert_eq!(ladder.len(), 3);
    assert!(ladder[0].wait_for_exit, "open -a VS Code waits for its exit code");
    assert!(ladder[1].wait_for_exit, "open -a Insiders waits for its exit code");
    assert_eq!(ladder[2].program, "code");
    assert!(!ladder[2].wait_for_exit, "the `code` CLI rung stays detached");
}

/// The Windows/Linux ladders NEVER wait — `explorer` exits non-zero after a
/// successful hand-off and an editor would keep us waiting for its session.
#[test]
fn windows_and_linux_specs_never_wait_for_exit() {
    for os in [TargetOs::Windows, TargetOs::Linux] {
        for s in editor_ladder(os, "", &p()) {
            assert!(!s.wait_for_exit, "{os:?} editor `{}` must not wait", s.program);
        }
        for s in terminal_ladder(os, "", &p()) {
            assert!(!s.wait_for_exit, "{os:?} terminal `{}` must not wait", s.program);
        }
        assert!(!reveal_spec(os, &p()).wait_for_exit, "{os:?} reveal must not wait");
    }
}

/// A configured program is arbitrary and long-lived, so it is never waited on —
/// even on macOS, even when it is literally named `open`.
#[test]
fn configured_program_specs_never_wait_for_exit() {
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        assert!(!editor_ladder(os, "open", &p())[0].wait_for_exit);
        assert!(!terminal_ladder(os, "alacritty", &p())[0].wait_for_exit);
    }
}

#[test]
fn editor_ladder_configured_program_overrides_to_single_spec() {
    // The editor is HANDED the folder, so a configured editor also launches
    // from the neutral cwd (LOW-1).
    assert_eq!(
        editor_ladder(TargetOs::MacOs, "subl", &p()),
        vec![spec("subl", &["/tmp/work"], &safe_cwd(), true, false)]
    );
}

// ---- ladder fallback logic (fake::FakeRunner — NEVER spawns) ----

#[test]
fn first_candidate_fails_second_succeeds_picks_second() {
    // wt unresolvable ⇒ falls through to PowerShell, which succeeds; cmd is
    // never tried.
    let runner = FakeRunner::new(&["powershell"]);
    open_in_terminal(&runner, TargetOs::Windows, "", &p()).expect("second candidate wins");
    assert_eq!(runner.calls(), vec!["wt", "powershell"]);
}

#[test]
fn all_candidates_fail_errors_naming_last_program() {
    // wt → powershell → cmd all fail: ExternalToolFailed names the LAST
    // program (cmd) and the "terminal" label.
    let runner = FakeRunner::new(&[]);
    let err = open_in_terminal(&runner, TargetOs::Windows, "", &p())
        .expect_err("all candidates fail");
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
    let err = reveal_in_file_manager(&bad, TargetOs::Windows, &p())
        .expect_err("reveal fails");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(err.to_string().contains("explorer"));
}

#[test]
fn editor_program_is_the_only_candidate_tried() {
    // A configured program short-circuits the auto ladder: only it is
    // attempted, and on failure it is what the error names.
    let runner = FakeRunner::new(&[]);
    let err = open_in_editor(&runner, TargetOs::Windows, "my-editor", &p())
        .expect_err("configured program missing");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(err.to_string().contains("my-editor"));
    assert_eq!(runner.calls(), vec!["my-editor"]);
}

/// MEDIUM-2, the load-bearing ordering: a refused setting must be rejected
/// BEFORE a ladder exists, so NOTHING is ever handed to a runner. Asserted on
/// both entry points, since each validates its own setting.
#[test]
fn a_refused_setting_reaches_no_runner() {
    for bad in ["powershell -NoProfile -Command calc", "code & calc", "./code"] {
        let runner = FakeRunner::new(&["powershell", "code", "wt", "cmd"]);
        let err = open_in_editor(&runner, TargetOs::Windows, bad, &p())
            .expect_err("the setting must be refused");
        assert!(matches!(err, AppError::ExternalToolFailed(_)));
        assert!(runner.calls().is_empty(), "nothing may be spawned for {bad:?}");

        let runner = FakeRunner::new(&["powershell", "code", "wt", "cmd"]);
        open_in_terminal(&runner, TargetOs::Windows, bad, &p())
            .expect_err("the setting must be refused");
        assert!(runner.calls().is_empty(), "nothing may be spawned for {bad:?}");
    }
}

/// …and an EMPTY setting must still reach the auto ladder: validation must not
/// turn "auto-detect" (the shipped default for both settings) into an error.
#[test]
fn an_empty_setting_still_runs_the_auto_ladder() {
    let runner = FakeRunner::new(&["code"]);
    open_in_editor(&runner, TargetOs::Windows, "", &p()).expect("auto ladder runs");
    assert_eq!(runner.calls(), vec!["code"]);
}
