//! On-disk bisect state persistence + the midpoint search math. Extracted
//! verbatim from `bisect.rs` (file-size discipline); the public command entry
//! points and worktree helpers stay in the module root.

use std::collections::HashSet;

use crate::error::AppError;

use super::{
    checkout_commit, estimated_steps, oid, BisectOutcome, BisectProgress, BisectState,
};

// ---------------------------------------------------------------- on-disk state

fn bonsai_dir(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join("bonsai-bisect")
}

fn state_path(repo: &git2::Repository) -> std::path::PathBuf {
    bonsai_dir(repo).join("state.json")
}

/// True iff a Bonsai bisect is in progress (state file present).
pub(crate) fn bisect_in_progress(repo: &git2::Repository) -> bool {
    state_path(repo).exists()
}

/// Guard for user-facing mutations. A Bonsai bisect runs on a DETACHED HEAD with
/// `repo.state() == Clean` (its progress lives in `.git/bonsai-bisect/`, not
/// libgit2 op-state), so the usual `state() != Clean` checks do NOT see it. Call
/// this at the START of every user-facing mutating entry point (commit / amend /
/// reset / stash create+apply+pop / merge / rebase / cherry-pick / revert) so a
/// mid-bisect mutation is refused. Never call it from the bisect or interactive-
/// rebase engines' own replay/reset paths (they must run mid-bisect).
pub(crate) fn require_no_bisect(repo: &git2::Repository) -> Result<(), AppError> {
    if bisect_in_progress(repo) {
        return Err(AppError::OperationInProgress(
            "a bisect is in progress — finish or reset it first".to_string(),
        ));
    }
    Ok(())
}

/// Why the bisect state file could not be read (F-A3-2 / F-A3-4): the caller
/// must distinguish "no file" (no bisect) from an io fault (surface the real
/// error) from "file exists but undecodable" (salvageable corruption).
pub(super) enum StateReadError {
    Missing,
    Io(std::io::Error),
    Corrupt(serde_json::Error),
}

pub(super) fn read_state_raw(repo: &git2::Repository) -> Result<BisectState, StateReadError> {
    let raw = match std::fs::read_to_string(state_path(repo)) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StateReadError::Missing),
        Err(e) => return Err(StateReadError::Io(e)),
    };
    serde_json::from_str(&raw).map_err(StateReadError::Corrupt)
}

/// Reads + parses `.git/bonsai-bisect/state.json`. Missing → "no bisect";
/// unreadable → the REAL io error (F-A3-4, not "missing"); corrupt → `Git`.
pub(crate) fn read_state(repo: &git2::Repository) -> Result<BisectState, AppError> {
    read_state_raw(repo).map_err(|e| match e {
        StateReadError::Missing => AppError::Git("bisect state is missing".to_string()),
        StateReadError::Io(e) => AppError::Git(format!("failed to read bisect state: {e}")),
        StateReadError::Corrupt(e) => AppError::Git(format!("bisect state is corrupt: {e}")),
    })
}

/// Writes the state file (create_dir_all + temp-file rename for atomicity).
pub(super) fn write_state(repo: &git2::Repository, state: &BisectState) -> Result<(), AppError> {
    let dir = bonsai_dir(repo);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| AppError::Git(format!("failed to serialize bisect state: {e}")))?;
    let tmp = dir.join("state.json.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, dir.join("state.json"))?;
    Ok(())
}

/// Removes `.git/bonsai-bisect/` (best-effort).
pub(super) fn remove_state(repo: &git2::Repository) {
    let _ = std::fs::remove_dir_all(bonsai_dir(repo));
}

// ---------------------------------------------------------------- midpoint math

/// Candidate set = commits reachable from `bad` but NOT from any `good`
/// (topological, bad-first; includes `bad`, excludes good & their ancestors).
pub(super) fn candidate_oids(
    repo: &git2::Repository,
    state: &BisectState,
) -> Result<Vec<git2::Oid>, AppError> {
    let bad = oid(&state.bad)?;
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL)?;
    walk.push(bad)?;
    for g in &state.good {
        walk.hide(oid(g)?)?;
    }
    Ok(walk.filter_map(Result::ok).collect())
}

/// The next action after the current bounds: test a midpoint, converge on the
/// first-bad, or report all-skipped.
enum Next {
    Testing(git2::Oid, u32, u32),
    Converged(git2::Oid),
    AllSkipped,
}

fn pick_next(repo: &git2::Repository, state: &BisectState) -> Result<Next, AppError> {
    let bad = oid(&state.bad)?;
    let skipped: HashSet<git2::Oid> =
        state.skipped.iter().filter_map(|s| git2::Oid::from_str(s).ok()).collect();
    let cand = candidate_oids(repo, state)?;
    // Testable = candidates minus `bad` (already known bad) minus skipped.
    let testable: Vec<git2::Oid> = cand
        .iter()
        .copied()
        .filter(|c| *c != bad && !skipped.contains(c))
        .collect();

    if testable.is_empty() {
        // Nothing left to test. If a skipped candidate remains unresolved we
        // cannot determine the culprit; otherwise every ancestor is good, so the
        // current `bad` bound IS the first-bad commit.
        let unresolved_skipped = cand.iter().any(|c| *c != bad && skipped.contains(c));
        if unresolved_skipped {
            Ok(Next::AllSkipped)
        } else {
            Ok(Next::Converged(bad))
        }
    } else {
        // Count-based positional split (contract §2.1, decision 4).
        let mid = testable[testable.len() / 2];
        let remaining = testable.len() as u32;
        Ok(Next::Testing(mid, remaining, estimated_steps(remaining)))
    }
}

/// Recompute + check out the next midpoint (or converge), persisting the state.
pub(super) fn drive(
    repo: &git2::Repository,
    state: &mut BisectState,
) -> Result<BisectOutcome, AppError> {
    match pick_next(repo, state)? {
        Next::Testing(mid, remaining, steps) => {
            checkout_commit(repo, mid)?;
            state.current = Some(mid.to_string());
            state.first_bad = None;
            write_state(repo, state)?;
            Ok(BisectOutcome::Testing {
                current: mid.to_string(),
                revisions_remaining: remaining,
                estimated_steps: steps,
            })
        }
        Next::Converged(first_bad) => {
            // Leave HEAD detached at the culprit so the worktree shows it.
            checkout_commit(repo, first_bad)?;
            state.current = None;
            state.first_bad = Some(first_bad.to_string());
            write_state(repo, state)?;
            Ok(BisectOutcome::Found {
                first_bad: first_bad.to_string(),
            })
        }
        Next::AllSkipped => {
            state.current = None;
            write_state(repo, state)?;
            Ok(BisectOutcome::CannotDetermine {
                skipped: state.skipped.clone(),
            })
        }
    }
}

/// Read-only projection of the current bounds for the banner/opstate.
pub(super) fn progress_from_state(
    repo: &git2::Repository,
    state: &BisectState,
) -> Result<BisectProgress, AppError> {
    let (remaining, steps) = if state.first_bad.is_some() {
        (0, 0)
    } else {
        let bad = oid(&state.bad)?;
        let skipped: HashSet<git2::Oid> =
            state.skipped.iter().filter_map(|s| git2::Oid::from_str(s).ok()).collect();
        let cand = candidate_oids(repo, state)?;
        let testable = cand
            .iter()
            .filter(|c| **c != bad && !skipped.contains(c))
            .count() as u32;
        (testable, estimated_steps(testable))
    };
    Ok(BisectProgress {
        bad: state.bad.clone(),
        good: state.good.clone(),
        skipped: state.skipped.clone(),
        current: state.current.clone(),
        first_bad: state.first_bad.clone(),
        revisions_remaining: remaining,
        estimated_steps: steps,
    })
}
