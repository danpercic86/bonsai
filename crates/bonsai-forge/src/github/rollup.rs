//! GitHub-specific CI/status vocabulary → neutral [`CheckRollup`].
//!
//! Only the GitHub string mapping lives here; the provider-NEUTRAL precedence +
//! counts + cap (and the batch helper) live in [`crate::rollup`], shared with
//! the other providers. `dto.rs` maps GitHub wire fields into [`StatusContext`]
//! via these functions, then delegates to `crate::rollup::build_commit_status`.

use crate::types::CheckRollup;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
