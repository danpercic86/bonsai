//! Discovery (contract §4.1) — manifests and lockfiles ONLY `[HIGH-1 item 1]`.
//!
//! Registry configs are NOT taken from here: `registry_ctx` walks for them
//! itself, so a gitignored `.npmrc` is still seen. Nothing here parses.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use git2::{Repository, StatusOptions};

use crate::adapter::{file_name, ADAPTERS};
use crate::catalog::{not_tracked_label, SKIPPED_SEGMENTS};
use crate::fsread::{path_is_clean, safe_stat};
use crate::model::{FileStamp, MAX_MANIFESTS};

const MODE_MASK: u32 = 0o170000;
const MODE_SYMLINK: u32 = 0o120000;
const MODE_GITLINK: u32 = 0o160000;

/// The manifests, lockfiles and not-tracked files of one repo, as sorted,
/// clean, repo-relative `/` paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileSet {
    manifests: Vec<String>,
    lockfiles: Vec<String>,
    not_tracked: Vec<String>,
    /// `[LOW-3]` manifests and not-tracked files dropped by the
    /// [`MAX_MANIFESTS`] cap; the check reports them as one `LimitExceeded`
    /// group.
    pub overflow: usize,
}

/// Why discovery could not run. No text `[MEDIUM-1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverError {
    /// A bare repository has no workdir to scan.
    Bare,
    Workdir,
    Index,
    Status,
}

/// What discovery does with a file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Manifest,
    Lockfile,
    NotTracked,
}

fn kind_of(name: &str) -> Option<Kind> {
    if ADAPTERS.iter().any(|a| a.is_manifest(name)) {
        Some(Kind::Manifest)
    } else if ADAPTERS.iter().any(|a| a.is_lockfile(name)) {
        Some(Kind::Lockfile)
    } else if not_tracked_label(name).is_some() {
        Some(Kind::NotTracked)
    } else {
        None
    }
}

/// `[MEDIUM-3]` clean, and no skipped segment (`node_modules`, `vendor`, …).
fn acceptable(rel: &str) -> bool {
    path_is_clean(rel) && !rel.split('/').any(|s| SKIPPED_SEGMENTS.contains(&s))
}

impl FileSet {
    /// Classifies candidate paths: unknown names, unclean paths and skipped
    /// segments are dropped; manifests beyond [`MAX_MANIFESTS`] (sorted) are
    /// counted in `overflow`.
    pub fn from_paths<I: IntoIterator<Item = String>>(paths: I) -> FileSet {
        let unique: BTreeSet<String> = paths.into_iter().filter(|p| acceptable(p)).collect();
        let mut set = FileSet::default();
        for p in unique {
            match kind_of(file_name(&p)) {
                Some(Kind::Manifest) => set.manifests.push(p),
                Some(Kind::Lockfile) => set.lockfiles.push(p),
                Some(Kind::NotTracked) => set.not_tracked.push(p),
                None => {}
            }
        }
        if set.manifests.len() > MAX_MANIFESTS {
            set.overflow = set.manifests.len() - MAX_MANIFESTS;
            set.manifests.truncate(MAX_MANIFESTS);
        }
        // The same cap bounds the not-tracked groups; what it drops is
        // counted in the same `overflow` signal.
        if set.not_tracked.len() > MAX_MANIFESTS {
            set.overflow += set.not_tracked.len() - MAX_MANIFESTS;
            set.not_tracked.truncate(MAX_MANIFESTS);
        }
        set
    }

    pub fn manifests(&self) -> &[String] {
        &self.manifests
    }

    pub fn lockfiles(&self) -> &[String] {
        &self.lockfiles
    }

    pub fn not_tracked(&self) -> &[String] {
        &self.not_tracked
    }

    /// Whether `rel` is one of the discovered manifests or lockfiles.
    pub fn contains(&self, rel: &str) -> bool {
        let has = |v: &[String]| v.binary_search_by(|p| p.as_str().cmp(rel)).is_ok();
        has(&self.manifests) || has(&self.lockfiles)
    }

    fn all(&self) -> impl Iterator<Item = &String> {
        self.manifests
            .iter()
            .chain(&self.lockfiles)
            .chain(&self.not_tracked)
    }
}

/// The canonical workdir of `repo`.
pub fn workdir_canon(repo: &Repository) -> Result<PathBuf, DiscoverError> {
    let wd = repo.workdir().ok_or(DiscoverError::Bare)?;
    fs::canonicalize(wd).map_err(|_| DiscoverError::Workdir)
}

/// Index paths that are neither symlinks nor gitlinks `[MEDIUM-3]`.
fn index_paths(repo: &Repository) -> Result<Vec<String>, DiscoverError> {
    let index = repo.index().map_err(|_| DiscoverError::Index)?;
    Ok(index
        .iter()
        .filter(|e| !matches!(e.mode & MODE_MASK, MODE_SYMLINK | MODE_GITLINK))
        .filter_map(|e| String::from_utf8(e.path).ok())
        .collect())
}

/// §4.1, check-time only: index entries ∪ untracked (not ignored) files
/// whose basename an adapter or the not-tracked catalog knows. Untracked
/// symlinks and special files are dropped via `safe_stat`.
pub fn discover_full(repo: &Repository) -> Result<FileSet, DiscoverError> {
    let wd = workdir_canon(repo)?;
    let mut paths = index_paths(repo)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .exclude_submodules(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|_| DiscoverError::Status)?;
    for entry in statuses.iter() {
        if !entry.status().is_wt_new() {
            continue;
        }
        let Ok(p) = entry.path() else { continue };
        if kind_of(file_name(p)).is_some() && safe_stat(&wd, p).is_some() {
            paths.push(p.to_string());
        }
    }
    Ok(FileSet::from_paths(paths))
}

/// Stamp of a discovered file that cannot be stat'ed (deleted from the
/// workdir, turned into a symlink, …). Keeping it stamped stops
/// [`probe_stale`] from reporting the same index path as "new" forever.
const ABSENT_STAMP: (u64, i64) = (u64::MAX, i64::MIN);

fn stat_or_absent(workdir_canon: &Path, rel: &str) -> (u64, i64) {
    safe_stat(workdir_canon, rel).unwrap_or(ABSENT_STAMP)
}

/// `(len, mtime_ms)` stamps for every discovered file (symlink-aware).
pub fn stamps(workdir_canon: &Path, files: &FileSet) -> Vec<FileStamp> {
    files
        .all()
        .map(|p| {
            let (len, mtime_ms) = stat_or_absent(workdir_canon, p);
            FileStamp {
                path: p.clone(),
                len,
                mtime_ms,
            }
        })
        .collect()
}

/// Cheap staleness probe for the watcher / focus path: a stamped file
/// changed (or vanished), or the index gained a discoverable path. No
/// untracked walk. Any error reads as stale.
pub fn probe_stale(repo: &Repository, cached: &[FileStamp]) -> bool {
    let Ok(wd) = workdir_canon(repo) else {
        return true;
    };
    if cached
        .iter()
        .any(|s| stat_or_absent(&wd, &s.path) != (s.len, s.mtime_ms))
    {
        return true;
    }
    let Ok(paths) = index_paths(repo) else {
        return true;
    };
    let known: BTreeSet<&str> = cached.iter().map(|s| s.path.as_str()).collect();
    paths
        .iter()
        .any(|p| acceptable(p) && kind_of(file_name(p)).is_some() && !known.contains(p.as_str()))
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod tests;
