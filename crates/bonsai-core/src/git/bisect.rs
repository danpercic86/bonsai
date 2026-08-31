//! git bisect — a Bonsai-owned binary-search state machine on a detached HEAD,
//! with an on-disk JSON state file under `.git/bonsai-bisect/` (P39 contract).
//! libgit2 has no bisect sequencer, so this module mirrors `rebase_interactive.rs`:
//! a versioned `state.json`, atomic writes, re-read on every IPC call, never held
//! in memory, and a force-restore of the ORIGINAL HEAD/branch on reset.
//!
//! The ORIGINAL branch ref is NEVER moved during bisect (we only move a DETACHED
//! HEAD across midpoints), so reset just re-attaches HEAD to `original_branch`
//! (or force-detaches to `original_head` when originally detached) and deletes
//! the state dir. Pure git2, no Tauri types, no network.

use std::path::Path;

use crate::error::AppError;
use crate::git::repo::read_head_info;
use crate::git::stage::{ensure_no_untracked_collision, open_workdir_repo};

/// Persisted bisect progress. Re-read on every IPC call; deleted on reset.
/// NEVER held across calls. Wire-internal only (the frontend sees the
/// `RepoOpState::Bisect` projection, contract §5/§6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectState {
    /// schema version = 1
    pub version: u32,
    /// 40-hex HEAD commit before bisect started (abort restore of the worktree).
    pub original_head: String,
    /// Short branch name HEAD pointed at before bisect. `None` when bisect
    /// started from a detached HEAD → reset detaches back to `original_head`.
    pub original_branch: Option<String>,
    /// The known-BAD commit that bounds the search (40-hex).
    pub bad: String,
    /// Known-GOOD commits (ancestors excluded from the candidate set).
    pub good: Vec<String>,
    /// Commits the user marked SKIP — excluded as answers but NOT as ancestors.
    pub skipped: Vec<String>,
    /// The midpoint currently checked out and awaiting a verdict (40-hex). None
    /// only in the terminal `found` / `cannotDetermine` phases.
    pub current: Option<String>,
    /// Terminal result: the first-bad commit once the range converges.
    pub first_bad: Option<String>,
}

/// Outcome of start / mark / skip — drives the banner and any auto-checkout.
/// Wire: tagged "kind", camelCase (mirrored in TS, contract §5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BisectOutcome {
    /// Still searching. `current` is now checked out (detached HEAD).
    Testing {
        current: String,
        revisions_remaining: u32,
        estimated_steps: u32,
    },
    /// Range converged. `first_bad` is the culprit; HEAD is detached at it until
    /// the user resets.
    Found { first_bad: String },
    /// Every remaining candidate is skipped → cannot determine. State is kept so
    /// the user can reset. No new checkout.
    CannotDetermine { skipped: Vec<String> },
}

/// Flattened progress the banner needs (matches `RepoOpState::Bisect` fields,
/// contract §6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BisectProgress {
    pub bad: String,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    pub current: Option<String>,
    pub first_bad: Option<String>,
    pub revisions_remaining: u32,
    pub estimated_steps: u32,
}

// On-disk state persistence + the midpoint search math live in the `engine`
// submodule; re-exported here so `crate::git::bisect::<item>` paths (and the
// sibling modules that reuse the guards) are unchanged.
mod engine;

pub(crate) use engine::{bisect_in_progress, read_state, require_no_bisect};
use engine::{
    candidate_oids, drive, progress_from_state, read_state_raw, remove_state, write_state,
    StateReadError,
};

// ---------------------------------------------------------------- helpers

pub(super) fn oid(s: &str) -> Result<git2::Oid, AppError> {
    git2::Oid::from_str(s).map_err(|_| AppError::Git("corrupt bisect state: invalid oid".to_string()))
}

/// `ceil(log2(remaining))` — the count-based step estimate. 0 when `remaining`
/// is 0 or 1 (no further branching needed).
pub(super) fn estimated_steps(remaining: u32) -> u32 {
    if remaining <= 1 {
        return 0;
    }
    u32::BITS - (remaining - 1).leading_zeros()
}

/// Clean-index + clean-worktree guard (mirrors `start_interactive_rebase`).
/// Compares the index to whatever HEAD points at (branch tip on start; the
/// detached midpoint on each step) and refuses on any tracked unstaged change.
fn ensure_clean(repo: &git2::Repository) -> Result<(), AppError> {
    let head_commit = repo.head()?.peel_to_commit()?;
    let mut index = repo.index()?;
    if index.has_conflicts() || index.write_tree_to(repo)? != head_commit.tree_id() {
        return Err(AppError::Git(
            "cannot bisect: your index contains uncommitted changes — commit or stash them first"
                .to_string(),
        ));
    }
    drop(index);
    let mut sopts = git2::StatusOptions::new();
    sopts.include_untracked(false).include_ignored(false);
    if !repo.statuses(Some(&mut sopts))?.is_empty() {
        return Err(AppError::Git(
            "cannot bisect: you have unstaged changes — commit or stash them first".to_string(),
        ));
    }
    Ok(())
}

/// Guards that HEAD is still detached on the midpoint we recorded (contract §2):
/// mark/skip trust `state.current`, so a HEAD moved externally between steps
/// (still clean → `ensure_clean` passes) would otherwise record the verdict
/// against the WRONG commit and corrupt the search. `current` must be Some and
/// equal the resolved HEAD commit. Returns the verified midpoint oid on success.
fn ensure_on_current(
    repo: &git2::Repository,
    current: &Option<String>,
) -> Result<String, AppError> {
    let expected = current.clone().ok_or_else(|| {
        AppError::Git(
            "cannot mark: no commit is currently under test — reset to finish the bisect"
                .to_string(),
        )
    })?;
    let head = repo.head()?.peel_to_commit()?.id().to_string();
    if head != expected {
        return Err(AppError::Git(
            "worktree is not on the bisect commit; run reset or re-checkout".to_string(),
        ));
    }
    Ok(expected)
}

/// Clean detached checkout onto `target` (worktree already verified clean).
pub(super) fn checkout_commit(repo: &git2::Repository, target: git2::Oid) -> Result<(), AppError> {
    let commit = repo.find_commit(target)?;
    // Data-loss guard: refuse before touching HEAD if the force checkout would
    // clobber an untracked, non-ignored worktree file present in the target tree.
    ensure_no_untracked_collision(repo, &commit.tree()?)?;
    repo.set_head_detached(target)?;
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_tree(commit.as_object(), Some(&mut co))?;
    Ok(())
}

// ---------------------------------------------------------------- start

/// Start a bisect. `bad` = known-bad commit, `good` = one or more known-good
/// ancestors. Detaches HEAD onto the first midpoint. Errors:
/// OperationInProgress (a bisect/other op is active), Git (unborn / bad oid /
/// non-ancestor good / same good&bad / degenerate range / dirty worktree).
pub fn start_bisect(
    workdir: &Path,
    bad: &str,
    good: &[String],
) -> Result<BisectOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    if bisect_in_progress(&repo) {
        return Err(AppError::OperationInProgress(
            "a bisect is already in progress — reset it first".to_string(),
        ));
    }
    // F-A3-3: the Bonsai interactive-rebase sequencer also runs on a detached
    // HEAD with `repo.state() == Clean`, so the check below does not see it —
    // guard against it explicitly (mirrors `start_interactive_rebase`).
    if crate::git::rebase_interactive::interactive_in_progress(&repo) {
        return Err(AppError::OperationInProgress(
            "an interactive rebase is in progress — continue or abort it first".to_string(),
        ));
    }
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — commit or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot bisect: the repository has no commits yet".to_string(),
        ));
    }

    if bad.trim().is_empty() {
        return Err(AppError::Git("invalid commit id".to_string()));
    }
    let bad_oid =
        git2::Oid::from_str(bad).map_err(|_| AppError::Git("invalid commit id".to_string()))?;
    repo.find_commit(bad_oid)?;

    if good.is_empty() {
        return Err(AppError::Git(
            "cannot bisect: at least one good commit is required".to_string(),
        ));
    }
    let mut good_oids: Vec<String> = Vec::new();
    for g in good {
        if g.trim().is_empty() {
            return Err(AppError::Git("invalid commit id".to_string()));
        }
        let g_oid =
            git2::Oid::from_str(g).map_err(|_| AppError::Git("invalid commit id".to_string()))?;
        repo.find_commit(g_oid)?;
        if g_oid == bad_oid {
            return Err(AppError::Git(
                "nothing to bisect: good and bad are the same commit".to_string(),
            ));
        }
        if !repo.graph_descendant_of(bad_oid, g_oid)? {
            return Err(AppError::Git(format!(
                "good commit {} is not an ancestor of the bad commit",
                &g_oid.to_string()[..7]
            )));
        }
        if !good_oids.contains(&g_oid.to_string()) {
            good_oids.push(g_oid.to_string());
        }
    }

    ensure_clean(&repo)?;

    let mut state = BisectState {
        version: 1,
        original_head: head.oid.clone(),
        original_branch: if head.detached {
            None
        } else {
            head.branch_name.clone()
        },
        bad: bad_oid.to_string(),
        good: good_oids,
        skipped: Vec::new(),
        current: None,
        first_bad: None,
    };

    // At least one testable commit must exist between good and bad.
    let cand = candidate_oids(&repo, &state)?;
    if cand.iter().filter(|c| **c != bad_oid).count() == 0 {
        return Err(AppError::Git(
            "nothing to bisect: no testable commits between the good and bad commits".to_string(),
        ));
    }

    write_state(&repo, &state)?;
    // The first checkout happens inside `drive`. If it refuses (e.g. an untracked
    // file would be clobbered by the midpoint checkout — the guard fires BEFORE
    // `set_head_detached`, so HEAD/worktree are untouched), drop the just-written
    // state so Start stays atomic: either the bisect begins or nothing does.
    // NOTE: this assumes a first-drive failure is pre-checkout — on Start every
    // fallible step (`pick_next`, the collision guard, `find_commit`) runs before
    // `set_head_detached`, so HEAD has not moved when we roll the state back.
    match drive(&repo, &mut state) {
        Ok(outcome) => Ok(outcome),
        Err(e) => {
            remove_state(&repo);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------- mark / skip

/// Mark the currently-checked-out midpoint good/bad, then recompute + check out
/// the next midpoint (or converge). Errors: NoOperationInProgress, Git (dirty
/// worktree / no commit under test).
pub fn bisect_mark(workdir: &Path, is_good: bool) -> Result<BisectOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !bisect_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no bisect in progress".to_string(),
        ));
    }
    let mut state = read_state(&repo)?;
    let current = ensure_on_current(&repo, &state.current)?;
    ensure_clean(&repo)?;

    if is_good {
        if !state.good.contains(&current) {
            state.good.push(current);
        }
    } else {
        state.bad = current;
    }
    drive(&repo, &mut state)
}

/// Skip the current (untestable) midpoint — bounds unchanged — then pick an
/// adjacent candidate; all-skipped → CannotDetermine. Errors as `bisect_mark`.
pub fn bisect_skip(workdir: &Path) -> Result<BisectOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !bisect_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no bisect in progress".to_string(),
        ));
    }
    let mut state = read_state(&repo)?;
    let current = ensure_on_current(&repo, &state.current)?;
    ensure_clean(&repo)?;

    if !state.skipped.contains(&current) {
        state.skipped.push(current);
    }
    drive(&repo, &mut state)
}

// ---------------------------------------------------------------- reset

/// Abort/finish: force-restore the ORIGINAL HEAD/branch + worktree, delete
/// `.git/bonsai-bisect/`. Errors: NoOperationInProgress, Git (checkout failure).
/// A CORRUPT state file is salvaged (F-A3-2): the state dir is cleared so the
/// app is no longer wedged, HEAD is left in place, and a distinct Git error
/// explains what happened.
pub fn bisect_reset(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !bisect_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no bisect in progress".to_string(),
        ));
    }
    let state = match read_state_raw(&repo) {
        Ok(s) => s,
        // F-A3-2 salvage: the state file EXISTS but cannot be decoded. Without
        // this, reset fails forever while `require_no_bisect` (existence-only)
        // keeps blocking every mutation — an in-app deadlock. Clear the state
        // dir, leave HEAD exactly where it is, and say so.
        Err(StateReadError::Corrupt(e)) => {
            remove_state(&repo);
            return Err(AppError::Git(format!(
                "the bisect state file was corrupt ({e}); it has been cleared and the bisect \
                 abandoned. HEAD was left where it is — check out your original branch manually \
                 if needed"
            )));
        }
        Err(StateReadError::Missing) => {
            return Err(AppError::NoOperationInProgress(
                "no bisect in progress".to_string(),
            ))
        }
        Err(StateReadError::Io(e)) => {
            return Err(AppError::Git(format!("failed to read bisect state: {e}")))
        }
    };
    restore_to_original(&repo, &state)
}

/// Re-attach HEAD to the original branch (its ref never moved) or force-detach
/// to the original head, hard-restore the worktree+index, drop the state file.
fn restore_to_original(repo: &git2::Repository, state: &BisectState) -> Result<(), AppError> {
    let orig = oid(&state.original_head)?;
    let orig_commit = repo.find_commit(orig)?;
    // Data-loss guard (mirrors `checkout_commit`): refuse BEFORE touching HEAD
    // if the force checkout would clobber an untracked file (e.g. one generated
    // during a bisect test run) present in the original tree. The state file is
    // only removed at the end, so a refusal leaves the bisect intact — the user
    // can remove/stash the file and retry the reset.
    ensure_no_untracked_collision(repo, &orig_commit.tree()?)?;
    match &state.original_branch {
        Some(name) => {
            repo.set_head(&format!("refs/heads/{name}"))?;
        }
        None => {
            repo.set_head_detached(orig)?;
        }
    }
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_tree(orig_commit.as_object(), Some(&mut co))?;
    let mut idx = repo.index()?;
    idx.read_tree(&orig_commit.tree()?)?;
    idx.write()?;
    remove_state(repo);
    Ok(())
}

// ---------------------------------------------------------------- projection

/// Read-only projection of the current state for opstate/banner. Returns None
/// when no bisect is in progress.
pub fn get_bisect_state(workdir: &Path) -> Result<Option<BisectProgress>, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !bisect_in_progress(&repo) {
        return Ok(None);
    }
    let state = read_state(&repo)?;
    Ok(Some(progress_from_state(&repo, &state)?))
}

#[cfg(test)]
mod tests;
