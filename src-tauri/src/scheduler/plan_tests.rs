use super::test_support::MIN;
use super::*;

// ---------------- planner unit tests (pure, fake time) ----------------

/// Disabled jobs are never due, regardless of state (§9).
#[test]
fn plan_disabled_never_due() {
    assert_eq!(
        plan(false, 5 * MIN, 1_000_000_000, Some(0), false, 0),
        PlanDecision::Wait {
            next_run_ms: i64::MAX
        }
    );
    assert_eq!(
        plan(false, 5 * MIN, 0, None, false, 7),
        PlanDecision::Wait {
            next_run_ms: i64::MAX
        }
    );
}

/// First sight (`last_run == None`) waits one full interval (D13).
#[test]
fn plan_first_seen_waits_full_interval() {
    let now = 10 * MIN;
    assert_eq!(
        plan(true, 5 * MIN, now, None, false, 0),
        PlanDecision::Wait {
            next_run_ms: now + 5 * MIN
        }
    );
}

/// Not yet due → Wait with the exact next-run time; due → Run.
#[test]
fn plan_due_and_not_due() {
    let last = 100 * MIN;
    // One ms early: wait.
    assert_eq!(
        plan(true, 5 * MIN, last + 5 * MIN - 1, Some(last), false, 0),
        PlanDecision::Wait {
            next_run_ms: last + 5 * MIN
        }
    );
    // Exactly at the boundary: run.
    assert_eq!(
        plan(true, 5 * MIN, last + 5 * MIN, Some(last), false, 0),
        PlanDecision::Run
    );
    // Late: still run.
    assert_eq!(
        plan(true, 5 * MIN, last + 50 * MIN, Some(last), false, 0),
        PlanDecision::Run
    );
}

/// Due but still running → SkipOverlap (D4); running but NOT due → Wait.
#[test]
fn plan_overlap_guard() {
    let last = 0;
    assert_eq!(
        plan(true, 5 * MIN, 6 * MIN, Some(last), true, 0),
        PlanDecision::SkipOverlap
    );
    assert_eq!(
        plan(true, 5 * MIN, 2 * MIN, Some(last), true, 0),
        PlanDecision::Wait {
            next_run_ms: 5 * MIN
        }
    );
}

/// Backoff progression 1×/1×/1×/2×/4×/8×/8× and the D6 formula (base for
/// failures 0–2; base*2^(f-2) for ≥3; capped 8×).
#[test]
fn effective_interval_backoff_progression() {
    let base = 5 * MIN;
    assert_eq!(effective_interval_ms(base, 0), base);
    assert_eq!(effective_interval_ms(base, 1), base);
    assert_eq!(effective_interval_ms(base, 2), base);
    assert_eq!(effective_interval_ms(base, 3), 2 * base);
    assert_eq!(effective_interval_ms(base, 4), 4 * base);
    assert_eq!(effective_interval_ms(base, 5), 8 * base);
    assert_eq!(effective_interval_ms(base, 6), 8 * base); // cap
    assert_eq!(effective_interval_ms(base, 100), 8 * base); // cap, no overflow
}

/// In backoff, the job is not due at base interval but IS due at the
/// backed-off interval; a success resets to base (D6).
#[test]
fn plan_respects_backoff_and_reset() {
    let base = 5 * MIN;
    let last = 0;
    // 3 failures → 2× interval: not due at base + 1.
    assert_eq!(
        plan(true, base, base + 1, Some(last), false, 3),
        PlanDecision::Wait {
            next_run_ms: 2 * base
        }
    );
    assert_eq!(plan(true, base, 2 * base, Some(last), false, 3), PlanDecision::Run);
    // After a success (failures reset to 0) the base interval applies.
    assert_eq!(plan(true, base, base, Some(last), false, 0), PlanDecision::Run);
}

/// next_run_estimate: None when disabled or never seen; otherwise
/// last + effective interval.
#[test]
fn next_run_estimate_semantics() {
    assert_eq!(next_run_estimate_ms(false, 5 * MIN, Some(0), 0), None);
    assert_eq!(next_run_estimate_ms(true, 5 * MIN, None, 0), None);
    assert_eq!(next_run_estimate_ms(true, 5 * MIN, Some(100), 0), Some(100 + 5 * MIN));
    assert_eq!(
        next_run_estimate_ms(true, 5 * MIN, Some(100), 4),
        Some(100 + 20 * MIN)
    );
}
