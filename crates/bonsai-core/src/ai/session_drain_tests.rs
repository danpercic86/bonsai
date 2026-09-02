//! The post-stdout-EOF stderr drain, tested DETERMINISTICALLY (P68a review S1).
//!
//! The end-to-end stub test (`session_io_tests`) cannot pin this down: a stub that
//! writes stderr and exits normally lets the stderr reader win the race almost
//! every time, so it would pass with or without the drain. Here the ordering is
//! forced — the stderr line is queued only AFTER the decision point is reached, so
//! without the drain the message is the useless generic one.
//!
//! P91 — that forcing used to smuggle in a second, unwanted race: a sender thread
//! sleeping 30–50 ms against a 150 ms per-`recv` grace. On a loaded box the sender
//! simply wasn't scheduled inside the window and the drain gave up, so the test
//! failed for a scheduling reason rather than a behavioural one. Every test that
//! spawns such a sender now raises the session's grace to [`PATIENT_GRACE`], which
//! makes the LATENESS of the send irrelevant and leaves only the ordering claim.
//! The two tests that are about the BOUND itself keep the production constants —
//! that bound is what they exist to check.

use std::sync::mpsc::channel;
use std::thread;

// `super` is now `session_drain`, so the names the assertions need that the drain
// itself does not use are imported explicitly (unchanged otherwise).
use super::*;
use crate::ai::clock::TestClock;
use crate::ai::session::SessionDeps;
use crate::ai::{AiRunEvent, AiRunRegistry, RunControl};

/// A throwaway registry entry's `RunControl`. The CALLER keeps it: since P68b a
/// session only BORROWS its control (a bulk run drives several sessions from one),
/// so the value has to outlive the session.
fn control() -> RunControl {
    AiRunRegistry::default().register().1
}

/// Long enough that no sender thread can lose its scheduling slot inside one
/// `recv` window, short enough that a genuinely broken drain still fails the run
/// rather than hanging the suite. Only ever used by the tests that spawn a
/// deliberately LATE sender.
const PATIENT_GRACE: Duration = Duration::from_secs(30);

/// A session wired to `ctl` and a no-op event sink, on a frozen clock (nothing
/// here consults it — the drain's own waits are real by design).
fn session<'a>(
    ctl: &'a RunControl,
    on_event: &'a (dyn Fn(AiRunEvent) + Send + Sync),
    clock: &'a TestClock,
) -> ClaudeSession<'a> {
    ClaudeSession::new(SessionDeps { ctl, on_event, clock })
}

/// [`session`], with the stderr-drain grace widened so a late sender cannot lose
/// the race (see the module docs). The ordering under test is unchanged: the
/// drain still has to LOOK at the channel for any of these to pass.
fn patient_session<'a>(
    ctl: &'a RunControl,
    on_event: &'a (dyn Fn(AiRunEvent) + Send + Sync),
    clock: &'a TestClock,
) -> ClaudeSession<'a> {
    let mut s = session(ctl, on_event, clock);
    s.stderr_grace = PATIENT_GRACE;
    s.stderr_grace_total = PATIENT_GRACE;
    s
}

/// The drain is limits-independent; only a promoted `result` consults them.
fn limits() -> RunLimits {
    RunLimits::default()
}

/// `ended_without_result` must not turn a failure into a success here, so unwrap
/// the expected `Err` in one place.
fn failure(end: Result<AiResult, LoopEnd>) -> String {
    match end {
        Err(LoopEnd::Failed(m)) => m,
        Err(LoopEnd::Cancelled) => panic!("the drain must not change the outcome kind"),
        Ok(res) => panic!("no `result` line was queued, yet the run succeeded: {res:?}"),
    }
}

#[test]
fn stderr_arriving_after_stdout_eof_still_reaches_the_failure_message() {
    let noop = |_ev: AiRunEvent| {};
    let ctl = control();
    let clock = TestClock::new();
    let mut s = patient_session(&ctl, &noop, &clock);
    let (tx, rx) = channel::<Msg>();
    // Queued 50 ms LATE: the exact ordering mpsc refuses to guarantee against
    // `OutEof`, and the one a bad flag / expired login produces in practice. HOW
    // late no longer matters — the drain waits `PATIENT_GRACE` for it.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let _ = tx.send(Msg::Err("error: invalid API key · please run /login".to_string()));
        let _ = tx.send(Msg::ErrEof);
    });

    let m = failure(s.ended_without_result(&rx, None, &limits()));
    assert!(
        m.contains("invalid API key") && m.starts_with("Claude exited without a result:"),
        "got {m}"
    );
}

/// On the `WriteErr` entry path stdout is STILL OPEN, so a `result` line can
/// legitimately arrive inside the drain window. Downgrading it to a log — and the
/// run to a failure — would report a failure for a run that actually succeeded.
/// (Unreachable from the `OutEof` path: per-sender FIFO puts stdout's lines ahead
/// of stdout's own EOF.)
#[test]
fn a_result_arriving_during_the_drain_completes_the_run() {
    const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"LATE_BODY","total_cost_usd":0.02,"session_id":"sess-late"}"#;
    let noop = |_ev: AiRunEvent| {};
    let ctl = control();
    let clock = TestClock::new();
    let mut s = patient_session(&ctl, &noop, &clock);
    let (tx, rx) = channel::<Msg>();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        // One sender, so FIFO: the stderr line is consumed first, then the result.
        let _ = tx.send(Msg::Err("warning: retrying".to_string()));
        let _ = tx.send(Msg::Out(RESULT_LINE.to_string()));
        let _ = tx.send(Msg::ErrEof);
    });

    match s.ended_without_result(&rx, Some("Broken pipe".to_string()), &limits()) {
        Ok(res) => {
            assert_eq!(res.text, "LATE_BODY");
            assert_eq!(res.cost_usd, Some(0.02));
        }
        Err(LoopEnd::Failed(m)) => panic!("a completed turn was downgraded to a failure: {m}"),
        Err(LoopEnd::Cancelled) => panic!("wrong outcome kind"),
    }
}

#[test]
fn a_write_error_reports_stderr_when_there_is_any_and_the_io_error_otherwise() {
    let noop = |_ev: AiRunEvent| {};
    let ctl = control();

    let clock = TestClock::new();

    // Nothing on stderr: the io error IS the diagnosis (§3.3's wording).
    // Every sender is already gone here, so this one needs no extra patience.
    let mut bare = session(&ctl, &noop, &clock);
    let (tx, rx) = channel::<Msg>();
    drop(tx);
    let m = failure(bare.ended_without_result(
        &rx,
        Some("Broken pipe (os error 32)".to_string()),
        &limits(),
    ));
    assert_eq!(m, "Claude closed its input: Broken pipe (os error 32)");

    // With stderr, the child's own words win over our `BrokenPipe` — the two race
    // by construction, so the message must not depend on who got there first.
    let mut with_stderr = patient_session(&ctl, &noop, &clock);
    let (tx, rx) = channel::<Msg>();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        let _ = tx.send(Msg::Err("error: unknown option --verbose".to_string()));
        let _ = tx.send(Msg::ErrEof);
    });
    let m =
        failure(with_stderr.ended_without_result(&rx, Some("Broken pipe".to_string()), &limits()));
    assert!(m.contains("unknown option --verbose"), "got {m}");
}

/// The drain is BOUNDED: a child that keeps spewing stderr must not stall
/// shutdown, so the total grace is capped regardless of how much is queued.
#[test]
fn a_chatty_stderr_cannot_stall_the_drain() {
    let noop = |_ev: AiRunEvent| {};
    let ctl = control();
    let clock = TestClock::new();
    // PRODUCTION graces on purpose: the cap is the property, so widening it would
    // erase the test.
    let mut s = session(&ctl, &noop, &clock);
    let (tx, rx) = channel::<Msg>();
    // Never sends ErrEof; stops only when the receiver goes away. THROTTLED on
    // purpose: the consumer does an O(MAX_EVENT_TEXT) trim plus an event build per
    // line, so an unthrottled producer outruns it by orders of magnitude and grows
    // the (unbounded) mpsc queue for the whole 1 s cap — hundreds of MB on CI. The
    // real producer is rate-limited by the child's pipe. What the assertion needs
    // is only that a line is always available inside each `STDERR_GRACE` window,
    // which a 200 µs cadence oversupplies by ~3 orders of magnitude.
    thread::spawn(move || {
        while tx.send(Msg::Err("noise".to_string())).is_ok() {
            thread::sleep(Duration::from_micros(200));
        }
    });

    let started = Instant::now();
    let outcome = s.ended_without_result(&rx, None, &limits());
    let took = started.elapsed();
    // A REAL timing assertion, kept because "bounded" is the whole claim. The
    // slack is 2 s rather than 500 ms: the alternative to a bounded drain is an
    // UNBOUNDED one, so any small constant separates pass from fail, and the
    // larger one also absorbs the scheduling jitter of the producer thread on a
    // saturated box (which is what made 500 ms marginal).
    assert!(took < STDERR_GRACE_TOTAL + Duration::from_secs(2), "drain took {took:?}");
    // Still the load-bearing half: without the drain there is no stderr at all in
    // the message, only the generic wording.
    let m = failure(outcome);
    assert!(m.contains("noise"), "got {m}");
}

/// No stderr at all and an immediate EOF: the generic message, and no waiting.
#[test]
fn a_silent_exit_yields_the_generic_message_without_waiting() {
    let noop = |_ev: AiRunEvent| {};
    let ctl = control();
    let clock = TestClock::new();
    // The grace is raised to 30 s precisely so "did not wait" has somewhere to
    // fail: against the 150 ms production value the assertion was one scheduling
    // hiccup wide, whereas here a drain that waited would blow the budget by two
    // orders of magnitude and 5 s of slack cannot mask it.
    let mut s = patient_session(&ctl, &noop, &clock);
    let (tx, rx) = channel::<Msg>();
    let _ = tx.send(Msg::ErrEof);
    let started = Instant::now();
    let m = failure(s.ended_without_result(&rx, None, &limits()));
    assert_eq!(m, "Claude exited without a result");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "an already-queued ErrEof must not be waited on"
    );
}
