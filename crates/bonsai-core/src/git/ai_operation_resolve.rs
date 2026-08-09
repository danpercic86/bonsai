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
mod tests {
    //! P55 resolution/preview unit tests (§11.3–§11.8). These assert Rust's
    //! resolution of a PARSED intent (the model's text transform is exercised
    //! end-to-end in `tests/ai_operation_cli.rs`); the read-only guarantee for
    //! ALL ten intents is proven by `plan_never_mutates` in `ai_operation`.

    use super::*;
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;
    use std::path::Path;

    // ----------------------------------------------------------- fixtures

    /// git2-init a scratch repo with identity + autocrlf off (mirrors ai_explain).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// Commit `file`=`content` with `msg` on the current branch; returns full oid.
    fn commit(dir: &Path, file: &str, content: &str, msg: &str) -> String {
        std::fs::write(dir.join(file), content).expect("write");
        stage_paths(dir, &[file.to_string()]).expect("stage");
        create_commit(dir, msg, None, false).expect("commit").oid
    }

    fn oid(s: &str) -> git2::Oid {
        git2::Oid::from_str(s).expect("oid")
    }

    /// Linear A→B repo (HEAD=B). Returns (dir, a_oid, b_oid).
    fn linear_repo() -> (tempfile::TempDir, String, String) {
        let dir = init_scratch();
        let p = dir.path();
        let a = commit(p, "a.txt", "a\n", "A");
        let b = commit(p, "b.txt", "b\n", "B");
        (dir, a, b)
    }

    /// Repo whose HEAD is a MERGE commit M with parents [A(main), B(feature)].
    /// Uses A's tree for every commit so the worktree stays clean. Returns
    /// (dir, a_oid, m_oid, head_branch_name).
    fn merge_repo() -> (tempfile::TempDir, String, String, String) {
        let dir = init_scratch();
        let p = dir.path();
        let a = commit(p, "a.txt", "a\n", "A");
        let repo = git2::Repository::open(p).expect("open");
        let head_branch = repo
            .head()
            .expect("head")
            .shorthand()
            .expect("shorthand")
            .to_string();
        let a_c = repo.find_commit(oid(&a)).expect("A");
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let tree = a_c.tree().expect("tree");
        let b = repo
            .commit(Some("refs/heads/feature"), &sig, &sig, "B", &tree, &[&a_c])
            .expect("feature commit");
        let b_c = repo.find_commit(b).expect("B");
        let m = repo
            .commit(
                Some(&format!("refs/heads/{head_branch}")),
                &sig,
                &sig,
                "Merge branch 'feature'",
                &tree,
                &[&a_c, &b_c],
            )
            .expect("merge commit");
        (dir, a, m.to_string(), head_branch)
    }

    fn expect_proposed(o: PlanOutcome) -> ProposedOperation {
        match o {
            PlanOutcome::Proposed { operation } => *operation,
            other => panic!("expected Proposed, got {other:?}"),
        }
    }

    fn expect_unsupported(o: PlanOutcome) -> String {
        match o {
            PlanOutcome::Unsupported { reason, .. } => reason,
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // ------------------------------------------------------- §11.3 undoLastCommit

    /// §11.3: undoLastCommit targets HEAD's parent; Mixed (keep) vs Hard
    /// (discard) by `keepChanges`; dropped = [HEAD].
    #[test]
    fn undo_last_commit_targets_head_parent() {
        let (dir, a, b) = linear_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let short_b: String = b.chars().take(7).collect();

        // keepChanges=true → Mixed, Caution, no worktree warning.
        let op = expect_proposed(
            resolve_intent(&repo, AiOpIntent::UndoLastCommit { keep_changes: true }, None)
                .expect("Ok"),
        );
        match &op.op {
            SafeOp::Reset {
                target_oid, mode, ..
            } => {
                assert_eq!(target_oid, &a, "target = HEAD's parent (A)");
                assert_eq!(*mode, ResetMode::Mixed);
            }
            other => panic!("expected Reset, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Caution));
        assert!(op.preview.worktree_warning.is_none());
        assert_eq!(op.preview.dropped_commits.len(), 1, "dropped = [HEAD]");
        assert_eq!(op.preview.dropped_commits[0].short, short_b);

        // keepChanges=false → Hard, Destructive, worktree warning present.
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::UndoLastCommit { keep_changes: false },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Reset { mode, .. } => assert_eq!(*mode, ResetMode::Hard),
            other => panic!("expected Reset, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Destructive));
        assert!(op.preview.worktree_warning.is_some());
    }

    // -------------------------------------------------------- §11.4 undoLastMerge

    /// §11.4: undoLastMerge on a merge HEAD → Reset{first parent, Mixed},
    /// Destructive, with the upstream shared-history warning when an upstream
    /// exists; a non-merge HEAD → Unsupported.
    #[test]
    fn undo_last_merge_requires_merge_head() {
        let (dir, a, m, head_branch) = merge_repo();
        let p = dir.path();
        let repo = git2::Repository::open(p).expect("open");
        let short_m: String = m.chars().take(7).collect();

        let op =
            expect_proposed(resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok"));
        match &op.op {
            SafeOp::Reset {
                target_oid, mode, ..
            } => {
                assert_eq!(target_oid, &a, "target = merge's FIRST parent (A)");
                assert_eq!(*mode, ResetMode::Mixed);
            }
            other => panic!("expected Reset, got {other:?}"),
        }
        assert!(
            matches!(op.preview.danger, DangerLevel::Destructive),
            "undoLastMerge is always Destructive (OQ2)"
        );
        assert!(
            op.preview.dropped_commits.iter().any(|c| c.short == short_m),
            "the merge commit leaves the branch"
        );
        // No upstream yet → no shared-history warning.
        assert!(op.preview.worktree_warning.is_none());

        // Add an upstream → the shared-history warning appears.
        repo.remote("origin", "https://example.invalid/x.git").expect("remote");
        repo.reference(
            &format!("refs/remotes/origin/{head_branch}"),
            oid(&m),
            true,
            "seed upstream",
        )
        .expect("remote-tracking ref");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str(&format!("branch.{head_branch}.remote"), "origin")
                .expect("remote cfg");
            cfg.set_str(
                &format!("branch.{head_branch}.merge"),
                &format!("refs/heads/{head_branch}"),
            )
            .expect("merge cfg");
        }
        let op =
            expect_proposed(resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok"));
        let warn = op.preview.worktree_warning.expect("upstream warning present");
        assert!(warn.contains("rewrites history"), "got: {warn}");
        assert!(warn.contains(&format!("origin/{head_branch}")), "got: {warn}");

        // A non-merge HEAD → Unsupported.
        let (dir2, _a2, _b2) = linear_repo();
        let repo2 = git2::Repository::open(dir2.path()).expect("open");
        let reason =
            expect_unsupported(resolve_intent(&repo2, AiOpIntent::UndoLastMerge, None).expect("Ok"));
        assert!(reason.contains("isn't a merge"), "got: {reason}");
    }

    // ------------------------------------------------------ §11.5 resetToCommit

    /// §11.5: resetToCommit resolves a SHORT hash from the state to a full oid;
    /// a bad ref → Unsupported.
    #[test]
    fn reset_to_commit_resolves_short_hash() {
        let (dir, a, _b) = linear_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let short_a: String = a.chars().take(7).collect();

        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::ResetToCommit {
                    commit: short_a.clone(),
                    keep_changes: true,
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Reset {
                target_oid,
                target_short,
                mode,
            } => {
                assert_eq!(target_oid, &a, "short hash resolved to A's FULL oid");
                assert_eq!(target_short, &short_a);
                assert_eq!(*mode, ResetMode::Mixed);
            }
            other => panic!("expected Reset, got {other:?}"),
        }

        let bad = resolve_intent(
            &repo,
            AiOpIntent::ResetToCommit {
                commit: "deadbeefdeadbeef".to_string(),
                keep_changes: false,
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(bad).contains("couldn't find a commit"));
    }

    // -------------------------------------------------- §11.6 switch local/remote

    /// §11.6: switchBranch — a LOCAL branch → `remote:false`; a name matching
    /// ONLY a remote-tracking branch → `remote:true`; no match → Unsupported.
    #[test]
    fn switch_branch_local_vs_remote() {
        let (dir, _a, b) = linear_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");

        // A second LOCAL branch (non-current) at HEAD.
        let head_c = repo.find_commit(oid(&b)).expect("B");
        repo.branch("other", &head_c, false).expect("local branch");
        // A remote-tracking ref with NO matching local branch.
        repo.reference("refs/remotes/origin/feature", oid(&b), true, "seed remote")
            .expect("remote-tracking ref");

        // Local → remote:false.
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::SwitchBranch {
                    branch: "other".to_string(),
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::SwitchBranch { name, remote } => {
                assert_eq!(name, "other");
                assert!(!remote, "a local branch resolves to remote:false");
            }
            other => panic!("expected SwitchBranch, got {other:?}"),
        }

        // Only-remote match → remote:true.
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::SwitchBranch {
                    branch: "origin/feature".to_string(),
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::SwitchBranch { name, remote } => {
                assert_eq!(name, "origin/feature");
                assert!(remote, "an only-remote match resolves to remote:true");
            }
            other => panic!("expected SwitchBranch, got {other:?}"),
        }

        // No match → Unsupported.
        let reason = expect_unsupported(
            resolve_intent(
                &repo,
                AiOpIntent::SwitchBranch {
                    branch: "does-not-exist".to_string(),
                },
                None,
            )
            .expect("Ok"),
        );
        assert!(reason.contains("couldn't find a branch"), "got: {reason}");
    }

    // --------------------------------------- §11.7 discard → tracked-modified only

    /// §11.7: discardChanges intersects with the tracked-modified (unstaged) set;
    /// unknown/clean paths are dropped; none valid → Unsupported; a valid
    /// tracked-modified path → `Discard` (Destructive, worktree warning).
    #[test]
    fn discard_filters_to_tracked_modified() {
        let (dir, _a, _b) = linear_repo();
        let p = dir.path();
        let repo = git2::Repository::open(p).expect("open");
        // a.txt tracked+committed → modify it unstaged so it is tracked-modified.
        std::fs::write(p.join("a.txt"), "changed\n").expect("edit a.txt");

        // Unknown + clean paths dropped; only a.txt kept.
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::DiscardChanges {
                    paths: vec![
                        "a.txt".to_string(),
                        "b.txt".to_string(),        // tracked but clean → dropped
                        "no-such.txt".to_string(),  // unknown → dropped
                    ],
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Discard { paths } => assert_eq!(paths, &vec!["a.txt".to_string()]),
            other => panic!("expected Discard, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Destructive));
        assert!(op.preview.worktree_warning.is_some(), "discard warns");

        // None valid → Unsupported.
        let reason = expect_unsupported(
            resolve_intent(
                &repo,
                AiOpIntent::DiscardChanges {
                    paths: vec!["b.txt".to_string(), "no-such.txt".to_string()],
                },
                None,
            )
            .expect("Ok"),
        );
        assert!(reason.contains("uncommitted changes to discard"), "got: {reason}");
    }

    // ------------------------------ §11.8 op-in-progress blocks ALL mutating intents

    /// §11.8: with a mid-flight op (a written MERGE_HEAD ⇒ `repo.state()` ==
    /// Merge), EACH of the ten mutating intents resolves to Unsupported (the
    /// global precondition, §4). The `unsupported` escape hatch is not a
    /// mutating intent and is excluded.
    #[test]
    fn op_in_progress_blocks_all_mutating_intents() {
        let (dir, a, _b) = linear_repo();
        let p = dir.path();
        // Force a Merge state: libgit2 derives RepositoryState::Merge from the
        // presence of .git/MERGE_HEAD (no real conflict needed).
        {
            let repo = git2::Repository::open(p).expect("open");
            std::fs::write(repo.path().join("MERGE_HEAD"), format!("{a}\n"))
                .expect("write MERGE_HEAD");
        }
        let repo = git2::Repository::open(p).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Merge,
            "MERGE_HEAD must put the repo in Merge state"
        );

        let short_a: String = a.chars().take(7).collect();
        let intents = vec![
            AiOpIntent::UndoLastCommit { keep_changes: true },
            AiOpIntent::UndoLastMerge,
            AiOpIntent::ResetToCommit {
                commit: short_a,
                keep_changes: true,
            },
            AiOpIntent::RevertCommit {
                commit: a.clone(),
            },
            AiOpIntent::SwitchBranch {
                branch: "whatever".to_string(),
            },
            AiOpIntent::CreateBranch {
                name: "new-branch".to_string(),
                at_commit: None,
            },
            AiOpIntent::DeleteBranch {
                branch: "whatever".to_string(),
            },
            AiOpIntent::StashChanges {
                message: None,
                include_untracked: true,
            },
            AiOpIntent::DiscardChanges {
                paths: vec!["a.txt".to_string()],
            },
            AiOpIntent::MergeBranch {
                branch: "whatever".to_string(),
            },
        ];
        assert_eq!(intents.len(), 10, "all ten mutating intents are enumerated");

        for intent in intents {
            let label = format!("{intent:?}");
            let reason = expect_unsupported(
                resolve_intent(&repo, intent, None).expect("Ok(Unsupported)"),
            );
            assert!(
                reason.contains("in-progress"),
                "{label} must be blocked by the op-in-progress guard, got: {reason}"
            );
        }
    }

    // ----------------------------------------------- switch/create/delete/stash/merge

    /// The remaining P55b happy paths (complement to §11.6/§11.7): createBranch
    /// validates + resolves, deleteBranch rejects current/non-local, stash needs
    /// a dirty tree, merge needs a resolvable branch.
    #[test]
    fn create_delete_stash_merge_resolution() {
        let (dir, a, b) = linear_repo();
        let p = dir.path();
        let repo = git2::Repository::open(p).expect("open");
        let head_branch = repo.head().expect("head").shorthand().expect("sh").to_string();

        // createBranch at HEAD (at_oid = None), Safe.
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::CreateBranch {
                    name: "feat/x".to_string(),
                    at_commit: None,
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::CreateBranch { name, at_oid } => {
                assert_eq!(name, "feat/x");
                assert_eq!(at_oid, &None, "no atCommit → create at HEAD");
            }
            other => panic!("expected CreateBranch, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Safe));

        // createBranch atCommit resolves to a full oid.
        let short_a: String = a.chars().take(7).collect();
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::CreateBranch {
                    name: "feat/at-a".to_string(),
                    at_commit: Some(short_a),
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::CreateBranch { at_oid, .. } => assert_eq!(at_oid.as_deref(), Some(a.as_str())),
            other => panic!("expected CreateBranch, got {other:?}"),
        }

        // Invalid branch name → Unsupported.
        let bad = resolve_intent(
            &repo,
            AiOpIntent::CreateBranch {
                name: "bad name~with^junk".to_string(),
                at_commit: None,
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(bad).contains("isn't a valid branch name"));

        // deleteBranch: the CURRENT branch → Unsupported.
        let cur = resolve_intent(
            &repo,
            AiOpIntent::DeleteBranch {
                branch: head_branch.clone(),
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(cur).contains("current branch"));

        // deleteBranch: a non-local name → Unsupported.
        let missing = resolve_intent(
            &repo,
            AiOpIntent::DeleteBranch {
                branch: "ghost".to_string(),
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(missing).contains("no local branch"));

        // deleteBranch: a real local, non-current branch → Caution DeleteBranch.
        let head_c = repo.find_commit(oid(&b)).expect("B");
        repo.branch("stale", &head_c, false).expect("branch");
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::DeleteBranch {
                    branch: "stale".to_string(),
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::DeleteBranch { name } => assert_eq!(name, "stale"),
            other => panic!("expected DeleteBranch, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Caution));

        // stashChanges on a CLEAN tree → Unsupported.
        let clean = resolve_intent(
            &repo,
            AiOpIntent::StashChanges {
                message: None,
                include_untracked: false,
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(clean).contains("no changes to stash"));

        // Dirty the tree → stashChanges proposes a Safe stash.
        std::fs::write(p.join("a.txt"), "dirty\n").expect("edit");
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::StashChanges {
                    message: Some("wip".to_string()),
                    include_untracked: false,
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Stash {
                message,
                include_untracked,
            } => {
                assert_eq!(message.as_deref(), Some("wip"));
                assert!(!include_untracked);
            }
            other => panic!("expected Stash, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Safe));

        // mergeBranch: unresolvable → Unsupported; resolvable local → Caution Merge.
        let bad = resolve_intent(
            &repo,
            AiOpIntent::MergeBranch {
                branch: "ghost".to_string(),
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(bad).contains("couldn't find a branch"));

        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::MergeBranch {
                    branch: "stale".to_string(),
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Merge { name } => assert_eq!(name, "stale"),
            other => panic!("expected Merge, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Caution));
    }

    // ------------------------------------------- F-A2-1 model-echo sanitization

    /// F-A2-1: model-derived text surfaced to the UI is sanitized — the
    /// Unsupported.reason passthrough and the branch/commit echoes in resolver
    /// messages strip control/bidi chars and are length-capped.
    #[test]
    fn model_echoes_are_sanitized() {
        let (dir, _a, _b) = linear_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");

        // Unsupported.reason passthrough: controls + bidi stripped, capped.
        let evil = format!(
            "run\u{202e}\x1b[31m rm -rf\n{}",
            "A".repeat(500)
        );
        let reason = expect_unsupported(
            resolve_intent(&repo, AiOpIntent::Unsupported { reason: evil }, None).expect("Ok"),
        );
        assert!(!reason.contains('\u{202e}'), "bidi stripped: {reason:?}");
        assert!(!reason.contains('\x1b'), "ESC stripped: {reason:?}");
        assert!(!reason.contains('\n'), "newline replaced: {reason:?}");
        assert!(reason.chars().count() <= 201, "capped: {}", reason.chars().count());
        assert!(reason.ends_with('…'), "truncation marker present");

        // Branch echo in an Unsupported message: bidi/control chars removed.
        let reason = expect_unsupported(
            resolve_intent(
                &repo,
                AiOpIntent::SwitchBranch {
                    branch: "gh\u{202e}\x07ost".to_string(),
                },
                None,
            )
            .expect("Ok"),
        );
        assert!(reason.contains("'ghost'"), "sanitized echo, got: {reason:?}");

        // Commit echo: a non-hex spec is gated (F-A2-2) and echoed sanitized.
        let reason = expect_unsupported(
            resolve_intent(
                &repo,
                AiOpIntent::ResetToCommit {
                    commit: "HEAD~1\u{2066}\n".to_string(),
                    keep_changes: true,
                },
                None,
            )
            .expect("Ok"),
        );
        assert!(reason.contains("'HEAD~1 '"), "sanitized echo, got: {reason:?}");
    }
}
