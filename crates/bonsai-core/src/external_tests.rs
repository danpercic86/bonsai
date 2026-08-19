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

// ---- P72: validate_web_url / url_ladder / open_url ----

const OK_URL: &str = "https://dev.azure.com/org/proj/_git/repo/pullrequest/7";

/// Every accepted form from the contract's table (§3.2).
#[test]
fn validate_web_url_accepts_plain_http_and_https() {
    for url in [
        "https://github.com/settings/tokens",
        "http://localhost:3000/x?y=1#z",
        // Scheme match is case-INSENSITIVE.
        "HTTPS://EXAMPLE.COM/a",
        OK_URL,
        // No path at all is still a valid link.
        "https://example.com",
    ] {
        assert!(validate_web_url(url).is_ok(), "should accept: {url}");
    }
}

/// Every rejected form, plus the two invariants that matter: the variant is
/// `ExternalToolFailed` and the message NEVER echoes the untrusted input.
#[test]
fn validate_web_url_rejects_every_non_web_form_without_echoing_it() {
    // (input, a distinctive substring that must NOT appear in the message, the
    // CATEGORY the message must report). The category column is load-bearing:
    // without it all three rejection arms could be collapsed into one string and
    // every assertion here would still pass — the same vacuous-assertion class
    // the Increment-A review caught in the Azure 404 test.
    const MALFORMED: &str = "refused to open a malformed link";
    const NOT_WEB: &str = "refused to open a link that is not http or https";
    const NO_HOST: &str = "refused to open a link with no host";
    let long = format!("https://ok.example.com/{}", "a".repeat(2100));
    let cases: &[(&str, &str, &str)] = &[
        ("javascript:alert(1)", "alert", NOT_WEB),
        ("file:///C:/Windows/System32/calc.exe", "calc.exe", NOT_WEB),
        ("data:text/html,<h1>x", "<h1>", NOT_WEB),
        ("ms-msdt:/id", "ms-msdt", NOT_WEB),
        ("\\\\server\\share\\x", "server", NOT_WEB),
        ("example.com", "example.com", NOT_WEB),
        ("https://", "https://", NO_HOST),
        ("http:///path", "path", NO_HOST),
        ("-https://x.com", "x.com", MALFORMED),
        ("--url=https://x.com", "--url", MALFORMED),
        ("", "\u{1}", MALFORMED), // no distinctive part; the variant check applies
        ("   ", "\u{1}", MALFORMED),
        // A space ANYWHERE is now malformed (whole-url screen), so this case
        // moved category — it used to be caught by the host-only check.
        ("https://ex ample.com/x", "ex ample", MALFORMED),
        ("https://ex\\ample.com", "ex\\ample", NO_HOST),
        // ---- P72 security-audit additions ----
        // LOW-3: userinfo impersonation. Every other rule passes; the apparent
        // host is NOT the host the browser would navigate to.
        ("https://github.com@evil.example/x", "evil.example", NO_HOST),
        ("https://github.com%2Foctocat@evil.example/", "evil.example", NO_HOST),
        // LOW-2: control characters outside the host (the old check was host-only).
        ("https://ok.example.com/a\nb", "ok.example", MALFORMED),
        ("https://ok.example.com/a\tb", "ok.example", MALFORMED),
        ("https://ok.example.com/a\rb", "ok.example", MALFORMED),
        // LOW-2: over-length fails closed HERE, not as an OS spawn error later.
        (&long, "ok.example", MALFORMED),
    ];
    for (url, needle, category) in cases {
        let err = validate_web_url(url).expect_err("should reject");
        assert!(
            matches!(err, AppError::ExternalToolFailed(_)),
            "wrong variant for {url}: {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains(category),
            "wrong rejection category for {url}: got {msg:?}, expected {category:?}"
        );
        assert!(
            !msg.contains(needle),
            "message echoed the rejected URL ({url}): {msg}"
        );
        assert!(
            url.is_empty() || !msg.contains(url),
            "message echoed the rejected URL ({url}): {msg}"
        );
    }
}

#[test]
fn url_ladder_windows_is_explorer_then_rundll32() {
    let ladder = url_ladder(TargetOs::Windows, OK_URL);
    assert_eq!(
        ladder,
        vec![
            LaunchSpec {
                program: "explorer".to_string(),
                args: vec![OK_URL.to_string()],
                cwd: PathBuf::from("."),
                hide_console: true,
                wait_for_exit: false,
            },
            LaunchSpec {
                program: "rundll32".to_string(),
                args: vec![
                    "url.dll,FileProtocolHandler".to_string(),
                    OK_URL.to_string(),
                ],
                cwd: PathBuf::from("."),
                hide_console: true,
                wait_for_exit: false,
            },
        ]
    );
}

#[test]
fn url_ladder_macos_uses_open_and_waits() {
    // `open` gets wait_for_exit (F-MAC-1 rule) and is the only rung.
    assert_eq!(
        url_ladder(TargetOs::MacOs, OK_URL),
        vec![LaunchSpec {
            program: "open".to_string(),
            args: vec![OK_URL.to_string()],
            cwd: PathBuf::from("."),
            hide_console: false,
            wait_for_exit: true,
        }]
    );
}

#[test]
fn url_ladder_linux_is_xdg_open() {
    assert_eq!(
        url_ladder(TargetOs::Linux, OK_URL),
        vec![LaunchSpec {
            program: "xdg-open".to_string(),
            args: vec![OK_URL.to_string()],
            cwd: PathBuf::from("."),
            hide_console: true,
            wait_for_exit: false,
        }]
    );
}

/// The URL is always exactly ONE argv token, and no rung on any OS routes
/// through a shell (`cmd`, `start`, `/c`, `powershell`).
#[test]
fn url_ladder_never_uses_a_shell_and_keeps_the_url_in_one_token() {
    for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
        for s in url_ladder(os, OK_URL) {
            assert_eq!(
                s.args.iter().filter(|a| a.contains(OK_URL)).count(),
                1,
                "URL must occupy exactly one argv token: {s:?}"
            );
            assert!(s.args.iter().any(|a| a == OK_URL), "URL not a whole token: {s:?}");
            for banned in ["cmd", "start", "/c", "powershell"] {
                assert!(!s.program.contains(banned), "shell in program: {s:?}");
                assert!(
                    !s.args
                        .iter()
                        .any(|a| a.split(OK_URL).any(|part| part.contains(banned))),
                    "shell in args: {s:?}"
                );
            }
        }
    }
}

#[test]
fn open_url_windows_falls_through_to_rundll32() {
    let runner = FakeRunner::new(&["rundll32"]);
    open_url(&runner, TargetOs::Windows, OK_URL).expect("second rung wins");
    assert_eq!(*runner.calls.borrow(), vec!["explorer", "rundll32"]);
}

#[test]
fn open_url_all_rungs_fail_names_last_program_and_no_url() {
    let runner = FakeRunner::new(&[]);
    let err = open_url(&runner, TargetOs::Windows, OK_URL).expect_err("both rungs fail");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    let msg = err.to_string();
    assert!(msg.contains("rundll32"), "names the last program: {msg}");
    assert!(msg.contains("browser"), "carries the label: {msg}");
    assert!(!msg.contains(OK_URL), "must not echo the URL: {msg}");
    assert_eq!(*runner.calls.borrow(), vec!["explorer", "rundll32"]);
}

#[test]
fn open_url_validates_before_spawning_anything() {
    let runner = FakeRunner::new(&["explorer", "rundll32"]);
    let err =
        open_url(&runner, TargetOs::Windows, "javascript:alert(1)").expect_err("scheme rejected");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(runner.calls.borrow().is_empty(), "nothing may be spawned");
}
