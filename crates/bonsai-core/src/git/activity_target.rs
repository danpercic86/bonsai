//! P87b FU-1: where a git-activity run's `target` comes from.
//!
//! One read-only, **infallible** resolution of "what ref is this run about?",
//! run once per op BEFORE it starts and only while the dock is subscribed (the
//! command layer gates on `GitActivityHub::is_active`). Every failure path
//! yields `None` — this is fire-and-forget observability and must never gate or
//! change an op's success/error (FU-1 §1). Blocking (git2): callers run it
//! inside `spawn_blocking`.
//!
//! **Why this is a second resolution, not the op's.**
//! `push_current_with_activity` / `force_push_with_lease_with_activity` resolve
//! the same upstream, but their versions are error-carrying: each `Err` branch
//! is load-bearing for the push error taxonomy (`NoUpstream` vs `NoRemote` vs
//! `PushRejected`). Folding an infallible observability read into them would
//! either weaken that taxonomy or wrap it in `Option` at the wrong layer. The
//! drift is pinned by tests instead of by shared code (FU-1 §3/§9.2):
//! `push_target_agrees_with_push_result_remote` for the remote name — all
//! `PushResult::UpToDate` can witness, since it reports the LOCAL branch — plus
//! `resolves_table`'s `renamed` fixture for the upstream branch.

use std::path::Path;

use crate::git::activity::{ActivityTarget, GitActivityCategory};
use crate::git::opstate::{read_op_state, RepoOpState};
use crate::git::remote::open_repo_at;
use crate::git::repo::read_head_info;

/// P119 §2.3: the ONLY cross-crate input for an arg-carried target. Core maps
/// the raw command argument through the one sanitize+cap funnel
/// ([`ActivityTarget`]'s private `new`); src-tauri still cannot build an
/// [`ActivityTarget`] itself.
#[derive(Debug, Clone, Copy)]
pub enum TargetArg<'a> {
    /// A local branch (`refs/heads/` stripped).
    Branch(&'a str),
    /// Any ref (ONE of `refs/heads/`, `refs/remotes/`, `refs/tags/` stripped).
    Ref(&'a str),
    /// A remote NAME — never its URL.
    Remote(&'a str),
    /// A tag (`refs/tags/` stripped).
    Tag(&'a str),
    /// A full or abbreviated hex oid → its 7-char short form; a revspec → `None`.
    Commit(&'a str),
    /// A stash index → `stash@{N}`.
    Stash(usize),
    /// A raw name/path identifier; may contain spaces (§2.4).
    Name(&'a str),
    /// The last component of a filesystem path (either separator; trailing
    /// separators ignored).
    PathLeaf(&'a str),
}

/// PURE, infallible, no repo open, no I/O: the target for an arg-carried run.
pub fn arg_activity_target(arg: TargetArg<'_>) -> Option<ActivityTarget> {
    match arg {
        TargetArg::Branch(b) => ActivityTarget::branch(b),
        TargetArg::Ref(r) => ActivityTarget::any_ref(r),
        TargetArg::Remote(r) => ActivityTarget::remote(r),
        TargetArg::Tag(t) => ActivityTarget::tag(t),
        TargetArg::Commit(oid) => ActivityTarget::commit(oid),
        TargetArg::Stash(i) => ActivityTarget::stash(i),
        TargetArg::Name(n) => ActivityTarget::name(n),
        TargetArg::PathLeaf(p) => ActivityTarget::name(path_leaf(p)?),
    }
}

/// Platform-independent last path component; `None` when nothing is left.
///
/// SECURITY: `None` for URL-shaped input (`://` anywhere, or an `@` in the
/// leaf). A clone URL can carry `user:token@host`, and the "leaf" of
/// `https://user:tok@host` is exactly the credential — callers must pass a
/// filesystem path, and anything that looks like a URL is refused, not trimmed.
fn path_leaf(p: &str) -> Option<&str> {
    if p.contains("://") {
        return None;
    }
    let leaf = p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next()?;
    (!leaf.is_empty() && !leaf.contains('@')).then_some(leaf)
}

/// What a run is about: at most one target, OR a count of ≥2 items — never both
/// (P119 §2.7). Fields are crate-private, so only this module's constructors
/// can produce one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunSubject {
    pub(crate) target: Option<ActivityTarget>,
    pub(crate) count: Option<u32>,
}

impl From<Option<ActivityTarget>> for RunSubject {
    fn from(target: Option<ActivityTarget>) -> Self {
        RunSubject {
            target,
            count: None,
        }
    }
}

impl RunSubject {
    /// 0 items → empty; 1 → that item's target; ≥2 → `count = n` (saturating
    /// u32) with NO target, so a multi-item run never carries a phrase.
    pub fn many<'a>(mut items: impl ExactSizeIterator<Item = TargetArg<'a>>) -> RunSubject {
        match items.len() {
            0 => RunSubject::default(),
            1 => RunSubject::from(items.next().and_then(arg_activity_target)),
            n => RunSubject {
                target: None,
                count: Some(u32::try_from(n).unwrap_or(u32::MAX)),
            },
        }
    }

    /// The single target, if any (read-only view for tests/callers).
    pub fn target(&self) -> Option<&ActivityTarget> {
        self.target.as_ref()
    }

    /// The item count of a multi-item run (≥2), if any.
    pub fn count(&self) -> Option<u32> {
        self.count
    }
}

/// Best-effort target for a run that is ABOUT to start (FU-1 §4, P119 §2.3).
///
/// `None` — meaning "this run has no target", rendered as the bare category noun
/// — for: `Fetch` (fetch-all touches *every* remote, so no single ref), every
/// arg-carried / multi-item category (their target comes from
/// [`arg_activity_target`], not HEAD), `BisectReset`, an unborn or detached HEAD,
/// a missing/unreadable upstream, or any git2 failure.
pub fn resolve_activity_target(
    workdir: &Path,
    category: GitActivityCategory,
) -> Option<ActivityTarget> {
    use GitActivityCategory as C;

    // 1. Cheapest exit first, before any repo open. Fetch-all has no one target;
    // the A/N rows never call this (the guard keeps it free if one does).
    if is_arg_or_untargeted(category) {
        return None;
    }

    // 2. Rebase in progress. MUST precede the detached check below: HEAD is
    // detached mid-rebase, but the run is about the branch being rebased.
    if matches!(category, C::RebaseContinue | C::RebaseSkip | C::RebaseAbort) {
        return match read_op_state(workdir).ok()? {
            RepoOpState::Rebase {
                head_name: Some(h), ..
            } => ActivityTarget::branch(&h),
            _ => None,
        };
    }

    let repo = open_repo_at(workdir).ok()?;

    // 3. Bisect midpoint: HEAD is detached on the commit being judged.
    if matches!(category, C::BisectGood | C::BisectBad | C::BisectSkip) {
        let oid = repo.head().ok()?.peel_to_commit().ok()?.id();
        return ActivityTarget::commit(&oid.to_string());
    }

    let head = read_head_info(&repo).ok()?;
    // Load-bearing: `read_head_info` DOES report a branch name for an unborn
    // HEAD (the symbolic target names the branch-to-be), and FU-1 §3.6-1 wants
    // `null` there. A detached HEAD has no branch the run is "on" at all, and
    // inventing `HEAD` / `(detached)` would claim a fact (FU-1 §4).
    if head.unborn || head.detached {
        return None;
    }
    let branch = head.branch_name?;

    // 4. HEAD branch. Every variant is named — no `_` arm — so a new category
    // fails to compile until someone decides where its target comes from.
    match category {
        C::Commit
        | C::Amend
        | C::MergeCommit
        | C::AbortMerge
        | C::CherryPickContinue
        | C::CherryPickAbort
        | C::RevertContinue
        | C::RevertAbort
        | C::StashCreate
        | C::ComposeCommits => ActivityTarget::branch(&branch),
        // No upstream ⇒ the op itself returns `NoUpstream`; the row reads the
        // bare noun rather than naming a ref that does not exist.
        C::Pull | C::ForcePush => {
            let (remote, remote_branch) = configured_upstream(&repo, &branch)?;
            ActivityTarget::remote_branch(&remote, &remote_branch)
        }
        // Unreachable: steps 1–3 already returned for all of these. Kept as
        // `None` rather than `unreachable!()` because this resolver must never
        // panic — it runs for observability only, on a caller's op path.
        C::Fetch
        | C::CheckoutBranch
        | C::CheckoutCommit
        | C::CheckoutRemote
        | C::CreateBranch
        | C::DeleteBranch
        | C::DeleteBranches
        | C::RenameBranch
        | C::DeleteRemoteTracking
        | C::Merge
        | C::Rebase
        | C::InteractiveRebase
        | C::RebaseContinue
        | C::RebaseSkip
        | C::RebaseAbort
        | C::CherryPick
        | C::Revert
        | C::ResetSoft
        | C::ResetMixed
        | C::ResetHard
        | C::StashApply
        | C::StashPop
        | C::StashDrop
        | C::CreateTag
        | C::DeleteTag
        | C::PushTag
        | C::DeleteRemoteTag
        | C::ForceRefreshTag
        | C::SubmoduleAdd
        | C::SubmoduleInit
        | C::SubmoduleUpdate
        | C::SubmoduleSync
        | C::SubmoduleDeinit
        | C::SubmoduleRemove
        | C::WorktreeAdd
        | C::WorktreeRemove
        | C::WorktreeLock
        | C::WorktreeUnlock
        | C::Discard
        | C::BisectStart
        | C::BisectGood
        | C::BisectBad
        | C::BisectSkip
        | C::BisectReset
        | C::CloneRepo
        | C::InitRepo
        | C::AddRemote
        | C::RemoveRemote
        | C::RenameRemote
        | C::SetRemoteUrl => None,
        C::Push => {
            if let Some((remote, remote_branch)) = configured_upstream(&repo, &branch) {
                return ActivityTarget::remote_branch(&remote, &remote_branch);
            }
            // No upstream: `push_current` defaults to `origin/<branch>` and sets
            // the upstream afterwards — so that IS the ref it will push to. No
            // `origin` either ⇒ the op errors `NoRemote`; bare noun.
            if repo.find_remote("origin").is_ok() {
                return ActivityTarget::remote_branch("origin", &branch);
            }
            None
        }
    }
}

/// Step 1 of the resolver: `true` for the categories whose target never comes
/// from the repo (`Fetch`, `BisectReset`, and every §1 A/N row). Exhaustive —
/// no `_` arm — so a new category must be classified here explicitly.
fn is_arg_or_untargeted(category: GitActivityCategory) -> bool {
    use GitActivityCategory as C;
    match category {
        C::Fetch
        | C::BisectReset
        | C::CheckoutBranch
        | C::CheckoutCommit
        | C::CheckoutRemote
        | C::CreateBranch
        | C::DeleteBranch
        | C::DeleteBranches
        | C::RenameBranch
        | C::DeleteRemoteTracking
        | C::Merge
        | C::Rebase
        | C::InteractiveRebase
        | C::CherryPick
        | C::Revert
        | C::ResetSoft
        | C::ResetMixed
        | C::ResetHard
        | C::StashApply
        | C::StashPop
        | C::StashDrop
        | C::CreateTag
        | C::DeleteTag
        | C::PushTag
        | C::DeleteRemoteTag
        | C::ForceRefreshTag
        | C::SubmoduleAdd
        | C::SubmoduleInit
        | C::SubmoduleUpdate
        | C::SubmoduleSync
        | C::SubmoduleDeinit
        | C::SubmoduleRemove
        | C::WorktreeAdd
        | C::WorktreeRemove
        | C::WorktreeLock
        | C::WorktreeUnlock
        | C::Discard
        | C::BisectStart
        | C::CloneRepo
        | C::InitRepo
        | C::AddRemote
        | C::RemoveRemote
        | C::RenameRemote
        | C::SetRemoteUrl => true,
        // H (HEAD branch), R (rebase branch), C (bisect midpoint) rows.
        C::Commit
        | C::Amend
        | C::MergeCommit
        | C::Push
        | C::ForcePush
        | C::Pull
        | C::AbortMerge
        | C::CherryPickContinue
        | C::CherryPickAbort
        | C::RevertContinue
        | C::RevertAbort
        | C::StashCreate
        | C::ComposeCommits
        | C::RebaseContinue
        | C::RebaseSkip
        | C::RebaseAbort
        | C::BisectGood
        | C::BisectBad
        | C::BisectSkip => false,
    }
}

/// `branch.<name>.remote` + `branch.<name>.merge`, read-only and never erroring.
/// Returns `(remote, remote-branch SHORT name)`.
///
/// Config-only by design: it must not require a `refs/remotes/<remote>/<branch>`
/// tracking ref to exist (a branch can be configured before its first fetch).
fn configured_upstream(repo: &git2::Repository, branch: &str) -> Option<(String, String)> {
    let refname = format!("refs/heads/{branch}");
    let remote_buf = repo.branch_upstream_remote(&refname).ok()?;
    let remote = remote_buf.as_str().ok()?.to_string();
    // `branch.<name>.merge` already IS `refs/heads/<x>` (remote.rs §2.6); the
    // prefix is stripped again inside `ActivityTarget::branch`, so this is belt
    // and braces for a hand-edited config that stored a short name.
    let merge = repo
        .config()
        .ok()?
        .get_string(&format!("branch.{branch}.merge"))
        .ok()?;
    let short = merge
        .strip_prefix("refs/heads/")
        .unwrap_or(&merge)
        .to_string();
    Some((remote, short))
}

#[cfg(test)]
#[path = "activity_target_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "activity_target_arg_tests.rs"]
mod arg_tests;
