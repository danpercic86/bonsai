//! The idle watchdog, driven entirely by VIRTUAL time (P91 §10.1) — the D3 pair.
//!
//! Split from [`super`] so that module is the streaming PROTOCOL end to end and
//! nothing else. These two are the only tests that advance the clock at all: one
//! is the watchdog's positive control, the other proves the same advance is
//! declined while the run is parked on the sentinel. They are stated as a pair
//! and only mean anything as a pair, which is why they share a file.
//!
//! A `#[path]`-included CHILD module of `session_tests` on purpose: it reaches
//! that module's private harness (`drive`, `limits`, `expect_awaiting`, the
//! budgets) exactly as before the split, so NOTHING had to be widened.

use std::thread;
use std::time::Duration;

use super::{drive, expect_awaiting, STUB_STARTUP_BUDGET, WATCHDOG_IDLE};
use crate::ai::clock::TestClock;
use crate::ai::testutil::{env_lock, set_mode, wait_until, Sink};
use crate::ai::{AiRunEvent, AiRunEventKind, AiRunRegistry, RunLimits};
use crate::error::AppError;

/// The watchdog's POSITIVE control, and the ordering half of the D3 pair below.
///
/// Nothing about wall time is asserted: the run cannot age at all until
/// `clock.advance` says so, and the only thing that can end it is the watchdog
/// (the stub then stays silent for ~30 s, far past anything this test does). So
/// the property is causal — *`init` line observed, then idle time introduced, so
/// the watchdog fires and the already-collected log survives* — which is exactly
/// what D2 claims.
#[test]
fn stream_slow_watchdog_fails_and_keeps_the_collected_log() {
    let _g = env_lock();
    set_mode("stream_slow");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };
    let clock = TestClock::new();
    // `RunControl` owns an mpsc `Receiver`, which is `Send` but not `Sync`, so it
    // has to MOVE into the session thread; the clock stays here, shared by
    // reference, because advancing it is the whole point.
    let clock_ref = &clock;

    let err = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            drive(
                clock_ref,
                "payload",
                RunLimits { idle_timeout: WATCHDOG_IDLE, ..RunLimits::default() },
                &ctl,
                &collect,
            )
        });
        // The CAUSAL precondition for the D2 assertion below — the log line has to
        // exist before the watchdog can be asked to preserve it. Previously this
        // was implicit ("2 s is surely enough for the stub to start"), and it is
        // the exact assumption that failed on a loaded box.
        assert!(
            wait_until(|| sink.has_text("session sess-slow"), STUB_STARTUP_BUDGET),
            "the stub never logged its init line: {:?}",
            sink.texts()
        );
        // The ONLY source of idle time in this run.
        clock.advance(WATCHDOG_IDLE + Duration::from_secs(1));
        handle.join().expect("session thread should not panic")
    })
    .expect_err("an elapsed idle limit must end the run");
    reg.finish(&run_id);

    match &err {
        AppError::AiFailed(m) => assert!(m.contains("no output for 2s"), "got {m}"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    // D2 — THE regression guard: today's `run_process` would have thrown this away.
    assert!(
        sink.has_text("session sess-slow"),
        "watchdog must keep the lines already read: {:?}",
        sink.texts()
    );
    let failed = sink.of_kind(AiRunEventKind::Failed);
    assert_eq!(failed.len(), 1, "exactly one terminal event");
    // Always PRESENT, and empty here because `stream_slow` emits no assistant
    // prose before it goes quiet — only the `init` system line, which is log-only
    // decoration (A5). The content-bearing case is `stream_partial` below.
    assert_eq!(
        failed[0].partial_text.as_deref(),
        Some(""),
        "no assistant prose arrived, so the echo must be present-but-empty"
    );
    assert_eq!(sink.kinds().last(), Some(&AiRunEventKind::Failed));
}

/// D3, as an ORDERING fact rather than "we slept longer than the limit".
///
/// The run is parked on the sentinel, then handed 100× its idle limit of virtual
/// time. The tick loop reads the clock exactly once per tick, so waiting for
/// [`TestClock::reads`] to grow proves the loop actually LOOKED at that time and
/// declined to act on it — the sensitivity a `thread::sleep` only ever assumed,
/// and lost first on a loaded box. That the same advance DOES kill an unparked
/// run is the sibling test above; together they pin the pause, not the timing.
#[test]
fn watchdog_does_not_fire_while_awaiting_input() {
    let _g = env_lock();
    set_mode("stream_ask");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };
    let clock = TestClock::new();
    let clock_ref = &clock;

    let res = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            drive(
                clock_ref,
                "payload",
                // A live watchdog, and (below) a human who takes far longer than it
                // to answer. Without the D3 pause this run would be killed.
                RunLimits { idle_timeout: WATCHDOG_IDLE, ..RunLimits::default() },
                &ctl,
                &collect,
            )
        });
        expect_awaiting(&reg, &run_id, || handle.is_finished());
        let ticks_before = clock.reads();
        clock.advance(WATCHDOG_IDLE * 100);
        // Four full ticks with the watchdog long overdue and nothing arriving on
        // stdout: every one of them is an opportunity to fire that D3 must refuse.
        assert!(
            wait_until(|| clock.reads() >= ticks_before + 4, STUB_STARTUP_BUDGET),
            "the tick loop stopped consulting the clock — it is no longer awaiting"
        );
        reg.reply(&run_id, "take theirs".to_string()).expect("reply accepted");
        handle.join().expect("session thread should not panic")
    })
    .expect("D3: a run waiting on a human must never be killed");
    reg.finish(&run_id);
    assert_eq!(res.text, "ANSWERED_BODY");
    assert!(sink.of_kind(AiRunEventKind::Failed).is_empty(), "no watchdog failure");
}
