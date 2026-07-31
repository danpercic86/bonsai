//! Per-line blame + per-file commit history (P23 contract §9).
//!
//! Pure git2 logic, no Tauri types. Both fns are BLOCKING (wrapped in
//! `spawn_blocking` at the command layer). `blame_file` attributes each line of
//! a COMMITTED version of a file (as of HEAD or an explicit `at_oid`, OPEN #8 —
//! dirty-worktree blame is out of scope v1). `file_history` walks HEAD's
//! first-parent-ish history for commits that touched a path, best-effort
//! following a single rename (OPEN #10).

use std::collections::HashMap;
use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{open_workdir_repo, validate_rel_path};

/// Hard cap on blamed lines (OPEN #9): a larger file is rejected rather than
/// streamed. Streaming blame over a channel is a later item.
pub const MAX_BLAME_LINES: usize = 50_000;

/// Built-in cap on file-history length when the caller passes `limit == 0`.
pub const MAX_HISTORY: usize = 1000;

/// One blamed line (contract §9.1). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    /// 40-hex of the commit that last touched this line.
    pub oid: String,
    /// Author display name (lossy UTF-8).
    pub author_name: String,
    /// Author email (lossy UTF-8).
    pub author_email: String,
    /// Author time, seconds since the Unix epoch (UTC).
    pub author_ts: i64,
    /// First line of that commit's message (gutter hover).
    pub summary: String,
    /// 1-based line number in the introducing commit.
    pub orig_line_no: u32,
    /// 1-based line number in the blamed version.
    pub final_line_no: u32,
    /// Line content without its trailing newline (lossy UTF-8).
    pub line_text: String,
}

/// One commit that touched a file (contract §9.2). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryEntry {
    pub oid: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub author_ts: i64,
}

/// Cached per-commit author identity + summary, so blame does not call
/// `find_commit` once per line (only once per distinct commit).
struct CommitMeta {
    author_name: String,
    author_email: String,
    author_ts: i64,
    summary: String,
}

fn commit_meta(repo: &git2::Repository, oid: git2::Oid) -> Result<CommitMeta, AppError> {
    let commit = repo.find_commit(oid)?;
    let author = commit.author();
    Ok(CommitMeta {
        author_name: String::from_utf8_lossy(author.name_bytes()).into_owned(),
        author_email: String::from_utf8_lossy(author.email_bytes()).into_owned(),
        author_ts: author.when().seconds(),
        summary: commit
            .summary_bytes()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default(),
    })
}

/// Reads `path`'s blob content from `tree`, rejecting a binary blob or a path
/// that is absent / not a blob (directory, submodule).
fn read_tree_blob(repo: &git2::Repository, tree: &git2::Tree, path: &str) -> Result<Vec<u8>, AppError> {
    let entry = tree
        .get_path(Path::new(path))
        .map_err(|_| AppError::Git(format!("path not found in commit: {path}")))?;
    let object = entry.to_object(repo)?;
    let blob = object
        .as_blob()
        .ok_or_else(|| AppError::Git(format!("not a file: {path}")))?;
    if blob.is_binary() {
        return Err(AppError::Git("cannot blame a binary file".to_string()));
    }
    Ok(blob.content().to_vec())
}

/// Blocking. Per-line blame of `path` as of `at_oid` (`None` -> HEAD, OPEN #8).
///
/// Rejects traversing paths (`validate_rel_path` -> `Other`); binary files,
/// unknown paths, and invalid oids -> `Git`; caps at `MAX_BLAME_LINES`.
///
/// Lines are mapped to commits by iterating the blame hunks: each hunk covers
/// `lines_in_hunk` contiguous lines starting at `final_start_line` and resolves
/// to a single `final_commit_id`, so the introducing commit / author is looked
/// up ONCE per hunk (cached across hunks). `line_text` is drawn from the blamed
/// blob content by `final_line_no`.
pub fn blame_file(
    workdir: &Path,
    path: &str,
    at_oid: Option<&str>,
) -> Result<Vec<BlameLine>, AppError> {
    validate_rel_path(path)?;
    let repo = open_workdir_repo(workdir)?;

    // Resolve the newest commit for both the blame walk and the blob read.
    let newest = match at_oid {
        Some(o) => {
            let oid = git2::Oid::from_str(o).map_err(|_| AppError::Git("invalid commit id".to_string()))?;
            repo.find_commit(oid)?
        }
        None => repo.head()?.peel_to_commit()?,
    };

    // Blame (rename/copy tracking OFF for v1).
    let mut opts = git2::BlameOptions::new();
    opts.newest_commit(newest.id());
    let blame = repo.blame_file(Path::new(path), Some(&mut opts))?;

    // The blamed blob's content (NOT the worktree, OPEN #8). `str::lines`
    // splits on `\n`, strips a trailing `\r`, and yields no phantom trailing
    // empty line for a final newline.
    let content = read_tree_blob(&repo, &newest.tree()?, path)?;
    let text = String::from_utf8_lossy(&content);
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_BLAME_LINES {
        return Err(AppError::Git(format!(
            "file too large to blame (> {MAX_BLAME_LINES} lines)"
        )));
    }

    let mut cache: HashMap<git2::Oid, CommitMeta> = HashMap::new();
    let mut out: Vec<BlameLine> = Vec::with_capacity(lines.len());

    for hunk in blame.iter() {
        let commit_oid = hunk.final_commit_id();
        let meta = match cache.entry(commit_oid) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(commit_meta(&repo, commit_oid)?)
            }
        };
        let final_start = hunk.final_start_line();
        let orig_start = hunk.orig_start_line();
        let oid_hex = commit_oid.to_string();
        for i in 0..hunk.lines_in_hunk() {
            let final_line_no = final_start + i;
            let line_text = lines
                .get(final_line_no - 1)
                .map(|s| (*s).to_string())
                .unwrap_or_default();
            out.push(BlameLine {
                oid: oid_hex.clone(),
                author_name: meta.author_name.clone(),
                author_email: meta.author_email.clone(),
                author_ts: meta.author_ts,
                summary: meta.summary.clone(),
                orig_line_no: (orig_start + i) as u32,
                final_line_no: final_line_no as u32,
                line_text,
            });
        }
    }

    Ok(out)
}

/// True iff `diff` contains a delta whose new OR old path equals `path`.
fn touches(diff: &git2::Diff, path: &str) -> bool {
    diff.deltas().any(|d| {
        d.new_file().path().and_then(|p| p.to_str()) == Some(path)
            || d.old_file().path().and_then(|p| p.to_str()) == Some(path)
    })
}

/// Blocking. Commits that modified `path`, newest-first, best-effort following a
/// SINGLE rename (OPEN #10). `limit` caps the result; `0` -> `MAX_HISTORY`.
///
/// A commit is kept iff it TOUCHES the currently-followed path relative to its
/// first parent (the root commit's add counts). Rename-follow: when the path is
/// ADDED at a commit relative to its parent, a rename-detecting diff of that
/// commit is consulted; if a `Renamed` delta produced the followed path, the
/// walk retargets to the old name for older commits. If the rename cannot be
/// traced unambiguously, following stops (the history simply ends at the add) —
/// this is the documented degrade-to-no-follow behaviour.
///
/// An unknown path at HEAD yields an empty history (`[]`), not an error.
pub fn file_history(workdir: &Path, path: &str, limit: u32) -> Result<Vec<FileHistoryEntry>, AppError> {
    validate_rel_path(path)?;
    let repo = open_workdir_repo(workdir)?;

    let cap = if limit == 0 {
        MAX_HISTORY
    } else {
        (limit as usize).min(MAX_HISTORY)
    };

    // Unborn HEAD -> no history.
    let head_commit = match repo.head() {
        Ok(h) => h.peel_to_commit()?,
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(e) => return Err(e.into()),
    };

    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.push(head_commit.id())?;

    let mut current_path = path.to_string();
    let mut out: Vec<FileHistoryEntry> = Vec::new();

    for oid in walk {
        if out.len() >= cap {
            break;
        }
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let new_tree = commit.tree()?;
        let parent = commit.parent(0).ok();
        let old_tree = match &parent {
            Some(p) => Some(p.tree()?),
            None => None,
        };

        // Cheap, rename-agnostic touched check restricted to the followed path.
        let mut opts = git2::DiffOptions::new();
        opts.pathspec(&current_path).disable_pathspec_match(true);
        let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;

        if !touches(&diff, &current_path) {
            continue;
        }

        let author = commit.author();
        out.push(FileHistoryEntry {
            oid: oid.to_string(),
            summary: commit
                .summary_bytes()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default(),
            author_name: String::from_utf8_lossy(author.name_bytes()).into_owned(),
            author_email: String::from_utf8_lossy(author.email_bytes()).into_owned(),
            author_ts: author.when().seconds(),
        });

        // Rename-follow: only worth checking when the path first appears here
        // relative to the parent (i.e. it is NOT present in the parent tree).
        // A full rename-detecting diff of this commit then tells us the old name.
        let appeared = parent
            .as_ref()
            .map(|p| p.tree().ok().and_then(|t| t.get_path(Path::new(&current_path)).ok()).is_none())
            .unwrap_or(false);
        if appeared {
            if let Some(old_name) = detect_rename_origin(&repo, old_tree.as_ref(), &new_tree, &current_path)? {
                current_path = old_name;
            }
        }
    }

    Ok(out)
}

/// Runs a rename-detecting diff of `old_tree -> new_tree` (unrestricted, so both
/// the deleted old side and the added new side are present to pair) and returns
/// the OLD path of a `Renamed`/`Copied` delta whose NEW path is `path`, if any.
fn detect_rename_origin(
    repo: &git2::Repository,
    old_tree: Option<&git2::Tree>,
    new_tree: &git2::Tree,
    path: &str,
) -> Result<Option<String>, AppError> {
    let mut opts = git2::DiffOptions::new();
    let mut diff = repo.diff_tree_to_tree(old_tree, Some(new_tree), Some(&mut opts))?;
    let mut find = git2::DiffFindOptions::new();
    find.renames(true).copies(true);
    diff.find_similar(Some(&mut find))?;

    for d in diff.deltas() {
        let is_rename = matches!(d.status(), git2::Delta::Renamed | git2::Delta::Copied);
        if !is_rename {
            continue;
        }
        let new_path = d.new_file().path().and_then(|p| p.to_str());
        if new_path == Some(path) {
            if let Some(old) = d.old_file().path().and_then(|p| p.to_str()) {
                if old != path {
                    return Ok(Some(old.to_string()));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `BlameLine` serializes with EXACTLY the camelCase keys the TS wire type
    /// declares (contract §9.3 / §10.1).
    #[test]
    fn blame_line_wire_shape_is_camel_case() {
        let v = serde_json::to_value(BlameLine {
            oid: "abc".to_string(),
            author_name: "Ada".to_string(),
            author_email: "ada@example.com".to_string(),
            author_ts: 1_700_000_000,
            summary: "init".to_string(),
            orig_line_no: 1,
            final_line_no: 2,
            line_text: "let x = 1;".to_string(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "oid": "abc",
                "authorName": "Ada",
                "authorEmail": "ada@example.com",
                "authorTs": 1_700_000_000i64,
                "summary": "init",
                "origLineNo": 1,
                "finalLineNo": 2,
                "lineText": "let x = 1;"
            })
        );
    }

    /// `FileHistoryEntry` serializes with EXACTLY the camelCase keys the TS wire
    /// type declares.
    #[test]
    fn file_history_entry_wire_shape_is_camel_case() {
        let v = serde_json::to_value(FileHistoryEntry {
            oid: "def".to_string(),
            summary: "edit".to_string(),
            author_name: "Grace".to_string(),
            author_email: "grace@example.com".to_string(),
            author_ts: 42,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "oid": "def",
                "summary": "edit",
                "authorName": "Grace",
                "authorEmail": "grace@example.com",
                "authorTs": 42
            })
        );
    }

    /// A traversing / absolute / backslash path is rejected as `Other` BEFORE
    /// any repo access (reuses `validate_rel_path`).
    #[test]
    fn blame_rejects_bad_path() {
        let dir = std::env::temp_dir();
        let err = blame_file(&dir, "../secret", None).expect_err("must reject ..");
        assert!(matches!(err, AppError::Other(_)), "got {err:?}");

        let err = file_history(&dir, "../secret", 10).expect_err("must reject ..");
        assert!(matches!(err, AppError::Other(_)), "got {err:?}");
    }
}
