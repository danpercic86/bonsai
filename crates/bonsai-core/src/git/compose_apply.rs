//! AI commit composer — APPLY side (P54b). Applies a user-finalized
//! [`ComposePlan`] as an ORDERED stage+commit sequence, turning a messy working
//! tree into N logical commits. NOT AI-gated (pure git2 — the house shape of
//! `commit`, not the `ai_*` triple); the plan is whatever the user reviewed/edited.
//!
//! # The three safety guarantees (contract §0 D4/D5, §4.1)
//! 1. **ATOMIC.** The WHOLE plan is validated BEFORE anything mutates (identity via
//!    [`resolve_signature`], every path present in the current change set + assigned
//!    to exactly one group, non-empty messages, non-empty file lists). Any failure
//!    during the commit loop rolls back so ZERO commits land.
//! 2. **HEAD + index ROLLBACK.** The original HEAD (peeled to a commit oid; `None`
//!    when unborn) is recorded before mutating. On failure [`rollback`] restores the
//!    branch/detached HEAD (or deletes the branch the loop created, when started
//!    unborn) and re-reads the index to match.
//! 3. **WORKING TREE IS NEVER TOUCHED.** No checkout, no hard reset, no workdir
//!    writes — ever. [`reset_index_to_head`] is a mixed-reset equivalent (read the
//!    HEAD tree into the index; `clear()` when unborn). On success, failure, or
//!    cancel the bytes on disk are exactly as the user left them; files in no group
//!    stay uncommitted (unstaged) in the working tree.
//!
//! Because it is a FILE-LEVEL partition (D2), after the index reset staging group K
//! only advances files untouched by earlier groups, so each commit's delta-to-parent
//! is exactly its group's files (no line renumbering — v1 stages whole files).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::AppError;
use crate::git::ai_compose::ComposeGroup;
use crate::git::ai_explain::gather_worktree;
use crate::git::bisect::require_no_bisect;
use crate::git::commit::{create_commit, resolve_signature};
use crate::git::stage::{open_workdir_repo, stage_paths, validate_rel_path};

/// User-finalized plan to apply: an ORDERED list of groups (first = oldest
/// commit). A changed file absent from every group is intentionally left
/// uncommitted in the working tree. COMMAND INPUT (Deserialize).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePlan {
    pub groups: Vec<ComposeGroup>,
}

/// Result of applying a plan: created commits, oldest→newest. Serialize only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeApplyResult {
    pub commits: Vec<ComposeCommit>,
}

/// One created commit (contract §2.1). `oid` is the full 40-hex id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeCommit {
    /// Full 40-char hex oid of the new commit.
    pub oid: String,
    /// First line of the (cleaned) commit message.
    pub summary: String,
}

/// Blocking. Applies `plan` as an ORDERED stage+commit sequence (contract §4.1).
/// ATOMIC: validates the whole plan first, resets the index to HEAD (working tree
/// UNTOUCHED), then commits each group; ANY mid-sequence failure rolls HEAD+index
/// back so NOTHING is committed. Does NOT emit `repo-changed` — the caller refetches.
///
/// Errors: `NoRepo` (via `open_workdir_repo`) | `OperationInProgress` (mid-op) |
/// `Git` (unresolved conflicts) | `EmptyMessage` | `ConfigMissing` (identity unset) |
/// `NothingToCommit` (empty plan) | `Other` (unknown/duplicate path, empty group,
/// a group whose staged files net to no change / stale plan).
pub fn apply_composed_commits(
    workdir: &Path,
    plan: &ComposePlan,
) -> Result<ComposeApplyResult, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A Bonsai bisect runs on a clean detached HEAD, invisible to `state()` below.
    require_no_bisect(&repo)?;
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — finish or abort it before composing commits"
                .to_string(),
        ));
    }
    if repo.index()?.has_conflicts() {
        return Err(AppError::Git(
            "cannot compose: unresolved conflicts".to_string(),
        ));
    }

    // ---- validate the WHOLE plan first; nothing mutates yet (safety guarantee 1) ----
    if plan.groups.is_empty() {
        return Err(AppError::NothingToCommit);
    }
    // Identity resolves to a signature or `ConfigMissing` — EARLY, before any commit
    // (so an unset identity never lands a partial sequence).
    resolve_signature(&repo.config()?.snapshot()?)?;

    // The authoritative HEAD→workdir change set (index-aware, incl. untracked). Every
    // planned path must belong to it; renames also carry their OLD path so staging a
    // rename's NEW path stages both the add and the delete side.
    let worktree = gather_worktree(workdir)?;
    let changed: HashSet<&str> = worktree.iter().map(|f| f.path.as_str()).collect();
    let rename_origs: HashMap<&str, &str> = worktree
        .iter()
        .filter_map(|f| f.orig_path.as_deref().map(|o| (f.path.as_str(), o)))
        .collect();

    let mut seen: HashSet<&str> = HashSet::new();
    for g in &plan.groups {
        if g.message.trim().is_empty() {
            return Err(AppError::EmptyMessage);
        }
        if g.files.is_empty() {
            return Err(AppError::Other("a group has no files".to_string()));
        }
        for f in &g.files {
            validate_rel_path(f)?;
            if !changed.contains(f.as_str()) {
                return Err(AppError::Other(format!(
                    "file '{f}' is not in the working changes; refresh the composer"
                )));
            }
            if !seen.insert(f.as_str()) {
                return Err(AppError::Other(format!(
                    "file '{f}' is assigned to more than one group"
                )));
            }
        }
    }

    // ---- rollback anchor: HEAD peeled to a commit oid (None on unborn HEAD) ----
    let orig_head: Option<git2::Oid> = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.id());

    // ---- take over the index so uncovered changes cannot leak into any commit ----
    reset_index_to_head(&repo, orig_head)?;

    // ---- create commits in order; atomic on any failure ----
    let mut commits: Vec<ComposeCommit> = Vec::with_capacity(plan.groups.len());
    for (idx, g) in plan.groups.iter().enumerate() {
        let paths = files_with_rename_origs(&rename_origs, &g.files);
        // Stage ONLY this group, then commit the whole (cumulative) index. Because
        // the partition is disjoint, the commit's delta-to-parent is exactly `g`.
        // skip_hooks = TRUE (P59; F-A4-4 DECISION, T2 Area 4 audit 2026-08-09): AI-composed
        // commits BYPASS ALL GIT HOOKS — deliberately. A re-staging pre-commit hook
        // (`git add -u` / lint-staged) would, via the commit's `index.read(true)`, pull OTHER
        // groups' working-tree changes into this commit and silently break that partition
        // invariant; a commit-msg hook would rewrite each generated group message. The composer
        // is a mechanical history-organizer, so hooks stay OFF for its split commits (a normal
        // commit still runs them). Consequence a hook-policy shop should know: commit-message
        // policy hooks do NOT vet composer-generated messages. Documented in the P59 user
        // checklist ("Known v1 hook divergences") and flagged FOR USER REVIEW in
        // docs/testing-campaign-2026-08/FINDINGS.md; revisit commit-msg-only execution later.
        let step =
            stage_paths(workdir, &paths).and_then(|()| create_commit(workdir, &g.message, None, true));
        match step {
            Ok(cr) => commits.push(ComposeCommit {
                oid: cr.oid,
                summary: cr.summary,
            }),
            Err(e) => {
                // Any failure (incl. a group that nets to no change / a stale plan =>
                // `create_commit` `NothingToCommit`) unwinds EVERYTHING landed so far.
                rollback(&repo, orig_head)?;
                return Err(annotate(e, idx));
            }
        }
    }

    Ok(ComposeApplyResult { commits })
}

/// Resets the index to `orig_head`'s tree (mixed-reset equivalent) — or empties it
/// when unborn — and writes it. The WORKING TREE IS NEVER TOUCHED (no checkout, no
/// `--hard`): only the on-disk index moves.
fn reset_index_to_head(
    repo: &git2::Repository,
    orig_head: Option<git2::Oid>,
) -> Result<(), AppError> {
    let mut index = repo.index()?;
    match orig_head {
        Some(oid) => {
            let tree = repo.find_commit(oid)?.tree()?;
            index.read_tree(&tree)?;
        }
        None => index.clear()?,
    }
    index.write()?;
    Ok(())
}

/// Expands a group's file list with the OLD path of any rename (from the
/// HEAD→workdir `rename_origs` map) so [`stage_paths`] stages both the delete (old)
/// and add (new) sides. Non-renames pass through unchanged; input order preserved.
fn files_with_rename_origs(rename_origs: &HashMap<&str, &str>, files: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(files.len());
    for f in files {
        out.push(f.clone());
        if let Some(orig) = rename_origs.get(f.as_str()) {
            out.push((*orig).to_string());
        }
    }
    out
}

/// Undoes every commit the loop landed (safety guarantee 2). Working tree untouched
/// ⇒ all original changes stay on disk; the index is re-read to match HEAD.
/// - `Some(oid)`: point HEAD back at `oid` — a branch HEAD moves its branch ref;
///   a detached HEAD is re-pointed directly — then reset the index to `oid`'s tree.
/// - `None` (started unborn): the loop created the branch tip HEAD symbolically
///   points at; delete that ref so HEAD is unborn again, then empty the index.
fn rollback(repo: &git2::Repository, orig_head: Option<git2::Oid>) -> Result<(), AppError> {
    match orig_head {
        Some(oid) => {
            let mut head_ref = repo.head()?;
            if head_ref.is_branch() {
                head_ref.set_target(oid, "bonsai: composer rollback")?;
            } else {
                repo.set_head_detached(oid)?;
            }
            reset_index_to_head(repo, Some(oid))?;
        }
        None => {
            // Started unborn: the loop's first commit created the branch tip HEAD
            // symbolically points at. Delete that ref so HEAD is unborn again.
            let mut branch_name: Option<String> = None;
            if let Ok(head_ref) = repo.find_reference("HEAD") {
                // git2 0.21: `symbolic_target` is `Result<Option<&str>>` (non-utf8
                // symbolic target => Err). A missing/invalid target just leaves HEAD
                // as-is — still unborn — which is the intended end state.
                if let Ok(Some(target)) = head_ref.symbolic_target() {
                    branch_name = Some(target.to_owned());
                }
            }
            if let Some(name) = branch_name {
                if let Ok(mut branch_ref) = repo.find_reference(&name) {
                    branch_ref.delete()?;
                }
            }
            let mut index = repo.index()?;
            index.clear()?;
            index.write()?;
        }
    }
    Ok(())
}

/// Enriches a group's failure with its 1-based index. A bare `NothingToCommit`
/// (a no-op / stale group) becomes an actionable `Other`; message-bearing variants
/// keep their kind with a `group N:` prefix. `EmptyMessage`/`ConfigMissing` are
/// validated up front, so they are preserved verbatim if ever seen here.
fn annotate(e: AppError, group_index: usize) -> AppError {
    let n = group_index + 1;
    match e {
        AppError::NothingToCommit => AppError::Other(format!(
            "group {n}: its staged files produce no change to commit \
             (a no-op group or a stale plan); refresh the composer"
        )),
        AppError::Git(m) => AppError::Git(format!("group {n}: {m}")),
        AppError::Io(m) => AppError::Io(format!("group {n}: {m}")),
        AppError::Other(m) => AppError::Other(format!("group {n}: {m}")),
        AppError::OperationInProgress(m) => {
            AppError::OperationInProgress(format!("group {n}: {m}"))
        }
        other => other,
    }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod apply_tests2;
