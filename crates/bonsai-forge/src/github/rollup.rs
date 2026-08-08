//! PURE CI/commit-status rollup math (§7), separated from GitHub wire mapping.
//!
//! This module operates ONLY on provider-neutral inputs — raw state/conclusion
//! strings, [`CheckRollup`], and [`StatusContext`]. The `Gh*` wire structs stay
//! private to `dto.rs`, which extracts their fields and delegates here. Keeping
//! the math here (and the JSON in `dto.rs`) holds both files under ~500 lines.

use crate::types::{CheckRollup, CommitStatus, StatusContext};

/// Legacy commit-status state → [`CheckRollup`].
pub(crate) fn normalize_status_state(state: &str) -> CheckRollup {
    match state {
        "success" => CheckRollup::Success,
        "pending" => CheckRollup::Pending,
        "failure" | "error" => CheckRollup::Failure,
        _ => CheckRollup::Error,
    }
}

/// A check-run's (status, conclusion) → [`CheckRollup`].
pub(crate) fn normalize_check_run(status: &str, conclusion: Option<&str>) -> CheckRollup {
    if status != "completed" {
        return CheckRollup::Pending;
    }
    match conclusion {
        Some("success") => CheckRollup::Success,
        Some("neutral") | Some("skipped") => CheckRollup::Neutral,
        Some("failure") | Some("timed_out") | Some("cancelled") | Some("action_required")
        | Some("startup_failure") => CheckRollup::Failure,
        _ => CheckRollup::Error,
    }
}

/// Per-context counts accompanying the overall rollup.
struct RollupCounts {
    total: u32,
    passed: u32,
    failed: u32,
    pending: u32,
}

/// Overall rollup + counts over a normalized context list (§7 precedence):
/// any Failure/Error ⇒ Failure; else any Pending ⇒ Pending; else any Success ⇒
/// Success; else any Neutral ⇒ Neutral; else None.
fn compute_rollup(states: &[CheckRollup]) -> (CheckRollup, RollupCounts) {
    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;
    let mut any_fail = false;
    let mut any_pending = false;
    let mut any_success = false;
    let mut any_neutral = false;

    for s in states {
        match s {
            CheckRollup::Success => {
                passed += 1;
                any_success = true;
            }
            CheckRollup::Failure | CheckRollup::Error => {
                failed += 1;
                any_fail = true;
            }
            CheckRollup::Pending => {
                pending += 1;
                any_pending = true;
            }
            CheckRollup::Neutral => any_neutral = true,
            CheckRollup::None => {}
        }
    }

    let state = if any_fail {
        CheckRollup::Failure
    } else if any_pending {
        CheckRollup::Pending
    } else if any_success {
        CheckRollup::Success
    } else if any_neutral {
        CheckRollup::Neutral
    } else {
        CheckRollup::None
    };

    (
        state,
        RollupCounts {
            total: states.len() as u32,
            passed,
            failed,
            pending,
        },
    )
}

/// Assemble a [`CommitStatus`] from the merged neutral context list: cap at 50
/// individual checks, then compute the overall rollup + counts. PURE — no wire
/// structs, no JSON.
pub(crate) fn build_commit_status(sha: &str, mut contexts: Vec<StatusContext>) -> CommitStatus {
    contexts.truncate(50);
    let states: Vec<CheckRollup> = contexts.iter().map(|c| c.state).collect();
    let (state, counts) = compute_rollup(&states);
    CommitStatus {
        sha: sha.to_string(),
        state,
        total: counts.total,
        passed: counts.passed,
        failed: counts.failed,
        pending: counts.pending,
        contexts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(name: &str, state: CheckRollup) -> StatusContext {
        StatusContext {
            name: name.to_string(),
            state,
            description: None,
            target_url: None,
        }
    }

    #[test]
    fn normalize_status_state_maps_legacy_values() {
        assert_eq!(normalize_status_state("success"), CheckRollup::Success);
        assert_eq!(normalize_status_state("pending"), CheckRollup::Pending);
        assert_eq!(normalize_status_state("failure"), CheckRollup::Failure);
        assert_eq!(normalize_status_state("error"), CheckRollup::Failure);
        assert_eq!(normalize_status_state("weird"), CheckRollup::Error);
    }

    #[test]
    fn normalize_check_run_maps_status_and_conclusion() {
        assert_eq!(normalize_check_run("queued", None), CheckRollup::Pending);
        assert_eq!(normalize_check_run("in_progress", None), CheckRollup::Pending);
        assert_eq!(
            normalize_check_run("completed", Some("success")),
            CheckRollup::Success
        );
        assert_eq!(
            normalize_check_run("completed", Some("neutral")),
            CheckRollup::Neutral
        );
        assert_eq!(
            normalize_check_run("completed", Some("skipped")),
            CheckRollup::Neutral
        );
        for c in ["failure", "timed_out", "cancelled", "action_required", "startup_failure"] {
            assert_eq!(
                normalize_check_run("completed", Some(c)),
                CheckRollup::Failure,
                "conclusion {c}"
            );
        }
        assert_eq!(
            normalize_check_run("completed", Some("mystery")),
            CheckRollup::Error
        );
        assert_eq!(normalize_check_run("completed", None), CheckRollup::Error);
    }

    #[test]
    fn rollup_precedence_failure_wins() {
        let (state, counts) = compute_rollup(&[
            CheckRollup::Success,
            CheckRollup::Pending,
            CheckRollup::Failure,
            CheckRollup::Neutral,
        ]);
        assert_eq!(state, CheckRollup::Failure);
        assert_eq!(counts.total, 4);
        assert_eq!(counts.passed, 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.pending, 1);
    }

    #[test]
    fn rollup_precedence_error_counts_as_failure() {
        let (state, counts) = compute_rollup(&[CheckRollup::Success, CheckRollup::Error]);
        assert_eq!(state, CheckRollup::Failure);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.passed, 1);
    }

    #[test]
    fn rollup_precedence_pending_over_success() {
        let (state, _) = compute_rollup(&[CheckRollup::Success, CheckRollup::Pending]);
        assert_eq!(state, CheckRollup::Pending);
    }

    #[test]
    fn rollup_precedence_success_over_neutral() {
        let (state, _) = compute_rollup(&[CheckRollup::Neutral, CheckRollup::Success]);
        assert_eq!(state, CheckRollup::Success);
    }

    #[test]
    fn rollup_precedence_neutral_when_only_neutral() {
        let (state, _) = compute_rollup(&[CheckRollup::Neutral, CheckRollup::Neutral]);
        assert_eq!(state, CheckRollup::Neutral);
    }

    #[test]
    fn rollup_none_when_empty() {
        let (state, counts) = compute_rollup(&[]);
        assert_eq!(state, CheckRollup::None);
        assert_eq!(counts.total, 0);
    }

    #[test]
    fn build_commit_status_caps_at_50_and_counts() {
        // 60 successes + one failure ⇒ capped to 50 contexts, overall Failure
        // only if the failure survives the cap; place it first so it does.
        let mut contexts = vec![ctx("fail", CheckRollup::Failure)];
        for i in 0..60 {
            contexts.push(ctx(&format!("ok{i}"), CheckRollup::Success));
        }
        let status = build_commit_status("sha", contexts);
        assert_eq!(status.contexts.len(), 50, "capped at 50");
        assert_eq!(status.total, 50);
        assert_eq!(status.state, CheckRollup::Failure);
        assert_eq!(status.failed, 1);
        assert_eq!(status.passed, 49);
    }
}
