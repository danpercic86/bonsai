//! Unit tests for [`super`] (`external_url.rs`) — P72's web-URL launch half,
//! split out of `external_tests.rs` alongside the code in the 2026-09-11
//! security increment.
//!
//! Declared with `#[path]` as a child module of `external_url`, so `super::*`
//! reaches the private `url_ladder`. The runner fake is the SHARED
//! `external::fake::FakeRunner` — one recording runner for both halves.

use super::*;
use crate::external::fake::FakeRunner;

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
        (
            "https://github.com%2Foctocat@evil.example/",
            "evil.example",
            NO_HOST,
        ),
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
                cwd: safe_cwd(),
                hide_console: true,
                wait_for_exit: false,
            },
            LaunchSpec {
                program: "rundll32".to_string(),
                args: vec![
                    "url.dll,FileProtocolHandler".to_string(),
                    OK_URL.to_string(),
                ],
                cwd: safe_cwd(),
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
            cwd: safe_cwd(),
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
            cwd: safe_cwd(),
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
            assert!(
                s.args.iter().any(|a| a == OK_URL),
                "URL not a whole token: {s:?}"
            );
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
    assert_eq!(runner.calls(), vec!["explorer", "rundll32"]);
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
    assert_eq!(runner.calls(), vec!["explorer", "rundll32"]);
}

#[test]
fn open_url_validates_before_spawning_anything() {
    let runner = FakeRunner::new(&["explorer", "rundll32"]);
    let err =
        open_url(&runner, TargetOs::Windows, "javascript:alert(1)").expect_err("scheme rejected");
    assert!(matches!(err, AppError::ExternalToolFailed(_)));
    assert!(runner.calls().is_empty(), "nothing may be spawned");
}
