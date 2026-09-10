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
use crate::git::remote::open_repo_at;
use crate::git::repo::read_head_info;

/// Best-effort target for a run that is ABOUT to start (FU-1 §4).
///
/// `None` — meaning "this run has no target", rendered as the bare category noun
/// — for: `Fetch` (fetch-all touches *every* remote, so no single ref), an
/// unborn or detached HEAD, a missing/unreadable upstream, or any git2 failure.
pub fn resolve_activity_target(
    workdir: &Path,
    category: GitActivityCategory,
) -> Option<ActivityTarget> {
    // Cheapest exit first, and before any repo open: `fetch_all_with_activity`
    // fetches every configured remote, so there is no one target (FU-1 §4).
    if category == GitActivityCategory::Fetch {
        return None;
    }

    let repo = open_repo_at(workdir).ok()?;
    let head = read_head_info(&repo).ok()?;
    // Load-bearing: `read_head_info` DOES report a branch name for an unborn
    // HEAD (the symbolic target names the branch-to-be), and FU-1 §3.6-1 wants
    // `null` there. A detached HEAD has no branch the run is "on" at all, and
    // inventing `HEAD` / `(detached)` would claim a fact (FU-1 §4).
    if head.unborn || head.detached {
        return None;
    }
    let branch = head.branch_name?;

    match category {
        // Unreachable: the early return above already took Fetch. Kept as `None`
        // rather than `unreachable!()` because this resolver must never panic —
        // it runs for observability only, on a caller's op path (module docs).
        GitActivityCategory::Fetch => None,
        GitActivityCategory::Commit
        | GitActivityCategory::Amend
        | GitActivityCategory::MergeCommit => ActivityTarget::branch(&branch),
        // No upstream ⇒ the op itself returns `NoUpstream`; the row reads the
        // bare noun rather than naming a ref that does not exist.
        GitActivityCategory::Pull | GitActivityCategory::ForcePush => {
            let (remote, remote_branch) = configured_upstream(&repo, &branch)?;
            ActivityTarget::remote_branch(&remote, &remote_branch)
        }
        GitActivityCategory::Push => {
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
