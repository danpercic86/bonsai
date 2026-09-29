//! The pure check planner (contract §6.2): no IO, no clock, no locks.

/// Why a check starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// First activation of the repo in this process, or never attempted.
    Open,
    Interval,
    /// `deps_check_now`.
    Manual,
    /// The tracker was just switched on (`apply_config` false → true).
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepPlan {
    Run(Trigger),
    Wait { next_ms: Option<i64> },
}

/// §6.2. Failed attempts stamp `last_attempt_ms` too, so there is no
/// backoff and no fast retry. A `last_attempt_ms` more than one interval in
/// the future (clock moved backwards) is treated as due.
pub fn plan_dep_check(
    enabled: bool,
    is_active: bool,
    running: bool,
    opened_unchecked: bool,
    interval_ms: i64,
    now_ms: i64,
    last_attempt_ms: Option<i64>,
) -> DepPlan {
    if !enabled || !is_active || running {
        return DepPlan::Wait { next_ms: None };
    }
    let Some(t) = last_attempt_ms.filter(|_| !opened_unchecked) else {
        return DepPlan::Run(Trigger::Open);
    };
    let next = t.saturating_add(interval_ms);
    if now_ms >= next || t > now_ms.saturating_add(interval_ms) {
        DepPlan::Run(Trigger::Interval)
    } else {
        DepPlan::Wait {
            next_ms: Some(next),
        }
    }
}
