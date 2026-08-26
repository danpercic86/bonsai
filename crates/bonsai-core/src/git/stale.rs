//! Stale-branch cleanup core (P25 contract §4, B4).
//!
//! Pure git2 logic, no Tauri types — testable against the git CLI oracle
//! (`git branch --merged`). All functions blocking; the command layer wraps
//! them in `spawn_blocking`. Two entry points:
//!
//! - [`find_stale_branches`] — read-only classifier of local branches that are
//!   safe to delete (merged into the base OR upstream-gone). Touches nothing.
//! - [`delete_branches`] — confirm-gated batch deleter. Its ONLY safety is a
//!   server-side re-verification against a freshly recomputed safe set plus the
//!   not-current / not-base guards — it NEVER trusts the caller's classification.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::AppError;
use crate::git::repo::read_head_info;

/// Why a branch is safe to delete. Field-less enum → serializes to the bare
/// camelCase string ("merged" | "goneUpstream").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StaleReason {
    Merged,
    GoneUpstream,
}

/// One local branch classified as stale.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleBranch {
    /// Shorthand, e.g. "feature/sidebar".
    pub name: String,
    /// 40-hex tip oid.
    pub tip: String,
    /// First line of the tip commit's message (lossy).
    pub last_commit_summary: String,
    /// Tip commit author name (lossy).
    pub last_commit_author: String,
    /// Tip committer time, epoch seconds.
    pub last_commit_time: i64,
    /// Primary reason: Merged when merged (even if also gone), else GoneUpstream.
    pub reason: StaleReason,
    /// Raw flags (a branch may be both).
    pub merged: bool,
    pub gone_upstream: bool,
    /// Configured upstream shorthand (e.g. "origin/feature"), if any — present
    /// even when gone.
    pub upstream: Option<String>,
    /// Ahead/behind the BASE (best-effort; None on any lookup error). ahead =
    /// commits on the branch not in base (0 when merged); behind = base commits
    /// not on the branch.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Always false in returned entries (the current branch is excluded, OPEN #9);
    /// defensive wire field.
    pub is_current: bool,
}

/// The read-only classification result.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleReport {
    /// Resolved base shorthand (e.g. "main" / "origin/main").
    pub base: String,
    /// 40-hex base commit oid.
    pub base_oid: String,
    /// Stale candidates, case-insensitively sorted by name. Excludes the base
    /// branch and the current HEAD branch.
    pub branches: Vec<StaleBranch>,
}

/// Per-branch outcome of a batch delete. Field-less enum → bare camelCase string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BranchDeleteStatus {
    /// Successfully deleted.
    Deleted,
    /// Is the checked-out branch.
    SkippedCurrent,
    /// Is the resolved base branch.
    SkippedBase,
    /// Not in the freshly-recomputed safe set.
    SkippedNotStale,
    /// No such local branch.
    SkippedNotFound,
    /// git2 delete error (message carries detail).
    Failed,
}

/// One result row from [`delete_branches`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDeleteResult {
    pub name: String,
    pub status: BranchDeleteStatus,
    /// Human detail. Skipped/failed rows carry the reason; Deleted rows carry
    /// `"was at <short-oid>"` — the deleted tip, for recovery via reflog/undo
    /// (F-A7-5).
    pub message: Option<String>,
}

/// Opens the repo at `workdir` with `NO_SEARCH` (same as every git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Case-insensitive name ordering (ties broken case-sensitively so the order
/// is total and stable) — matches `branches::ci_cmp`.
fn ci_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// Resolved base identity for the stale scan (F-A7-1 / F-A7-4).
struct BaseIdentity<'r> {
    /// Shorthand echoed to the caller (`StaleReport::base`).
    name: String,
    /// Base tip commit.
    commit: git2::Commit<'r>,
    /// LOCAL branch names that must never be classified stale: the base itself
    /// when it names (or resolves to) a local branch, the local counterpart of
    /// a remote-tracking base (F-A7-4), and the repo's default branch
    /// (origin/HEAD's target — never auto-classified).
    protected: HashSet<String>,
    /// True when the base carried NO local-branch identity (bare OID, tag, or
    /// other non-branch ref): any local branch AT the base tip is then treated
    /// as the base itself and protected by oid identity (F-A7-1).
    protect_tip: bool,
}

/// `"refs/remotes/<remote>/<branch>"` → `Some("<branch>")` (F-A7-4). Remote
/// names cannot contain `/`, so the first component after the prefix is the
/// remote name.
fn local_counterpart(refname: &str) -> Option<String> {
    let rest = refname.strip_prefix("refs/remotes/")?;
    let (_, branch) = rest.split_once('/')?;
    if branch.is_empty() || branch == "HEAD" {
        return None;
    }
    Some(branch.to_string())
}

/// The repo's default branch's LOCAL name, best-effort: origin/HEAD's resolved
/// target's local counterpart (`None` when origin/HEAD is absent/unreadable).
fn default_branch_local_name(repo: &git2::Repository) -> Option<String> {
    let head_ref = repo.find_reference("refs/remotes/origin/HEAD").ok()?;
    let resolved = head_ref.resolve().ok()?;
    resolved.name().ok().and_then(local_counterpart)
}

/// Resolves the base for merged-detection to a full [`BaseIdentity`].
/// Precedence (OPEN #8): explicit `base` (revparse) → `origin/HEAD` target →
/// local `main` → local `master` → current HEAD (attached) → Err(Git).
fn resolve_stale_base<'r>(
    repo: &'r git2::Repository,
    base: Option<&str>,
) -> Result<BaseIdentity<'r>, AppError> {
    // The default branch is never auto-classified, whatever the base is.
    let mut protected: HashSet<String> = HashSet::new();
    if let Some(default) = default_branch_local_name(repo) {
        protected.insert(default);
    }

    // 1. Explicit base wins (any ref/oid the caller pins). Resolve to the ref
    //    identity, not just the string (F-A7-1): `refs/heads/main`, `main`, a
    //    remote-tracking `origin/main` (F-A7-4), an OID, or a tag at the tip
    //    must all protect the branch they denote.
    if let Some(b) = base {
        let bad_base = || AppError::Git(format!("cannot resolve base '{b}' to a commit"));
        let (obj, reference) = repo.revparse_ext(b).map_err(|_| bad_base())?;
        let commit = obj.peel_to_commit().map_err(|_| bad_base())?;
        let mut protect_tip = true;
        if let Some(r) = reference {
            if let Ok(refname) = r.name() {
                if let Some(local) = refname.strip_prefix("refs/heads/") {
                    protected.insert(local.to_string());
                    protect_tip = false;
                } else if let Some(local) = local_counterpart(refname) {
                    // Remote-tracking base: protect the local counterpart.
                    protected.insert(local);
                    protect_tip = false;
                }
            }
        }
        return Ok(BaseIdentity {
            name: b.to_string(),
            commit,
            protected,
            protect_tip,
        });
    }

    // 2. origin/HEAD → its resolved target (e.g. "origin/main"); protect the
    //    local counterpart (F-A7-4).
    if let Ok(head_ref) = repo.find_reference("refs/remotes/origin/HEAD") {
        if let Ok(resolved) = head_ref.resolve() {
            if let Ok(commit) = resolved.peel_to_commit() {
                let refname = resolved.name().ok().map(str::to_string);
                let shorthand = refname
                    .as_deref()
                    .and_then(|n| n.strip_prefix("refs/remotes/"))
                    .map(str::to_string)
                    .unwrap_or_else(|| "origin/HEAD".to_string());
                if let Some(local) = refname.as_deref().and_then(local_counterpart) {
                    protected.insert(local);
                }
                return Ok(BaseIdentity {
                    name: shorthand,
                    commit,
                    protected,
                    protect_tip: false,
                });
            }
        }
    }

    // 3. local `main`, then 4. local `master`.
    for name in ["main", "master"] {
        if let Ok(branch) = repo.find_branch(name, git2::BranchType::Local) {
            if let Ok(commit) = branch.into_reference().peel_to_commit() {
                protected.insert(name.to_string());
                return Ok(BaseIdentity {
                    name: name.to_string(),
                    commit,
                    protected,
                    protect_tip: false,
                });
            }
        }
    }

    // 5. current HEAD (attached, born).
    let head = read_head_info(repo)?;
    if let Some(name) = head.branch_name {
        if !head.unborn {
            if let Ok(commit) = repo.head().and_then(|h| h.peel_to_commit()) {
                protected.insert(name.clone());
                return Ok(BaseIdentity {
                    name,
                    commit,
                    protected,
                    protect_tip: false,
                });
            }
        }
    }

    Err(AppError::Git(
        "cannot determine a base branch to review against; specify one explicitly".to_string(),
    ))
}

/// Reads a branch's upstream state (§4.2). Returns `(upstream_shorthand, gone)`:
///
/// - not configured (`branch.<name>.merge` unset) → `(None, false)`.
/// - configured and the remote-tracking ref exists → `(Some(shorthand), false)`.
/// - configured but the remote-tracking ref is missing → `(Some(reconstructed), true)`
///   (reconstructed from `branch.<name>.remote` + short of `branch.<name>.merge`;
///   `None` if that read hiccups, but `gone` is still true).
fn upstream_state(
    cfg: &git2::Config,
    name: &str,
    branch: &git2::Branch,
) -> (Option<String>, bool) {
    let configured = cfg.get_string(&format!("branch.{name}.merge")).is_ok();
    if !configured {
        return (None, false);
    }

    match branch.upstream() {
        Ok(u) => {
            // Tracking ref exists → not gone; carry its shorthand.
            let shorthand = u.name().ok().flatten().map(str::to_string);
            (shorthand, false)
        }
        Err(_) => {
            // Configured but the remote-tracking ref is gone. Reconstruct
            // "<remote>/<short merge branch>" from config, best-effort.
            let remote = cfg.get_string(&format!("branch.{name}.remote")).ok();
            let merge = cfg.get_string(&format!("branch.{name}.merge")).ok();
            let short = merge.map(|m| {
                m.strip_prefix("refs/heads/")
                    .map(str::to_string)
                    .unwrap_or(m)
            });
            let upstream = match (remote, short) {
                (Some(r), Some(s)) => Some(format!("{r}/{s}")),
                _ => None,
            };
            (upstream, true)
        }
    }
}

/// Blocking. Classifies local branches safe to delete against `base`
/// (`None` => auto-resolve, OPEN #8). Read-only; touches nothing. Errors:
/// `git` (bad base / bare / no resolvable base) | `noRepo` (command layer).
pub fn find_stale_branches(workdir: &Path, base: Option<&str>) -> Result<StaleReport, AppError> {
    stale_scan(workdir, base).map(|(report, _)| report)
}

/// Shared scan core: the [`StaleReport`] plus the set of protected local
/// branch names (base identity + remote-base local counterpart + default
/// branch + branches at the base tip under an OID/tag base). The protected set
/// is what [`delete_branches`] uses for its `SkippedBase` guard.
fn stale_scan(
    workdir: &Path,
    base: Option<&str>,
) -> Result<(StaleReport, HashSet<String>), AppError> {
    let repo = open_repo_at(workdir)?;
    let base = resolve_stale_base(&repo, base)?;
    let base_oid = base.commit.id();
    let mut protected = base.protected;
    // Some(name) when HEAD is attached to a branch; None when detached/unborn.
    let current = read_head_info(&repo)?.branch_name;
    let cfg = repo.config()?;

    let mut out = Vec::new();
    for item in repo.branches(Some(git2::BranchType::Local))? {
        // Best-effort: one unreadable ref must not abort the scan (F-A7-9).
        let (branch, _) = match item {
            Ok(b) => b,
            Err(e) => {
                eprintln!("bonsai: skipping unreadable local branch ref: {}", e.message());
                continue;
            }
        };
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping local branch with non-UTF-8 name");
                continue;
            }
        };

        // Never the base (by name OR resolved identity, F-A7-1/F-A7-4), never
        // the current HEAD branch (OPEN #9).
        if name == base.name || protected.contains(&name) {
            continue;
        }
        if current.as_deref() == Some(name.as_str()) {
            continue;
        }

        // Tip oid; direct local branches always have a target — defensive skip.
        let tip = match branch.get().target() {
            Some(oid) => oid,
            None => {
                eprintln!("bonsai: skipping symbolic/targetless local branch");
                continue;
            }
        };

        // F-A7-1: under an OID/tag base (no branch identity) a local branch AT
        // the base tip IS the base — protect it instead of classifying it.
        if base.protect_tip && tip == base_oid {
            protected.insert(name);
            continue;
        }

        // merged = base contains every commit of the branch. A dangling/corrupt
        // tip must not abort the whole scan (F-A7-9) — skip that branch.
        let merged = tip == base_oid
            || match repo.graph_descendant_of(base_oid, tip) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!(
                        "bonsai: skipping branch '{name}' (unreadable tip {tip}): {}",
                        e.message()
                    );
                    continue;
                }
            };
        let (upstream, gone) = upstream_state(&cfg, &name, &branch);
        if !(merged || gone) {
            continue;
        }

        // Ahead/behind vs the BASE, best-effort (None on any lookup error).
        let (ahead, behind) = match repo.graph_ahead_behind(tip, base_oid) {
            Ok((a, b)) => (u32::try_from(a).ok(), u32::try_from(b).ok()),
            Err(_) => (None, None),
        };

        let reason = if merged {
            StaleReason::Merged
        } else {
            StaleReason::GoneUpstream
        };

        // Missing tip object (corrupt/dangling ref): skip, don't abort (F-A7-9).
        let commit = match repo.find_commit(tip) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "bonsai: skipping branch '{name}' (tip commit {tip} unreadable): {}",
                    e.message()
                );
                continue;
            }
        };
        let last_commit_summary = commit.summary().ok().flatten().unwrap_or("").to_string();
        let last_commit_author = commit.author().name().unwrap_or("").to_string();
        let last_commit_time = commit.time().seconds();

        out.push(StaleBranch {
            name,
            tip: tip.to_string(),
            last_commit_summary,
            last_commit_author,
            last_commit_time,
            reason,
            merged,
            gone_upstream: gone,
            upstream,
            ahead,
            behind,
            is_current: false,
        });
    }
    out.sort_by(|a, b| ci_cmp(&a.name, &b.name));

    Ok((
        StaleReport {
            base: base.name,
            base_oid: base_oid.to_string(),
            branches: out,
        },
        protected,
    ))
}

/// First 7 hex chars of an oid (a `to_string` is always 40 hex — safe slice).
fn short_oid(oid: git2::Oid) -> String {
    oid.to_string()[..7].to_string()
}

mod delete;

pub use delete::delete_branches;
// `recheck_tip` is exercised only by the test modules (via `super::*`), so its
// re-export is test-only.
#[cfg(test)]
pub(crate) use delete::recheck_tip;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests_detect;
#[cfg(test)]
mod tests_base;
