//! Unit tests for [`super`] (`external.rs`) — kept in a sibling file so the
//! module itself stays under the ~500-line soft limit. Declared with `#[path]`
//! as a child module of `external`, so `super::*` reaches the private pure
//! builders (`spec`, `open_spec`, `tokenize`).
//!
//! Covers: template tokenization/substitution safety, the per-`TargetOs`
//! ladder tables (including the F-MAC-1 `wait_for_exit` flags), and the
//! `launch_first` fallback logic driven by a `FakeRunner` that never spawns.

use super::*;
use std::cell::RefCell;

fn p() -> PathBuf {
    PathBuf::from("/tmp/work")
}

// ---- parse_template ----

#[test]
fn parse_template_standalone_path_token() {
    let s = parse_template("myterm {path}", &p(), false).expect("parsed");
    assert_eq!(s.program, "myterm");
    assert_eq!(s.args, vec!["/tmp/work".to_string()]);
    assert_eq!(s.cwd, p());
    assert!(!s.hide_console);
}

#[test]
fn parse_template_embedded_path_in_flag() {
    let s = parse_template("gnome-terminal --working-directory={path}", &p(), false)
        .expect("parsed");
    assert_eq!(s.program, "gnome-terminal");
    assert_eq!(s.args, vec!["--working-directory=/tmp/work".to_string()]);
}

#[test]
fn parse_template_quoted_token_keeps_spaces_and_strips_quotes() {
    // A quoted program name AND a quoted arg with spaces both survive as one
    // token each, with the surrounding quotes removed.
    let s = parse_template("\"my editor\" \"C:\\Program Files\\x\"", &p(), true)
        .expect("parsed");
    assert_eq!(s.program, "my editor");
    assert_eq!(s.args, vec!["C:\\Program Files\\x".to_string()]);
    assert!(s.hide_console);
}

#[test]
fn parse_template_substitutes_path_with_spaces_into_one_arg() {
    let path = PathBuf::from("/tmp/my repo");
    let s = parse_template("code {path}", &path, true).expect("parsed");
    assert_eq!(s.program, "code");
    // Space preserved AND not split into two args.
    assert_eq!(s.args, vec!["/tmp/my repo".to_string()]);
}

#[test]
fn parse_template_empty_or_whitespace_is_none() {
    assert!(parse_template("", &p(), false).is_none());
    assert!(parse_template("   \t  ", &p(), false).is_none());
}

#[test]
fn parse_template_shell_metachars_stay_literal_args() {
    // A `;` (or `&&`, `|`) is NOT a command separator — it is one more argv
    // token, immune to Windows Terminal's `;` sub-command delimiter.
    let s = parse_template("wt {path} ; rm -rf /", &p(), false).expect("parsed");
    assert_eq!(s.program, "wt");
    assert_eq!(
        s.args,
        vec!["/tmp/work", ";", "rm", "-rf", "/"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );
}

// ---- builder tables (per TargetOs, empty template = auto ladder) ----

#[test]
fn terminal_ladder_windows_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::Windows, "", &p()),
        vec![
            spec("wt", &["-d", "/tmp/work"], &p(), false, false),
            spec("powershell", &[], &p(), false, false),
            spec("cmd", &["/K"], &p(), false, false),
        ]
    );
}

#[test]
fn terminal_ladder_macos_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::MacOs, "", &p()),
        vec![spec("open", &["-a", "Terminal", "/tmp/work"], &p(), false, true)]
    );
}

#[test]
fn terminal_ladder_linux_auto() {
    assert_eq!(
        terminal_ladder(TargetOs::Linux, "", &p()),
        vec![
            spec("gnome-terminal", &["--working-directory=/tmp/work"], &p(), false, false),
            spec("konsole", &["--workdir", "/tmp/work"], &p(), false, false),
            spec("x-terminal-emulator", &[], &p(), false, false),
        ]
    );
}

#[test]
fn terminal_ladder_template_overrides_to_single_spec() {
    // A user template yields exactly one candidate on every OS, hide_console
    // false (visible terminal).
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        assert_eq!(
            terminal_ladder(os, "alacritty --working-directory {path}", &p()),
            vec![spec(
                "alacritty",
                &["--working-directory", "/tmp/work"],
                &p(),
                false,
                // A template is NEVER waited on, not even on macOS.
                false
            )]
        );
    }
}

#[test]
fn reveal_spec_per_os() {
    assert_eq!(
        reveal_spec(TargetOs::Windows, &p()),
        spec("explorer", &["/tmp/work"], &p(), true, false)
    );
    assert_eq!(
        reveal_spec(TargetOs::MacOs, &p()),
        spec("open", &["/tmp/work"], &p(), true, true)
    );
    assert_eq!(
        reveal_spec(TargetOs::Linux, &p()),
        spec("xdg-open", &["/tmp/work"], &p(), true, false)
    );
}

#[test]
fn editor_ladder_windows_and_linux_auto() {
    let expected = vec![
        spec("code", &["/tmp/work"], &p(), true, false),
        spec("code-insiders", &["/tmp/work"], &p(), true, false),
    ];
    assert_eq!(editor_ladder(TargetOs::Windows, "", &p()), expected);
    assert_eq!(editor_ladder(TargetOs::Linux, "", &p()), expected);
}

#[test]
fn editor_ladder_macos_auto() {
    assert_eq!(
        editor_ladder(TargetOs::MacOs, "", &p()),
        vec![
            spec("open", &["-a", "Visual Studio Code", "/tmp/work"], &p(), true, true),
            spec(
                "open",
                &["-a", "Visual Studio Code - Insiders", "/tmp/work"],
                &p(),
                true,
                true
            ),
            spec("code", &["/tmp/work"], &p(), true, false),
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

/// A user template is an arbitrary long-lived program, so it is never
/// waited on — even on macOS, even when it literally starts with `open`.
#[test]
fn template_specs_never_wait_for_exit() {
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        assert!(!editor_ladder(os, "open -a Foo {path}", &p())[0].wait_for_exit);
        assert!(!terminal_ladder(os, "alacritty {path}", &p())[0].wait_for_exit);
    }
}

#[test]
fn editor_ladder_template_overrides_to_single_spec() {
    assert_eq!(
        editor_ladder(TargetOs::MacOs, "subl {path}", &p()),
        vec![spec("subl", &["/tmp/work"], &p(), true, false)]
    );
}

// ---- ladder fallback logic (FakeRunner — NEVER spawns) ----

/// Records every `run` call and succeeds only for the programs in `succeed`.
struct FakeRunner {
    succeed: Vec<String>,
    calls: RefCell<Vec<String>>,
}

impl FakeRunner {
    fn new(succeed: &[&str]) -> FakeRunner {
        FakeRunner {
            succeed: succeed.iter().map(|s| s.to_string()).collect(),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
        self.calls.borrow_mut().push(spec.program.clone());
        if self.succeed.iter().any(|s| s == &spec.program) {
            Ok(())
        } else {
            Err(format!("mock: `{}` not found", spec.program))
        }
    }
}

#[test]
fn first_candidate_fails_second_succeeds_picks_second() {
    // wt unresolvable ⇒ falls through to PowerShell, which succeeds; cmd is
    // never tried.
    let runner = FakeRunner::new(&["powershell"]);
    open_in_terminal(&runner, TargetOs::Windows, "", &p()).expect("second candidate wins");
    assert_eq!(*runner.calls.borrow(), vec!["wt", "powershell"]);
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
    assert_eq!(*runner.calls.borrow(), vec!["wt", "powershell", "cmd"]);
}

#[test]
fn reveal_single_candidate_success_and_failure() {
    // Success: the one reveal spec runs.
    let ok = FakeRunner::new(&["explorer"]);
    reveal_in_file_manager(&ok, TargetOs::Windows, &p()).expect("reveal spawns");
    assert_eq!(*ok.calls.borrow(), vec!["explorer"]);

    // Failure: the single candidate fails ⇒ ExternalToolFailed naming it.
    let bad = FakeRunner::new(&[]);
    let err = reveal_in_file_manager(&bad, TargetOs::Windows, &p())
        .expect_err("reveal fails");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(err.to_string().contains("explorer"));
}

#[test]
fn editor_template_is_the_only_candidate_tried() {
    // A configured template short-circuits the auto ladder: only the
    // template program is attempted, and on failure it is what the error
    // names.
    let runner = FakeRunner::new(&[]);
    let err = open_in_editor(&runner, TargetOs::Windows, "my-editor {path}", &p())
        .expect_err("template program missing");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(err.to_string().contains("my-editor"));
    assert_eq!(*runner.calls.borrow(), vec!["my-editor"]);
}
