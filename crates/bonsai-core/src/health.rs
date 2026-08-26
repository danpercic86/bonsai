//! Repo-health dashboard core (P29 contract §2–§6). READ-ONLY: composes
//! shipped primitives (`git::{stale, status, opstate, worktree, submodule,
//! branches-style ahead/behind}` + `assets::scan_inventory`) plus the capped
//! ODB/dir scans defined here. Never writes to the repo — no ODB writes, no
//! index writes, no file creation (§6). Blocking; the command layer wraps
//! `collect_repo_health` in `spawn_blocking`.

use std::path::Path;

use crate::error::AppError;
use crate::git::opstate::{read_op_state, RepoOpState};
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;
use crate::git::stale::find_stale_branches;
use crate::git::status::read_status;
use crate::git::submodule::{list_submodules, SubmoduleStatus};
use crate::git::worktree::list_worktrees;

// ------------------------------------------------------------ caps (§5)

/// Max commits walked for count/authors/30-day stats.
pub const REVWALK_CAP: usize = 100_000;
/// Max objects visited by `odb.foreach` (header-only reads).
pub const ODB_SCAN_CAP: usize = 500_000;
/// Max entries visited by the workdir walk.
pub const WORKDIR_WALK_CAP: usize = 200_000;
/// Max entries visited by the `.git` dir walk.
pub const GITDIR_WALK_CAP: usize = 200_000;
/// Top-N size for largest blobs / largest files.
pub const TOP_N: usize = 10;
/// Warn threshold for `large_file_count` (D13).
pub const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Test-shadowable cap bundle for [`collect_stats_with_caps`] (§10.1).
#[derive(Debug, Clone, Copy)]
struct StatsCaps {
    revwalk: usize,
    odb: usize,
    workdir: usize,
    gitdir: usize,
}

const DEFAULT_CAPS: StatsCaps = StatsCaps {
    revwalk: REVWALK_CAP,
    odb: ODB_SCAN_CAP,
    workdir: WORKDIR_WALK_CAP,
    gitdir: GITDIR_WALK_CAP,
};

// ------------------------------------------------------------ wire types (§4)

/// Per-section envelope (D4). Exactly one of `data`/`error` is Some.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section<T: serde::Serialize> {
    pub data: Option<T>,
    /// Human message from `AppError::to_string()`.
    pub error: Option<String>,
    pub elapsed_ms: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoHealth {
    pub stats: Section<StatsSection>,
    pub branches: Section<BranchesSection>,
    pub working_state: Section<WorkingStateSection>,
    pub structure: Section<StructureSection>,
    /// Epoch seconds.
    pub generated_at: i64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSection {
    pub commit_count: u32,
    /// Revwalk cap hit.
    pub commit_count_capped: bool,
    /// Within the same capped walk.
    pub commits_last_30d: u32,
    pub authors_last_30d: u32,
    /// Distinct within the capped walk.
    pub authors_total: u32,
    pub object_count: u64,
    /// `odb.foreach` cap hit.
    pub object_scan_capped: bool,
    /// Top 10 desc by size (D6).
    pub largest_blobs: Vec<BlobStat>,
    pub workdir_file_count: u32,
    pub workdir_bytes: u64,
    pub workdir_scan_capped: bool,
    /// Top 10 desc; paths forward-slash repo-relative.
    pub largest_files: Vec<FileStat>,
    /// Files >= LARGE_FILE_BYTES (D13).
    pub large_file_count: u32,
    pub git_dir_bytes: u64,
    pub git_dir_scan_capped: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobStat {
    /// 40-hex oid.
    pub oid: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchesSection {
    pub local_count: u32,
    pub remote_count: u32,
    pub tag_count: u32,
    /// None = detached/unborn.
    pub current_branch: Option<String>,
    pub detached: bool,
    pub unborn: bool,
    /// vs upstream, best-effort (None on any lookup failure).
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub upstream: Option<String>,
    /// None when the stale scan failed (D9).
    pub stale: Option<StaleRollup>,
    pub stale_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleRollup {
    pub base: String,
    pub merged_count: u32,
    pub gone_upstream_count: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingStateSection {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    /// Reuses the P3c wire type verbatim.
    pub op_state: RepoOpState,
    pub stash_count: u32,
    pub has_gitignore: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureSection {
    pub submodule_count: u32,
    pub submodules_uninitialized: u32,
    pub submodules_out_of_sync: u32,
    pub submodules_modified: u32,
    /// Includes the synthesized main row.
    pub worktree_count: u32,
    pub worktrees_locked: u32,
    pub worktrees_prunable: u32,
    pub worktrees_invalid: u32,
    pub asset_drifted_count: u32,
    pub assets_in_sync: bool,
}

// ------------------------------------------------------------ entry point

/// Blocking. Collects all four health sections. Never `Err` as a whole:
/// each section collector's error is folded into its `Section.error` (D4).
pub fn collect_repo_health(workdir: &Path) -> RepoHealth {
    RepoHealth {
        stats: run_section(|| collect_stats_with_caps(workdir, DEFAULT_CAPS)),
        branches: run_section(|| collect_branches(workdir)),
        working_state: run_section(|| collect_working_state(workdir)),
        structure: run_section(|| collect_structure(workdir)),
        generated_at: epoch_now(),
    }
}

/// Runs one collector, folding its Result + wall time into a `Section` (D4).
fn run_section<T: serde::Serialize>(f: impl FnOnce() -> Result<T, AppError>) -> Section<T> {
    let start = std::time::Instant::now();
    let result = f();
    let elapsed_ms = u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);
    match result {
        Ok(data) => Section {
            data: Some(data),
            error: None,
            elapsed_ms,
        },
        Err(e) => Section {
            data: None,
            error: Some(e.to_string()),
            elapsed_ms,
        },
    }
}

fn epoch_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

// ------------------------------------------------------------ stats (§3)

mod stats;
use stats::collect_stats_with_caps;

// ------------------------------------------------------------ branches (§3)

fn collect_branches(workdir: &Path) -> Result<BranchesSection, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let count_glob = |glob: &str| -> Result<u32, AppError> {
        Ok(u32::try_from(repo.references_glob(glob)?.count()).unwrap_or(u32::MAX))
    };
    let local_count = count_glob("refs/heads/*")?;
    let remote_count = count_glob("refs/remotes/*")?;
    let tag_count = count_glob("refs/tags/*")?;

    let head = read_head_info(&repo)?;
    let current_branch = head.branch_name.clone();

    // Ahead/behind vs upstream, best-effort — mirrors branches.rs `list_refs`.
    let (mut ahead, mut behind, mut upstream) = (None, None, None);
    if let Some(name) = current_branch.as_deref() {
        if !head.unborn {
            if let Ok(branch) = repo.find_branch(name, git2::BranchType::Local) {
                if let Ok(up) = branch.upstream() {
                    upstream = up.name().ok().flatten().map(str::to_string);
                    if let (Some(local_oid), Some(up_oid)) =
                        (branch.get().target(), up.get().target())
                    {
                        if let Ok((a, b)) = repo.graph_ahead_behind(local_oid, up_oid) {
                            ahead = u32::try_from(a).ok();
                            behind = u32::try_from(b).ok();
                        }
                    }
                }
            }
        }
    }

    // Stale rollup is a SUB-metric (D9): its failure never fails the section.
    let (stale, stale_error) = match find_stale_branches(workdir, None) {
        Ok(report) => {
            let merged = report.branches.iter().filter(|b| b.merged).count();
            let gone = report.branches.iter().filter(|b| b.gone_upstream).count();
            (
                Some(StaleRollup {
                    base: report.base,
                    merged_count: u32::try_from(merged).unwrap_or(u32::MAX),
                    gone_upstream_count: u32::try_from(gone).unwrap_or(u32::MAX),
                }),
                None,
            )
        }
        Err(e) => (None, Some(e.to_string())),
    };

    Ok(BranchesSection {
        local_count,
        remote_count,
        tag_count,
        current_branch,
        detached: head.detached,
        unborn: head.unborn,
        ahead,
        behind,
        upstream,
        stale,
        stale_error,
    })
}

// ------------------------------------------------------------ working state (§3)

fn collect_working_state(workdir: &Path) -> Result<WorkingStateSection, AppError> {
    let snapshot = read_status(workdir)?;
    let op_state = read_op_state(workdir)?;

    // stash_foreach takes &mut but performs no writes (§6).
    let mut repo = open_workdir_repo(workdir)?;
    let mut stash_count: u32 = 0;
    repo.stash_foreach(|_, _, _| {
        stash_count = stash_count.saturating_add(1);
        true
    })?;

    Ok(WorkingStateSection {
        staged: u32::try_from(snapshot.staged.len()).unwrap_or(u32::MAX),
        unstaged: u32::try_from(snapshot.unstaged.len()).unwrap_or(u32::MAX),
        untracked: u32::try_from(snapshot.untracked.len()).unwrap_or(u32::MAX),
        conflicted: u32::try_from(snapshot.conflicted.len()).unwrap_or(u32::MAX),
        op_state,
        stash_count,
        has_gitignore: workdir.join(".gitignore").is_file(),
    })
}

// ------------------------------------------------------------ structure (§3)

fn collect_structure(workdir: &Path) -> Result<StructureSection, AppError> {
    let submodules = list_submodules(workdir)?;
    let sub_count = |s: SubmoduleStatus| submodules.iter().filter(|m| m.status == s).count();
    let submodules_uninitialized = sub_count(SubmoduleStatus::Uninitialized);
    let submodules_out_of_sync = sub_count(SubmoduleStatus::OutOfSync);
    let submodules_modified = sub_count(SubmoduleStatus::ModifiedWorkdir);

    let worktrees = list_worktrees(workdir)?;
    let worktrees_locked = worktrees.iter().filter(|w| w.locked).count();
    let worktrees_prunable = worktrees.iter().filter(|w| w.prunable).count();
    let worktrees_invalid = worktrees.iter().filter(|w| !w.valid).count();

    let inventory = crate::assets::inventory::scan_inventory(workdir, None)?;
    let drift = &inventory.drift;
    let asset_drifted_count = drift
        .entries
        .iter()
        .filter(|e| e.comparable && e.exists && !e.in_sync)
        .count();

    let c = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Ok(StructureSection {
        submodule_count: c(submodules.len()),
        submodules_uninitialized: c(submodules_uninitialized),
        submodules_out_of_sync: c(submodules_out_of_sync),
        submodules_modified: c(submodules_modified),
        worktree_count: c(worktrees.len()),
        worktrees_locked: c(worktrees_locked),
        worktrees_prunable: c(worktrees_prunable),
        worktrees_invalid: c(worktrees_invalid),
        asset_drifted_count: c(asset_drifted_count),
        assets_in_sync: drift.in_sync,
    })
}

// ============================================================ tests (§10.1)

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests_stats;
#[cfg(test)]
mod tests_sections;
