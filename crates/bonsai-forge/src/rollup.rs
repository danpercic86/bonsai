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

use crate::types::{CheckRollup, CommitStatus, StatusContext};

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
///     account/transport-level ⇒ propagate and fail the whole batch, so the
///     caller can back off.
///
/// Returns only the resolved shas (callers key by `status.sha`, so the order
/// among them is irrelevant).
pub(crate) fn batch_commit_statuses<F>(
    shas: &[String],
    mut one: F,
) -> Result<Vec<CommitStatus>, AppError>
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
            Err(e) => return Err(e),         // fatal ⇒ fail the batch
        }
    }
    Ok(out)
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
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn batch_omits_not_found_and_propagates_fatal() {
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
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|s| s.sha != "b"));

        // A fatal error (AuthFailed) fails the whole batch.
        let err = batch_commit_statuses(&shas, |sha| {
            if sha == "b" {
                Err(AppError::AuthFailed("nope".into()))
            } else {
                Ok(ok_status(sha))
            }
        })
        .unwrap_err();
        assert!(matches!(err, AppError::AuthFailed(_)));
    }
}
