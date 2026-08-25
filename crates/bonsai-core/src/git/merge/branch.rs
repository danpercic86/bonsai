//! The `merge_branch` entry point (P3c contract §4.1–§4.3): preconditions,
//! autostash, fast-forward, and the normal-merge orchestration. Clean merges
//! auto-commit via [`finalize_merge_commit`]; conflicts pause into
//! `RepoOpState::Merge`.

use std::path::Path;

use crate::error::AppError;
use crate::git::autostash::{self, PopResult};
use crate::git::bisect::require_no_bisect;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::hooks::hooks_enabled;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

use super::{finalize_merge_commit, prepared_merge_message, MergeHooks, MergeOutcome};

/// Blocking. Merges `branch_name` (local shorthand "feature/x" OR
/// remote-tracking shorthand "origin/main") into the current branch.
///
/// Preconditions (contract §4.1, checked in order BEFORE anything mutates):
/// state Clean; HEAD attached + born; branch resolvable (local then remote);
/// index matches HEAD (unstaged worktree changes ARE allowed — they only fail
/// as CheckoutConflict if the merge would overwrite them, in which case
/// nothing is left behind); git identity configured (a clean merge
/// auto-commits).
///
/// `skip_hooks` (F-A4-2): the clean auto-merge commit runs the `commit-msg`
/// hook (only — see [`MergeHooks::MessageOnly`]); `true` ≡ `--no-verify`
/// bypasses it, as does `bonsai.runHooks=false`. A commit-msg rejection
/// returns [`AppError::HookRejected`] with the merge left PAUSED (MERGE_HEAD
/// retained — recover via commit_merge or abort_merge), the one deliberate
/// exception to the "failed merge_branch leaves state Clean" guarantee.
pub fn merge_branch(
    workdir: &Path,
    branch_name: &str,
    skip_hooks: bool,
) -> Result<MergeOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — commit or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot merge: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot merge: HEAD is detached".to_string()));
    }
    let head_branch = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot merge: HEAD has no branch name".to_string()))?;

    // Resolve incoming: local first, then remote-tracking. Merging the
    // current branch by name falls out as UpToDate naturally.
    let (incoming, incoming_is_remote) =
        match repo.find_branch(branch_name, git2::BranchType::Local) {
            Ok(b) => (b, false),
            Err(_) => match repo.find_branch(branch_name, git2::BranchType::Remote) {
                Ok(b) => (b, true),
                Err(_) => {
                    return Err(AppError::BranchNotFound(format!(
                        "branch '{branch_name}' not found (local or remote-tracking)"
                    )));
                }
            },
        };

    // Identity check EARLY: a clean merge auto-commits, so ConfigMissing must
    // surface before the worktree is touched. `sig` is also the autostash
    // stasher identity (§2.3 step 5).
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Analysis BEFORE any stash (§2.3 steps 6-7). Extract the Copy result +
    // Oid, then release every borrow of `repo` (annotated / incoming) so the
    // later &mut stash / set_target / pop calls are legal.
    let (analysis, incoming_id) = {
        let annotated = repo.reference_to_annotated_commit(incoming.get())?;
        let id = annotated.id();
        let (analysis, _pref) = repo.merge_analysis(&[&annotated])?;
        (analysis, id)
    };
    drop(incoming);

    // An up-to-date no-op must never create a stash.
    if analysis.is_up_to_date() {
        return Ok(MergeOutcome::UpToDate);
    }

    // Dirty = any TRACKED change (staged or unstaged); untracked/ignored
    // excluded (§2.2, mirrors git's autostash default §2.4).
    // Keep the saved stash OID so every later apply/drop addresses it by
    // IDENTITY, never by stack position (F-A7-6).
    let stash_oid = if autostash::is_dirty(&repo)? {
        Some(autostash::stash_save(
            &mut repo,
            &sig,
            "bonsai: autostash before merge",
        )?)
    } else {
        None
    };
    let stashed = stash_oid.is_some();

    // SAFETY NOTE (applies to every bare `?` between here and the terminal
    // outcome — find_object, find_annotated_commit, repo.index(), the MERGE_MSG
    // writes, finalize_merge_commit): if any of these fails AFTER stash_save,
    // the autostash is RETAINED at stash@{0} (never dropped). These paths are
    // effectively unreachable — they operate on the already-validated
    // `incoming_id` oid or are plain filesystem writes — so we deliberately do
    // not wire a rollback into each; the stash is recoverable via the CLI.
    if analysis.is_fast_forward() {
        // merge.ff config NOT consulted in v1 — FF whenever possible.
        // Same safe-FF recipe as remote.rs pull_ff: checkout BEFORE set_target.
        // `obj` is scoped so its borrow of `repo` ends before the &mut calls.
        let checkout_res = {
            let obj = repo.find_object(incoming_id, None)?;
            let mut opts = git2::build::CheckoutBuilder::new();
            opts.safe(); // DEFAULT SAFE MODE — never .force()
            repo.checkout_tree(&obj, Some(&mut opts))
        };
        match checkout_res {
            Ok(()) => {}
            Err(e) if e.code() == git2::ErrorCode::Conflict => {
                // Nothing mutated yet (set_target not run); restore the stash.
                let msg = "cannot merge: local changes would be overwritten. \
                     Commit or discard them first.";
                return Err(autostash::rollback_and_map(
                    &mut repo,
                    stash_oid,
                    AppError::CheckoutConflict(msg.to_string()),
                ));
            }
            Err(e) => return Err(autostash::rollback_and_map(&mut repo, stash_oid, e.into())),
        }
        // `.map(|_| ())` discards the returned Reference so no borrow of `repo`
        // is retained across the following &mut rollback / pop calls.
        let ff_res = repo
            .find_reference(&format!("refs/heads/{head_branch}"))
            .and_then(|mut r| {
                r.set_target(incoming_id, &format!("merge {branch_name}: fast-forward"))
            })
            .map(|_| ());
        if let Err(e) = ff_res {
            // The ref move itself failed; worktree already checked out but the
            // branch still points at the old tip — restore the stash so the
            // user's original state is recoverable.
            return Err(autostash::rollback_and_map(&mut repo, stash_oid, e.into()));
        }
        let to = incoming_id.to_string();
        if let Some(oid) = stash_oid {
            return Ok(match autostash::pop_after_success(&mut repo, workdir, oid)? {
                PopResult::Restored => MergeOutcome::FastForwarded {
                    branch: head_branch,
                    to,
                    stashed: true,
                },
                PopResult::Conflicted(paths) => {
                    MergeOutcome::StashPopConflicts { head: to, paths }
                }
            });
        }
        return Ok(MergeOutcome::FastForwarded {
            branch: head_branch,
            to,
            stashed: false,
        });
    }

    // analysis.is_normal(): true merge.
    let mut message = prepared_merge_message(branch_name, incoming_is_remote);
    let mut merge_opts = git2::MergeOptions::new(); // defaults: find_renames on
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout
        .safe()
        .allow_conflicts(true)
        .conflict_style_merge(true); // <<<<<<< ======= >>>>>>> markers

    // Rebuild a NAME-BEARING annotated commit for the merge so libgit2 labels
    // the conflict "theirs" marker with the branch name (e.g. `>>>>>>> topic`)
    // rather than the bare 40-char oid. Must be rebuilt HERE (after stashing)
    // because stash_save2 needed &mut repo and an AnnotatedCommit borrows repo
    // immutably. reference_to_annotated_commit carries the ref name;
    // find_annotated_commit(oid) does NOT. Resolve local-first then
    // remote-tracking, matching the original resolution order. Scoped so both
    // borrows end before the &mut cleanup / rollback below.
    let merge_res = {
        let incoming_branch = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .or_else(|_| repo.find_branch(branch_name, git2::BranchType::Remote))
            .map_err(|_| {
                AppError::BranchNotFound(format!(
                    "branch '{branch_name}' not found (local or remote-tracking)"
                ))
            })?;
        let annotated = repo.reference_to_annotated_commit(incoming_branch.get())?;
        repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))
    };
    if let Err(e) = merge_res {
        // libgit2 may have written MERGE_HEAD/MERGE_MSG/MERGE_MODE before the
        // checkout failed. Guarantee: a failed merge_branch leaves state Clean.
        let _ = repo.cleanup_state();
        if let Ok(head_commit) = repo.head().and_then(|h| h.peel_to_commit()) {
            if let Ok(tree) = head_commit.tree() {
                // Re-open the index to see whatever repo.merge() left on disk.
                if let Ok(mut index) = repo.index() {
                    let _ = index.read_tree(&tree);
                    let _ = index.write();
                }
            }
        }
        let mapped = if e.code() == git2::ErrorCode::Conflict {
            AppError::CheckoutConflict(
                "cannot merge: local changes would be overwritten. \
                 Commit or discard them first."
                    .to_string(),
            )
        } else {
            e.into()
        };
        return Err(autostash::rollback_and_map(&mut repo, stash_oid, mapped));
    }

    let index = repo.index()?;
    if index.has_conflicts() {
        let paths: Vec<String> = list_conflicts(workdir)?
            .into_iter()
            .map(|c| c.path)
            .collect();
        // Conflicts block, exactly like git (contract §4.3); overwrite
        // libgit2's MERGE_MSG so the on-disk message is deterministic.
        message.push_str("\n\nConflicts:\n");
        message.push_str(
            &paths
                .iter()
                .map(|p| format!("\t{p}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        std::fs::write(repo.path().join("MERGE_MSG"), format!("{message}\n"))?;
        // PAUSE. Do NOT pop — reapplying into a conflicted worktree is unsafe
        // (OPEN Q #2). The stash (if any) is RETAINED on the stack.
        return Ok(MergeOutcome::Conflicts { paths, stashed });
    }

    // Clean: auto-commit (git-like), then cleanup. Keep the on-disk message
    // equal to the committed message until cleanup_state removes it.
    std::fs::write(repo.path().join("MERGE_MSG"), format!("{message}\n"))?;
    // `annotated` / `incoming` already dropped above; release the index borrow
    // before the &mut finalize below.
    drop(index);
    // Auto-commit follows commit.gpgsign (None) — a clean merge signs iff the
    // user opted in; byte-identical to pre-P58 when gpgsign is off (the default).
    // Hooks (F-A4-2): the auto-merge commit runs the `commit-msg` hook only
    // (git parity for message policy; pre-merge-commit/prepare-commit-msg are
    // unsupported — see MergeHooks::MessageOnly). If commit-msg REJECTS, the
    // `?` propagates HookRejected and the merge is left PAUSED — MERGE_HEAD
    // retained, merged content staged, HEAD unchanged — exactly git's "Not
    // committing merge; use 'git commit' to complete the merge." state. The
    // user can fix the message via commit_merge (optionally skipping hooks)
    // or abort_merge; a pre-merge autostash stays retained on the stack
    // (same recoverable pause as the Conflicts outcome above).
    let hooks = if hooks_enabled(&repo.config()?.snapshot()?, skip_hooks) {
        MergeHooks::MessageOnly
    } else {
        MergeHooks::Off
    };
    // `merge_branch` is not an activity-wrapped op (no dedicated category); the
    // auto-merge commit records no activity (None).
    let result = finalize_merge_commit(&mut repo, &message, None, hooks, None)?;
    let oid = result.oid;
    if let Some(stash) = stash_oid {
        return Ok(match autostash::pop_after_success(&mut repo, workdir, stash)? {
            PopResult::Restored => MergeOutcome::Merged { oid, stashed: true },
            PopResult::Conflicted(paths) => MergeOutcome::StashPopConflicts { head: oid, paths },
        });
    }
    Ok(MergeOutcome::Merged { oid, stashed: false })
}
