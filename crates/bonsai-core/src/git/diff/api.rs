//! Public diff entry points (M4/P5 contract §2.2): workdir, commit, and
//! HEAD→commit comparisons, plus the small commit/tree/annotation helpers they
//! share (also reused by `image_diff`).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{open_workdir_repo, validate_rel_path};
use crate::git::status::FileStatus;

use super::{
    apply_find_similar, build_diff_options, collect_file_diff, collect_headers, head_tree, lossy,
    CommitDetails, CommitDiff, CompareDiff, CompareEndpoint, FileDiff,
};

/// Details of `commit` (§2.3 tail): lossy strings, trailing-whitespace-trimmed
/// full message, first-parent-first parent oids.
fn commit_details(commit: &git2::Commit) -> CommitDetails {
    let author = commit.author();
    CommitDetails {
        oid: commit.id().to_string(),
        summary: commit
            .summary_bytes()
            .map(lossy)
            .unwrap_or_default(),
        message: String::from_utf8_lossy(commit.message_bytes())
            .trim_end()
            .to_string(),
        author_name: lossy(author.name_bytes()),
        author_email: lossy(author.email_bytes()),
        author_ts: author.when().seconds(),
        committer_ts: commit.committer().when().seconds(),
        parents: commit.parent_ids().map(|id| id.to_string()).collect(),
    }
}

/// Trees for the commit-vs-first-parent diff: `(parent tree or None for a
/// root commit, the commit's own tree)`. `pub(crate)` so `image_diff` (P61b)
/// can resolve the old/new blob for the Commit request without a diff walk.
pub(crate) fn commit_trees<'r>(
    commit: &git2::Commit<'r>,
) -> Result<(Option<git2::Tree<'r>>, git2::Tree<'r>), AppError> {
    let old = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };
    Ok((old, commit.tree()?))
}

/// P61a: when `intraline` is set (and the file has renderable hunks), run the
/// word-level pass on each hunk in place. Binary / too-large diffs carry no
/// hunks, so they are left untouched. `intraline=false` is a no-op, keeping the
/// `FileDiff` wire byte-identical to pre-P61a.
pub(crate) fn maybe_annotate(mut fd: FileDiff, intraline: bool) -> FileDiff {
    if intraline && !fd.binary && !fd.too_large {
        for hunk in &mut fd.hunks {
            crate::git::intraline::annotate_hunk(hunk);
        }
    }
    fd
}

/// Pathspec list for one file: the path itself plus the rename OLD side.
pub(crate) fn pathspecs<'a>(path: &'a str, orig_path: Option<&'a str>) -> Vec<&'a str> {
    let mut paths = vec![path];
    if let Some(op) = orig_path {
        if op != path {
            paths.push(op);
        }
    }
    paths
}

/// Diff of ONE working-dir file (contract §2.2).
///
/// `staged == false` -> index vs workdir (`git diff -- <path>`); untracked
/// files come back as an all-Add hunk with status `Untracked`.
/// `staged == true` -> HEAD tree vs index (`git diff --cached -- <path>`);
/// unborn HEAD -> old side is the empty tree (everything shows as Added).
///
/// `orig_path`: `Some` for renames; the pathspec then includes BOTH paths and
/// `find_similar` pairs them into one Renamed delta.
///
/// If the pathspec matches no delta (file racing to clean), returns an empty
/// `FileDiff` (status Modified, no hunks) — NOT an error.
pub fn workdir_file_diff(
    workdir: &Path,
    path: &str,
    orig_path: Option<&str>,
    staged: bool,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    let repo = open_workdir_repo(workdir)?;
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths, full_context);
    let mut diff = if staged {
        let old = head_tree(&repo)?;
        repo.diff_tree_to_index(old.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };
    apply_find_similar(&mut diff)?;
    let fd = collect_file_diff(&diff)?.unwrap_or_else(|| FileDiff {
        path: path.to_string(),
        orig_path: None,
        status: FileStatus::Modified,
        binary: false,
        too_large: false,
        hunks: Vec::new(),
    });
    Ok(maybe_annotate(fd, intraline))
}

/// Commit details + per-file headers for `oid` vs its FIRST parent
/// (contract §2.2). Root commit -> vs empty tree. Merge -> first parent only.
/// Bad/unknown/non-commit oid -> `AppError::Git`.
pub fn commit_diff(workdir: &Path, oid: &str) -> Result<CommitDiff, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let commit = repo.find_commit(git2::Oid::from_str(oid)?)?;
    let details = commit_details(&commit);
    let (old_tree, new_tree) = commit_trees(&commit)?;
    let mut opts = build_diff_options(&[], false);
    let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let files = collect_headers(&diff)?;
    Ok(CommitDiff { details, files })
}

/// Hunks for ONE file of the commit-vs-first-parent diff (contract §2.2).
/// No matching delta -> `AppError::Git` (immutable input: the header list came
/// from the same commit, so this cannot be a benign race).
pub fn commit_file_diff(
    workdir: &Path,
    oid: &str,
    path: &str,
    orig_path: Option<&str>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    let repo = open_workdir_repo(workdir)?;
    let commit = repo.find_commit(git2::Oid::from_str(oid)?)?;
    let (old_tree, new_tree) = commit_trees(&commit)?;
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths, full_context);
    let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let fd = collect_file_diff(&diff)?
        .ok_or_else(|| AppError::Git(format!("path not changed in commit: {path}")))?;
    Ok(maybe_annotate(fd, intraline))
}

/// Resolve HEAD (attached or detached) as the OLD endpoint of a comparison
/// plus its tree. Unborn HEAD / `NotFound` -> `CompareEndpoint{"",""}` and no
/// tree (the compare-vs-empty-tree side, so everything shows Added).
/// `pub(crate)` so `image_diff` (P61b) can resolve the HEAD-side blob for the
/// Compare request.
pub(crate) fn head_endpoint(
    repo: &git2::Repository,
) -> Result<(CompareEndpoint, Option<git2::Tree<'_>>), AppError> {
    match repo.head() {
        Ok(h) => {
            let commit = h.peel_to_commit()?;
            let endpoint = CompareEndpoint {
                oid: commit.id().to_string(),
                summary: commit.summary_bytes().map(lossy).unwrap_or_default(),
            };
            let tree = commit.tree()?;
            Ok((endpoint, Some(tree)))
        }
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            Ok((
                CompareEndpoint {
                    oid: String::new(),
                    summary: String::new(),
                },
                None,
            ))
        }
        Err(e) => Err(e.into()),
    }
}

/// `git diff HEAD <to_oid>` as headers (P5 §2.2). HEAD (old side) is resolved
/// internally: attached or detached both work via `repo.head()`; unborn HEAD ->
/// old tree is the empty tree (everything shows Added) and `from` =
/// `CompareEndpoint{"",""}`. `from.oid == to_oid` (comparing HEAD to itself) ->
/// empty `files`, not an error. Bad/unknown/non-commit `to_oid` -> `AppError::Git`.
pub fn compare_head_diff(workdir: &Path, to_oid: &str) -> Result<CompareDiff, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let to_commit = repo.find_commit(git2::Oid::from_str(to_oid)?)?;
    let to_tree = to_commit.tree()?;
    let (from, old_tree) = head_endpoint(&repo)?;
    let to = CompareEndpoint {
        oid: to_commit.id().to_string(),
        summary: to_commit.summary_bytes().map(lossy).unwrap_or_default(),
    };
    let mut opts = build_diff_options(&[], false);
    let mut diff =
        repo.diff_tree_to_tree(old_tree.as_ref(), Some(&to_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let files = collect_headers(&diff)?;
    Ok(CompareDiff { from, to, files })
}

/// Hunks for ONE file of the HEAD → `to_oid` comparison (P5 §2.2; shape mirrors
/// `commit_file_diff`). No matching delta -> `AppError::Git`.
pub fn compare_head_file_diff(
    workdir: &Path,
    to_oid: &str,
    path: &str,
    orig_path: Option<&str>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    let repo = open_workdir_repo(workdir)?;
    let to_commit = repo.find_commit(git2::Oid::from_str(to_oid)?)?;
    let to_tree = to_commit.tree()?;
    let (_from, old_tree) = head_endpoint(&repo)?;
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths, full_context);
    let mut diff =
        repo.diff_tree_to_tree(old_tree.as_ref(), Some(&to_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let fd = collect_file_diff(&diff)?
        .ok_or_else(|| AppError::Git(format!("path not changed in comparison: {path}")))?;
    Ok(maybe_annotate(fd, intraline))
}
