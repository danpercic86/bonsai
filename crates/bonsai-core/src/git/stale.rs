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

/// F-A7-3 (TOCTOU guard): re-read the branch tip at delete time. Returns
/// `Some(Failed row)` when the tip no longer matches the freshly-scanned
/// `expected` oid (the branch moved between scan and delete — do NOT delete),
/// `None` when it is unchanged and safe to delete.
fn recheck_tip(
    branch: &git2::Branch,
    name: &str,
    expected: git2::Oid,
) -> Option<BranchDeleteResult> {
    match branch.get().target() {
        Some(now) if now == expected => None,
        Some(now) => Some(BranchDeleteResult {
            name: name.to_string(),
            status: BranchDeleteStatus::Failed,
            message: Some(format!(
                "tip moved since scan ({} -> {}); not deleted — re-run the scan",
                short_oid(expected),
                short_oid(now)
            )),
        }),
        None => Some(BranchDeleteResult {
            name: name.to_string(),
            status: BranchDeleteStatus::Failed,
            message: Some("tip changed since scan (no longer a direct ref); not deleted".to_string()),
        }),
    }
}

/// Blocking. Deletes each caller-supplied name that is STILL safe, refusing the
/// current branch, the base branch, and anything not in a freshly-recomputed
/// stale set (defense-in-depth — NEVER trusts the client). Deletes directly via
/// git2 `Branch::delete()` (OPEN #10 — `branches::delete_branch`'s merged-into-HEAD
/// guard would wrongly block a branch merged into the base while a different
/// branch is checked out). Returns a per-branch result; a per-branch failure is
/// reported, NEVER a whole-call error. `base` mirrors `find_stale_branches` so
/// the safe set is recomputed against the same base. Each branch's tip is
/// re-read immediately before deletion and the delete refused if it moved
/// since the recompute (F-A7-3). Errors (whole-call): `git` (bad base / bare)
/// | `noRepo` (command layer).
pub fn delete_branches(
    workdir: &Path,
    names: &[String],
    base: Option<&str>,
) -> Result<Vec<BranchDeleteResult>, AppError> {
    // Recompute the safe set + base identity from scratch — the load-bearing
    // guard. Carry each safe branch's SCANNED tip oid into the delete loop so
    // a tip that moves between scan and delete is refused (F-A7-3).
    let (report, protected) = stale_scan(workdir, base)?;
    let safe: HashMap<&str, git2::Oid> = report
        .branches
        .iter()
        .filter_map(|b| git2::Oid::from_str(&b.tip).ok().map(|oid| (b.name.as_str(), oid)))
        .collect();

    let repo = open_repo_at(workdir)?;
    let current = read_head_info(&repo)?.branch_name;

    let mut results = Vec::with_capacity(names.len());
    for name in names {
        if current.as_deref() == Some(name.as_str()) {
            results.push(BranchDeleteResult {
                name: name.clone(),
                status: BranchDeleteStatus::SkippedCurrent,
                message: Some("checked-out branch".to_string()),
            });
            continue;
        }
        // Base by name OR resolved identity (F-A7-1/F-A7-4) OR default branch.
        if name == &report.base || protected.contains(name.as_str()) {
            results.push(BranchDeleteResult {
                name: name.clone(),
                status: BranchDeleteStatus::SkippedBase,
                message: Some("base/default branch".to_string()),
            });
            continue;
        }
        let Some(&expected_tip) = safe.get(name.as_str()) else {
            results.push(BranchDeleteResult {
                name: name.clone(),
                status: BranchDeleteStatus::SkippedNotStale,
                message: Some("not detected as stale".to_string()),
            });
            continue;
        };

        match repo.find_branch(name, git2::BranchType::Local) {
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                results.push(BranchDeleteResult {
                    name: name.clone(),
                    status: BranchDeleteStatus::SkippedNotFound,
                    message: Some("not found".to_string()),
                });
            }
            Err(e) => {
                results.push(BranchDeleteResult {
                    name: name.clone(),
                    status: BranchDeleteStatus::Failed,
                    message: Some(e.message().to_string()),
                });
            }
            Ok(mut branch) => {
                // F-A7-3: refuse if the tip moved since the scan above.
                if let Some(row) = recheck_tip(&branch, name, expected_tip) {
                    results.push(row);
                    continue;
                }
                match branch.delete() {
                    Ok(()) => results.push(BranchDeleteResult {
                        name: name.clone(),
                        status: BranchDeleteStatus::Deleted,
                        // F-A7-5: record the deleted tip for recovery.
                        message: Some(format!("was at {}", short_oid(expected_tip))),
                    }),
                    Err(e) => results.push(BranchDeleteResult {
                        name: name.clone(),
                        status: BranchDeleteStatus::Failed,
                        message: Some(e.message().to_string()),
                    }),
                }
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    //! P25c (contract §9.1(6–9) + §9.2 CLI oracle). Fixtures built with git2 in
    //! a scratch `TempDir` under `D:\Temp\bonsai-scratch` (deterministic
    //! identity, `core.autocrlf=false`), mirroring `branches.rs`. The load-bearing
    //! oracle (§9.2) shells out to the real `git branch --merged` and skips when
    //! `git` is absent.

    use super::*;
    use std::collections::BTreeSet;
    use std::process::Command;

    /// Init a scratch repo with a deterministic identity + autocrlf off
    /// (== branches.rs `cbh_init`). Pins the initial branch to "main" via
    /// `initial_head` rather than relying on `init.defaultBranch` — libgit2
    /// falls back to "master" when that config is unset, which these fixtures
    /// (and the base-resolution assertions below) assume is "main".
    fn init(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
            .expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    /// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
    /// Returns the new HEAD oid.
    fn commit(dir: &Path, msg: &str, files: &[(&str, &str)]) -> git2::Oid {
        use crate::git::stage::stage_paths;
        for (name, content) in files {
            std::fs::write(dir.join(name), content).expect("write file");
        }
        stage_paths(
            dir,
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
        let repo = git2::Repository::open(dir).expect("open");
        let oid = repo.head().expect("HEAD").peel_to_commit().expect("peel").id();
        oid
    }

    /// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or
    /// the worktree (== branches.rs `cbh_commit_on_ref`). Builds a divergent tip.
    fn commit_on_ref(
        repo: &git2::Repository,
        refname: &str,
        parent_oid: git2::Oid,
        files: &[(&str, &str)],
        msg: &str,
    ) -> git2::Oid {
        let parent = repo.find_commit(parent_oid).expect("parent commit");
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut tb = repo
            .treebuilder(Some(&parent.tree().expect("parent tree")))
            .expect("treebuilder");
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
        repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[&parent])
            .expect("commit on ref")
    }

    /// Create a local branch `name` at `oid` (no checkout).
    fn branch_at(repo: &git2::Repository, name: &str, oid: git2::Oid) {
        let commit = repo.find_commit(oid).expect("find commit");
        repo.branch(name, &commit, false).expect("create branch");
    }

    /// True when local branch `name` still exists.
    fn branch_exists(dir: &Path, name: &str) -> bool {
        let repo = git2::Repository::open(dir).expect("open");
        let exists = repo.find_branch(name, git2::BranchType::Local).is_ok();
        exists
    }

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    // ------------------------------------------------------- §9.1(6) wire shapes

    /// `serde_json` asserts camelCase keys and bare-string enum encodings.
    #[test]
    fn wire_shapes_serialize_camelcase() {
        let sb = StaleBranch {
            name: "feature/x".to_string(),
            tip: "a".repeat(40),
            last_commit_summary: "do a thing".to_string(),
            last_commit_author: "Test User".to_string(),
            last_commit_time: 1_700_000_000,
            reason: StaleReason::GoneUpstream,
            merged: false,
            gone_upstream: true,
            upstream: Some("origin/feature/x".to_string()),
            ahead: Some(3),
            behind: Some(1),
            is_current: false,
        };
        let report = StaleReport {
            base: "main".to_string(),
            base_oid: "b".repeat(40),
            branches: vec![sb],
        };
        let v = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(v["base"], "main");
        assert_eq!(v["baseOid"], "b".repeat(40));
        let b0 = &v["branches"][0];
        assert_eq!(b0["lastCommitSummary"], "do a thing");
        assert_eq!(b0["lastCommitAuthor"], "Test User");
        assert_eq!(b0["lastCommitTime"], 1_700_000_000_i64);
        assert_eq!(b0["goneUpstream"], true);
        assert_eq!(b0["isCurrent"], false);
        // Field-less enum → bare camelCase string.
        assert_eq!(b0["reason"], "goneUpstream");

        let del = BranchDeleteResult {
            name: "feature/x".to_string(),
            status: BranchDeleteStatus::SkippedCurrent,
            message: Some("checked-out branch".to_string()),
        };
        let dv = serde_json::to_value(&del).expect("serialize delete result");
        assert_eq!(dv["name"], "feature/x");
        assert_eq!(dv["status"], "skippedCurrent");
        assert_eq!(dv["message"], "checked-out branch");
        assert_eq!(
            serde_json::to_value(StaleReason::Merged).expect("reason"),
            "merged"
        );
        assert_eq!(
            serde_json::to_value(BranchDeleteStatus::Deleted).expect("status"),
            "deleted"
        );
    }

    // ------------------------------------------------------- §9.1(7) merged

    /// A branch fully merged into base is listed `merged` with `ahead:0`; a
    /// branch with a unique commit is not; the base and the current HEAD branch
    /// are never listed.
    #[test]
    fn merged_detection() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        let _c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip

        // Merged: tip C1 is an ancestor of main → main descends from it.
        branch_at(&repo, "feat-merged", c1);
        // Unmerged: a unique commit off C0 that main never sees.
        branch_at(&repo, "wip", c0);
        commit_on_ref(&repo, "refs/heads/wip", c0, &[("w.txt", "w\n")], "wip work");
        // A merged branch that we CHECK OUT → excluded because it is current.
        branch_at(&repo, "cur-merged", c1);
        crate::git::branches::checkout_branch(d, "cur-merged").expect("checkout");

        let report = find_stale_branches(d, Some("main")).expect("classify");
        assert_eq!(report.base, "main");

        let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"feat-merged"), "merged branch listed: {names:?}");
        assert!(!names.contains(&"wip"), "unmerged branch NOT listed: {names:?}");
        assert!(!names.contains(&"main"), "base never listed: {names:?}");
        assert!(
            !names.contains(&"cur-merged"),
            "current HEAD branch never listed: {names:?}"
        );

        let feat = report
            .branches
            .iter()
            .find(|b| b.name == "feat-merged")
            .expect("feat-merged present");
        assert_eq!(feat.reason, StaleReason::Merged);
        assert!(feat.merged);
        assert_eq!(feat.ahead, Some(0), "merged → 0 commits ahead of base");
    }

    // ------------------------------------------------------- §9.1(8) gone upstream

    /// A branch with a configured upstream whose remote-tracking ref is missing
    /// is listed `goneUpstream` (merged:false); a branch with a live upstream is
    /// not gone (and, being unmerged, not listed at all).
    #[test]
    fn gone_upstream_detection() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let _c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip

        // A remote so upstream mapping (refspec → refs/remotes/origin/*) resolves.
        repo.remote("origin", "https://example.invalid/x.git")
            .expect("add remote");

        // `gone`: unique commit (so NOT merged) + configured upstream, but no
        // matching refs/remotes/origin/gone → upstream() errs → gone.
        branch_at(&repo, "gone", c0);
        commit_on_ref(&repo, "refs/heads/gone", c0, &[("g.txt", "g\n")], "gone work");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("branch.gone.remote", "origin").expect("remote");
            cfg.set_str("branch.gone.merge", "refs/heads/gone")
                .expect("merge");
        }

        // `live`: unique commit + a present remote-tracking ref → upstream Ok.
        let live_tip =
            commit_on_ref(&repo, "refs/heads/live", c0, &[("l.txt", "l\n")], "live work");
        repo.reference(
            "refs/remotes/origin/live",
            live_tip,
            true,
            "seed remote-tracking",
        )
        .expect("remote ref");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("branch.live.remote", "origin").expect("remote");
            cfg.set_str("branch.live.merge", "refs/heads/live")
                .expect("merge");
        }

        let report = find_stale_branches(d, Some("main")).expect("classify");
        let gone = report
            .branches
            .iter()
            .find(|b| b.name == "gone")
            .expect("gone listed");
        assert_eq!(gone.reason, StaleReason::GoneUpstream);
        assert!(gone.gone_upstream, "gone flag set");
        assert!(!gone.merged, "gone branch is not merged");
        assert_eq!(gone.upstream.as_deref(), Some("origin/gone"));

        assert!(
            !report.branches.iter().any(|b| b.name == "live"),
            "branch with a live upstream (and unmerged) is not listed"
        );
    }

    // ----------------------------------------------- §9.1(9) delete-branches safety

    /// A set mixing a stale name, the current branch, the base, a non-stale
    /// branch, and a missing name yields the matching statuses; ONLY the stale
    /// branch is actually gone afterward; a fabricated non-stale name is NEVER
    /// deleted (defense-in-depth — the server ignores the caller's list).
    #[test]
    fn delete_branches_safety() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        let _c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip, HEAD=main (current+base)

        branch_at(&repo, "merged-stale", c1); // merged → safe
        branch_at(&repo, "not-stale", c0);
        commit_on_ref(&repo, "refs/heads/not-stale", c0, &[("n.txt", "n\n")], "unique");

        // Sanity: the classifier sees exactly `merged-stale` as safe.
        let report = find_stale_branches(d, Some("main")).expect("classify");
        let safe: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(safe, vec!["merged-stale"], "only merged-stale is safe");

        let names = vec![
            "merged-stale".to_string(), // Deleted
            "main".to_string(),         // SkippedCurrent (main is BOTH current HEAD and base;
            // the current check runs first per §4.3)
            "not-stale".to_string(), // SkippedNotStale
            "ghost".to_string(),     // SkippedNotStale (not even a branch, never in safe set)
        ];
        let results = delete_branches(d, &names, Some("main")).expect("delete");

        let status_of = |n: &str| {
            results
                .iter()
                .find(|r| r.name == n)
                .map(|r| r.status)
                .unwrap_or_else(|| panic!("no result for {n}"))
        };
        assert_eq!(status_of("merged-stale"), BranchDeleteStatus::Deleted);
        // `main` is BOTH current and base; the current check runs first (§4.3 order).
        assert_eq!(status_of("main"), BranchDeleteStatus::SkippedCurrent);
        assert_eq!(status_of("not-stale"), BranchDeleteStatus::SkippedNotStale);
        assert_eq!(status_of("ghost"), BranchDeleteStatus::SkippedNotStale);

        // F-A7-5: the Deleted row records the deleted tip for recovery.
        let deleted_row = results
            .iter()
            .find(|r| r.name == "merged-stale")
            .expect("row present");
        assert!(
            deleted_row.message.as_deref().unwrap_or("").starts_with("was at "),
            "Deleted row must carry 'was at <short-oid>', got {:?}",
            deleted_row.message
        );

        // Only the stale branch is gone; every other ref survives.
        assert!(!branch_exists(d, "merged-stale"), "stale branch deleted");
        assert!(branch_exists(d, "not-stale"), "non-stale branch untouched");
        assert!(branch_exists(d, "main"), "base branch untouched");
    }

    /// The current HEAD branch is refused even if the caller forces it, and a
    /// non-stale name is never deleted even when explicitly listed.
    #[test]
    fn delete_branches_refuses_current() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip

        // Check out a merged branch so current != base.
        branch_at(&repo, "topic", c1);
        crate::git::branches::checkout_branch(d, "topic").expect("checkout topic");
        // A stale (merged) branch to prove the batch still deletes the safe one.
        branch_at(&repo, "old", c0);

        // current=topic, base=main → `main` exercises the distinct SkippedBase arm.
        let results = delete_branches(
            d,
            &["topic".to_string(), "main".to_string(), "old".to_string()],
            Some("main"),
        )
        .expect("delete");

        let status_of = |n: &str| {
            results
                .iter()
                .find(|r| r.name == n)
                .map(|r| r.status)
                .expect("result present")
        };
        assert_eq!(status_of("topic"), BranchDeleteStatus::SkippedCurrent);
        assert_eq!(status_of("main"), BranchDeleteStatus::SkippedBase);
        assert_eq!(status_of("old"), BranchDeleteStatus::Deleted);
        assert!(branch_exists(d, "topic"), "current branch never deleted");
        assert!(branch_exists(d, "main"), "base branch never deleted");
        assert!(!branch_exists(d, "old"), "merged branch deleted");
    }

    // --------------------------------------------------- base resolution ordering

    /// With no explicit base and no origin/HEAD, resolution falls to local
    /// `main`; a repo with neither main/master nor an attached HEAD errors.
    #[test]
    fn base_resolution_falls_to_main() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);
        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        branch_at(&repo, "feat", c0);

        // main exists; no origin/HEAD → base resolves to "main".
        let report = find_stale_branches(d, None).expect("classify");
        assert_eq!(report.base, "main");
    }

    /// Explicit base wins over everything; a bad base is a whole-call `git` error.
    #[test]
    fn base_resolution_explicit_and_bad() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);
        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let _c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        branch_at(&repo, "release", c0);

        let report = find_stale_branches(d, Some("release")).expect("classify");
        assert_eq!(report.base, "release");

        match find_stale_branches(d, Some("no-such-ref")) {
            Err(AppError::Git(_)) => {}
            other => panic!("bad base must be Git error, got {other:?}"),
        }
    }

    // --------------------------------------------- F-A7-1/4 base identity guard

    /// The base given as `refs/heads/main`, a bare OID, or a tag at the tip
    /// must all protect `main` (F-A7-1). A twin branch AT the base tip is only
    /// protected under the OID/tag forms (no branch identity) — under
    /// `refs/heads/main` it stays a normal merged candidate.
    #[test]
    fn base_identity_protects_main_for_refname_oid_and_tag() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let _c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        let c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip

        branch_at(&repo, "dead", c1); // merged, below the tip
        branch_at(&repo, "twin", c2); // merged, AT the base tip
        // HEAD off main so the base guard (not the current guard) is exercised.
        branch_at(&repo, "topic", c2);
        crate::git::branches::checkout_branch(d, "topic").expect("checkout");

        let tip_commit = repo.find_commit(c2).expect("tip");
        repo.tag_lightweight("release", tip_commit.as_object(), false)
            .expect("tag");

        let oid_spec = c2.to_string();
        for (spec, twin_protected) in [
            ("refs/heads/main", false),
            (oid_spec.as_str(), true),
            ("release", true),
        ] {
            let report = find_stale_branches(d, Some(spec)).expect("classify");
            let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
            assert!(!names.contains(&"main"), "base {spec}: main never listed: {names:?}");
            assert!(names.contains(&"dead"), "base {spec}: dead still listed: {names:?}");
            assert_eq!(
                !names.contains(&"twin"),
                twin_protected,
                "base {spec}: twin protection mismatch: {names:?}"
            );

            let results =
                delete_branches(d, &["main".to_string()], Some(spec)).expect("delete");
            assert_eq!(
                results[0].status,
                BranchDeleteStatus::SkippedBase,
                "base {spec}: deleting main must be SkippedBase, got {results:?}"
            );
            assert!(branch_exists(d, "main"), "base {spec}: main survives");
        }
    }

    /// A remote-tracking base (`origin/main`) protects the LOCAL `main`
    /// (F-A7-4) even when it is fully merged relative to that base.
    #[test]
    fn remote_base_protects_local_counterpart() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip
        repo.reference("refs/remotes/origin/main", c1, true, "seed")
            .expect("remote ref");
        branch_at(&repo, "dead", c0);
        branch_at(&repo, "topic", c1);
        crate::git::branches::checkout_branch(d, "topic").expect("checkout");

        let report = find_stale_branches(d, Some("origin/main")).expect("classify");
        let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(!names.contains(&"main"), "local counterpart never listed: {names:?}");
        assert!(names.contains(&"dead"), "other merged branches still listed");

        let results = delete_branches(d, &["main".to_string()], Some("origin/main"))
            .expect("delete");
        assert_eq!(results[0].status, BranchDeleteStatus::SkippedBase);
        assert!(branch_exists(d, "main"), "local main survives a remote base");
    }

    /// The repo's default branch (origin/HEAD target) is never auto-classified
    /// stale, whatever base the caller reviews against (F-A7-4).
    #[test]
    fn default_branch_never_auto_classified() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let _c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        let _c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip, HEAD=main

        branch_at(&repo, "dev", c1); // merged — but it is the DEFAULT branch
        branch_at(&repo, "dead", c1); // merged — an ordinary candidate
        repo.reference("refs/remotes/origin/dev", c1, true, "seed")
            .expect("remote dev");
        repo.reference_symbolic(
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/dev",
            true,
            "seed origin/HEAD",
        )
        .expect("origin/HEAD");

        let report = find_stale_branches(d, Some("main")).expect("classify");
        let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(!names.contains(&"dev"), "default branch never listed: {names:?}");
        assert!(names.contains(&"dead"), "ordinary merged branch still listed");

        let results = delete_branches(d, &["dev".to_string()], Some("main")).expect("delete");
        assert_eq!(results[0].status, BranchDeleteStatus::SkippedBase);
        assert!(branch_exists(d, "dev"), "default branch survives");
    }

    // ------------------------------------------------- F-A7-3 tip-moved guard

    /// The delete-time tip recheck: unchanged tip → proceed (None); a moved
    /// tip → a Failed row naming both oids, never a delete.
    #[test]
    fn recheck_tip_detects_moved_tip() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        branch_at(&repo, "b", c1);
        let branch = repo
            .find_branch("b", git2::BranchType::Local)
            .expect("find branch");

        assert!(
            recheck_tip(&branch, "b", c1).is_none(),
            "unchanged tip → safe to delete"
        );
        let row = recheck_tip(&branch, "b", c0).expect("moved tip must be refused");
        assert_eq!(row.status, BranchDeleteStatus::Failed);
        let msg = row.message.as_deref().unwrap_or("");
        assert!(msg.contains("tip moved"), "message names the move: {msg}");
        assert!(
            msg.contains(&c0.to_string()[..7]) && msg.contains(&c1.to_string()[..7]),
            "message carries both short oids: {msg}"
        );
    }

    // -------------------------------------------- F-A7-9 dangling-ref skipping

    /// One dangling branch ref (loose ref file pointing at a nonexistent
    /// object) must not abort the scan or the delete batch — it is skipped and
    /// everything else still works.
    #[test]
    fn dangling_branch_ref_is_skipped_not_fatal() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let _c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip
        branch_at(&repo, "dead", c0); // merged → stale

        // A loose ref to an object that does not exist in the odb.
        std::fs::write(
            d.join(".git").join("refs").join("heads").join("dangling"),
            format!("{}\n", "a".repeat(40)),
        )
        .expect("write dangling ref");

        let report = find_stale_branches(d, Some("main")).expect("scan survives");
        let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"dead"), "healthy stale branch listed: {names:?}");
        assert!(!names.contains(&"dangling"), "dangling ref never classified");

        let results = delete_branches(
            d,
            &["dead".to_string(), "dangling".to_string()],
            Some("main"),
        )
        .expect("delete survives");
        let status_of = |n: &str| {
            results
                .iter()
                .find(|r| r.name == n)
                .map(|r| r.status)
                .expect("row present")
        };
        assert_eq!(status_of("dead"), BranchDeleteStatus::Deleted);
        assert_eq!(status_of("dangling"), BranchDeleteStatus::SkippedNotStale);
        assert!(!branch_exists(d, "dead"));
    }

    // --------------------------------------------------- §9.2 CLI oracle (git)

    /// LOAD-BEARING: the merged set from `find_stale_branches(base="main")`
    /// equals `git branch --merged main` minus `main` and the current branch.
    /// Skips when `git` is absent (git2-only paths still cover detection).
    #[test]
    fn merged_matches_git_branch_merged_cli() {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);

        // main: C0 -> C1 -> C2.
        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        let _c2 = commit(d, "C2", &[("c.txt", "c\n")]);

        // Two branches fully merged into main (ancestors of the tip).
        branch_at(&repo, "merged-1", c0);
        branch_at(&repo, "merged-2", c1);
        // Two branches with unique commits → NOT merged.
        branch_at(&repo, "topic-a", c0);
        commit_on_ref(&repo, "refs/heads/topic-a", c0, &[("ta.txt", "x\n")], "ta");
        branch_at(&repo, "topic-b", c1);
        commit_on_ref(&repo, "refs/heads/topic-b", c1, &[("tb.txt", "y\n")], "tb");

        // git branch --merged main → set, minus main and the current branch.
        let out = Command::new("git")
            .args(["branch", "--merged", "main", "--format=%(refname:short)"])
            .current_dir(d)
            .output()
            .expect("git branch --merged");
        assert!(out.status.success(), "git branch --merged failed");
        let current = read_head_info(&repo).ok().and_then(|h| h.branch_name);
        let cli_merged: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().trim_start_matches('*').trim().to_string())
            .filter(|n| !n.is_empty() && n != "main" && Some(n) != current.as_ref())
            .collect();

        let report = find_stale_branches(d, Some("main")).expect("classify");
        let ours_merged: BTreeSet<String> = report
            .branches
            .iter()
            .filter(|b| b.reason == StaleReason::Merged)
            .map(|b| b.name.clone())
            .collect();

        assert_eq!(
            ours_merged, cli_merged,
            "our merged set must equal `git branch --merged main` (minus main + current)"
        );
    }
}
