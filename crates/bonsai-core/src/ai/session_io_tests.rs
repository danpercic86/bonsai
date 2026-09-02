//! The streaming session's stdin/stderr PLUMBING (P68a review S1/S2) — split from
//! `session_tests` because both cases need their own hostile stub mode and both are
//! about a race, not about the protocol mapping:
//!
//! 1. the child's real stderr must reach the failure message even though stdout EOF
//!    and the last stderr line arrive from different senders (S1);
//! 2. a run must stay cancellable while the stdin write is blocked on a child that
//!    never drains it (S2) — streaming has NO wall-clock deadline by design, so an
//!    on-the-loop-thread write would be unkillable.
//!
//! Both runs are driven through a frozen [`TestClock`] (P91), so the idle watchdog
//! and the hard cap cannot end them: what ends them is the property under test.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::clock::TestClock;
use super::session::SessionDeps;
use super::testutil::{
    assert_child_is_dead, env_lock, marker_path, set_mode, set_mode_with_marker, wait_until, Sink,
};
use super::{
    run_claude_streaming_with_clock, AiRunEvent, AiRunEventKind, AiRunRegistry, RunLimits, RunOpts,
};
use crate::error::AppError;

/// Larger than any OS pipe buffer (Windows ~4–64 KB, Linux 64 KB), so a stub that
/// never reads stdin CANNOT let the write finish.
const UNDRAINABLE_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Give-up bound on waiting for an observable event from the stub child, not a
/// margin anything is asserted against (see `session_tests::STUB_STARTUP_BUDGET`).
const STUB_STARTUP_BUDGET: Duration = Duration::from_secs(60);

/// The one REAL timing assertion left in the `ai` tests, and deliberately so: S2's
/// whole claim is that cancel is prompt EVEN THOUGH a 1 MiB write is stuck, so
/// there is nothing to virtualise — a clock the test controls would just make the
/// claim vacuous.
///
/// The number is chosen from both sides. Below it: cancelling costs a
/// `taskkill /T /F` process spawn plus a `wait()`, which is process-creation work
/// on a box that may be saturated — 5.2 s was the worst end-to-end time measured
/// with three `cargo nextest` suites and a `cargo build --workspace` running
/// concurrently, so 15 s is ~3x headroom over that. Above it: the stub holds
/// stdin for ~60 s, so a pass still proves by a factor of 4 that the cancel did
/// not simply wait the child out, which is the only way this assertion could pass
/// for the wrong reason.
const MAX_CANCEL_LATENCY: Duration = Duration::from_secs(15);

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

    let err = run_claude_streaming_with_clock(
        Path::new("."),
        "prompt",
        "payload",
        RunOpts::default(),
        RunLimits { idle_timeout: Duration::from_secs(10), ..RunLimits::default() },
        SessionDeps { ctl: &ctl, on_event: &collect, clock: &TestClock::new() },
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
///   waiting out the stub's 60 s sleep ([`MAX_CANCEL_LATENCY`]).
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
    let clock = TestClock::new();
    // `RunControl` holds a non-`Sync` mpsc `Receiver`, so it MOVES into the
    // session thread; the clock is shared by reference.
    let clock_ref = &clock;

    let mut cancelled_at = Instant::now();
    let outcome = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            run_claude_streaming_with_clock(
                Path::new("."),
                "prompt",
                &payload,
                RunOpts::default(),
                // No watchdog and no cap: cancel is the ONLY thing that can stop
                // this run, exactly as in the real (deadline-free) streaming path.
                RunLimits { idle_timeout: Duration::ZERO, hard_cap: None, ..RunLimits::default() },
                SessionDeps { ctl: &ctl, on_event: &collect, clock: clock_ref },
            )
        });
        // The stub's `init` line can only become an event if the loop is running
        // while the write is blocked.
        assert!(
            wait_until(|| sink.len() >= 2, STUB_STARTUP_BUDGET),
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
    assert!(
        cancel_latency < MAX_CANCEL_LATENCY,
        "cancel took {cancel_latency:?} — the blocked write delayed it"
    );
    assert_eq!(sink.kinds().last(), Some(&AiRunEventKind::Cancelled));
    assert!(sink.has_text("session sess-hang"), "log kept: {:?}", sink.texts());
    assert!(!reg.is_awaiting(&run_id), "awaiting flag cleared");

    // No surviving child (§10.1) — the 1 MiB write it was blocked on must not have
    // kept it (or its `ping`/`sleep`) alive past the reap.
    assert_child_is_dead(&marker);
}
