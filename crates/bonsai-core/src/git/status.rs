//! Working-directory status core (M1 contract §2–§3).
//!
//! Pure git2 logic, no Tauri types — testable against the `git status
//! --porcelain=v1 -z` oracle (see `tests/status_porcelain.rs`).

use crate::error::AppError;

/// Classification of a single file's state within one list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Typechange,
    Conflicted,
    Untracked,
}

/// One file in one status list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    /// Repo-relative path, forward slashes (as git2 reports). For renames: the NEW path.
    pub path: String,
    /// For renames: the OLD path. `None` otherwise.
    pub orig_path: Option<String>,
    pub status: FileStatus,
}

/// Split-lists status model (contract §2). A file staged AND re-modified
/// appears in both `staged` and `unstaged`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub staged: Vec<StatusEntry>,     // index vs HEAD
    pub unstaged: Vec<StatusEntry>,   // workdir vs index (tracked files only)
    pub untracked: Vec<StatusEntry>,  // status == Untracked
    pub conflicted: Vec<StatusEntry>, // status == Conflicted
}

/// Lossy-decodes an optional byte path (non-UTF-8 paths must not error).
fn lossy_path(bytes: Option<&[u8]>) -> Option<String> {
    bytes.map(|b| String::from_utf8_lossy(b).into_owned())
}

/// Path of the entry as seen on the HEAD→index side (staged rows).
fn staged_path(entry: &git2::StatusEntry) -> String {
    entry
        .head_to_index()
        .and_then(|d| lossy_path(d.new_file().path_bytes()))
        .unwrap_or_else(|| String::from_utf8_lossy(entry.path_bytes()).into_owned())
}

/// Path of the entry as seen on the index→workdir side (unstaged/untracked rows).
fn workdir_path(entry: &git2::StatusEntry) -> String {
    entry
        .index_to_workdir()
        .and_then(|d| lossy_path(d.new_file().path_bytes()))
        .unwrap_or_else(|| String::from_utf8_lossy(entry.path_bytes()).into_owned())
}

/// Whole-second mtime (since the Unix epoch) of a filesystem entry, if knowable.
///
/// Windows-only: it feeds [`wt_modified_is_stat_clean`], which is itself gated
/// to Windows. Gating this avoids a `-D dead_code` failure on the macOS/Linux
/// CI legs, where the suppression it supports does not run.
#[cfg(windows)]
fn mtime_secs(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// True when git's stat-cache would treat this worktree file as UNCHANGED, so a
/// libgit2 `WT_MODIFIED` on it must be suppressed to agree with the porcelain
/// oracle.
///
/// **Windows-only by design.** `git status` on Git for Windows (a non-`USE_NSEC`
/// build) compares the index's cached stat against the worktree with
/// **whole-second** mtime granularity. If the size and whole-second mtime still
/// match the cache, git trusts the cache and does NOT re-hash — so an in-place
/// rewrite that keeps the byte length and lands in the same clock second as the
/// cached mtime is reported as clean (a benign racy-git artifact). libgit2,
/// however, compares sub-second mtime, re-hashes, sees the new content, and
/// reports `WT_MODIFIED`. That is the sole divergence this guard removes.
///
/// On macOS/Linux git is normally built with `USE_NSEC` and compares mtime at
/// **nanosecond** resolution, so a genuine same-second in-place rewrite has a
/// different sub-second mtime that git DOES detect — git and libgit2 agree and
/// there is no phantom. Applying whole-second suppression there would instead
/// hide real unstaged edits and diverge from porcelain, so the caller only
/// invokes this on Windows. Do NOT drop the `#[cfg(windows)]` gate.
///
/// The racy-clean rule is preserved: if the entry's cached mtime is not strictly
/// older than the index file's own mtime, git cannot trust the cache and
/// re-hashes (and would then surface the real change), so we do NOT suppress.
#[cfg(windows)]
fn wt_modified_is_stat_clean(
    index: &git2::Index,
    index_mtime_secs: Option<i64>,
    workdir: &std::path::Path,
    rel_path: &str,
) -> bool {
    let Some(entry) = index.get_path(std::path::Path::new(rel_path), 0) else {
        return false;
    };
    let Ok(meta) = std::fs::symlink_metadata(workdir.join(rel_path)) else {
        return false;
    };
    if !meta.is_file() || u64::from(entry.file_size) != meta.len() {
        return false;
    }
    let entry_secs = i64::from(entry.mtime.seconds());
    if mtime_secs(&meta) != Some(entry_secs) {
        return false;
    }
    // Racy-clean guard: only trust the cache when the entry is strictly older
    // than the index file itself (matching git's `ie_match_stat` racy check).
    match index_mtime_secs {
        Some(idx_secs) => entry_secs < idx_secs,
        None => false,
    }
}

/// Blocking. Opens the repo at `workdir` (`NO_SEARCH`, like `read_repo_info`)
/// and computes the status snapshot.
///
/// Errors: path not a repo -> `AppError::Git`; bare repo -> `AppError::Git`
/// with message "cannot compute status: repository is bare" (defensive;
/// `open_repo` already gates bare repos).
pub fn read_status(workdir: &std::path::Path) -> Result<StatusSnapshot, AppError> {
    let repo = git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?;
    if repo.is_bare() {
        return Err(AppError::Git(
            "cannot compute status: repository is bare".to_string(),
        ));
    }

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true) // individual files, matching --untracked-files=all
        .include_ignored(false)
        .include_unmodified(false)
        .renames_head_to_index(true) // staged rename detection
        // Do NOT enable renames_index_to_workdir: `git status --porcelain` does
        // NOT rename-detect an untracked destination in the worktree. With it on,
        // git2 collapses a delete + identical-bytes untracked file into a single
        // WT_RENAMED, diverging from porcelain (F-T5-3). Off ⇒ the delete shows as
        // WT_DELETED and the new file as WT_NEW/untracked, matching git exactly.
        .renames_index_to_workdir(false)
        .exclude_submodules(true); // v1: no submodule support
                                   // Do NOT set update_index(true): status stays strictly read-only.

    let statuses = repo.statuses(Some(&mut opts))?;

    // Windows-only: snapshot the index + its own mtime once, to replicate git's
    // whole-second stat-cache trust (see `wt_modified_is_stat_clean`). Both are
    // best-effort: if either is unavailable we simply never suppress. On
    // macOS/Linux (nsec git) there is no phantom, so this context is not built.
    #[cfg(windows)]
    let racy_ctx = {
        let index = repo.index().ok();
        let index_mtime_secs = std::fs::metadata(repo.path().join("index"))
            .ok()
            .as_ref()
            .and_then(mtime_secs);
        let wd = repo.workdir().unwrap_or(workdir);
        (index, index_mtime_secs, wd)
    };

    let mut snapshot = StatusSnapshot::default();
    for entry in statuses.iter() {
        let st = entry.status();

        // CONFLICTED entries go ONLY to `conflicted` (suppress INDEX_*/WT_* companions).
        if st.is_conflicted() {
            snapshot.conflicted.push(StatusEntry {
                path: workdir_path(&entry),
                orig_path: None,
                status: FileStatus::Conflicted,
            });
            continue;
        }

        // Index vs HEAD → staged.
        if st.is_index_new() {
            snapshot.staged.push(StatusEntry {
                path: staged_path(&entry),
                orig_path: None,
                status: FileStatus::Added,
            });
        }
        if st.is_index_modified() {
            snapshot.staged.push(StatusEntry {
                path: staged_path(&entry),
                orig_path: None,
                status: FileStatus::Modified,
            });
        }
        if st.is_index_deleted() {
            snapshot.staged.push(StatusEntry {
                path: staged_path(&entry),
                orig_path: None,
                status: FileStatus::Deleted,
            });
        }
        if st.is_index_renamed() {
            snapshot.staged.push(StatusEntry {
                path: staged_path(&entry),
                orig_path: entry
                    .head_to_index()
                    .and_then(|d| lossy_path(d.old_file().path_bytes())),
                status: FileStatus::Renamed,
            });
        }
        if st.is_index_typechange() {
            snapshot.staged.push(StatusEntry {
                path: staged_path(&entry),
                orig_path: None,
                status: FileStatus::Typechange,
            });
        }

        // Workdir vs index → unstaged (tracked files).
        if st.is_wt_modified() {
            let path = workdir_path(&entry);
            // On Windows, suppress the libgit2 sub-second racy-git phantom (see
            // `wt_modified_is_stat_clean`). Everywhere else git uses nsec mtime
            // and agrees with libgit2, so always emit.
            #[cfg(windows)]
            let stat_clean = {
                let (index, index_mtime_secs, wd) = &racy_ctx;
                index
                    .as_ref()
                    .is_some_and(|idx| wt_modified_is_stat_clean(idx, *index_mtime_secs, wd, &path))
            };
            #[cfg(not(windows))]
            let stat_clean = false;
            if !stat_clean {
                snapshot.unstaged.push(StatusEntry {
                    path,
                    orig_path: None,
                    status: FileStatus::Modified,
                });
            }
        }
        if st.is_wt_deleted() {
            snapshot.unstaged.push(StatusEntry {
                path: workdir_path(&entry),
                orig_path: None,
                status: FileStatus::Deleted,
            });
        }
        if st.is_wt_renamed() {
            snapshot.unstaged.push(StatusEntry {
                path: workdir_path(&entry),
                orig_path: entry
                    .index_to_workdir()
                    .and_then(|d| lossy_path(d.old_file().path_bytes())),
                status: FileStatus::Renamed,
            });
        }
        if st.is_wt_typechange() {
            snapshot.unstaged.push(StatusEntry {
                path: workdir_path(&entry),
                orig_path: None,
                status: FileStatus::Typechange,
            });
        }

        // New in workdir → untracked. (IGNORED and CURRENT are excluded entirely.)
        if st.is_wt_new() {
            snapshot.untracked.push(StatusEntry {
                path: workdir_path(&entry),
                orig_path: None,
                status: FileStatus::Untracked,
            });
        }
    }

    // Deterministic ordering: byte-wise ascending by path (String's Ord).
    snapshot.staged.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot.unstaged.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot.untracked.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot.conflicted.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(snapshot)
}
