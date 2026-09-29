//! E4 AC8: the §6.2 planner table.

use super::plan::{plan_dep_check, DepPlan, Trigger};

const H: i64 = 3_600_000;
const I: i64 = 12 * H;
const NOW: i64 = 1_000 * H;

fn p(enabled: bool, active: bool, running: bool, opened: bool, last: Option<i64>) -> DepPlan {
    plan_dep_check(enabled, active, running, opened, I, NOW, last)
}

#[test]
fn disabled_background_or_running_always_waits_without_a_next_time() {
    let wait = DepPlan::Wait { next_ms: None };
    assert_eq!(p(false, true, false, true, None), wait);
    assert_eq!(p(true, false, false, true, None), wait);
    assert_eq!(p(true, true, true, true, None), wait);
    assert_eq!(p(false, true, false, false, Some(NOW - 2 * I)), wait);
}

#[test]
fn an_unchecked_open_or_a_first_attempt_runs_as_open() {
    assert_eq!(
        p(true, true, false, true, None),
        DepPlan::Run(Trigger::Open)
    );
    assert_eq!(
        p(true, true, false, false, None),
        DepPlan::Run(Trigger::Open)
    );
    // Opened this process, even though an attempt was recorded before.
    assert_eq!(
        p(true, true, false, true, Some(NOW - H)),
        DepPlan::Run(Trigger::Open)
    );
}

#[test]
fn the_interval_decides_after_the_first_attempt() {
    assert_eq!(
        p(true, true, false, false, Some(NOW - H)),
        DepPlan::Wait {
            next_ms: Some(NOW - H + I)
        }
    );
    assert_eq!(
        p(true, true, false, false, Some(NOW - I)),
        DepPlan::Run(Trigger::Interval)
    );
    assert_eq!(
        p(true, true, false, false, Some(NOW - 3 * I)),
        DepPlan::Run(Trigger::Interval)
    );
}

#[test]
fn a_last_attempt_far_in_the_future_is_due() {
    // Clock moved backwards by more than one interval.
    assert_eq!(
        p(true, true, false, false, Some(NOW + I + 1)),
        DepPlan::Run(Trigger::Interval)
    );
    // Slightly in the future (within one interval) just waits.
    assert_eq!(
        p(true, true, false, false, Some(NOW + H)),
        DepPlan::Wait {
            next_ms: Some(NOW + H + I)
        }
    );
}
