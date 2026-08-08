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
use crate::git::conflict::list_conflicts;
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

// ---------------------------------------------------------------- on-disk state

fn bonsai_dir(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join("bonsai-rebase")
}

fn state_path(repo: &git2::Repository) -> std::path::PathBuf {
    bonsai_dir(repo).join("state.json")
}

/// True iff a Bonsai interactive rebase is in progress (state file present).
pub(crate) fn interactive_in_progress(repo: &git2::Repository) -> bool {
    state_path(repo).exists()
}

/// Reads + parses `.git/bonsai-rebase/state.json`. Missing/corrupt -> `Git`.
pub(crate) fn read_state(repo: &git2::Repository) -> Result<InteractiveState, AppError> {
    let raw = std::fs::read_to_string(state_path(repo))
        .map_err(|_| AppError::Git("interactive rebase state is missing".to_string()))?;
    serde_json::from_str(&raw)
        .map_err(|e| AppError::Git(format!("interactive rebase state is corrupt: {e}")))
}

/// Writes the state file (create_dir_all + temp-file rename for atomicity).
fn write_state(repo: &git2::Repository, state: &InteractiveState) -> Result<(), AppError> {
    let dir = bonsai_dir(repo);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| AppError::Git(format!("failed to serialize rebase state: {e}")))?;
    let tmp = dir.join("state.json.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, dir.join("state.json"))?;
    Ok(())
}

/// Removes `.git/bonsai-rebase/` (best-effort — a leftover dir is harmless and
/// `interactive_in_progress` keys on `state.json` specifically).
fn remove_state(repo: &git2::Repository) {
    let _ = std::fs::remove_dir_all(bonsai_dir(repo));
}

/// Number of non-`Drop` todos == the "total steps" the UI shows.
pub(crate) fn effective_total(state: &InteractiveState) -> u32 {
    state
        .todos
        .iter()
        .filter(|t| t.action != RebaseAction::Drop)
        .count() as u32
}

// ---------------------------------------------------------------- message helpers

/// CRLF/CR -> `\n`, trim, single trailing newline (shared with cherrypick.rs /
/// commit.rs). Empty after trim -> empty string.
fn normalize_message(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    format!("{trimmed}\n")
}

/// Default squash message when `new_message` is None: `<head>\n\n<pick>`.
fn concat_messages(head_msg: &str, pick_msg: &str) -> String {
    format!("{}\n\n{}", head_msg.trim(), pick_msg.trim())
}

/// A git2 error raised while APPLYING a pick: `Conflict` -> friendly
/// CheckoutConflict; else the generic `From` (`AppError::Git`).
fn map_pick_err(e: git2::Error) -> AppError {
    if e.code() == git2::ErrorCode::Conflict {
        AppError::CheckoutConflict(
            "cannot rebase: local changes would be overwritten. Commit or discard them first."
                .to_string(),
        )
    } else {
        e.into()
    }
}

// ---------------------------------------------------------------- validation

/// Rejects a plan BEFORE any mutation (contract §2.6). Structural checks first
/// (so a bad shape never depends on oid resolution), then per-oid resolution.
fn validate_todos(repo: &git2::Repository, todos: &[RebaseTodoOp]) -> Result<(), AppError> {
    let first_kept = todos.iter().find(|t| t.action != RebaseAction::Drop);
    match first_kept {
        None => {
            return Err(AppError::Git(
                "nothing to rebase: the plan drops every commit".to_string(),
            ));
        }
        Some(op) => {
            if !matches!(op.action, RebaseAction::Pick | RebaseAction::Reword) {
                return Err(AppError::Git("a squash/fixup must follow a pick".to_string()));
            }
        }
    }

    for op in todos {
        if op.action == RebaseAction::Reword
            && op
                .new_message
                .as_ref()
                .map(|m| m.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(AppError::Git("reword requires a message".to_string()));
        }
    }

    for op in todos {
        if op.action == RebaseAction::Drop {
            continue;
        }
        let oid = git2::Oid::from_str(&op.oid)
            .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
        repo.find_commit(oid)?;
    }
    Ok(())
}

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

// ---------------------------------------------------------------- drive loop

/// Drives the todo list from `state.cursor`: cherry-pick each kept op onto the
/// moving detached tip; pause on conflict (persisting the cursor); finish when
/// the list is exhausted.
fn drive(
    workdir: &Path,
    repo: &git2::Repository,
    state: &mut InteractiveState,
    sig: &git2::Signature,
) -> Result<RebaseOutcome, AppError> {
    loop {
        if state.cursor >= state.todos.len() {
            return finish_interactive(repo, state);
        }
        let op = state.todos[state.cursor].clone();
        if op.action == RebaseAction::Drop {
            state.cursor += 1;
            write_state(repo, state)?;
            continue;
        }

        // S1 safety: a squash/fixup that has become the FIRST applied op (its
        // predecessor was dropped or skipped -> no commit applied yet in this
        // run, so the detached tip is still `onto`) would otherwise reparent onto
        // the base's PARENT and silently rewrite the base. Refuse instead of
        // corrupting — checked BEFORE the cherry-pick so nothing is materialized.
        if matches!(op.action, RebaseAction::Squash | RebaseAction::Fixup)
            && state.committed == 0
        {
            return Err(AppError::Git(
                "cannot squash/fixup: no preceding commit to combine into \
                 (its predecessor was dropped or skipped)"
                    .to_string(),
            ));
        }

        let pick_oid = git2::Oid::from_str(&op.oid)
            .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
        let pick = repo.find_commit(pick_oid)?;

        // Materialize the pick onto HEAD (the current detached tip) into the
        // worktree+index (real conflict markers when it conflicts).
        if let Err(e) = repo.cherrypick(&pick, None) {
            // Could not even apply -> abort the whole rebase and surface the error.
            let _ = restore_to_original(repo, state);
            return Err(map_pick_err(e));
        }

        if repo.index()?.has_conflicts() {
            state.paused = true;
            write_state(repo, state)?; // cursor stays at this op
            let paths: Vec<String> = list_conflicts(workdir)?
                .into_iter()
                .map(|c| c.path)
                .collect();
            return Ok(RebaseOutcome::Conflicts {
                paths,
                current_step: state.committed + 1,
                total_steps: effective_total(state),
            });
        }

        commit_current_op(repo, state, &op, &pick, sig)?;
    }
}

/// Commits the CURRENT op from the RESOLVED repo index (clean path AND Continue
/// after resolution), advancing the cursor + committed counter. Parent, author
/// and message depend on the action (contract §2.5). An empty result (result
/// tree == the current tip's tree) is dropped like default `git rebase` (a
/// no-op pick is dropped; a no-op squash/fixup keeps the predecessor as-is).
fn commit_current_op(
    repo: &git2::Repository,
    state: &mut InteractiveState,
    op: &RebaseTodoOp,
    pick: &git2::Commit,
    committer: &git2::Signature,
) -> Result<(), AppError> {
    // S1 safety net (also enforced in `drive`, but this covers a resumed
    // `interactive_continue` reached with a hand-edited cursor): a squash/fixup
    // with no applied predecessor would reparent onto the base's parent.
    if matches!(op.action, RebaseAction::Squash | RebaseAction::Fixup) && state.committed == 0 {
        return Err(AppError::Git(
            "cannot squash/fixup: no preceding commit to combine into \
             (its predecessor was dropped or skipped)"
                .to_string(),
        ));
    }

    let tree = repo.find_tree(repo.index()?.write_tree()?)?;
    let head = repo.head()?.peel_to_commit()?; // current detached tip
    let head_tree_id = head.tree_id();

    let (parent, author, message) = match op.action {
        RebaseAction::Pick => {
            let msg = pick.message().unwrap_or("").to_string();
            (head, pick.author().to_owned(), msg)
        }
        RebaseAction::Reword => {
            let msg = op
                .new_message
                .clone()
                .ok_or_else(|| AppError::Git("reword requires a message".to_string()))?;
            (head, pick.author().to_owned(), msg)
        }
        RebaseAction::Squash => {
            let author = head.author().to_owned();
            let msg = op.new_message.clone().unwrap_or_else(|| {
                concat_messages(head.message().unwrap_or(""), pick.message().unwrap_or(""))
            });
            let parent = head.parent(0)?; // replace head, keep ITS parent
            (parent, author, msg)
        }
        RebaseAction::Fixup => {
            let author = head.author().to_owned();
            let msg = head.message().unwrap_or("").to_string(); // discard op's message
            let parent = head.parent(0)?;
            (parent, author, msg)
        }
        RebaseAction::Drop => unreachable!("drop handled in drive"),
    };

    // Empty-result guard (§0 #6 + N1). Compare against the CURRENT TIP's tree
    // (`head_tree_id`), NOT `parent.tree_id()`: for squash/fixup the parent is
    // `head.parent(0)`, so a no-op fixup (adds nothing) must be detected as
    // `tree == head.tree`, keeping the predecessor. For pick/reword the parent IS
    // head, so this is equivalent to the original condition.
    if tree.id() == head_tree_id {
        // Dropping an empty PICK matches default `git rebase` and is silent. A
        // REWORD, though, is a message-only intent: dropping it discards the
        // user's new message, so record a warning to surface on the final
        // Rebased outcome (the frontend toasts it) rather than losing it quietly.
        if op.action == RebaseAction::Reword {
            let short: String = op.oid.chars().take(7).collect();
            state.warnings.push(format!(
                "reword of {short} was dropped: the commit became empty on the new base, so its new message was not applied"
            ));
        }
        repo.cleanup_state()?;
        state.cursor += 1;
        state.paused = false;
        write_state(repo, state)?;
        return Ok(());
    }

    let normalized = normalize_message(&message);
    let new = repo.commit(None, &author, committer, &normalized, &tree, &[&parent])?;
    repo.set_head_detached(new)?; // advance the detached tip
    repo.cleanup_state()?; // remove CHERRY_PICK_HEAD -> worktree/index intact
    state.committed += 1;
    state.cursor += 1;
    state.paused = false;
    write_state(repo, state)?;
    Ok(())
}

/// Moves the ORIGINAL branch ref to the final detached tip, re-attaches HEAD,
/// drops the state file.
///
/// M2 partial-failure safety: `remove_state` runs LAST, so if `set_head` or
/// `checkout_head` fails AFTER the ref move (e.g. a Windows file lock), the
/// state file is left intact and `interactive_in_progress` stays true — the user
/// can `rebase_abort`, whose `restore_to_original` FORCE-resets the branch ref
/// back to `original_tip`, fully recovering the pre-rebase branch tip regardless
/// of how far this finish progressed. The worktree already matches `final_tip`
/// (HEAD is detached there), so the checkout is a consistency refresh.
fn finish_interactive(
    repo: &git2::Repository,
    state: &InteractiveState,
) -> Result<RebaseOutcome, AppError> {
    let final_tip = repo.head()?.peel_to_commit()?.id();
    // Data-loss guard (mirrors Start, rebase_interactive.rs `start`): refuse
    // BEFORE any ref mutation if the force checkout would clobber an untracked
    // file present in the final tree. The state file survives a refusal (it is
    // removed last), so the rebase stays finishable/abortable after the user
    // clears the file.
    ensure_no_untracked_collision(repo, &repo.find_commit(final_tip)?.tree()?)?;
    // Clear any residual sequencer state (e.g. a lingering CHERRY_PICK_HEAD when
    // finish is reached via the M1 out-of-range-cursor path, which skips
    // `commit_current_op`'s own cleanup).
    let _ = repo.cleanup_state();
    let branch_ref = format!("refs/heads/{}", state.head_name);
    repo.reference(&branch_ref, final_tip, true, "rebase -i (finish)")?;
    repo.set_head(&branch_ref)?;
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_head(Some(&mut co))?;
    remove_state(repo);
    Ok(RebaseOutcome::Rebased {
        branch: state.head_name.clone(),
        head: final_tip.to_string(),
        steps: state.committed,
        warnings: state.warnings.clone(),
    })
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
/// `.git/bonsai-rebase/`. Reused via `rebase::rebase_abort`.
pub fn interactive_abort(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    if !interactive_in_progress(&repo) {
        return Err(AppError::NoOperationInProgress(
            "no rebase in progress".to_string(),
        ));
    }
    let state = read_state(&repo)?;
    restore_to_original(&repo, &state)
}

/// The ONE recovery helper (N2): FORCE-reset the original branch ref back to
/// `state.original_tip`, re-attach HEAD to it, hard-restore the worktree+index
/// to that tip, and drop the state file. Resetting the ref is essential — a
/// PARTIALLY-COMPLETED `finish_interactive` may have already moved the branch to
/// the rewritten tip (M2), and at Start the ref is still at `original_tip` so the
/// force-reset is a safe no-op. Shared by abort, Start-failure recovery, and the
/// un-appliable-pick recovery in `drive`.
fn restore_to_original(
    repo: &git2::Repository,
    state: &InteractiveState,
) -> Result<(), AppError> {
    let orig_oid = git2::Oid::from_str(&state.original_tip)
        .map_err(|_| AppError::Git("corrupt state: bad original tip".to_string()))?;
    let orig = repo.find_commit(orig_oid)?;
    // Data-loss guard (mirrors Start): refuse BEFORE any ref mutation if the
    // force checkout back to the original tip would clobber an untracked file
    // (e.g. one created mid-rebase). The state file is removed last, so a
    // refusal keeps the rebase abortable — clear the file and retry.
    ensure_no_untracked_collision(repo, &orig.tree()?)?;
    let _ = repo.cleanup_state();
    let branch_ref = format!("refs/heads/{}", state.head_name);
    // Move the branch ref back FIRST (a partial finish may have advanced it).
    repo.reference(&branch_ref, orig_oid, true, "rebase -i (abort)")?;
    repo.set_head(&branch_ref)?;
    let mut co = git2::build::CheckoutBuilder::new();
    co.force();
    repo.checkout_tree(orig.as_object(), Some(&mut co))?;
    let mut idx = repo.index()?;
    idx.read_tree(&orig.tree()?)?;
    idx.write()?;
    remove_state(repo);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::scratch_dir;

    fn sig() -> git2::Signature<'static> {
        git2::Signature::now("Test", "test@example.com").expect("sig")
    }

    /// Linear repo of `n` commits on the default branch (each adds `f{i}.txt`).
    /// Returns (dir, oids oldest-first). Sets a repo-local identity.
    fn linear_repo(n: usize) -> (tempfile::TempDir, Vec<String>) {
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
            let mut idx = repo.index().expect("index");
            idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("add");
            idx.write().expect("write index");
            let tree = repo.find_tree(idx.write_tree().expect("tree")).expect("find tree");
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
            let oid = repo
                .commit(Some("HEAD"), &s, &s, &format!("c{i}"), &tree, &parent_refs)
                .expect("commit");
            oids.push(oid.to_string());
            parents = vec![repo.find_commit(oid).expect("find commit")];
        }
        (dir, oids)
    }

    // ------------------------------------------------ wire shapes (TS mirrors)

    #[test]
    fn rebase_action_round_trips() {
        for (json, action) in [
            ("\"pick\"", RebaseAction::Pick),
            ("\"reword\"", RebaseAction::Reword),
            ("\"squash\"", RebaseAction::Squash),
            ("\"fixup\"", RebaseAction::Fixup),
            ("\"drop\"", RebaseAction::Drop),
        ] {
            let a: RebaseAction = serde_json::from_str(json).expect("de");
            assert_eq!(a, action);
            assert_eq!(serde_json::to_string(&action).expect("ser"), json);
        }
    }

    #[test]
    fn todo_op_wire_shape_is_camel_case() {
        let op = RebaseTodoOp {
            oid: "a".repeat(40),
            action: RebaseAction::Reword,
            new_message: Some("hi".to_string()),
        };
        let v = serde_json::to_value(&op).expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "oid": "a".repeat(40), "action": "reword", "newMessage": "hi" })
        );
        // newMessage defaults to None when absent.
        let s = format!("{{\"oid\":\"{}\",\"action\":\"pick\"}}", "b".repeat(40));
        let op2: RebaseTodoOp = serde_json::from_str(&s).expect("de");
        assert_eq!(op2.action, RebaseAction::Pick);
        assert_eq!(op2.new_message, None);
    }

    // ------------------------------------------------------- get plan

    #[test]
    fn plan_is_oldest_first_all_pick() {
        let (dir, oids) = linear_repo(3);
        let plan = get_interactive_plan(dir.path(), &oids[0]).expect("plan");
        assert_eq!(plan.len(), 2, "base..HEAD excludes the base commit");
        assert_eq!(plan[0].oid, oids[1], "oldest kept commit first");
        assert_eq!(plan[1].oid, oids[2]);
        assert!(plan
            .iter()
            .all(|t| t.action == RebaseAction::Pick && t.new_message.is_none()));
    }

    #[test]
    fn plan_rejects_non_ancestor_base() {
        let (dir, _oids) = linear_repo(2);
        let err = get_interactive_plan(dir.path(), &"0".repeat(40)).expect_err("bad base");
        assert!(matches!(err, AppError::Git(_)));
    }

    // ------------------------------------------------------- validate_todos

    #[test]
    fn validate_rejects_bad_plans() {
        let dir = scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        let o = "a".repeat(40);

        // empty
        assert!(matches!(validate_todos(&repo, &[]), Err(AppError::Git(_))));

        // all drop
        let all_drop = vec![RebaseTodoOp {
            oid: o.clone(),
            action: RebaseAction::Drop,
            new_message: None,
        }];
        assert!(matches!(
            validate_todos(&repo, &all_drop),
            Err(AppError::Git(_))
        ));

        // squash first (no predecessor)
        let squash_first = vec![RebaseTodoOp {
            oid: o.clone(),
            action: RebaseAction::Squash,
            new_message: None,
        }];
        match validate_todos(&repo, &squash_first) {
            Err(AppError::Git(m)) => assert!(m.contains("must follow a pick"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // reword without a message
        let reword_no_msg = vec![RebaseTodoOp {
            oid: o.clone(),
            action: RebaseAction::Reword,
            new_message: None,
        }];
        match validate_todos(&repo, &reword_no_msg) {
            Err(AppError::Git(m)) => assert!(m.contains("reword requires a message"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    // ------------------------------------------------------- preconditions

    #[test]
    fn start_and_ops_on_fresh_repo() {
        let dir = scratch_dir();
        git2::Repository::init(dir.path()).expect("init");

        // Unborn HEAD refuses.
        let err = start_interactive_rebase(dir.path(), &"0".repeat(40), vec![]).expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // continue / skip / abort with no interactive rebase.
        assert!(matches!(
            interactive_continue(dir.path()).expect_err("no op"),
            AppError::NoOperationInProgress(_)
        ));
        assert!(matches!(
            interactive_skip(dir.path()).expect_err("no op"),
            AppError::NoOperationInProgress(_)
        ));
        assert!(matches!(
            interactive_abort(dir.path()).expect_err("no op"),
            AppError::NoOperationInProgress(_)
        ));
    }

    #[test]
    fn state_round_trips_on_disk() {
        let (dir, oids) = linear_repo(2);
        let repo = git2::Repository::open(dir.path()).expect("open");
        let state = InteractiveState {
            version: 1,
            head_name: "main".to_string(),
            original_tip: oids[1].clone(),
            onto: oids[0].clone(),
            todos: vec![RebaseTodoOp {
                oid: oids[1].clone(),
                action: RebaseAction::Pick,
                new_message: None,
            }],
            cursor: 0,
            committed: 0,
            paused: false,
            warnings: Vec::new(),
        };
        assert!(!interactive_in_progress(&repo));
        write_state(&repo, &state).expect("write");
        assert!(interactive_in_progress(&repo));
        assert_eq!(read_state(&repo).expect("read"), state);
        assert_eq!(effective_total(&state), 1);
        remove_state(&repo);
        assert!(!interactive_in_progress(&repo));
    }
}
