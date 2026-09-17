//! End-to-end tests for the streaming session (P68a §9): the stub CLI speaks
//! NDJSON, so these cover the whole loop — event ordering, turn accounting, the
//! idle watchdog, cancel, and the D2 guarantee that partial output survives.
//!
//! P91 — every run here is driven through a [`TestClock`] the test owns, so NO
//! assertion in this file depends on wall-clock time:
//!
//! - tests that are about the PROTOCOL never advance the clock, so the idle
//!   watchdog and the hard cap literally cannot fire and cannot kill a run over a
//!   limit the test does not care about;
//! - the two tests that ARE about the watchdog advance it explicitly, which makes
//!   "the watchdog fired" a consequence of a test action instead of a race
//!   between the idle limit and however long `cmd.exe` took to start.
//!
//! Real time survives only as GIVE-UP BOUNDS on waits for observable events
//! ([`STUB_STARTUP_BUDGET`]) — nothing asserts how much of one is used, so a
//! slower box simply uses more of it.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use super::clock::TestClock;
use super::session::SessionDeps;
use super::testutil::{
    assert_child_is_dead, env_lock, marker_path, set_mode, set_mode_with_marker, wait_until, Sink,
    STUB_MODE_ENV,
};
use super::{
    run_claude_streaming_with_clock, AiResult, AiRunEvent, AiRunEventKind, AiRunRegistry,
    RunControl, RunLimits, RunOpts, ToolPolicy, CLAUDE_BIN_ENV,
};
use crate::error::AppError;

/// How long a test waits for something the stub child must OBSERVABLY do (its
/// first log line, its sentinel). Deliberately enormous relative to the ~100 ms a
/// quiet box needs: it is the point at which the test gives up and reports a
/// broken stub, NOT a margin anything is asserted against. A saturated box just
/// consumes more of it — 4 s was the worst measured while three full `cargo
/// nextest` suites and a `cargo build --workspace` ran concurrently.
const STUB_STARTUP_BUDGET: Duration = Duration::from_secs(60);

/// Test limits: an idle window that never elapses, because the clock these tests
/// hand the session never moves unless they move it.
fn limits(idle_secs: u64) -> RunLimits {
    RunLimits {
        idle_timeout: Duration::from_secs(idle_secs),
        ..RunLimits::default()
    }
}

/// VIRTUAL idle limit for the watchdog tests (§10.1). The number is arbitrary
/// now — it is only ever compared against virtual time the test itself adds — and
/// is kept at the production-ish 2 s purely so the failure message the assertions
/// match on ("no output for 2s") reads like a real one.
const WATCHDOG_IDLE: Duration = Duration::from_secs(2);

/// Drive one streaming run against a clock the TEST owns (see the module docs).
fn drive(
    clock: &TestClock,
    payload: &str,
    limits: RunLimits,
    ctl: &RunControl,
    on_event: &(dyn Fn(AiRunEvent) + Send + Sync),
) -> Result<AiResult, AppError> {
    run_claude_streaming_with_clock(
        Path::new("."),
        "prompt",
        payload,
        RunOpts::default(),
        limits,
        SessionDeps {
            ctl,
            on_event,
            clock,
        },
    )
}

/// Wait for the sentinel to block the run. A failure here almost always means the
/// run ALREADY ENDED rather than a broken sentinel, so report which it was.
fn expect_awaiting(reg: &AiRunRegistry, run_id: &str, finished: impl Fn() -> bool) {
    if wait_until(|| reg.is_awaiting(run_id), STUB_STARTUP_BUDGET) {
        return;
    }
    panic!(
        "the sentinel should have blocked the run (session already finished: {})",
        finished()
    );
}

/// Every event of a run: same `runId`, gap-free monotonic `seq` from 0, and seq 0
/// is `Started` (D8 — the UI cannot cancel a run whose id it never saw).
fn assert_sequence(sink: &Sink, run_id: &str) {
    let events = sink.events();
    assert!(!events.is_empty(), "no events emitted");
    for (i, ev) in events.iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "seq must be monotonic from 0: {ev:?}");
        assert_eq!(ev.run_id, run_id, "runId must be stable: {ev:?}");
    }
    assert_eq!(events[0].kind, AiRunEventKind::Started);
    assert_eq!(events[0].turn, 0, "Started carries turn 0 (A6)");
}

#[test]
fn stream_success_emits_started_logs_turn_end_and_done() {
    let _g = env_lock();
    set_mode("stream_success");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let res = drive(&TestClock::new(), "payload", limits(10), &ctl, &collect)
        .expect("stream_success should resolve");
    // §3.7: after `complete()` the shared pid must be back to 0 so a late
    // `cancel_all` cannot kill a recycled pid.
    assert_eq!(
        ctl.pid.load(Ordering::Relaxed),
        0,
        "pid must reset after completion"
    );
    reg.finish(&run_id);

    assert_eq!(res.text, "MERGED_STREAM_BODY");
    assert_eq!(res.cost_usd, Some(0.0238));
    assert_eq!(res.session_id.as_deref(), Some("sess-stream"));

    assert_sequence(&sink, &run_id);
    let kinds = sink.kinds();
    assert_eq!(
        kinds.last(),
        Some(&AiRunEventKind::Done),
        "kinds: {kinds:?}"
    );
    assert_eq!(sink.of_kind(AiRunEventKind::TurnEnd).len(), 1);
    let done = sink.of_kind(AiRunEventKind::Done);
    assert_eq!(done[0].cost_usd, Some(0.0238));
    assert_eq!(done[0].turn, 1, "one turn completed");

    // The mapping table, observed through the session.
    assert!(
        sink.has_text("session sess-stream · model sonnet · tools: Read, Grep, Glob"),
        "init line missing: {:?}",
        sink.texts()
    );
    assert!(
        sink.has_text("MERGED_STREAM_BODY"),
        "assistant text missing"
    );
    assert!(sink.has_text("summary: status=review_ready needsAction=false"));
    // A4: heartbeats never become a LOG LINE.
    assert!(!sink.has_text("thinking"), "heartbeat leaked into the log");
    // P68d: they DO surface their cumulative `estimated_tokens` as a metrics-only
    // event — `kind: Log`, `text: None` — which is the run's only live spend proxy
    // before the first `cost_usd` (that arrives only at a turn boundary).
    let metrics: Vec<_> = sink
        .of_kind(AiRunEventKind::Log)
        .into_iter()
        .filter(|e| e.text.is_none())
        .collect();
    assert_eq!(metrics.len(), 1, "one heartbeat -> one metrics event");
    assert_eq!(metrics[0].thinking_tokens, Some(420));
    // And no ordinary log line ever carries the field, so a consumer can key on it.
    assert!(
        sink.of_kind(AiRunEventKind::Log)
            .iter()
            .all(|e| e.text.is_none() == e.thinking_tokens.is_some()),
        "text and thinkingTokens must be mutually exclusive on a log event"
    );
}

#[test]
fn cancel_mid_run_keeps_partial_output_and_leaves_no_child() {
    let _g = env_lock();
    let marker = marker_path("cancel");
    set_mode_with_marker("stream_slow", &marker);

    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };
    let clock = TestClock::new();
    let clock_ref = &clock;
    // `ctl` moves into the session thread (see the watchdog test), so keep the
    // shared pid cell out here for the §3.7 assertion below.
    let pid = std::sync::Arc::clone(&ctl.pid);

    let outcome = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            drive(
                clock_ref,
                "payload",
                // No watchdog at all, AND a clock that never moves: the ONLY thing
                // that stops this run is cancel.
                RunLimits {
                    idle_timeout: Duration::ZERO,
                    ..RunLimits::default()
                },
                &ctl,
                &collect,
            )
        });
        // Cancel once the child has actually produced its first line.
        assert!(
            wait_until(|| sink.len() >= 2, STUB_STARTUP_BUDGET),
            "stub produced no output to cancel into"
        );
        assert!(reg.cancel(&run_id), "registry should know the run");
        handle.join().expect("session thread should not panic")
    });
    // §3.7: after `reap()` the shared pid must be back to 0 so a late
    // `cancel_all` cannot kill a recycled pid.
    assert_eq!(
        pid.load(Ordering::Relaxed),
        0,
        "pid must reset after cancellation"
    );
    reg.finish(&run_id);

    match &outcome {
        Err(AppError::AiCancelled(m)) => assert_eq!(m, "cancelled by user"),
        other => panic!("expected AiCancelled, got {other:?}"),
    }
    let cancelled = sink.of_kind(AiRunEventKind::Cancelled);
    assert_eq!(cancelled.len(), 1);
    // Present, and empty for the same reason as the watchdog test: `stream_slow`
    // is cancelled after its `init` line, before any assistant prose (A5).
    assert_eq!(
        cancelled[0].partial_text.as_deref(),
        Some(""),
        "D2: the echo is always present"
    );
    assert!(
        sink.has_text("session sess-slow"),
        "log kept: {:?}",
        sink.texts()
    );

    // No surviving child (§10.1). Delete-then-stay-gone rather than never-appeared:
    // a loaded box can let the stub tick once BEFORE the kill lands, which says
    // nothing about survival (this test flaked exactly that way).
    assert_child_is_dead(&marker);
}

#[test]
fn stream_ask_completes_after_a_registry_reply() {
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
        let handle = scope.spawn(move || drive(clock_ref, "payload", limits(10), &ctl, &collect));
        expect_awaiting(&reg, &run_id, || handle.is_finished());
        reg.reply(&run_id, "the plural form".to_string())
            .expect("reply accepted");
        handle.join().expect("session thread should not panic")
    })
    .expect("the second turn should resolve");
    reg.finish(&run_id);

    assert_eq!(res.text, "ANSWERED_BODY");
    // A10/spike §1.8: the LAST result's cost wins; summing would give 0.0501.
    assert_eq!(res.cost_usd, Some(0.0263));

    assert_sequence(&sink, &run_id);
    let asked = sink.of_kind(AiRunEventKind::AwaitingInput);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].text.as_deref(), Some("which locale wins?"));
    assert_eq!(asked[0].turn, 1);
    assert_eq!(
        sink.of_kind(AiRunEventKind::TurnEnd).len(),
        2,
        "one per result line"
    );
    let done = sink.of_kind(AiRunEventKind::Done);
    assert_eq!(done[0].turn, 2);
    assert_eq!(done[0].cost_usd, Some(0.0263));
    assert!(
        sink.has_text("» answered (15 bytes)"),
        "reply log: {:?}",
        sink.texts()
    );
    assert!(
        !reg.is_awaiting(&run_id),
        "awaiting flag cleared after the reply"
    );
}

#[test]
fn turn_budget_fails_a_repeatedly_questioning_model() {
    let _g = env_lock();
    set_mode("stream_ask");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let err = drive(
        &TestClock::new(),
        "payload",
        RunLimits {
            max_turns: 1,
            ..limits(10)
        },
        &ctl,
        &collect,
    )
    .expect_err("one turn of budget must not allow a question");
    reg.finish(&run_id);

    match &err {
        AppError::AiFailed(m) => assert!(
            m.contains("asked 1 questions without producing a resolution"),
            "got {m}"
        ),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    assert!(
        sink.of_kind(AiRunEventKind::AwaitingInput).is_empty(),
        "never blocks on a reply"
    );
}

#[test]
fn one_shot_mode_rejects_a_question_it_cannot_answer() {
    let _g = env_lock();
    set_mode("stream_ask");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    // Non-interactive: the prompt is positional argv and stdin is closed after the
    // payload, so nobody could deliver an answer.
    let err = drive(
        &TestClock::new(),
        "payload\n",
        RunLimits {
            interactive: false,
            tools: ToolPolicy::None,
            ..limits(10)
        },
        &ctl,
        &collect,
    )
    .expect_err("a sentinel in one-shot mode is a failure");
    reg.finish(&run_id);
    match &err {
        AppError::AiFailed(m) => assert!(m.contains("not interactive"), "got {m}"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
}

#[test]
fn stream_partial_fails_naming_the_missing_result_and_keeps_the_body() {
    let _g = env_lock();
    set_mode("stream_partial");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let err = drive(&TestClock::new(), "payload", limits(10), &ctl, &collect)
        .expect_err("a child that exits before `result` is a failure");
    reg.finish(&run_id);

    match &err {
        AppError::AiFailed(m) => assert!(m.contains("without a result"), "got {m}"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
    // D2: the assistant text that DID arrive is both logged and echoed back.
    assert!(sink.has_text("HALF_A_BODY"), "log: {:?}", sink.texts());
    let failed = sink.of_kind(AiRunEventKind::Failed);
    assert!(
        failed[0]
            .partial_text
            .as_deref()
            .unwrap_or_default()
            .contains("HALF_A_BODY"),
        "partialText must carry the assistant prose: {:?}",
        failed[0].partial_text
    );
}

#[test]
fn stream_garbage_lines_degrade_to_logs_and_the_run_still_succeeds() {
    let _g = env_lock();
    set_mode("stream_garbage");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let res = drive(&TestClock::new(), "payload", limits(10), &ctl, &collect)
        .expect("D12: unknown lines must never fail a run");
    reg.finish(&run_id);
    assert_eq!(res.text, "GARBAGE_TOLERATED");
    assert!(
        sink.has_text("this is not json at all"),
        "log: {:?}",
        sink.texts()
    );
    assert!(
        sink.has_text("brand_new_event"),
        "unknown type kept verbatim"
    );
    assert_eq!(sink.kinds().last(), Some(&AiRunEventKind::Done));
}

#[test]
fn stream_bulk_returns_both_delimited_blocks() {
    let _g = env_lock();
    set_mode("stream_bulk");
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let res = drive(&TestClock::new(), "payload", limits(10), &ctl, &collect)
        .expect("bulk stub should resolve");
    reg.finish(&run_id);
    // The split itself is P68b; P68a only guarantees the body arrives intact.
    assert!(res.text.contains("===== BONSAI RESULT: a/one.json ====="));
    assert!(res.text.contains("ONE_BODY"));
    assert!(res.text.contains("===== BONSAI RESULT: b/two.json ====="));
    assert!(res.text.contains("TWO_BODY"));
}

#[test]
fn missing_binary_emits_failed_then_returns_ai_unavailable() {
    let _g = env_lock();
    std::env::set_var(CLAUDE_BIN_ENV, "D:/nonexistent/claude-does-not-exist.exe");
    std::env::remove_var(STUB_MODE_ENV);
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let err = drive(&TestClock::new(), "payload", limits(10), &ctl, &collect)
        .expect_err("a missing CLI is AiUnavailable, as in run_claude");
    reg.finish(&run_id);
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");
    // Started still reached the UI first (D8), and the failure is still an event.
    assert_eq!(
        sink.kinds(),
        vec![AiRunEventKind::Started, AiRunEventKind::Failed]
    );
    assert_sequence(&sink, &run_id);
}

/// The two watchdog tests (D3), the only ones here that advance the clock. A
/// child module so they keep reaching the harness above without widening it;
/// kept in their own file to keep this one focused on the protocol.
#[path = "session_watchdog_tests.rs"]
mod watchdog_tests;
