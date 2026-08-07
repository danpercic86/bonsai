//! Repo-health dashboard core (P29 contract §2–§6). READ-ONLY: composes
//! shipped primitives (`git::{stale, status, opstate, worktree, submodule,
//! branches-style ahead/behind}` + `assets::scan_inventory`) plus the capped
//! ODB/dir scans defined here. Never writes to the repo — no ODB writes, no
//! index writes, no file creation (§6). Blocking; the command layer wraps
//! `collect_repo_health` in `spawn_blocking`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
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

/// Cap-parameterized stats collector (§10.1: caps shadowable in tests).
fn collect_stats_with_caps(workdir: &Path, caps: StatsCaps) -> Result<StatsSection, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // --- one revwalk from HEAD (plain push_head; unborn HEAD → zero counts).
    let mut commit_count: u32 = 0;
    let mut commit_count_capped = false;
    let mut commits_last_30d: u32 = 0;
    let mut authors_30d: HashSet<String> = HashSet::new();
    let mut authors_all: HashSet<String> = HashSet::new();
    let cutoff_30d = epoch_now() - 30 * 86_400;

    let mut revwalk = repo.revwalk()?;
    if revwalk.push_head().is_ok() {
        for oid in revwalk {
            let oid = oid?;
            if (commit_count as usize) >= caps.revwalk {
                commit_count_capped = true;
                break;
            }
            commit_count += 1;
            // Degrade gracefully on an unreadable/corrupt commit object: the
            // commit is still counted, only author/date extraction is skipped
            // (P29a review carry-forward — one bad object must not sink the
            // whole stats section).
            if let Ok(commit) = repo.find_commit(oid) {
                let email = commit
                    .author()
                    .email()
                    .unwrap_or("(non-utf8)")
                    .to_string();
                if commit.time().seconds() >= cutoff_30d {
                    commits_last_30d += 1;
                    authors_30d.insert(email.clone());
                }
                authors_all.insert(email);
            }
        }
    }

    // --- ODB scan: object count + top-10 blobs, header reads ONLY (D6).
    let odb = repo.odb()?;
    let mut object_count: u64 = 0;
    let mut object_scan_capped = false;
    // Min-heap of size TOP_N (Reverse => smallest on top), O(n log N).
    let mut blob_heap: BinaryHeap<Reverse<(u64, String)>> = BinaryHeap::with_capacity(TOP_N + 1);
    let scan = odb.foreach(|oid| {
        if object_count >= caps.odb as u64 {
            object_scan_capped = true;
            return false; // early-stop the foreach
        }
        object_count += 1;
        if let Ok((size, kind)) = odb.read_header(*oid) {
            if kind == git2::ObjectType::Blob {
                push_top_n(&mut blob_heap, (size as u64, oid.to_string()));
            }
        }
        true
    });
    // The cap-stop surfaces as a callback error from libgit2 — swallow only then.
    if let Err(e) = scan {
        if !object_scan_capped {
            return Err(e.into());
        }
    }
    let largest_blobs: Vec<BlobStat> = drain_top_n(blob_heap)
        .into_iter()
        .map(|(size, oid)| BlobStat { oid, size })
        .collect();

    // --- workdir walk (skip .git, never follow symlinks; gitignore-aware:
    // ignored dirs are pruned and ignored files skipped so build/dep noise
    // like node_modules/target/dist does not inflate the stats).
    let wd = walk_dir(workdir, true, caps.workdir, Some(workdir), Some(&repo))?;
    let largest_files: Vec<FileStat> = drain_top_n(wd.largest)
        .into_iter()
        .map(|(size, path)| FileStat { path, size })
        .collect();

    // --- .git dir byte size (capped walk; no .git skip, no per-file stats,
    // UNFILTERED — it sizes git internals, so no ignore predicate).
    let gd = walk_dir(repo.path(), false, caps.gitdir, None, None)?;

    Ok(StatsSection {
        commit_count,
        commit_count_capped,
        commits_last_30d,
        authors_last_30d: u32::try_from(authors_30d.len()).unwrap_or(u32::MAX),
        authors_total: u32::try_from(authors_all.len()).unwrap_or(u32::MAX),
        object_count,
        object_scan_capped,
        largest_blobs,
        workdir_file_count: wd.file_count,
        workdir_bytes: wd.bytes,
        workdir_scan_capped: wd.capped,
        largest_files,
        large_file_count: wd.large_count,
        git_dir_bytes: gd.bytes,
        git_dir_scan_capped: gd.capped,
    })
}

/// Keeps only the TOP_N largest items in the min-heap.
fn push_top_n<K: Ord>(heap: &mut BinaryHeap<Reverse<K>>, item: K) {
    heap.push(Reverse(item));
    if heap.len() > TOP_N {
        heap.pop(); // drop the smallest
    }
}

/// Heap → Vec sorted descending by the key.
fn drain_top_n<K: Ord>(heap: BinaryHeap<Reverse<K>>) -> Vec<K> {
    let mut v: Vec<K> = heap.into_iter().map(|r| r.0).collect();
    v.sort_by(|a, b| b.cmp(a));
    v
}

/// Result of one capped directory walk.
struct WalkResult {
    /// Files only (dirs are visited but not counted here).
    file_count: u32,
    bytes: u64,
    capped: bool,
    /// (size, fwd-slash path relative to `rel_root`) — only when tracking.
    largest: BinaryHeap<Reverse<(u64, String)>>,
    large_count: u32,
}

/// Iterative, symlink-refusing walk. Counts every directory ENTRY visited
/// against `cap`. `skip_git` skips any directory named `.git`. When
/// `rel_root` is Some, per-file top-N + large-file tracking is enabled.
/// When `repo` is Some, the walk is gitignore-aware: ignored directories are
/// pruned (whole subtree skipped) and ignored files are not counted, via
/// `repo.is_path_ignored` on the fwd-slash `rel_root`-relative path. An ignore
/// lookup that errors is treated as NOT ignored (defensive — include it).
/// Unreadable subdirectories are skipped (read-only robustness), only a
/// failure to read `root` itself errors.
fn walk_dir(
    root: &Path,
    skip_git: bool,
    cap: usize,
    rel_root: Option<&Path>,
    repo: Option<&git2::Repository>,
) -> Result<WalkResult, AppError> {
    let mut out = WalkResult {
        file_count: 0,
        bytes: 0,
        capped: false,
        largest: BinaryHeap::with_capacity(TOP_N + 1),
        large_count: 0,
    };
    let mut visited: usize = 0;
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    let mut first = true;

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if first => return Err(AppError::Io(e.to_string())),
            Err(_) => continue, // unreadable subdir: skip, stay read-only
        };
        first = false;
        for entry in entries.flatten() {
            if visited >= cap {
                out.capped = true;
                return Ok(out);
            }
            visited += 1;
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_symlink() {
                continue; // never follow symlinks/junctions
            }
            // Fwd-slash `rel_root`-relative path (used for ignore checks and
            // top-N). Only when a base is present (None for the .git walk).
            let rel = rel_root.map(|base| rel_fwd(&entry.path(), base));
            if ft.is_dir() {
                if skip_git && entry.file_name() == ".git" {
                    continue;
                }
                // Prune whole ignored subtrees (node_modules/target/dist/…).
                if let (Some(r), Some(rel)) = (repo, rel.as_deref()) {
                    if r.is_path_ignored(rel).unwrap_or(false) {
                        continue;
                    }
                }
                stack.push(entry.path());
            } else if ft.is_file() {
                // Skip gitignored files so they do not inflate the stats.
                if let (Some(r), Some(rel)) = (repo, rel.as_deref()) {
                    if r.is_path_ignored(rel).unwrap_or(false) {
                        continue;
                    }
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.file_count = out.file_count.saturating_add(1);
                out.bytes = out.bytes.saturating_add(size);
                if let Some(rel) = rel {
                    if size >= LARGE_FILE_BYTES {
                        out.large_count = out.large_count.saturating_add(1);
                    }
                    push_top_n(&mut out.largest, (size, rel));
                }
            }
        }
    }
    Ok(out)
}

/// Fwd-slash path of `path` relative to `base` (falls back to the full path
/// if `path` is not under `base`). Backslashes are normalized so libgit2's
/// ignore matcher and the reported `largestFiles` paths are consistent.
fn rel_fwd(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

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
mod tests {
    //! P29a AI gate. Fixtures under `crate::testutil::scratch_dir()`
    //! (D:\Temp\bonsai-scratch), deterministic identity, autocrlf off —
    //! mirroring `stale.rs`. CLI oracles skip when `git` is absent.

    use super::*;
    use std::process::Command;

    /// Pins the initial branch to "main" via `initial_head` rather than
    /// relying on `init.defaultBranch` — libgit2 falls back to "master" when
    /// that config is unset, which this fixture's fixed "main" refs assume.
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

    /// Stage + commit `files` on the current branch; returns the new HEAD oid.
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
        crate::git::commit::create_commit(dir, msg).expect("commit");
        let repo = git2::Repository::open(dir).expect("open");
        let oid = repo.head().expect("HEAD").peel_to_commit().expect("peel").id();
        oid
    }

    fn branch_at(repo: &git2::Repository, name: &str, oid: git2::Oid) {
        let c = repo.find_commit(oid).expect("find commit");
        repo.branch(name, &c, false).expect("create branch");
    }

    fn have_git() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    fn caps(revwalk: usize, workdir: usize) -> StatsCaps {
        StatsCaps {
            revwalk,
            odb: ODB_SCAN_CAP,
            workdir,
            gitdir: GITDIR_WALK_CAP,
        }
    }

    // ------------------------------------------------------- wire shape

    /// Full `RepoHealth` serializes camelCase, incl. nested `Section<T>` and
    /// the reused `RepoOpState` tagged union.
    #[test]
    fn wire_shapes_serialize_camelcase() {
        let health = RepoHealth {
            stats: Section {
                data: Some(StatsSection {
                    commit_count: 5,
                    commit_count_capped: true,
                    commits_last_30d: 2,
                    authors_last_30d: 1,
                    authors_total: 3,
                    object_count: 42,
                    object_scan_capped: false,
                    largest_blobs: vec![BlobStat {
                        oid: "a".repeat(40),
                        size: 1234,
                    }],
                    workdir_file_count: 7,
                    workdir_bytes: 999,
                    workdir_scan_capped: false,
                    largest_files: vec![FileStat {
                        path: "big/file.bin".to_string(),
                        size: 999,
                    }],
                    large_file_count: 1,
                    git_dir_bytes: 100,
                    git_dir_scan_capped: false,
                }),
                error: None,
                elapsed_ms: 12,
            },
            branches: Section {
                data: Some(BranchesSection {
                    local_count: 2,
                    remote_count: 1,
                    tag_count: 0,
                    current_branch: Some("main".to_string()),
                    detached: false,
                    unborn: false,
                    ahead: Some(2),
                    behind: Some(5),
                    upstream: Some("origin/main".to_string()),
                    stale: Some(StaleRollup {
                        base: "main".to_string(),
                        merged_count: 3,
                        gone_upstream_count: 1,
                    }),
                    stale_error: None,
                }),
                error: None,
                elapsed_ms: 3,
            },
            working_state: Section {
                data: Some(WorkingStateSection {
                    staged: 1,
                    unstaged: 2,
                    untracked: 3,
                    conflicted: 0,
                    op_state: RepoOpState::Merge {
                        incoming: "feature/x".to_string(),
                        message: "Merge branch 'feature/x'".to_string(),
                    },
                    stash_count: 2,
                    has_gitignore: true,
                }),
                error: None,
                elapsed_ms: 1,
            },
            structure: Section {
                data: None,
                error: Some("boom".to_string()),
                elapsed_ms: 0,
            },
            generated_at: 1_700_000_000,
        };
        let v = serde_json::to_value(&health).expect("serialize");
        assert_eq!(v["generatedAt"], 1_700_000_000_i64);
        let s = &v["stats"];
        assert_eq!(s["elapsedMs"], 12);
        assert_eq!(s["data"]["commitCountCapped"], true);
        assert_eq!(s["data"]["commitsLast30d"], 2);
        assert_eq!(s["data"]["authorsLast30d"], 1);
        assert_eq!(s["data"]["objectScanCapped"], false);
        assert_eq!(s["data"]["largestBlobs"][0]["oid"], "a".repeat(40));
        assert_eq!(s["data"]["largestFiles"][0]["path"], "big/file.bin");
        assert_eq!(s["data"]["largeFileCount"], 1);
        assert_eq!(s["data"]["gitDirBytes"], 100);
        assert_eq!(s["data"]["workdirScanCapped"], false);
        let b = &v["branches"]["data"];
        assert_eq!(b["localCount"], 2);
        assert_eq!(b["currentBranch"], "main");
        assert_eq!(b["stale"]["mergedCount"], 3);
        assert_eq!(b["stale"]["goneUpstreamCount"], 1);
        assert_eq!(b["staleError"], serde_json::Value::Null);
        let w = &v["workingState"]["data"];
        assert_eq!(w["opState"]["kind"], "merge");
        assert_eq!(w["opState"]["incoming"], "feature/x");
        assert_eq!(w["stashCount"], 2);
        assert_eq!(w["hasGitignore"], true);
        // Error envelope: data null, error set.
        assert_eq!(v["structure"]["data"], serde_json::Value::Null);
        assert_eq!(v["structure"]["error"], "boom");
    }

    // ------------------------------------------------------- stats

    /// N commits → commitCount == N == `git rev-list --count HEAD`; a >10 MiB
    /// file shows in largestFiles + largeFileCount; objectCount ≥ commits.
    #[test]
    fn stats_counts_and_large_files() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "C0", &[("a.txt", "a\n")]);
        commit(d, "C1", &[("b.txt", "b\n")]);
        commit(d, "C2", &[("c.txt", "c\n")]);

        // One file over the 10 MiB threshold (untracked, non-ignored → counts).
        let big = vec![0u8; (LARGE_FILE_BYTES + 1) as usize];
        std::fs::write(d.join("big.bin"), &big).expect("write big file");

        let stats = collect_stats_with_caps(d, DEFAULT_CAPS).expect("stats");
        assert_eq!(stats.commit_count, 3);
        assert!(!stats.commit_count_capped);
        assert_eq!(stats.commits_last_30d, 3, "fresh commits are within 30d");
        assert_eq!(stats.authors_last_30d, 1);
        assert_eq!(stats.authors_total, 1);
        assert!(
            stats.object_count >= 3,
            "at least the commit objects: {}",
            stats.object_count
        );
        assert!(!stats.largest_blobs.is_empty(), "blobs exist in the odb");

        assert_eq!(stats.large_file_count, 1);
        assert_eq!(stats.largest_files[0].path, "big.bin");
        assert_eq!(stats.largest_files[0].size, LARGE_FILE_BYTES + 1);
        assert!(stats.workdir_file_count >= 4, "a,b,c + big.bin");
        assert!(stats.workdir_bytes > LARGE_FILE_BYTES);
        assert!(stats.git_dir_bytes > 0);
        assert!(!stats.workdir_scan_capped);
        assert!(!stats.git_dir_scan_capped);

        // CLI oracle (skip when git absent).
        if have_git() {
            let out = Command::new("git")
                .args(["rev-list", "--count", "HEAD"])
                .current_dir(d)
                .output()
                .expect("git rev-list");
            assert!(out.status.success());
            let cli: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().expect("count");
            assert_eq!(stats.commit_count, cli, "matches git rev-list --count");
        }
    }

    /// Gitignored files/dirs are EXCLUDED from workdir stats (P44b): a >10 MiB
    /// ignored `ignored.bin` and an ignored `node_modules/` subtree do NOT
    /// appear in `largest_files`, do NOT bump `large_file_count`, and are not
    /// counted in `workdir_file_count`/`workdir_bytes` — only the non-ignored
    /// control file + the `.gitignore` itself count. This proves exclusion
    /// (the `stats_counts_and_large_files` test only confirms a NON-ignored
    /// large file still counts).
    #[test]
    fn stats_excludes_gitignored_files() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        // One committed, non-ignored control file.
        commit(d, "C0", &[("control.txt", "control\n")]);

        // .gitignore covering a single file and a whole directory.
        std::fs::write(d.join(".gitignore"), "ignored.bin\nnode_modules/\n")
            .expect("write .gitignore");

        // A >10 MiB IGNORED file (same construction as the large-file test):
        // must NOT count nor appear in largest_files.
        let big = vec![0u8; (LARGE_FILE_BYTES + 1) as usize];
        std::fs::write(d.join("ignored.bin"), &big).expect("write big ignored file");

        // An ignored directory with a file inside: the whole subtree is excluded.
        std::fs::create_dir(d.join("node_modules")).expect("mkdir node_modules");
        std::fs::write(d.join("node_modules").join("dep.js"), "module.exports={}\n")
            .expect("write node_modules file");

        let stats = collect_stats_with_caps(d, DEFAULT_CAPS).expect("stats");

        // The ignored large file is absent from largestFiles.
        assert!(
            !stats.largest_files.iter().any(|f| f.path == "ignored.bin"),
            "ignored.bin must be excluded from largestFiles: {:?}",
            stats.largest_files
        );
        // No node_modules/* path leaks into largestFiles either.
        assert!(
            !stats
                .largest_files
                .iter()
                .any(|f| f.path.starts_with("node_modules/")),
            "node_modules subtree must be excluded: {:?}",
            stats.largest_files
        );
        // The only >10 MiB file is ignored → large_file_count is not bumped.
        assert_eq!(stats.large_file_count, 0, "ignored large file must not count");
        // Only the two non-ignored files (control.txt + .gitignore) are counted;
        // ignored.bin and node_modules/dep.js are not.
        assert_eq!(
            stats.workdir_file_count, 2,
            "only control.txt + .gitignore count, got {}",
            stats.workdir_file_count
        );
        // workdir_bytes excludes the 10 MiB ignored blob (both counted files are
        // tiny text) — proves the ignored bytes are not summed in.
        assert!(
            stats.workdir_bytes < LARGE_FILE_BYTES,
            "workdir_bytes must exclude the ignored 10 MiB file: {}",
            stats.workdir_bytes
        );
        // Sanity: the non-ignored control file IS present.
        assert!(
            stats.largest_files.iter().any(|f| f.path == "control.txt"),
            "control.txt should be counted: {:?}",
            stats.largest_files
        );
    }

    /// Shadowed caps: revwalk stops at the cap with capped=true and
    /// count == cap; the workdir walk sets its capped flag on overflow.
    #[test]
    fn stats_capped_flags() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        for i in 0..5 {
            commit(d, &format!("C{i}"), &[("a.txt", &format!("{i}\n"))]);
        }
        let stats = collect_stats_with_caps(d, caps(3, WORKDIR_WALK_CAP)).expect("stats");
        assert_eq!(stats.commit_count, 3, "count equals the cap");
        assert!(stats.commit_count_capped);

        let stats = collect_stats_with_caps(d, caps(REVWALK_CAP, 1)).expect("stats");
        assert!(stats.workdir_scan_capped, "workdir cap of 1 entry overflows");
    }

    /// Unborn repo: stats section still Ok with zero commits.
    #[test]
    fn stats_unborn_repo_ok() {
        let dir = crate::testutil::scratch_dir();
        init(dir.path());
        let stats = collect_stats_with_caps(dir.path(), DEFAULT_CAPS).expect("stats");
        assert_eq!(stats.commit_count, 0);
        assert!(!stats.commit_count_capped);
    }

    // ------------------------------------------------------- branches

    /// Local/remote/tag counts match `git for-each-ref`; stale rollup matches
    /// `find_stale_branches`; ahead/behind vs a seeded upstream.
    #[test]
    fn branches_counts_and_stale_rollup() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);
        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
        let c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip

        branch_at(&repo, "merged-1", c0); // merged into main
        branch_at(&repo, "merged-2", c1); // merged into main
        repo.remote("origin", "https://example.invalid/x.git").expect("remote");
        // gone-upstream branch with a unique commit.
        branch_at(&repo, "gone", c0);
        {
            let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
            let parent = repo.find_commit(c0).expect("c0");
            let tree = parent.tree().expect("tree");
            repo.commit(Some("refs/heads/gone"), &sig, &sig, "gone work\n", &tree, &[&parent])
                .expect("commit on gone");
            let mut cfg = repo.config().expect("config");
            cfg.set_str("branch.gone.remote", "origin").expect("cfg");
            cfg.set_str("branch.gone.merge", "refs/heads/gone").expect("cfg");
        }
        // Upstream for main: remote-tracking ref at C1 → ahead 1, behind 0.
        repo.reference("refs/remotes/origin/main", c1, true, "seed").expect("ref");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("branch.main.remote", "origin").expect("cfg");
            cfg.set_str("branch.main.merge", "refs/heads/main").expect("cfg");
        }
        repo.reference("refs/tags/v1", c2, true, "tag").expect("tag");

        let b = collect_branches(d).expect("branches");
        assert_eq!(b.local_count, 4, "main + merged-1 + merged-2 + gone");
        assert_eq!(b.remote_count, 1);
        assert_eq!(b.tag_count, 1);
        assert_eq!(b.current_branch.as_deref(), Some("main"));
        assert!(!b.detached);
        assert!(!b.unborn);
        assert_eq!(b.upstream.as_deref(), Some("origin/main"));
        assert_eq!(b.ahead, Some(1));
        assert_eq!(b.behind, Some(0));

        // Stale rollup mirrors find_stale_branches exactly.
        let report = find_stale_branches(d, None).expect("stale");
        let stale = b.stale.expect("stale rollup present");
        assert_eq!(stale.base, report.base);
        assert_eq!(
            stale.merged_count as usize,
            report.branches.iter().filter(|x| x.merged).count()
        );
        assert_eq!(
            stale.gone_upstream_count as usize,
            report.branches.iter().filter(|x| x.gone_upstream).count()
        );
        assert_eq!(stale.merged_count, 2);
        assert_eq!(stale.gone_upstream_count, 1);
        assert!(b.stale_error.is_none());

        // CLI oracle for ref counts (skip when git absent).
        if have_git() {
            let count_refs = |prefix: &str| -> usize {
                let out = Command::new("git")
                    .args(["for-each-ref", "--format=%(refname)", prefix])
                    .current_dir(d)
                    .output()
                    .expect("git for-each-ref");
                assert!(out.status.success());
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count()
            };
            assert_eq!(b.local_count as usize, count_refs("refs/heads"));
            assert_eq!(b.remote_count as usize, count_refs("refs/remotes"));
            assert_eq!(b.tag_count as usize, count_refs("refs/tags"));
        }
    }

    /// Detached HEAD → detached=true, currentBranch=None, section still Ok.
    #[test]
    fn branches_detached_head() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);
        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        commit(d, "C1", &[("b.txt", "b\n")]);
        repo.set_head_detached(c0).expect("detach");

        let b = collect_branches(d).expect("branches");
        assert!(b.detached);
        assert_eq!(b.current_branch, None);
        assert_eq!(b.ahead, None);
        assert_eq!(b.upstream, None);
    }

    /// Unborn repo: the section succeeds; the stale SUB-metric fails into
    /// stale_error (D9) without failing the section.
    #[test]
    fn branches_unborn_repo_ok() {
        let dir = crate::testutil::scratch_dir();
        init(dir.path());
        let b = collect_branches(dir.path()).expect("branches section Ok on unborn");
        assert!(b.unborn);
        assert_eq!(b.local_count, 0);
        assert!(b.stale.is_none());
        assert!(b.stale_error.is_some(), "stale base unresolvable → sub-error");
    }

    // ------------------------------------------------------- working state

    /// Counts match read_status; stash (created via git2 in the FIXTURE only)
    /// counts 1; .gitignore flag flips with the file.
    #[test]
    fn working_state_counts_stash_gitignore() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "C0", &[("a.txt", "a\n"), ("tracked.txt", "t\n")]);

        // stash: modify a tracked file, then stash it away.
        std::fs::write(d.join("tracked.txt"), "changed\n").expect("modify");
        {
            let mut repo = git2::Repository::open(d).expect("open");
            let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
            repo.stash_save(&sig, "wip", None).expect("stash save");
        }

        // staged: new file staged; unstaged: modify a.txt; untracked: new file.
        std::fs::write(d.join("staged.txt"), "s\n").expect("write");
        crate::git::stage::stage_paths(d, &["staged.txt".to_string()]).expect("stage");
        std::fs::write(d.join("a.txt"), "a2\n").expect("modify a");
        std::fs::write(d.join("untracked.txt"), "u\n").expect("write untracked");

        let ws = collect_working_state(d).expect("working state");
        let snap = read_status(d).expect("status");
        assert_eq!(ws.staged as usize, snap.staged.len());
        assert_eq!(ws.unstaged as usize, snap.unstaged.len());
        assert_eq!(ws.untracked as usize, snap.untracked.len());
        assert_eq!(ws.conflicted as usize, snap.conflicted.len());
        assert_eq!(ws.staged, 1);
        assert_eq!(ws.unstaged, 1);
        assert_eq!(ws.untracked, 1);
        assert_eq!(ws.conflicted, 0);
        assert_eq!(ws.stash_count, 1);
        assert_eq!(ws.op_state, RepoOpState::None);
        assert!(!ws.has_gitignore);

        std::fs::write(d.join(".gitignore"), "*.log\n").expect("write gitignore");
        let ws2 = collect_working_state(d).expect("working state 2");
        assert!(ws2.has_gitignore);
    }

    // ------------------------------------------------------- structure

    /// Locked + prunable worktrees roll up to matching counts; a drifted
    /// CLAUDE.md/AGENTS.md pair yields assetDriftedCount ≥ 1 (in_sync false).
    #[test]
    fn structure_worktrees_and_drift() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init(d);
        commit(d, "C0", &[("a.txt", "a\n")]);

        // Linked worktrees live as siblings in their own scratch dir.
        let wt_root = crate::testutil::scratch_dir();
        let locked_path = wt_root.path().join("wt-locked");
        let prunable_path = wt_root.path().join("wt-prunable");
        let wt1 = repo
            .worktree("wt-locked", &locked_path, None)
            .expect("add worktree 1");
        wt1.lock(Some("testing")).expect("lock");
        repo.worktree("wt-prunable", &prunable_path, None)
            .expect("add worktree 2");
        // Make the second prunable: remove its working directory.
        std::fs::remove_dir_all(&prunable_path).expect("remove wt dir");

        // Drifted AI-asset pair (different normalized content).
        std::fs::write(d.join("CLAUDE.md"), "# Rules\nAlpha\n").expect("claude");
        std::fs::write(d.join("AGENTS.md"), "# Rules\nBeta\n").expect("agents");

        let s = collect_structure(d).expect("structure");
        let wts = list_worktrees(d).expect("worktrees");
        assert_eq!(s.worktree_count as usize, wts.len());
        assert_eq!(s.worktree_count, 3, "main + 2 linked");
        assert_eq!(s.worktrees_locked, 1);
        assert_eq!(
            s.worktrees_prunable as usize,
            wts.iter().filter(|w| w.prunable).count()
        );
        assert!(s.worktrees_prunable >= 1, "deleted workdir → prunable");
        assert_eq!(s.submodule_count, 0);
        assert!(s.asset_drifted_count >= 1, "CLAUDE.md vs AGENTS.md drifted");
        assert!(!s.assets_in_sync);
    }

    // ------------------------------------------------------- section isolation

    /// One failing collector → its Section carries the error while the other
    /// three carry data (D4 fold, exercised at the envelope level), and
    /// `collect_repo_health` on a healthy repo yields all four with data.
    #[test]
    fn section_isolation() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        commit(d, "C0", &[("a.txt", "a\n")]);

        // The fold: a failing collector never panics/propagates.
        let failing: Section<StatsSection> =
            run_section(|| Err(AppError::Git("simulated odb corruption".to_string())));
        assert!(failing.data.is_none());
        assert_eq!(
            failing.error.as_deref(),
            Some("git error: simulated odb corruption")
        );

        // Sibling sections on the same repo still produce data.
        let health = collect_repo_health(d);
        assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
        assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
        assert!(
            health.working_state.data.is_some(),
            "{:?}",
            health.working_state.error
        );
        assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
        assert!(health.generated_at > 0);

        // Whole-fn never errs even on a non-repo dir: every section reports
        // its own error instead.
        let empty = crate::testutil::scratch_dir();
        let health = collect_repo_health(empty.path());
        assert!(health.stats.data.is_none() && health.stats.error.is_some());
        assert!(health.branches.data.is_none() && health.branches.error.is_some());
        assert!(health.working_state.error.is_some());
        assert!(health.structure.error.is_some());
    }

    /// MIXED state with a REAL failing collector (P29a review carry-forward):
    /// deleting a parent commit's loose object makes the stats revwalk error
    /// mid-iteration, while branches / workingState / structure (which only
    /// need refs, the HEAD commit + its tree, and fs facts) still succeed.
    ///
    /// Note the companion carry-forward (health.rs `find_commit` degrade):
    /// a missing object aborts the revwalk ITERATOR itself (`oid?`), before
    /// `find_commit` runs, so the `if let Ok` path cannot be reached with a
    /// plain missing-object fixture — the degrade is covered by review, this
    /// test pins the section-isolation behavior around the same failure.
    #[test]
    fn mixed_state_real_collector_failure() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init(d);
        let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
        commit(d, "C1", &[("b.txt", "b\n")]);

        // Remove C0's loose commit object; clear read-only first (Windows
        // loose objects are written read-only).
        let hex = c0.to_string();
        let obj = d.join(".git/objects").join(&hex[..2]).join(&hex[2..]);
        let mut perms = std::fs::metadata(&obj).expect("object exists").permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&obj, perms).expect("clear readonly");
        std::fs::remove_file(&obj).expect("delete loose object");

        let health = collect_repo_health(d);
        assert!(
            health.stats.data.is_none() && health.stats.error.is_some(),
            "stats must fail on the missing parent object (error: {:?})",
            health.stats.error
        );
        assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
        assert!(
            health.working_state.data.is_some(),
            "{:?}",
            health.working_state.error
        );
        assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
    }

    // ------------------------------------------------------- perf ceiling (§5)

    /// On the shared 20k+ fixture the whole scan stays < 2 s and the stats
    /// section < 1.5 s. Coarse ceiling, not a benchmark: warm-up + best of 3.
    ///
    /// `#[ignore]`d to match the established perf-gate convention
    /// (`perf_gate.rs::layout_31k_under_500ms` / `serialize_31k_report`): it
    /// depends on the multi-second 31k fixture and is release-oriented, and in
    /// the *parallel* default `cargo test` suite CPU contention from the other
    /// ~430 tests inflates the best-of-3 (~3.1 s observed) even though the scan
    /// itself is well under budget in isolation. Run it EXPLICITLY (isolated),
    /// like the other perf gates, for perf tracking:
    ///
    /// ```text
    /// cargo test --release -p bonsai-core --lib \
    ///     health::tests::perf_ceiling_on_20k_fixture -- --ignored --nocapture
    /// ```
    ///
    /// P52 (commit-graph) result — measured isolated, best-of-3, on the fixture
    /// carrying `.git/objects/info/commit-graph`: stats ~300 ms (was ~1558),
    /// branches ~1280 ms (was ~6000), total ~1600 ms (was ~8100) — under the
    /// UNCHANGED 1500 / 2000 ms budgets. The residual cost is the branches
    /// merge-base scan; the graph already cut it ~4.7x and further cuts would
    /// need app-logic changes (out of P52 scope).
    #[test]
    #[ignore] // perf gate: run explicitly + isolated; see doc comment
    fn perf_ceiling_on_20k_fixture() {
        let repo_path = crate::fixture::ensure_default_fixture().expect("fixture");

        // P52: the fixture carries a commit-graph (written once by
        // ensure_default_fixture when git is available), so this gate measures
        // the realistic opened-repo state — libgit2 consumes the graph
        // unconditionally, cutting the branches merge-base + stats revwalk cost.
        if have_git() {
            assert!(
                repo_path.join(".git/objects/info/commit-graph").exists(),
                "P52: fixture must carry a commit-graph for the perf measurement"
            );
        }

        // Warm-up (page cache, odb) + correctness assertions.
        let warm = collect_repo_health(&repo_path);
        let stats = warm.stats.data.as_ref().unwrap_or_else(|| {
            panic!("stats section failed: {:?}", warm.stats.error)
        });
        assert!(
            stats.commit_count >= 20_000,
            "fixture has 20k+ commits, got {}",
            stats.commit_count
        );

        let mut best_total = u128::MAX;
        let mut best_stats = u32::MAX;
        let mut best_branches = u32::MAX;
        let mut best_working = u32::MAX;
        let mut best_structure = u32::MAX;
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let health = collect_repo_health(&repo_path);
            let total = start.elapsed().as_millis();
            eprintln!(
                "[health perf] stats={}ms branches={}ms workingState={}ms structure={}ms total={}ms",
                health.stats.elapsed_ms,
                health.branches.elapsed_ms,
                health.working_state.elapsed_ms,
                health.structure.elapsed_ms,
                total
            );
            best_total = best_total.min(total);
            best_stats = best_stats.min(health.stats.elapsed_ms);
            best_branches = best_branches.min(health.branches.elapsed_ms);
            best_working = best_working.min(health.working_state.elapsed_ms);
            best_structure = best_structure.min(health.structure.elapsed_ms);
        }
        // Best-of-3 per-section summary so the commit-graph effect (mainly on
        // the branches merge-base scan) is visible under `--nocapture`.
        eprintln!(
            "[health perf] best-of-3: stats={best_stats}ms branches={best_branches}ms \
             workingState={best_working}ms structure={best_structure}ms total={best_total}ms"
        );
        assert!(
            best_stats < 1_500,
            "stats section best-of-3 took {best_stats} ms (budget 1500)"
        );
        assert!(
            best_total < 2_000,
            "collect_repo_health best-of-3 took {best_total} ms (budget 2000)"
        );
    }
}
