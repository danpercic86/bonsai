//! The streaming session's stdin/stderr PLUMBING (P68a review S1/S2) — split from
//! `session_tests` because both cases need their own hostile stub mode and both are
//! about a race, not about the protocol mapping:
//!
//! 1. the child's real stderr must reach the failure message even though stdout EOF
//!    and the last stderr line arrive from different senders (S1);
//! 2. a run must stay cancellable while the stdin write is blocked on a child that
//!    never drains it (S2) — streaming has NO wall-clock deadline by design, so an
//!    on-the-loop-thread write would be unkillable.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::testutil::{
    assert_child_is_dead, env_lock, marker_path, set_mode, set_mode_with_marker, wait_until, Sink,
};
use super::{run_claude_streaming, AiRunEvent, AiRunEventKind, AiRunRegistry, RunLimits, RunOpts};
use crate::error::AppError;

/// Larger than any OS pipe buffer (Windows ~4–64 KB, Linux 64 KB), so a stub that
/// never reads stdin CANNOT let the write finish.
const UNDRAINABLE_PAYLOAD_BYTES: usize = 1024 * 1024;

fn sink_and_collect() -> (Sink, impl Fn(AiRunEvent) + Send + Sync) {
    let sink = Sink::default();
    let s = sink.clone();
    (sink, move |ev: AiRunEvent| s.push(ev))
}

/// S1: the CLI prints a usage/auth error to stderr and exits non-zero. Without the
/// bounded post-EOF stderr drain this reports the generic "exited without a result"
/// roughly half the time — losing the only thing that tells the user what to fix.
#[test]
fn a_stderr_only_failure_surfaces_the_cli_s_own_message() {
    let _g = env_lock();
    set_mode("stream_stderr_fail");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let (sink, collect) = sink_and_collect();

    let err = run_claude_streaming(
        Path::new("."),
        "prompt",
        "payload",
        RunOpts::default(),
        RunLimits { idle_timeout: Duration::from_secs(10), ..RunLimits::default() },
        &ctl,
        &collect,
    )
    .expect_err("a non-zero exit with no result is a failure");
    reg.finish(&run_id);

    match &err {
        AppError::AiFailed(m) => assert!(
            m.contains("STUB_USAGE_ERROR: unknown option --verbose"),
            "the child's stderr must be IN the error, got {m}"
        ),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    // Same text on the terminal event, and in the dock log.
    let failed = sink.of_kind(AiRunEventKind::Failed);
    assert_eq!(failed.len(), 1, "exactly one terminal event: {:?}", sink.kinds());
    assert!(
        failed[0].text.as_deref().unwrap_or_default().contains("STUB_USAGE_ERROR"),
        "terminal event text: {:?}",
        failed[0].text
    );
    assert!(sink.has_text("stderr: STUB_USAGE_ERROR"), "log: {:?}", sink.texts());
}

/// S2: the stub never reads stdin and outlives the test, so the 1 MiB write is
/// still in flight when we cancel. Passing proves TWO things that were false while
/// the write ran on the loop thread:
///
/// - the loop reached `pump` at all (the `init` log event below), and
/// - `ctl.cancel` was still being polled, so the run ends in ~a tick instead of
///   waiting out the stub's 20 s sleep.
///
/// The marker check at the end adds the third property DIRECTLY rather than by
/// argument: the blocked write must not have left the child alive — while it lives
/// the stub ticks the marker every second.
#[test]
fn cancel_works_while_the_stdin_write_is_blocked() {
    let _g = env_lock();
    let marker = marker_path("hang");
    set_mode_with_marker("stream_hang_stdin", &marker);
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let (sink, collect) = sink_and_collect();
    let payload = "x".repeat(UNDRAINABLE_PAYLOAD_BYTES);

    let mut cancelled_at = Instant::now();
    let outcome = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            run_claude_streaming(
                Path::new("."),
                "prompt",
                &payload,
                RunOpts::default(),
                // No watchdog and no cap: cancel is the ONLY thing that can stop
                // this run, exactly as in the real (deadline-free) streaming path.
                RunLimits { idle_timeout: Duration::ZERO, hard_cap: None, ..RunLimits::default() },
                &ctl,
                &collect,
            )
        });
        // The stub's `init` line can only become an event if the loop is running
        // while the write is blocked.
        assert!(
            wait_until(|| sink.len() >= 2, Duration::from_secs(10)),
            "no event past `started`: the loop is blocked in the stdin write ({:?})",
            sink.kinds()
        );
        cancelled_at = Instant::now();
        assert!(reg.cancel(&run_id), "registry should know the run");
        handle.join().expect("session thread should not panic")
    });
    let cancel_latency = cancelled_at.elapsed();
    reg.finish(&run_id);

    match &outcome {
        Err(AppError::AiCancelled(m)) => assert_eq!(m, "cancelled by user"),
        other => panic!("expected AiCancelled, got {other:?}"),
    }
    // Generous, but far below the stub's 20 s sleep: a blocked write must not
    // delay the cancel.
    assert!(
        cancel_latency < Duration::from_secs(8),
        "cancel took {cancel_latency:?} — the blocked write delayed it"
    );
    assert_eq!(sink.kinds().last(), Some(&AiRunEventKind::Cancelled));
    assert!(sink.has_text("session sess-hang"), "log kept: {:?}", sink.texts());
    assert!(!reg.is_awaiting(&run_id), "awaiting flag cleared");

    // No surviving child (§10.1) — the 1 MiB write it was blocked on must not have
    // kept it (or its `ping`/`sleep`) alive past the reap.
    assert_child_is_dead(&marker);
}
