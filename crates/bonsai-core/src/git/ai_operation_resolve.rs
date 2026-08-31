//! Allowlist RESOLUTION for the NL-operation planner (P55 safety layers L3/L4).
//!
//! Split out of `ai_operation.rs` (file-size discipline, P55b). Rust — never
//! the model — maps a parsed [`AiOpIntent`] to a fully-resolved [`SafeOp`] +
//! read-only preview, or a calm [`PlanOutcome::Unsupported`]. Every branch
//! name / oid is resolved HERE via `revparse` / branch-lookup / status; the
//! model only *references* items shown in the grounding and never yields an
//! oid. Any precondition or lookup miss (bad ref, HEAD-not-a-merge, op in
//! progress, invalid branch name, path with no changes, …) degrades to
//! `Ok(Unsupported{reason})`; only an unexpected git2 fault is an `Err`.
//!
//! This code path **mutates nothing** — it does status/revparse/branch-lookup
//! only (proven for ALL ten intents by `plan_never_mutates` in `ai_operation`).

use crate::error::AppError;
use crate::git::ai_operation::{
    current_branch_name, head_commit, revparse_commit, sanitize_model_text, short7, unsupported,
    AiOpIntent, DangerLevel, PlanOutcome, ProposedOperation, SafeOp,
};
use crate::git::ai_operation_preview::build_preview;
use crate::git::branches::validate_branch_name;
use crate::git::reset::ResetMode;
use crate::git::status::read_status;

/// Maps a parsed intent to a resolved op + preview, or `Unsupported`.
/// Precondition / lookup misses ⇒ `Ok(Unsupported{reason})` (§2 L4); only
/// unexpected git2 faults ⇒ `Err`. `cost_usd` from the CLI envelope is threaded
/// onto the outcome. Every MUTATING intent first rejects an in-progress
/// merge/rebase/cherry-pick/revert (§4 global precondition).
pub(crate) fn resolve_intent(
    repo: &git2::Repository,
    intent: AiOpIntent,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    match intent {
        // F-A2-1: the reason is FREE MODEL TEXT headed straight for the dialog —
        // sanitize (strip controls/bidi, cap length) before it reaches the UI.
        AiOpIntent::Unsupported { reason } => {
            Ok(unsupported(sanitize_model_text(&reason), cost_usd))
        }
        AiOpIntent::UndoLastCommit { keep_changes } => {
            resolve_undo_last_commit(repo, keep_changes, cost_usd)
        }
        AiOpIntent::UndoLastMerge => resolve_undo_last_merge(repo, cost_usd),
        AiOpIntent::ResetToCommit {
            commit,
            keep_changes,
        } => resolve_reset_to_commit(repo, &commit, keep_changes, cost_usd),
        AiOpIntent::RevertCommit { commit } => resolve_revert_commit(repo, &commit, cost_usd),
        AiOpIntent::SwitchBranch { branch } => resolve_switch_branch(repo, &branch, cost_usd),
        AiOpIntent::CreateBranch { name, at_commit } => {
            resolve_create_branch(repo, &name, at_commit.as_deref(), cost_usd)
        }
        AiOpIntent::DeleteBranch { branch } => resolve_delete_branch(repo, &branch, cost_usd),
        AiOpIntent::StashChanges {
            message,
            include_untracked,
        } => resolve_stash_changes(repo, message, include_untracked, cost_usd),
        AiOpIntent::DiscardChanges { paths } => resolve_discard_changes(repo, paths, cost_usd),
        AiOpIntent::MergeBranch { branch } => resolve_merge_branch(repo, &branch, cost_usd),
    }
}

// ------------------------------------------------------- reset / revert family

/// undoLastCommit (§4): HEAD must have ≥1 parent. `keepChanges` ⇒ Mixed
/// (Caution), else Hard (Destructive). Target = HEAD's first parent; dropped =
/// [HEAD].
fn resolve_undo_last_commit(
    repo: &git2::Repository,
    keep_changes: bool,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let head = match head_commit(repo)? {
        Some(c) => c,
        None => return Ok(unsupported("there are no commits to undo yet.".to_string(), cost_usd)),
    };
    if head.parent_count() < 1 {
        return Ok(unsupported(
            "there's no commit to undo — HEAD has no parent.".to_string(),
            cost_usd,
        ));
    }
    let parent = head.parent(0)?;
    let mode = if keep_changes {
        ResetMode::Mixed
    } else {
        ResetMode::Hard
    };
    let op = reset_op(&parent, mode);
    let branch = current_branch_name(repo);
    let mut preview = build_preview(repo, &op)?;
    preview.title = "Undo last commit".to_string();
    preview.confirm_label = if keep_changes {
        "Undo commit".to_string()
    } else {
        "Undo & discard".to_string()
    };
    preview.summary = format!(
        "Move `{branch}` back to {} — {} the changes from {}.",
        short7(parent.id()),
        if keep_changes { "keep" } else { "discard" },
        short7(head.id())
    );
    let rationale = format!(
        "Interpreted your request as undoing the most recent commit ({}).",
        short7(head.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// undoLastMerge (§4, OQ2 headline): HEAD must be a merge (≥2 parents). Target =
/// HEAD's FIRST parent, Mixed — but flagged Destructive regardless (it rewrites
/// history), with a shared-history warning when an upstream exists.
fn resolve_undo_last_merge(
    repo: &git2::Repository,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let head = match head_commit(repo)? {
        Some(c) => c,
        None => return Ok(unsupported("there are no commits yet.".to_string(), cost_usd)),
    };
    if head.parent_count() < 2 {
        return Ok(unsupported(
            "your last commit isn't a merge, so there's no merge to undo.".to_string(),
            cost_usd,
        ));
    }
    let parent = head.parent(0)?;
    let op = reset_op(&parent, ResetMode::Mixed);
    let branch = current_branch_name(repo);
    let mut preview = build_preview(repo, &op)?;
    preview.title = "Undo last merge".to_string();
    preview.confirm_label = "Undo merge".to_string();
    // OQ2: always Destructive (rewrites history), even though the reset is Mixed.
    preview.danger = DangerLevel::Destructive;
    preview.summary = format!(
        "Move `{branch}` back to {} (before the merge), keeping your working changes.",
        short7(parent.id())
    );
    if let Some(upstream) = current_upstream_name(repo) {
        preview.worktree_warning = Some(format!(
            "This rewrites history that may be shared with `{upstream}`."
        ));
    }
    let rationale = format!(
        "Interpreted your request as undoing the last merge by resetting to its first parent ({}).",
        short7(parent.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// resetToCommit (§4): `revparse_single(commit)` (fail ⇒ Unsupported, L4).
/// `keepChanges` ⇒ Mixed, else Hard.
fn resolve_reset_to_commit(
    repo: &git2::Repository,
    commit: &str,
    keep_changes: bool,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let target = match revparse_commit(repo, commit) {
        Some(c) => c,
        None => {
            return Ok(unsupported(
                format!(
                    "I couldn't find a commit matching '{}'.",
                    sanitize_model_text(commit)
                ),
                cost_usd,
            ))
        }
    };
    let mode = if keep_changes {
        ResetMode::Mixed
    } else {
        ResetMode::Hard
    };
    let op = reset_op(&target, mode);
    let branch = current_branch_name(repo);
    let mut preview = build_preview(repo, &op)?;
    preview.title = "Reset to commit".to_string();
    preview.confirm_label = if keep_changes {
        "Reset (keep changes)".to_string()
    } else {
        "Reset (discard changes)".to_string()
    };
    preview.summary = format!(
        "Move `{branch}` to {} — {} changes made after it.",
        short7(target.id()),
        if keep_changes { "keep" } else { "discard" }
    );
    let rationale = format!(
        "Interpreted your request as moving the current branch to {}.",
        short7(target.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// revertCommit (§4): `revparse` the oid (fail ⇒ Unsupported). Adds ONE commit.
fn resolve_revert_commit(
    repo: &git2::Repository,
    commit: &str,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let target = match revparse_commit(repo, commit) {
        Some(c) => c,
        None => {
            return Ok(unsupported(
                format!(
                    "I couldn't find a commit matching '{}'.",
                    sanitize_model_text(commit)
                ),
                cost_usd,
            ))
        }
    };
    let op = SafeOp::Revert {
        oid: target.id().to_string(),
        short: short7(target.id()),
    };
    let preview = build_preview(repo, &op)?;
    let rationale = format!(
        "Interpreted your request as reverting {} with a new commit.",
        short7(target.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

// ------------------------------------------------------------- P55b resolvers

/// switchBranch (§4): a LOCAL branch ⇒ `SwitchBranch{remote:false}`; else an
/// exact remote-tracking match ("origin/x") ⇒ `SwitchBranch{remote:true}` (OQ5);
/// no match ⇒ Unsupported.
fn resolve_switch_branch(
    repo: &git2::Repository,
    branch: &str,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let shown = sanitize_model_text(branch); // F-A2-1: model echo, display only
    let remote = if repo.find_branch(branch, git2::BranchType::Local).is_ok() {
        false
    } else if repo.find_branch(branch, git2::BranchType::Remote).is_ok() {
        true
    } else {
        return Ok(unsupported(
            format!("I couldn't find a branch named '{shown}' to switch to."),
            cost_usd,
        ));
    };
    let op = SafeOp::SwitchBranch {
        name: branch.to_string(),
        remote,
    };
    let preview = build_preview(repo, &op)?;
    let rationale = if remote {
        format!("Interpreted your request as checking out the remote branch `{shown}`.")
    } else {
        format!("Interpreted your request as switching to the local branch `{shown}`.")
    };
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// createBranch (§4): validate `name` with the SAME validator `create_branch`
/// uses (a miss ⇒ Unsupported); reject an existing name; resolve `atCommit`
/// (bad ⇒ Unsupported), else create at HEAD (`at_oid = None`).
fn resolve_create_branch(
    repo: &git2::Repository,
    name: &str,
    at_commit: Option<&str>,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let shown = sanitize_model_text(name); // F-A2-1: model echo, display only
    if validate_branch_name(name).is_err() {
        return Ok(unsupported(
            format!("'{shown}' isn't a valid branch name."),
            cost_usd,
        ));
    }
    if repo.find_branch(name, git2::BranchType::Local).is_ok() {
        return Ok(unsupported(
            format!("a branch named '{shown}' already exists."),
            cost_usd,
        ));
    }
    let at_oid = match at_commit {
        Some(spec) => match revparse_commit(repo, spec) {
            Some(c) => Some(c.id().to_string()),
            None => {
                return Ok(unsupported(
                    format!(
                        "I couldn't find a commit matching '{}'.",
                        sanitize_model_text(spec)
                    ),
                    cost_usd,
                ))
            }
        },
        None => None,
    };
    let op = SafeOp::CreateBranch {
        name: name.to_string(),
        at_oid,
    };
    let preview = build_preview(repo, &op)?;
    let rationale = format!("Interpreted your request as creating a new branch named `{shown}`.");
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// deleteBranch (§4): must be a LOCAL, NON-CURRENT branch (the command itself
/// blocks unmerged/no-force). Not local / is current ⇒ Unsupported.
fn resolve_delete_branch(
    repo: &git2::Repository,
    branch: &str,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let shown = sanitize_model_text(branch); // F-A2-1: model echo, display only
    let local = match repo.find_branch(branch, git2::BranchType::Local) {
        Ok(b) => b,
        Err(_) => {
            return Ok(unsupported(
                format!("there's no local branch named '{shown}' to delete."),
                cost_usd,
            ))
        }
    };
    if local.is_head() {
        return Ok(unsupported(
            format!("`{shown}` is the current branch — switch away before deleting it."),
            cost_usd,
        ));
    }
    let op = SafeOp::DeleteBranch {
        name: branch.to_string(),
    };
    let preview = build_preview(repo, &op)?;
    let rationale = format!("Interpreted your request as deleting the local branch `{shown}`.");
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// stashChanges (§4): the worktree must be dirty (tracked changes, plus
/// untracked when `includeUntracked`), else "you have no changes to stash".
fn resolve_stash_changes(
    repo: &git2::Repository,
    message: Option<String>,
    include_untracked: bool,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return Ok(unsupported("this repository has no working tree.".to_string(), cost_usd)),
    };
    let status = read_status(workdir)?;
    let has_changes = !status.staged.is_empty()
        || !status.unstaged.is_empty()
        || !status.conflicted.is_empty()
        || (include_untracked && !status.untracked.is_empty());
    if !has_changes {
        return Ok(unsupported("you have no changes to stash.".to_string(), cost_usd));
    }
    let op = SafeOp::Stash {
        message,
        include_untracked,
    };
    let preview = build_preview(repo, &op)?;
    let rationale = "Interpreted your request as stashing your working changes.".to_string();
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// discardChanges (§4): intersect `paths` with the TRACKED-MODIFIED set (the
/// `unstaged` list from status — worktree-vs-index, tracked files only); drop
/// unknown/clean paths; none valid ⇒ Unsupported. Untracked-file deletion is
/// out of v1. Destructive.
fn resolve_discard_changes(
    repo: &git2::Repository,
    paths: Vec<String>,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return Ok(unsupported("this repository has no working tree.".to_string(), cost_usd)),
    };
    let status = read_status(workdir)?;
    let modified: std::collections::HashSet<&str> =
        status.unstaged.iter().map(|e| e.path.as_str()).collect();
    // HashSet dedup (T2.2 NIT): the model may repeat paths; keep first
    // occurrence order without an O(n²) `Vec::contains` scan.
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut kept: Vec<String> = Vec::new();
    for p in &paths {
        if modified.contains(p.as_str()) && seen.insert(p.as_str()) {
            kept.push(p.clone());
        }
    }
    if kept.is_empty() {
        return Ok(unsupported(
            "none of those paths have uncommitted changes to discard.".to_string(),
            cost_usd,
        ));
    }
    let op = SafeOp::Discard { paths: kept };
    let preview = build_preview(repo, &op)?;
    let rationale =
        "Interpreted your request as discarding your uncommitted changes to those files.".to_string();
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// mergeBranch (§4): `branch` must resolve to a local or remote-tracking branch
/// (else Unsupported); the command handles FF / conflicts. Caution.
fn resolve_merge_branch(
    repo: &git2::Repository,
    branch: &str,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let shown = sanitize_model_text(branch); // F-A2-1: model echo, display only
    let resolves = repo.find_branch(branch, git2::BranchType::Local).is_ok()
        || repo.find_branch(branch, git2::BranchType::Remote).is_ok();
    if !resolves {
        return Ok(unsupported(
            format!("I couldn't find a branch named '{shown}' to merge."),
            cost_usd,
        ));
    }
    let op = SafeOp::Merge {
        name: branch.to_string(),
    };
    let preview = build_preview(repo, &op)?;
    let rationale = format!("Interpreted your request as merging `{shown}` into the current branch.");
    Ok(proposed(op, preview, rationale, cost_usd))
}

// ------------------------------------------------------------------- helpers

/// A `SafeOp::Reset` targeting `commit` in `mode` (full oid + 7-char display).
fn reset_op(commit: &git2::Commit<'_>, mode: ResetMode) -> SafeOp {
    SafeOp::Reset {
        target_oid: commit.id().to_string(),
        target_short: short7(commit.id()),
        mode,
    }
}

/// Wraps a resolved op + preview + Rust-generated rationale into a `Proposed`.
fn proposed(
    op: SafeOp,
    preview: crate::git::ai_operation::OperationPreview,
    rationale: String,
    cost_usd: Option<f64>,
) -> PlanOutcome {
    PlanOutcome::Proposed {
        operation: Box::new(ProposedOperation {
            op,
            preview,
            rationale,
            cost_usd,
        }),
    }
}

/// Configured upstream shorthand of the current branch, if any (read-only).
fn current_upstream_name(repo: &git2::Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let name = head.shorthand().ok()?;
    let branch = repo.find_branch(name, git2::BranchType::Local).ok()?;
    let upstream = branch.upstream().ok()?;
    upstream.name().ok().flatten().map(str::to_string)
}

/// Reason string when a merge/rebase/cherry-pick/revert is mid-flight, else
/// `None` (§4 global precondition). Read from `repo.state()` (matches the guard
/// `reset_branch` enforces at execution time).
fn op_in_progress_reason(repo: &git2::Repository) -> Option<String> {
    use git2::RepositoryState as S;
    let op = match repo.state() {
        S::Clean => return None,
        S::Merge => "merge",
        S::Rebase | S::RebaseInteractive | S::RebaseMerge => "rebase",
        S::CherryPick | S::CherryPickSequence => "cherry-pick",
        S::Revert | S::RevertSequence => "revert",
        _ => "operation",
    };
    Some(format!("finish or abort the in-progress {op} first."))
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod resolution_tests;
