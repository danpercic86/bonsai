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

use std::collections::HashSet;
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
enum StateReadError {
    Missing,
    Io(std::io::Error),
    Corrupt(serde_json::Error),
}

fn read_state_raw(repo: &git2::Repository) -> Result<BisectState, StateReadError> {
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
fn write_state(repo: &git2::Repository, state: &BisectState) -> Result<(), AppError> {
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
fn remove_state(repo: &git2::Repository) {
    let _ = std::fs::remove_dir_all(bonsai_dir(repo));
}

// ---------------------------------------------------------------- helpers

fn oid(s: &str) -> Result<git2::Oid, AppError> {
    git2::Oid::from_str(s).map_err(|_| AppError::Git("corrupt bisect state: invalid oid".to_string()))
}

/// `ceil(log2(remaining))` — the count-based step estimate. 0 when `remaining`
/// is 0 or 1 (no further branching needed).
fn estimated_steps(remaining: u32) -> u32 {
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
fn checkout_commit(repo: &git2::Repository, target: git2::Oid) -> Result<(), AppError> {
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

// ---------------------------------------------------------------- midpoint math

/// Candidate set = commits reachable from `bad` but NOT from any `good`
/// (topological, bad-first; includes `bad`, excludes good & their ancestors).
fn candidate_oids(
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
fn drive(repo: &git2::Repository, state: &mut BisectState) -> Result<BisectOutcome, AppError> {
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
fn progress_from_state(
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
mod tests {
    use super::*;
    use crate::testutil::scratch_dir;

    fn sig() -> git2::Signature<'static> {
        git2::Signature::now("Test", "test@example.com").expect("sig")
    }

    /// Linear repo of `n` commits on the default branch (each adds `f{i}.txt`);
    /// from commit index `bug_at` onward each also writes `bug.txt` (the marker
    /// the predicate greps). Returns (dir, oids oldest-first).
    fn linear_repo_with_bug(n: usize, bug_at: usize) -> (tempfile::TempDir, Vec<String>) {
        let dir = scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
        }
        let s = sig();
        let mut oids = Vec::new();
        let mut parents: Vec<git2::Commit> = Vec::new();
        for i in 0..n {
            std::fs::write(dir.path().join(format!("f{i}.txt")), format!("c{i}\n")).expect("write");
            if i >= bug_at {
                std::fs::write(dir.path().join("bug.txt"), "boom\n").expect("write bug");
            }
            let mut idx = repo.index().expect("index");
            idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("add");
            idx.write().expect("write index");
            let tree = repo
                .find_tree(idx.write_tree().expect("tree"))
                .expect("find tree");
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
            let commit_oid = repo
                .commit(Some("HEAD"), &s, &s, &format!("c{i}"), &tree, &parent_refs)
                .expect("commit");
            oids.push(commit_oid.to_string());
            parents = vec![repo.find_commit(commit_oid).expect("find commit")];
        }
        (dir, oids)
    }

    /// True iff the commit's tree carries the `bug.txt` marker.
    fn has_bug(repo: &git2::Repository, oid_str: &str) -> bool {
        let c = repo
            .find_commit(git2::Oid::from_str(oid_str).expect("oid"))
            .expect("commit");
        c.tree().expect("tree").get_name("bug.txt").is_some()
    }

    // ------------------------------------------------ wire shapes (TS mirrors)

    #[test]
    fn bisect_outcome_wire_shape_is_camel_case() {
        let v = serde_json::to_value(BisectOutcome::Testing {
            current: "a".repeat(40),
            revisions_remaining: 4,
            estimated_steps: 2,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "kind": "testing",
                "current": "a".repeat(40),
                "revisionsRemaining": 4,
                "estimatedSteps": 2
            })
        );

        let v = serde_json::to_value(BisectOutcome::Found {
            first_bad: "b".repeat(40),
        })
        .expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "found", "firstBad": "b".repeat(40) }));

        let v = serde_json::to_value(BisectOutcome::CannotDetermine {
            skipped: vec!["c".repeat(40)],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "cannotDetermine", "skipped": ["c".repeat(40)] })
        );
    }

    #[test]
    fn estimated_steps_is_ceil_log2() {
        assert_eq!(estimated_steps(0), 0);
        assert_eq!(estimated_steps(1), 0);
        assert_eq!(estimated_steps(2), 1);
        assert_eq!(estimated_steps(3), 2);
        assert_eq!(estimated_steps(4), 2);
        assert_eq!(estimated_steps(5), 3);
        assert_eq!(estimated_steps(8), 3);
        assert_eq!(estimated_steps(9), 4);
    }

    #[test]
    fn state_round_trips_on_disk() {
        let (dir, oids) = linear_repo_with_bug(3, 2);
        let repo = git2::Repository::open(dir.path()).expect("open");
        let state = BisectState {
            version: 1,
            original_head: oids[2].clone(),
            original_branch: Some("main".to_string()),
            bad: oids[2].clone(),
            good: vec![oids[0].clone()],
            skipped: Vec::new(),
            current: Some(oids[1].clone()),
            first_bad: None,
        };
        assert!(!bisect_in_progress(&repo));
        write_state(&repo, &state).expect("write");
        assert!(bisect_in_progress(&repo));
        assert_eq!(read_state(&repo).expect("read"), state);
        remove_state(&repo);
        assert!(!bisect_in_progress(&repo));
    }

    // ------------------------------------------------------- preconditions

    #[test]
    fn start_rejects_unborn() {
        let dir = scratch_dir();
        git2::Repository::init(dir.path()).expect("init");
        match start_bisect(dir.path(), &"0".repeat(40), &["0".repeat(40)]).expect_err("unborn") {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn start_rejects_blank_args() {
        let (dir, oids) = linear_repo_with_bug(3, 2);
        // Blank bad.
        assert!(matches!(
            start_bisect(dir.path(), "", &[oids[0].clone()]).expect_err("blank bad"),
            AppError::Git(_)
        ));
        // Empty good list.
        assert!(matches!(
            start_bisect(dir.path(), &oids[2], &[]).expect_err("no good"),
            AppError::Git(_)
        ));
        // Blank good entry.
        assert!(matches!(
            start_bisect(dir.path(), &oids[2], &["".to_string()]).expect_err("blank good"),
            AppError::Git(_)
        ));
    }

    #[test]
    fn start_rejects_same_good_bad() {
        let (dir, oids) = linear_repo_with_bug(3, 2);
        match start_bisect(dir.path(), &oids[2], &[oids[2].clone()]).expect_err("same") {
            AppError::Git(m) => assert!(m.contains("same commit"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn start_rejects_non_ancestor_good() {
        // Two independent roots: good on a sibling branch is not an ancestor of bad.
        let dir = scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
        }
        let s = sig();
        // main: one commit.
        std::fs::write(dir.path().join("a.txt"), "a\n").expect("write");
        let mut idx = repo.index().expect("index");
        idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).expect("add");
        idx.write().expect("write");
        let tree = repo.find_tree(idx.write_tree().expect("t")).expect("ft");
        let bad = repo.commit(Some("HEAD"), &s, &s, "bad", &tree, &[]).expect("c");
        // orphan branch with a disjoint root commit.
        std::fs::write(dir.path().join("b.txt"), "b\n").expect("write");
        let mut idx2 = repo.index().expect("index");
        idx2.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).expect("add");
        idx2.write().expect("write");
        let tree2 = repo.find_tree(idx2.write_tree().expect("t")).expect("ft");
        let good = repo
            .commit(Some("refs/heads/other"), &s, &s, "other", &tree2, &[])
            .expect("c2");
        match start_bisect(dir.path(), &bad.to_string(), &[good.to_string()])
            .expect_err("non-ancestor")
        {
            AppError::Git(m) => assert!(m.contains("not an ancestor"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    #[test]
    fn mark_and_skip_without_start_are_no_op_in_progress() {
        let (dir, _oids) = linear_repo_with_bug(2, 1);
        assert!(matches!(
            bisect_mark(dir.path(), true).expect_err("no op"),
            AppError::NoOperationInProgress(_)
        ));
        assert!(matches!(
            bisect_skip(dir.path()).expect_err("no op"),
            AppError::NoOperationInProgress(_)
        ));
        assert!(matches!(
            bisect_reset(dir.path()).expect_err("no op"),
            AppError::NoOperationInProgress(_)
        ));
    }

    // ------------------------------------------------------- convergence

    #[test]
    fn linear_bisect_converges() {
        let n = 12;
        let bug_at = 7;
        let (dir, oids) = linear_repo_with_bug(n, bug_at);
        let repo = git2::Repository::open(dir.path()).expect("open");

        let mut outcome =
            start_bisect(dir.path(), &oids[n - 1], &[oids[0].clone()]).expect("start");
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 50, "bisect did not converge");
            match outcome {
                BisectOutcome::Testing { current, .. } => {
                    let bad = has_bug(&repo, &current);
                    outcome = bisect_mark(dir.path(), !bad).expect("mark");
                }
                BisectOutcome::Found { first_bad } => {
                    assert_eq!(first_bad, oids[bug_at], "culprit is the bug-introducing commit");
                    // HEAD is detached at the culprit.
                    let re = git2::Repository::open(dir.path()).expect("open");
                    assert!(re.head_detached().expect("detached"));
                    assert_eq!(
                        re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
                        oids[bug_at]
                    );
                    break;
                }
                BisectOutcome::CannotDetermine { .. } => panic!("unexpected cannotDetermine"),
            }
        }
    }

    #[test]
    fn skip_picks_adjacent() {
        let (dir, oids) = linear_repo_with_bug(8, 5);
        let first = match start_bisect(dir.path(), &oids[7], &[oids[0].clone()]).expect("start") {
            BisectOutcome::Testing { current, .. } => current,
            other => panic!("expected Testing, got {other:?}"),
        };
        match bisect_skip(dir.path()).expect("skip") {
            BisectOutcome::Testing { current, .. } => {
                assert_ne!(current, first, "skip picks a different candidate");
            }
            other => panic!("expected Testing after skip, got {other:?}"),
        }
    }

    #[test]
    fn all_skipped_cannot_determine() {
        // 3 commits: good=c0, bad=c2 → only c1 is testable. Skipping it exhausts
        // the testable set with an unresolved skipped candidate.
        let (dir, oids) = linear_repo_with_bug(3, 2);
        match start_bisect(dir.path(), &oids[2], &[oids[0].clone()]).expect("start") {
            BisectOutcome::Testing { current, .. } => assert_eq!(current, oids[1]),
            other => panic!("expected Testing, got {other:?}"),
        }
        match bisect_skip(dir.path()).expect("skip") {
            BisectOutcome::CannotDetermine { skipped } => {
                assert_eq!(skipped, vec![oids[1].clone()]);
            }
            other => panic!("expected CannotDetermine, got {other:?}"),
        }
    }

    #[test]
    fn mark_and_skip_reject_when_head_moved_off_midpoint() {
        let (dir, oids) = linear_repo_with_bug(8, 5);
        let repo = git2::Repository::open(dir.path()).expect("open");

        let midpoint = match start_bisect(dir.path(), &oids[7], &[oids[0].clone()]).expect("start") {
            BisectOutcome::Testing { current, .. } => current,
            other => panic!("expected Testing, got {other:?}"),
        };
        let before = read_state(&repo).expect("read");

        // Move HEAD to a DIFFERENT (clean) commit externally via a clean checkout
        // so `ensure_clean` still passes but HEAD != state.current.
        let other = if midpoint == oids[0] { &oids[7] } else { &oids[0] };
        checkout_commit(&repo, git2::Oid::from_str(other).expect("oid")).expect("checkout");

        match bisect_mark(dir.path(), false).expect_err("head moved") {
            AppError::Git(m) => assert!(m.contains("not on the bisect commit"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
        match bisect_skip(dir.path()).expect_err("head moved") {
            AppError::Git(m) => assert!(m.contains("not on the bisect commit"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // The rejected mark/skip left the bisect state byte-identical (no verdict
        // recorded against the wrong commit).
        assert_eq!(read_state(&repo).expect("read"), before);
        assert_eq!(before.current.as_deref(), Some(midpoint.as_str()));
    }

    // ------------------------------------------------------- reset

    #[test]
    fn reset_restores_original_branch() {
        let (dir, oids) = linear_repo_with_bug(6, 3);
        let repo = git2::Repository::open(dir.path()).expect("open");
        let orig_tip = repo.head().expect("head").peel_to_commit().expect("c").id().to_string();

        start_bisect(dir.path(), &oids[5], &[oids[0].clone()]).expect("start");
        // Now HEAD is detached on some midpoint.
        assert!(bisect_in_progress(&repo));

        bisect_reset(dir.path()).expect("reset");
        let re = git2::Repository::open(dir.path()).expect("open");
        assert!(!bisect_in_progress(&re), "state dir removed");
        assert!(!re.head_detached().expect("detached"), "HEAD re-attached");
        assert_eq!(re.head().expect("head").shorthand().ok(), Some("main"));
        assert_eq!(
            re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
            orig_tip
        );
    }

    // ---------------------------------------------- user-mutation guard (Fix 1)

    /// While a Bonsai bisect is active the HEAD is detached and `repo.state()`
    /// is Clean, so the ordinary `state() != Clean` checks can't see it. The
    /// user-facing mutating cores must therefore refuse via `require_no_bisect`.
    #[test]
    fn active_bisect_blocks_user_mutations() {
        use crate::git::commit::create_commit;
        use crate::git::reset::{reset_branch, ResetMode};
        use crate::git::stash::{create_stash, StashScope};

        let (dir, oids) = linear_repo_with_bug(8, 5);
        let d = dir.path();

        match start_bisect(d, &oids[7], &[oids[0].clone()]).expect("start") {
            BisectOutcome::Testing { .. } => {}
            other => panic!("expected Testing, got {other:?}"),
        }
        let repo = git2::Repository::open(d).expect("open");
        assert!(bisect_in_progress(&repo));
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Clean,
            "a Bonsai bisect keeps libgit2 op-state Clean"
        );

        // A worktree edit so create_stash would otherwise have work to do.
        std::fs::write(d.join("f0.txt"), "dirty\n").expect("edit");

        assert!(
            matches!(
                create_commit(d, "blocked", None, false).expect_err("commit blocked"),
                AppError::OperationInProgress(_)
            ),
            "commit must be refused mid-bisect"
        );
        assert!(
            matches!(
                reset_branch(d, &oids[0], ResetMode::Soft).expect_err("reset blocked"),
                AppError::OperationInProgress(_)
            ),
            "reset must be refused mid-bisect"
        );
        assert!(
            matches!(
                create_stash(d, None, StashScope::All).expect_err("stash blocked"),
                AppError::OperationInProgress(_)
            ),
            "stash-create must be refused mid-bisect"
        );

        // The bisect state is intact — the refusals mutated nothing.
        assert!(bisect_in_progress(&repo), "bisect still active after refusals");
    }

    /// Audit 2026-08-07 §3.1: the restore path shares the untracked-clobber
    /// guard. An untracked file (e.g. generated during a bisect test run)
    /// colliding with a tracked path at the ORIGINAL head makes `bisect_reset`
    /// refuse: file content preserved, bisect state intact and retryable.
    #[test]
    fn reset_refuses_untracked_collision_and_stays_retryable() {
        let (dir, oids) = linear_repo_with_bug(6, 3);
        let d = dir.path();
        start_bisect(d, &oids[5], &[oids[0].clone()]).expect("start");
        let repo = git2::Repository::open(d).expect("open");
        assert!(bisect_in_progress(&repo));

        // The midpoint predates c5, so its checkout removed f5.txt. Plant an
        // UNTRACKED f5.txt that the restore's force checkout would clobber.
        assert!(!d.join("f5.txt").exists(), "midpoint must predate c5");
        std::fs::write(d.join("f5.txt"), "precious build artifact\n").expect("plant");

        let err = bisect_reset(d).expect_err("must refuse the clobber");
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("f5.txt")
                && m.contains("would be overwritten")),
            "got {err:?}"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("f5.txt")).expect("read"),
            "precious build artifact\n",
            "untracked file untouched"
        );
        assert!(
            bisect_in_progress(&repo),
            "state intact — the refusal must leave the bisect retryable"
        );

        // Clear the collision → the retry succeeds and restores the branch.
        std::fs::remove_file(d.join("f5.txt")).expect("remove");
        bisect_reset(d).expect("retry succeeds");
        let re = git2::Repository::open(d).expect("open");
        assert!(!bisect_in_progress(&re));
        assert_eq!(
            re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
            oids[5]
        );
    }

    #[test]
    fn reset_restores_detached_start() {
        let (dir, oids) = linear_repo_with_bug(6, 3);
        let repo = git2::Repository::open(dir.path()).expect("open");
        // Detach HEAD at the tip before starting.
        let tip = git2::Oid::from_str(&oids[5]).expect("oid");
        repo.set_head_detached(tip).expect("detach");

        start_bisect(dir.path(), &oids[5], &[oids[0].clone()]).expect("start");
        bisect_reset(dir.path()).expect("reset");

        let re = git2::Repository::open(dir.path()).expect("open");
        assert!(!bisect_in_progress(&re));
        assert!(re.head_detached().expect("detached"), "detached start stays detached");
        assert_eq!(
            re.head().expect("head").peel_to_commit().expect("c").id().to_string(),
            oids[5]
        );
    }
}
