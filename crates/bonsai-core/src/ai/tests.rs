//! `run_claude` / availability / envelope-parse unit tests (P13; moved out of
//! `mod.rs` in P68a to keep that module under the file-size limit — the module
//! path `ai::tests::*` is unchanged).

use super::testutil::{env_lock, set_mode, stub_path, STUB_MODE_ENV};
use super::*;

#[test]
fn run_claude_success_strips_and_parses() {
    let _g = env_lock();
    set_mode("success");
    let res = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
        .expect("success stub should yield Ok");
    assert_eq!(res.text, "MERGED_BODY_OK");
    assert_eq!(res.cost_usd, Some(0.012));
    assert_eq!(res.session_id.as_deref(), Some("sess-abc"));
}

#[test]
fn run_claude_strips_code_fence() {
    let _g = env_lock();
    set_mode("success_fence");
    let res = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
        .expect("fence stub should yield Ok");
    assert_eq!(res.text, "MERGED_FENCED");
}

#[test]
fn run_claude_is_error_maps_to_ai_failed() {
    let _g = env_lock();
    set_mode("error");
    let err = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
        .expect_err("is_error envelope should map to Err");
    match err {
        AppError::AiFailed(m) => assert_eq!(m, "boom"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
}

#[test]
fn run_claude_nonzero_exit_maps_to_ai_failed() {
    let _g = env_lock();
    set_mode("nonzero");
    let err = run_claude(Path::new("."), "prompt", Some("payload"), RunOpts::default())
        .expect_err("non-zero exit should map to Err");
    match err {
        AppError::AiFailed(m) => assert!(
            m.contains("something broke"),
            "stderr should surface, got: {m}"
        ),
        other => panic!("expected AiFailed, got {other:?}"),
    }
}

#[test]
fn run_claude_slow_times_out_and_reaps_child() {
    let _g = env_lock();
    set_mode("slow");
    let opts = RunOpts { timeout: Duration::from_secs(1), ..RunOpts::default() };
    let start = Instant::now();
    let err = run_claude(Path::new("."), "prompt", Some("payload"), opts)
        .expect_err("slow stub past the timeout should map to Err");
    let elapsed = start.elapsed();
    match err {
        AppError::AiFailed(m) => {
            assert!(m.contains("timed out"), "expected timeout message, got: {m}");
        }
        other => panic!("expected AiFailed, got {other:?}"),
    }
    // P68a §9: the assertion used to be `elapsed < 2500ms`, which measured 2.97s
    // under parallel load (it only passed in isolation). What actually matters is
    // (a) we never return BEFORE the deadline and (b) we do not hang: a monotonic
    // lower bound plus a generous upper bound is load-independent.
    assert!(
        elapsed >= Duration::from_secs(1),
        "must not return before the 1s deadline, took {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "should kill+reap at the deadline instead of hanging, took {elapsed:?}"
    );
}

#[test]
fn run_claude_missing_binary_maps_to_ai_unavailable() {
    let _g = env_lock();
    std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/claude-does-not-exist.exe");
    std::env::remove_var(STUB_MODE_ENV);
    let err = run_claude(Path::new("."), "prompt", None, RunOpts::default())
        .expect_err("missing binary should map to Err");
    assert!(
        matches!(err, AppError::AiUnavailable(_)),
        "expected AiUnavailable, got {err:?}"
    );
}

#[test]
fn run_claude_large_payload_round_trips_without_deadlock() {
    let _g = env_lock();
    set_mode("success");
    // > 128 KiB across many short lines (drain-and-poll proof).
    let payload = "abcdefghij\n".repeat(15_000);
    assert!(payload.len() > 128 * 1024);
    let res = run_claude(Path::new("."), "prompt", Some(&payload), RunOpts::default())
        .expect("large payload should round-trip");
    assert_eq!(res.text, "MERGED_BODY_OK");
}

#[test]
fn check_availability_version_stub_reports_installed() {
    let _g = env_lock();
    set_mode("version");
    let a = check_availability();
    assert!(a.installed);
    assert!(a.logged_in);
    assert_eq!(a.version.as_deref(), Some("2.1.220"));
    assert_eq!(a.detail, "Claude Code 2.1.220 ready");
}

#[test]
fn check_availability_missing_binary_reports_not_installed() {
    let _g = env_lock();
    std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/claude-does-not-exist.exe");
    std::env::remove_var(STUB_MODE_ENV);
    let a = check_availability();
    assert!(!a.installed);
    assert!(!a.logged_in);
    assert_eq!(a.version, None);
    assert_eq!(a.detail, "Claude Code CLI not found on PATH");
}

#[test]
fn strip_fence_only_removes_matching_fences() {
    // Unfenced text is returned verbatim.
    assert_eq!(strip_fence("hello\nworld"), "hello\nworld");
    // Fenced (with lang) -> inner only.
    assert_eq!(strip_fence("```rust\nfn a() {}\n```"), "fn a() {}");
    // Bare fence -> inner only.
    assert_eq!(strip_fence("```\njust text\n```"), "just text");
}

#[test]
fn stub_path_points_at_a_committed_fixture() {
    assert!(stub_path().is_file(), "stub fixture missing: {:?}", stub_path());
}

// ---- P68a: the `parse_result_envelope` extraction (spike §1.3) ----
// These five cases ARE the regression guard for the 13 untouched
// `RunOpts::default()` call sites: `run_claude` now delegates to this function,
// so its behaviour must stay branch-for-branch identical.

#[test]
fn parse_result_envelope_reproduces_all_five_branches() {
    // 5. Success (+ fence strip, cost + session carried through).
    let ok = parse_result_envelope(
        r#"{"result":"```\nBODY\n```","is_error":false,"total_cost_usd":0.5,"session_id":"s9"}"#,
        true,
        "",
    )
    .expect("well-formed envelope parses");
    assert_eq!(ok.text, "BODY");
    assert_eq!(ok.cost_usd, Some(0.5));
    assert_eq!(ok.session_id.as_deref(), Some("s9"));

    // 1. Unparseable + non-zero exit -> stderr tail (capped at 500 chars).
    let err = parse_result_envelope("not json", false, "  boom happened  ")
        .expect_err("unparseable + failure -> Err");
    assert!(matches!(&err, AppError::AiFailed(m) if m == "boom happened"), "got {err:?}");
    let long = "x".repeat(900);
    let err = parse_result_envelope("not json", false, &long).expect_err("Err");
    match err {
        AppError::AiFailed(m) => assert_eq!(m.chars().count(), 500),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    // 1b. Empty stderr on a non-zero exit still names the failure.
    let err = parse_result_envelope("not json", false, "   ").expect_err("Err");
    assert!(
        matches!(&err, AppError::AiFailed(m) if m == "Claude exited with a non-zero status"),
        "got {err:?}"
    );

    // 2. Unparseable + zero exit.
    let err = parse_result_envelope("not json", true, "").expect_err("Err");
    assert!(
        matches!(&err, AppError::AiFailed(m) if m.starts_with("could not parse Claude output")),
        "got {err:?}"
    );

    // 3. Explicit error envelope: result wins, else subtype, else a constant.
    let err = parse_result_envelope(r#"{"is_error":true,"result":"boom"}"#, true, "")
        .expect_err("Err");
    assert!(matches!(&err, AppError::AiFailed(m) if m == "boom"), "got {err:?}");
    let err = parse_result_envelope(
        r#"{"is_error":true,"result":null,"subtype":"error_max_turns"}"#,
        true,
        "",
    )
    .expect_err("Err");
    assert!(matches!(&err, AppError::AiFailed(m) if m == "error_max_turns"), "got {err:?}");
    let err = parse_result_envelope(r#"{"is_error":true}"#, true, "").expect_err("Err");
    assert!(
        matches!(&err, AppError::AiFailed(m) if m == "Claude reported an error"),
        "got {err:?}"
    );

    // 4. Empty / blank / absent result.
    for body in [r#"{"result":""}"#, r#"{"result":"  \n\t "}"#, r#"{"result":null}"#] {
        let err = parse_result_envelope(body, true, "").expect_err("Err");
        assert!(
            matches!(&err, AppError::AiFailed(m) if m == "Claude returned no output"),
            "{body} -> {err:?}"
        );
    }
}

#[test]
fn parse_result_envelope_accepts_a_streaming_result_line_verbatim() {
    // spike §1.3: the NDJSON `result` line is byte-compatible with the one-shot
    // `--output-format json` envelope, extra fields and all.
    let line = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":1200,"result":"MERGED","total_cost_usd":0.0238,"session_id":"sess-stream","usage":{"input_tokens":9}}"#;
    let res = parse_result_envelope(line, true, "").expect("streaming result line parses");
    assert_eq!(res.text, "MERGED");
    assert_eq!(res.cost_usd, Some(0.0238));
    assert_eq!(res.session_id.as_deref(), Some("sess-stream"));
}

#[test]
fn tool_policy_args_are_the_verified_allowlist() {
    assert_eq!(ToolPolicy::ReadOnly.arg(), "Read,Grep,Glob");
    assert_eq!(ToolPolicy::None.arg(), "");
    let d = RunLimits::default();
    assert_eq!(d.idle_timeout, DEFAULT_IDLE_TIMEOUT);
    assert_eq!(d.hard_cap, None);
    assert_eq!(d.max_turns, DEFAULT_MAX_TURNS);
    assert_eq!(d.tools, ToolPolicy::ReadOnly);
    assert_eq!(d.max_budget_usd, None);
    assert!(!d.include_partial_messages);
    assert!(d.interactive, "streaming defaults to the interactive mechanism");
}

#[test]
fn kill_pid_tree_ignores_pid_zero() {
    // "not spawned" must never turn into a kill of an unrelated process.
    kill_pid_tree(0);
}
