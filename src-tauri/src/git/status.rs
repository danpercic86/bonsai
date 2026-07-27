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
        .renames_index_to_workdir(true) // worktree rename detection
        .exclude_submodules(true); // v1: no submodule support
                                   // Do NOT set update_index(true): status stays strictly read-only.

    let statuses = repo.statuses(Some(&mut opts))?;

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
            snapshot.unstaged.push(StatusEntry {
                path: workdir_path(&entry),
                orig_path: None,
                status: FileStatus::Modified,
            });
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
