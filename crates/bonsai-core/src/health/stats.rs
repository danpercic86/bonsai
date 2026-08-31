//! Repository stats collection (§3): the object-database blob walk and the
//! working/git-dir file walks with their top-N heaps. Split out of `health.rs`;
//! behavior unchanged. `collect_stats_with_caps` is re-exported from the module
//! root so existing paths (and the test harness) resolve unchanged.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

use super::*;

/// Cap-parameterized stats collector (§10.1: caps shadowable in tests).
pub(super) fn collect_stats_with_caps(workdir: &Path, caps: StatsCaps) -> Result<StatsSection, AppError> {
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

