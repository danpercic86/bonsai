//! P87 activity-recording commit/amend cores (split from `commit.rs` for the
//! ~500-line limit).
//!
//! `create_commit_with_activity` / `amend_commit_with_activity` are
//! [`super::commit`]'s `create_commit` / `amend_commit` PLUS an optional
//! [`GitActivityRecorder`]: they emit `RunningHook` phases around each hook (via
//! the streaming hook variants), a `Finalizing` phase for the git2 write, and
//! stream hook output. `activity == None` is the **byte-for-byte** pre-P87 path;
//! the commit result + `HookRejected` behaviour are unchanged.

use std::path::Path;

use crate::error::AppError;
use crate::git::activity::{GitActivityRecorder, GitPhaseKind};
use crate::git::bisect::require_no_bisect;
use crate::git::commit::{
    branch_shorthand_after, normalize_message, resolve_signature, run_commit_msg_hook, CommitResult,
};
use crate::git::exec::SpawnGitExec;
use crate::git::hooks::{
    hooks_enabled, run_hook_nonblocking_streaming, run_hook_streaming, HookName,
};
use crate::git::signing::{self, resolve_signing};
use crate::git::stage::open_workdir_repo;

/// See the module doc. `activity == None` ≡ [`super::commit::create_commit`].
pub fn create_commit_with_activity(
    workdir: &Path,
    message: &str,
    sign: Option<bool>,
    skip_hooks: bool,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<CommitResult, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A Bonsai bisect runs on a clean detached HEAD, so `state()` below can't
    // see it — refuse a commit while one is active (would move the branch ref).
    require_no_bisect(&repo)?;

    // P3c contract §4.5 backend guard: a plain commit mid-merge would create
    // a 1-parent commit and silently drop MERGE_HEAD ancestry.
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — use 'Commit merge' or abort it".to_string(),
        ));
    }

    // One config snapshot drives the hook toggle, identity, and signing.
    let cfg = repo.config()?.snapshot()?;
    let hooks = hooks_enabled(&cfg, skip_hooks);

    // pre-commit runs BEFORE write_tree (git order); a non-zero exit aborts with
    // HookRejected before anything is written or any ref moves.
    if hooks {
        run_hook_streaming(&SpawnGitExec, workdir, HookName::PreCommit, &[], None, activity)?;
    }

    let mut index = repo.index()?;
    if hooks {
        // Reload from disk so a hook that re-staged (formatter, generator) is
        // included in the committed tree.
        index.read(true)?;
    }

    if index.has_conflicts() {
        return Err(AppError::Git(
            "cannot commit: unresolved conflicts".to_string(),
        ));
    }

    let mut msg = normalize_message(message);
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }

    // commit-msg may REWRITE the message file (trailer/template); re-read after.
    if hooks {
        msg = run_commit_msg_hook(&repo, workdir, &msg, activity)?;
    }

    // Identity: ConfigMissing surfaces before any object/index write.
    let sig = resolve_signature(&cfg)?;

    // The git2 object/ref write is the `Finalizing` phase (the hooks are done).
    if let Some(a) = activity {
        a.phase(GitPhaseKind::Finalizing, None);
    }

    let tree_oid = index.write_tree()?;

    let head = match repo.head() {
        Ok(h) => Some(h.peel_to_commit()?),
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            None
        }
        Err(e) => return Err(e.into()),
    };

    match &head {
        Some(h) if h.tree_id() == tree_oid => return Err(AppError::NothingToCommit),
        None if index.is_empty() => return Err(AppError::NothingToCommit),
        _ => {}
    }

    let full = format!("{msg}\n");
    let summary = msg.lines().next().unwrap_or(&msg).to_string();

    let signing = resolve_signing(&cfg, sign);
    let oid = if !signing.sign {
        // Unsigned path: byte-identical to pre-P58.
        let tree = repo.find_tree(tree_oid)?;
        let parents: Vec<&git2::Commit> = head.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, &full, &tree, &parents)?
    } else {
        let parent_oids: Vec<git2::Oid> = head.iter().map(git2::Commit::id).collect();
        let old_head = head.as_ref().map(git2::Commit::id);
        signing::create_signed_commit(
            &SpawnGitExec,
            workdir,
            tree_oid,
            &parent_oids,
            &sig,
            &sig,
            &full,
            old_head,
            &format!("commit: {summary}"),
        )?
    };

    // post-commit is best-effort: the commit already landed — never block on
    // it; a failure surfaces as a warning (audit #2 §3.3), never an error.
    let hook_warning = if hooks {
        run_hook_nonblocking_streaming(&SpawnGitExec, workdir, HookName::PostCommit, &[], activity)
            .warning(HookName::PostCommit)
    } else {
        None
    };

    let branch = branch_shorthand_after(&repo, workdir, signing.sign)?;

    Ok(CommitResult {
        oid: oid.to_string(),
        summary,
        branch,
        hook_warning,
    })
}

/// See the module doc. `activity == None` ≡ [`super::commit::amend_commit`].
pub fn amend_commit_with_activity(
    workdir: &Path,
    message: &str,
    sign: Option<bool>,
    skip_hooks: bool,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<CommitResult, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

    // Amending mid-merge/rebase/pick is nonsense — refuse before any read.
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — finish or abort it first".to_string(),
        ));
    }

    // HEAD commit to amend. Unborn / missing HEAD → nothing to amend.
    let head_commit = match repo.head() {
        Ok(h) => h.peel_to_commit()?,
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Err(AppError::Git(
                "nothing to amend: the repository has no commits yet".to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    };

    let cfg = repo.config()?.snapshot()?;
    let hooks = hooks_enabled(&cfg, skip_hooks);

    // pre-commit BEFORE write_tree (may re-stage); non-zero ⇒ HookRejected, abort.
    if hooks {
        run_hook_streaming(&SpawnGitExec, workdir, HookName::PreCommit, &[], None, activity)?;
    }

    // Tree from the current index. NO NothingToCommit guard — a message-only
    // amend (tree == HEAD's tree, 0 staged) is valid.
    let mut index = repo.index()?;
    if hooks {
        index.read(true)?; // pick up any hook re-staging
    }

    // Normalize line endings before trim, identical to `create_commit`.
    let mut msg = normalize_message(message);
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }
    if hooks {
        msg = run_commit_msg_hook(&repo, workdir, &msg, activity)?;
    }

    // Fresh committer (ConfigMissing before any write); original author preserved.
    let committer = resolve_signature(&cfg)?;
    let author = head_commit.author().to_owned();

    if let Some(a) = activity {
        a.phase(GitPhaseKind::Finalizing, None);
    }

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let full = format!("{msg}\n");
    let summary = msg.lines().next().unwrap_or(&msg).to_string();

    let signing = resolve_signing(&cfg, sign);
    let oid = if !signing.sign {
        // Unsigned path: byte-identical to pre-P58. `Commit::amend` REPLACES the
        // current commit onto its EXISTING parents (preserving merge parents) and
        // moves HEAD — unlike `repo.commit(Some("HEAD"), …)`, which would reject
        // the amend because the new first parent (HEAD^) is not the current tip.
        head_commit.amend(
            Some("HEAD"),
            Some(&author),
            Some(&committer),
            None,
            Some(&full),
            Some(&tree),
        )?
    } else {
        // Signed amend: rebuild on HEAD's ORIGINAL parents (not HEAD itself), with
        // the CAS old-oid = current HEAD so update-ref replaces the tip.
        let parent_oids: Vec<git2::Oid> = head_commit.parent_ids().collect();
        signing::create_signed_commit(
            &SpawnGitExec,
            workdir,
            tree_oid,
            &parent_oids,
            &author,
            &committer,
            &full,
            Some(head_commit.id()),
            &format!("commit (amend): {summary}"),
        )?
    };

    // Non-blocking; a failure surfaces as a warning (audit #2 §3.3).
    let hook_warning = if hooks {
        run_hook_nonblocking_streaming(&SpawnGitExec, workdir, HookName::PostCommit, &[], activity)
            .warning(HookName::PostCommit)
    } else {
        None
    };

    let branch = branch_shorthand_after(&repo, workdir, signing.sign)?;

    Ok(CommitResult {
        oid: oid.to_string(),
        summary,
        branch,
        hook_warning,
    })
}
