//! The cherry-pick replay engine: the drive loop, per-op commit, finish, and
//! the unified restore/recovery helper. Extracted verbatim from
//! `rebase_interactive.rs` (file-size discipline).

use std::path::Path;

use crate::error::AppError;
use crate::git::conflict::list_conflicts;
use crate::git::rebase::RebaseOutcome;
use crate::git::stage::ensure_no_untracked_collision;

use super::state::{
    concat_messages, effective_total, map_pick_err, normalize_message, remove_state, write_state,
};
use super::{InteractiveState, RebaseAction, RebaseTodoOp};

// ---------------------------------------------------------------- drive loop

/// Drives the todo list from `state.cursor`: cherry-pick each kept op onto the
/// moving detached tip; pause on conflict (persisting the cursor); finish when
/// the list is exhausted.
pub(super) fn drive(
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
pub(super) fn commit_current_op(
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
pub(super) fn finish_interactive(
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

/// The ONE recovery helper (N2): FORCE-reset the original branch ref back to
/// `state.original_tip`, re-attach HEAD to it, hard-restore the worktree+index
/// to that tip, and drop the state file. Resetting the ref is essential — a
/// PARTIALLY-COMPLETED `finish_interactive` may have already moved the branch to
/// the rewritten tip (M2), and at Start the ref is still at `original_tip` so the
/// force-reset is a safe no-op. Shared by abort, Start-failure recovery, and the
/// un-appliable-pick recovery in `drive`.
pub(super) fn restore_to_original(
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
