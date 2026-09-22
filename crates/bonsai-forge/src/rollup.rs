//! PURE, provider-NEUTRAL CI/commit-status math shared by every provider.
//!
//! This module operates ONLY on the neutral [`CheckRollup`]/[`StatusContext`]
//! types — never a provider wire struct. Each provider's `dto`/`rest` maps its
//! own CI vocabulary onto [`CheckRollup`] (GitHub's `normalize_*`, GitLab's
//! pipeline states, …) and then delegates the cap + rollup precedence + counts
//! here, so the (drift-prone) precedence algorithm lives in exactly one place.
//! The batch helper is likewise neutral: it dedups/caps shas and classifies
//! per-sha errors identically for all providers.

use std::collections::HashSet;

use bonsai_core::error::AppError;

use crate::types::{CheckRollup, CommitStatus, CommitStatusBatch, StatusContext};

/// Hard cap on the number of individual checks kept in a [`CommitStatus`].
const MAX_CONTEXTS: usize = 50;

/// Hard cap on the number of shas a single `commit_statuses` batch resolves
/// (contract §4 backstop). Bounds the serial HTTP calls regardless of caller;
/// the P63b `useForgeSignals` hook also caps/dedups — defense-in-depth.
pub(crate) const MAX_STATUS_BATCH: usize = 100;

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

/// Assemble a [`CommitStatus`] from the merged neutral context list: cap at
/// [`MAX_CONTEXTS`] individual checks, then compute the overall rollup + counts.
/// PURE — no wire structs, no JSON.
pub(crate) fn build_commit_status(sha: &str, mut contexts: Vec<StatusContext>) -> CommitStatus {
    contexts.truncate(MAX_CONTEXTS);
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

/// Resolve a [`CommitStatus`] for each sha via `one`, deduping the input and
/// capping it at [`MAX_STATUS_BATCH`] serial calls. Error policy (shared by all
/// providers):
///   * `ForgeApi` (almost always a 404 — the commit isn't on the remote, e.g.
///     an unpushed local branch tip or a fork PR head) ⇒ OMIT just that sha; a
///     single missing tip must not blank ALL CI dots.
///   * any OTHER error (auth / rate-limit / network / unsupported) is
///     account/transport-level ⇒ STOP the batch, because the next call would
///     fail the same way (and, for a rate limit, deepen the hole).
///
/// P113a — stopping is not the same as discarding. The statuses resolved before
/// the stop are returned in a [`CommitStatusBatch`] alongside the error that cut
/// the batch short: they are API work already paid for, and throwing them away
/// is what made the UI re-request the identical set and re-trigger the limit.
/// Only a stop with NOTHING resolved is an `Err` — there is no partial result to
/// hand back, and the caller's existing error path is the honest answer.
///
/// `statuses` holds only the resolved shas (callers key by `status.sha`, so the
/// order among them is irrelevant); the shas after the stop were never
/// attempted and are simply absent.
pub(crate) fn batch_commit_statuses<F>(
    shas: &[String],
    mut one: F,
) -> Result<CommitStatusBatch, AppError>
where
    F: FnMut(&str) -> Result<CommitStatus, AppError>,
{
    let mut seen: HashSet<&str> = HashSet::new();
    let deduped: Vec<&String> = shas
        .iter()
        .filter(|s| seen.insert(s.as_str()))
        .take(MAX_STATUS_BATCH)
        .collect();

    let mut out = Vec::with_capacity(deduped.len());
    for sha in deduped {
        match one(sha) {
            Ok(status) => out.push(status),
            Err(AppError::ForgeApi(_)) => {} // not-found ⇒ omit this sha
            // Fatal ⇒ stop here. Partial success when anything resolved.
            Err(e) => {
                if out.is_empty() {
                    return Err(e);
                }
                return Ok(CommitStatusBatch {
                    statuses: out,
                    stopped_by: Some(e),
                });
            }
        }
    }
    Ok(CommitStatusBatch::complete(out))
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

    fn ok_status(sha: &str) -> CommitStatus {
        build_commit_status(sha, vec![ctx("ci", CheckRollup::Success)])
    }

    #[test]
    fn batch_dedups_and_caps() {
        // Duplicate shas resolve once; the closure is invoked per unique sha.
        let mut calls = 0;
        let shas = vec!["a".to_string(), "a".to_string(), "b".to_string()];
        let out = batch_commit_statuses(&shas, |sha| {
            calls += 1;
            Ok(ok_status(sha))
        })
        .unwrap();
        assert_eq!(calls, 2, "deduped to 2 unique shas");
        assert_eq!(out.statuses.len(), 2);
        assert!(out.stopped_by.is_none());
    }

    #[test]
    fn batch_omits_not_found() {
        let shas = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        // A 404 (ForgeApi) on "b" is omitted; the rest resolve.
        let out = batch_commit_statuses(&shas, |sha| {
            if sha == "b" {
                Err(AppError::ForgeApi("not found".into()))
            } else {
                Ok(ok_status(sha))
            }
        })
        .unwrap();
        assert_eq!(out.statuses.len(), 2);
        assert!(out.statuses.iter().all(|s| s.sha != "b"));
        assert!(
            out.stopped_by.is_none(),
            "a 404 does not cut the batch short"
        );
    }

    /// P113a: the whole point — a rate limit mid-batch keeps what was resolved.
    #[test]
    fn batch_keeps_resolved_statuses_when_cut_short() {
        let shas = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut calls = 0;
        let out = batch_commit_statuses(&shas, |sha| {
            calls += 1;
            if sha == "c" {
                Err(AppError::forge_rate_limited("slow down", Some(30)))
            } else {
                Ok(ok_status(sha))
            }
        })
        .unwrap();
        assert_eq!(calls, 3);
        assert_eq!(out.statuses.len(), 2, "a + b survive the 429 on c");
        match out.stopped_by {
            Some(AppError::ForgeRateLimited {
                retry_after_secs, ..
            }) => assert_eq!(retry_after_secs, Some(30)),
            other => panic!("expected a rate-limit stop, got {other:?}"),
        }
    }

    /// Stopping BEFORE anything resolved has no partial result to report, so it
    /// stays an `Err` and the caller's error path is unchanged.
    #[test]
    fn batch_with_nothing_resolved_is_still_an_error() {
        let shas = vec!["a".to_string(), "b".to_string()];
        let err = batch_commit_statuses(&shas, |_sha| {
            Err::<CommitStatus, _>(AppError::AuthFailed("nope".into()))
        })
        .unwrap_err();
        assert!(matches!(err, AppError::AuthFailed(_)));
    }

    /// A stop after a 404 counts the 404 as "omitted", not as "resolved": the
    /// batch still has nothing to hand back, so it errors.
    #[test]
    fn batch_stop_after_only_omitted_shas_is_an_error() {
        let shas = vec!["a".to_string(), "b".to_string()];
        let err = batch_commit_statuses(&shas, |sha| {
            if sha == "a" {
                Err(AppError::ForgeApi("not found".into()))
            } else {
                Err(AppError::NetworkError("offline".into()))
            }
        })
        .unwrap_err();
        assert!(matches!(err, AppError::NetworkError(_)));
    }
}
