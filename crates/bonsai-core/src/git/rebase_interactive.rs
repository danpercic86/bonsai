//! Interactive rebase — a Bonsai-owned cherry-pick replay engine on a detached
//! HEAD, with an on-disk JSON sequencer under `.git/bonsai-rebase/` (P23 contract
//! §2). git2's `Rebase` API iterates a fixed linear plan and natively supports
//! neither reorder, squash, fixup, reword, nor drop; this module owns its own
//! todo list + progress so all five actions are expressible.
//!
//! The ORIGINAL branch ref is never moved until `finish_interactive`: Start
//! detaches HEAD at `onto` and replays each todo by cherry-picking onto the
//! moving detached tip. Abort is therefore trivial and safe (re-attach HEAD; the
//! branch still points at its original tip). Conflicts are materialized with
//! `repo.cherrypick` so libgit2 writes real `<<<<<<< ======= >>>>>>>` markers —
//! the SAME representation `conflict.rs` reads. No engine state is held in memory
//! across IPC calls: every command re-reads `state.json` and rewrites it.
//!
//! Continue/Skip/Abort are driven through the EXISTING `rebase::rebase_{continue,
//! skip,abort}` commands via a delegation branch (contract §3); opstate probes
//! this Bonsai file FIRST so a transient `CherryPick` state never wins (§4).
//!
//! Pure git2, no Tauri types, no network.

use std::path::Path;

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::rebase::RebaseOutcome;
use crate::git::repo::read_head_info;
use crate::git::stage::{ensure_no_untracked_collision, open_workdir_repo};

/// Per-op action. Wire: `"pick" | "reword" | "squash" | "fixup" | "drop"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RebaseAction {
    Pick,
    Reword,
    Squash,
    Fixup,
    Drop,
}

/// One todo-list entry. `oid` = the commit being replayed. `new_message` is
/// REQUIRED for Reword, OPTIONAL for Squash (None -> default concat), ignored
/// otherwise. Serialize (for `get_interactive_plan`) + Deserialize (for start
/// input and the state file).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseTodoOp {
    pub oid: String,
    pub action: RebaseAction,
    #[serde(default)]
    pub new_message: Option<String>,
}

/// Persisted interactive-rebase progress. Re-read on every IPC call; deleted on
/// finish/abort. NEVER held across calls.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InteractiveState {
    /// = 1
    pub version: u32,
    /// original branch short name (`refs/heads/<head_name>`)
    pub head_name: String,
    /// 40-hex branch tip before rebase (Abort restore + display)
    pub original_tip: String,
    /// 40-hex base commit the replay starts from
    pub onto: String,
    /// the editable plan, in execution order
    pub todos: Vec<RebaseTodoOp>,
    /// index of the NEXT todo to apply (0-based)
    pub cursor: usize,
    /// count of todos that produced a commit (for `steps`)
    pub committed: u32,
    /// true iff a conflict pause is active at `todos[cursor]`
    pub paused: bool,
    /// Non-fatal notes accumulated across the replay, surfaced on the final
    /// `RebaseOutcome::Rebased`. Currently: a Reword whose pick became empty and
    /// was dropped (its new message silently lost otherwise). `#[serde(default)]`
    /// so older on-disk state files still deserialize.
    #[serde(default)]
    pub warnings: Vec<String>,
}

// On-disk state persistence + message/validation helpers live in the `state`
// submodule; the cherry-pick replay engine lives in `engine`. Both are
// re-exported so `crate::git::rebase_interactive::<item>` paths are unchanged.
mod engine;
mod state;

pub(crate) use state::{effective_total, interactive_in_progress, read_state};
use engine::{commit_current_op, drive, finish_interactive, restore_to_original};
use state::{
    map_pick_err, read_state_raw, remove_state, validate_todos, write_state, StateReadError,
};

// ---------------------------------------------------------------- get plan

/// Blocking. Returns the DEFAULT todo list (every commit `Pick`, OLDEST first)
/// for the first-parent range `base..HEAD`, seeding the plan editor. Does NOT
/// mutate anything.
pub fn get_interactive_plan(
    workdir: &Path,
    base_oid: &str,
) -> Result<Vec<RebaseTodoOp>, AppError> {
    let repo = open_workdir_repo(workdir)?;

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git("no commits yet".to_string()));
    }
    if head.detached {
        return Err(AppError::Git("HEAD is detached".to_string()));
    }

    let base = repo.find_commit(
        git2::Oid::from_str(base_oid).map_err(|_| AppError::Git("invalid commit id".to_string()))?,
    )?;
    let head_commit = repo.head()?.peel_to_commit()?;

    let mut walk: Vec<RebaseTodoOp> = Vec::new();
    let mut c = head_commit;
    loop {
        if c.id() == base.id() {
            break;
        }
        walk.push(RebaseTodoOp {
            oid: c.id().to_string(),
            action: RebaseAction::Pick,
            new_message: None,
        });
        if c.parent_count() == 0 {
            return Err(AppError::Git(format!(
                "{base_oid} is not a first-parent ancestor of HEAD"
            )));
        }
        c = c.parent(0)?;
    }
    walk.reverse(); // execution order: oldest first
    if walk.is_empty() {
        return Err(AppError::Git("nothing to rebase".to_string()));
    }
    Ok(walk)
}

// ---------------------------------------------------------------- start

/// Blocking. Starts an interactive rebase: replays `todos` (in the given order)
/// onto `onto_oid` on a detached HEAD, persisting progress under
/// `.git/bonsai-rebase/`. Clean replay runs to completion; a conflict pauses.
pub fn start_interactive_rebase(
    workdir: &Path,
    onto_oid: &str,
    todos: Vec<RebaseTodoOp>,
) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;

    if interactive_in_progress(&repo) {
        return Err(AppError::OperationInProgress(
            "an interactive rebase is already in progress — continue or abort it first".to_string(),
        ));
    }
    // F-A3-3: a Bonsai bisect also runs on a detached HEAD with
    // `repo.state() == Clean`, so the check below does not see it — guard
    // against it explicitly (mirrors `start_bisect`).
    if crate::git::bisect::bisect_in_progress(&repo) {
        return Err(AppError::OperationInProgress(
            "a bisect is in progress — finish or reset it first".to_string(),
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
            "cannot rebase: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot rebase: HEAD is detached".to_string()));
    }
    let head_name = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot rebase: HEAD has no branch name".to_string()))?;

    // Clean index (matches HEAD) AND clean worktree (no tracked unstaged change).
    let head_commit = repo.head()?.peel_to_commit()?;
    let mut index = repo.index()?;
    if index.has_conflicts() || index.write_tree_to(&repo)? != head_commit.tree_id() {
        return Err(AppError::Git(
            "cannot rebase: your index contains uncommitted changes — commit or unstage them first"
                .to_string(),
        ));
    }
    drop(index);
    let mut sopts = git2::StatusOptions::new();
    sopts.include_untracked(false).include_ignored(false);
    if !repo.statuses(Some(&mut sopts))?.is_empty() {
        return Err(AppError::Git(
            "cannot rebase: you have unstaged changes — commit or stash them first".to_string(),
        ));
    }

    // Identity EARLY — replay creates commits, so ConfigMissing must precede any
    // worktree mutation.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    let onto = repo.find_commit(
        git2::Oid::from_str(onto_oid).map_err(|_| AppError::Git("invalid commit id".to_string()))?,
    )?;

    validate_todos(&repo, &todos)?;

    // Data-loss guard: refuse before writing any state if the force checkout onto
    // `onto` would clobber an untracked, non-ignored worktree file. Placed BEFORE
    // write_state so a refusal leaves no `.git/bonsai-rebase/` behind.
    ensure_no_untracked_collision(&repo, &onto.tree()?)?;

    let original_tip = head_commit.id();
    drop(head_commit);

    let mut state = InteractiveState {
        version: 1,
        head_name,
        original_tip: original_tip.to_string(),
        onto: onto.id().to_string(),
        todos,
        cursor: 0,
        committed: 0,
        paused: false,
        warnings: Vec::new(),
    };
    write_state(&repo, &state)?;

    // Detach + bring the worktree/index to onto's tree. Worktree verified clean
    // -> force is safe; failure is atomic (before any commit is rewritten).
    // Recovery = the unified restore (best-effort: the branch ref is untouched
    // at Start, so this simply re-attaches HEAD and drops the state file).
    if let Err(e) = repo.set_head_detached(onto.id()) {
        let _ = restore_to_original(&repo, &state);
        return Err(e.into());
    }
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    if let Err(e) = repo.checkout_tree(onto.as_object(), Some(&mut co)) {
        let _ = restore_to_original(&repo, &state);
        return Err(map_pick_err(e));
    }

    drive(workdir, &repo, &mut state, &sig)
}

// ---------------------------------------------------------------- continue/skip/abort

/// Blocking. Resumes a paused interactive rebase: commits the resolved current
/// op from the index, then replays on. Reused via `rebase::rebase_continue`.
pub fn interactive_continue(workdir: &Path) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !interactive_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }
    let mut state = read_state(&repo)?;

    // M1: an out-of-range cursor (a partial finish that left cursor == len, or a
    // hand-edited state file) has no paused op to commit — finish gracefully
    // instead of indexing `state.todos[cursor]` out of bounds (which would panic).
    if state.cursor >= state.todos.len() {
        return finish_interactive(&repo, &state);
    }

    if repo.index()?.has_conflicts() {
        let n = repo.index()?.conflicts()?.count();
        return Err(AppError::UnresolvedConflicts(format!(
            "cannot continue: {n} unresolved conflict(s) remain"
        )));
    }

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;
    let op = state.todos[state.cursor].clone(); // the paused op
    let pick_oid = git2::Oid::from_str(&op.oid)
        .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
    let pick = repo.find_commit(pick_oid)?;

    // HARD error -> Err, leaving the on-disk state intact (§2.7 / P3d §3.9).
    commit_current_op(&repo, &mut state, &op, &pick, &sig)?;
    drive(workdir, &repo, &mut state, &sig)
}

/// Blocking. Drops the current (conflicted or not) op and replays on. Reused via
/// `rebase::rebase_skip`.
pub fn interactive_skip(workdir: &Path) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !interactive_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }
    let mut state = read_state(&repo)?;
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Discard the current op's changes WITHOUT touching the detached tip: read
    // HEAD's tree into the index (clears conflict stages) and force-checkout so
    // the worktree drops markers.
    let head_tree = repo.head()?.peel_to_commit()?.tree()?;
    let mut idx = repo.index()?;
    idx.read_tree(&head_tree)?;
    idx.write()?;
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_index(Some(&mut idx), Some(&mut co))?;
    drop(idx);
    drop(head_tree);
    repo.cleanup_state()?; // clear CHERRY_PICK_HEAD

    state.cursor += 1;
    state.paused = false;
    write_state(&repo, &state)?;
    drive(workdir, &repo, &mut state, &sig)
}

/// Blocking. Aborts: re-attach HEAD to the original branch (its ref never
/// moved), restore the worktree to the original tip, remove
/// `.git/bonsai-rebase/`. Reused via `rebase::rebase_abort`. A CORRUPT state
/// file is salvaged (F-A3-2): the state dir is cleared so the app is no longer
/// wedged, HEAD is left in place, and a distinct Git error explains what
/// happened.
pub fn interactive_abort(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !interactive_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }
    let state = match read_state_raw(&repo) {
        Ok(s) => s,
        // F-A3-2 salvage: the state file EXISTS but cannot be decoded — abort
        // would otherwise fail forever while opstate/UI offer no escape.
        Err(StateReadError::Corrupt(e)) => {
            remove_state(&repo);
            return Err(AppError::Git(format!(
                "the interactive rebase state file was corrupt ({e}); it has been cleared and \
                 the rebase abandoned. HEAD was left where it is — check out your original \
                 branch manually if needed"
            )));
        }
        Err(StateReadError::Missing) => {
            return Err(AppError::NoOperationInProgress(
                "no rebase in progress".to_string(),
            ))
        }
        Err(StateReadError::Io(e)) => {
            return Err(AppError::Git(format!(
                "failed to read interactive rebase state: {e}"
            )))
        }
    };
    restore_to_original(&repo, &state)
}


#[cfg(test)]
mod tests;
