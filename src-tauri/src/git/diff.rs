//! Diff core (M4 contract §2).
//!
//! Pure git2 logic, no Tauri types. Three modes share one engine:
//! workdir unstaged (index vs workdir), staged (HEAD tree vs index, unborn ->
//! empty tree), and commit vs FIRST parent (root -> empty tree, merge ->
//! first parent only). Per-file hunk payloads are capped at
//! [`MAX_FILE_DIFF_LINES`] emitted lines, all-or-nothing (`too_large`).

use std::cell::RefCell;
use std::path::Path;

use crate::error::AppError;
use crate::git::stage::{open_workdir_repo, validate_rel_path};
use crate::git::status::FileStatus;

/// Total emitted [`DiffLine`]s per file (contract §2.6). Exceeded -> abort
/// iteration, `too_large: true`, `hunks: []` (all-or-nothing).
pub const MAX_FILE_DIFF_LINES: usize = 5_000;

/// Kind of one diff line. Serialized as `"context" | "add" | "del"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Context,
    Add,
    Del,
}

/// One line of a hunk (contract §2.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    /// Line number in the OLD file; `None` for Add lines.
    pub old_no: Option<u32>,
    /// Line number in the NEW file; `None` for Del lines.
    pub new_no: Option<u32>,
    /// Content WITHOUT the leading `+`/`-`/space and WITHOUT the trailing
    /// newline (§2.4: lossy UTF-8, strip one `\n` then one `\r`).
    pub content: String,
    /// True when this is the last line of a file that has no trailing newline
    /// (the CLI's `\ No newline at end of file` marker — never emitted as its
    /// own line).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_newline: bool,
}

/// One hunk. No raw header string on the wire: the frontend renders
/// `@@ -a,b +c,d @@` from the numbers (function-context tail dropped).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

/// Full diff of ONE file (contract §2.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    /// NEW path for renames; repo-relative, forward slashes.
    pub path: String,
    /// OLD path for renames; `None` otherwise.
    pub orig_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,    // true -> hunks empty
    pub too_large: bool, // true -> hunks empty (§2.6)
    pub hunks: Vec<Hunk>,
}

/// Per-file header (no hunks) for the commit-diff file list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffHeader {
    pub path: String,
    pub orig_path: Option<String>,
    pub status: FileStatus,
    pub additions: u32, // count of Add lines (0 for binary)
    pub deletions: u32, // count of Del lines (0 for binary)
    pub binary: bool,
}

/// Details of one commit (right-panel header).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub oid: String,     // full 40-char hex
    pub summary: String, // first line
    /// Full message, trailing whitespace trimmed. Includes the summary line.
    pub message: String,
    pub author_name: String, // lossy UTF-8
    pub author_email: String,
    pub author_ts: i64,    // seconds since epoch (UTC)
    pub committer_ts: i64, // carried for free; UI v1 shows author only
    /// Full parent oids, first parent first. len > 1 => merge commit.
    pub parents: Vec<String>,
}

/// Commit details + per-file headers (no hunks — lazy per-file fetch).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDiff {
    pub details: CommitDetails,
    /// Sorted by path ascending (byte-wise). Headers only.
    pub files: Vec<FileDiffHeader>,
}

/// Lossy decode of a byte path.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// §2.4: lossy UTF-8, strip exactly one trailing `\n` if present, then
/// exactly one trailing `\r` if present. Mid-line `\r` is preserved.
fn normalize_content(bytes: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(bytes).into_owned();
    if s.ends_with('\n') {
        s.pop();
    }
    if s.ends_with('\r') {
        s.pop();
    }
    s
}

/// §2.7: git2 `Delta` -> `FileStatus`. `Copied -> Renamed` (defensive; copies
/// are disabled). Anything unexpected -> `Modified` — never panic.
fn map_status(delta: git2::Delta) -> FileStatus {
    match delta {
        git2::Delta::Added => FileStatus::Added,
        git2::Delta::Deleted => FileStatus::Deleted,
        git2::Delta::Modified => FileStatus::Modified,
        git2::Delta::Renamed | git2::Delta::Copied => FileStatus::Renamed,
        git2::Delta::Typechange => FileStatus::Typechange,
        git2::Delta::Untracked => FileStatus::Untracked,
        git2::Delta::Conflicted => FileStatus::Conflicted,
        _ => FileStatus::Modified, // Unmodified | Ignored | Unreadable: unreachable in practice
    }
}

/// §2.3: fixed diff options (context 3; untracked content included — harmless
/// for tree-to-tree) restricted to `paths` when non-empty.
fn build_diff_options(paths: &[&str]) -> git2::DiffOptions {
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3)
        .include_untracked(true)
        .show_untracked_content(true)
        .recurse_untracked_dirs(true);
    for p in paths {
        opts.pathspec(p);
    }
    opts
}

/// Rename detection (renames only, no copies), applied AFTER the pathspec
/// restriction so old+new rename sides pair into one delta.
fn apply_find_similar(diff: &mut git2::Diff) -> Result<(), AppError> {
    let mut find = git2::DiffFindOptions::new();
    find.renames(true);
    diff.find_similar(Some(&mut find))?;
    Ok(())
}

/// HEAD tree, or `None` when HEAD is unborn (empty-tree diff side).
fn head_tree(repo: &git2::Repository) -> Result<Option<git2::Tree<'_>>, AppError> {
    match repo.head() {
        Ok(h) => Ok(Some(h.peel_to_tree()?)),
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

/// Shared state for the single-file `foreach` collection (§2.3). Callbacks
/// are separate closures, so the state lives in a `RefCell` they all borrow.
struct Collect {
    seen: bool,
    path: String,
    orig_path: Option<String>,
    status: FileStatus,
    binary: bool,
    aborted: bool,
    hunks: Vec<Hunk>,
    cur: Option<Hunk>,
    emitted: usize,
}

/// Walks a (pathspec-restricted) diff and collects ONE file's hunks.
/// `Ok(None)` when the diff contains no delta at all (pathspec matched
/// nothing) — callers decide whether that is benign (§2.2).
fn collect_file_diff(diff: &git2::Diff) -> Result<Option<FileDiff>, AppError> {
    let state = RefCell::new(Collect {
        seen: false,
        path: String::new(),
        orig_path: None,
        status: FileStatus::Modified,
        binary: false,
        aborted: false,
        hunks: Vec::new(),
        cur: None,
        emitted: 0,
    });

    let mut file_cb = |delta: git2::DiffDelta, _progress: f32| -> bool {
        let mut s = state.borrow_mut();
        s.seen = true;
        s.status = map_status(delta.status());
        s.path = delta
            .new_file()
            .path_bytes()
            .or_else(|| delta.old_file().path_bytes())
            .map(lossy)
            .unwrap_or_default();
        s.orig_path = match delta.status() {
            git2::Delta::Renamed | git2::Delta::Copied => {
                delta.old_file().path_bytes().map(lossy)
            }
            _ => None,
        };
        if delta.flags().is_binary() {
            s.binary = true;
        }
        true
    };
    // Required so libgit2 reports binary files at all (§2.3).
    let mut binary_cb = |_delta: git2::DiffDelta, _binary: git2::DiffBinary| -> bool {
        state.borrow_mut().binary = true;
        true
    };
    let mut hunk_cb = |_delta: git2::DiffDelta, hunk: git2::DiffHunk| -> bool {
        let mut s = state.borrow_mut();
        if let Some(prev) = s.cur.take() {
            s.hunks.push(prev);
        }
        s.cur = Some(Hunk {
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
            lines: Vec::new(),
        });
        true
    };
    let mut line_cb =
        |_delta: git2::DiffDelta, _hunk: Option<git2::DiffHunk>, line: git2::DiffLine| -> bool {
            let mut s = state.borrow_mut();
            match line.origin() {
                origin @ (' ' | '+' | '-') => {
                    if s.emitted >= MAX_FILE_DIFF_LINES {
                        s.aborted = true;
                        return false; // aborts foreach with GIT_EUSER
                    }
                    let kind = match origin {
                        ' ' => LineKind::Context,
                        '+' => LineKind::Add,
                        _ => LineKind::Del,
                    };
                    let dl = DiffLine {
                        kind,
                        old_no: line.old_lineno(),
                        new_no: line.new_lineno(),
                        content: normalize_content(line.content()),
                        no_newline: false,
                    };
                    if let Some(cur) = s.cur.as_mut() {
                        cur.lines.push(dl);
                        s.emitted += 1;
                    }
                    true
                }
                // EOFNL markers: flag the LAST pushed line; emit nothing.
                '=' | '>' | '<' => {
                    if let Some(last) = s.cur.as_mut().and_then(|c| c.lines.last_mut()) {
                        last.no_newline = true;
                    }
                    true
                }
                _ => true, // 'F' | 'H' | 'B': ignore
            }
        };

    let result = diff.foreach(
        &mut file_cb,
        Some(&mut binary_cb),
        Some(&mut hunk_cb),
        Some(&mut line_cb),
    );
    let mut s = state.into_inner();
    match result {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::User && s.aborted => {}
        Err(e) => return Err(e.into()),
    }

    if !s.seen {
        return Ok(None);
    }
    let (binary, too_large, hunks) = if s.binary {
        (true, false, Vec::new())
    } else if s.aborted {
        (false, true, Vec::new()) // all-or-nothing (§2.6)
    } else {
        if let Some(cur) = s.cur.take() {
            s.hunks.push(cur);
        }
        (false, false, s.hunks)
    };
    Ok(Some(FileDiff {
        path: s.path,
        orig_path: s.orig_path,
        status: s.status,
        binary,
        too_large,
        hunks,
    }))
}

/// Header collection over an UNRESTRICTED diff: counts only, no content
/// strings, no line budget (§2.3).
fn collect_headers(diff: &git2::Diff) -> Result<Vec<FileDiffHeader>, AppError> {
    let files: RefCell<Vec<FileDiffHeader>> = RefCell::new(Vec::new());

    let mut file_cb = |delta: git2::DiffDelta, _progress: f32| -> bool {
        files.borrow_mut().push(FileDiffHeader {
            path: delta
                .new_file()
                .path_bytes()
                .or_else(|| delta.old_file().path_bytes())
                .map(lossy)
                .unwrap_or_default(),
            orig_path: match delta.status() {
                git2::Delta::Renamed | git2::Delta::Copied => {
                    delta.old_file().path_bytes().map(lossy)
                }
                _ => None,
            },
            status: map_status(delta.status()),
            additions: 0,
            deletions: 0,
            binary: delta.flags().is_binary(),
        });
        true
    };
    let mut binary_cb = |_delta: git2::DiffDelta, _binary: git2::DiffBinary| -> bool {
        if let Some(last) = files.borrow_mut().last_mut() {
            last.binary = true;
        }
        true
    };
    let mut line_cb =
        |_delta: git2::DiffDelta, _hunk: Option<git2::DiffHunk>, line: git2::DiffLine| -> bool {
            if let Some(last) = files.borrow_mut().last_mut() {
                match line.origin() {
                    '+' => last.additions += 1,
                    '-' => last.deletions += 1,
                    _ => {}
                }
            }
            true
        };

    diff.foreach(&mut file_cb, Some(&mut binary_cb), None, Some(&mut line_cb))?;

    let mut files = files.into_inner();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

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
/// root commit, the commit's own tree)`.
fn commit_trees<'r>(
    commit: &git2::Commit<'r>,
) -> Result<(Option<git2::Tree<'r>>, git2::Tree<'r>), AppError> {
    let old = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };
    Ok((old, commit.tree()?))
}

/// Pathspec list for one file: the path itself plus the rename OLD side.
fn pathspecs<'a>(path: &'a str, orig_path: Option<&'a str>) -> Vec<&'a str> {
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
) -> Result<FileDiff, AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    let repo = open_workdir_repo(workdir)?;
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths);
    let mut diff = if staged {
        let old = head_tree(&repo)?;
        repo.diff_tree_to_index(old.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };
    apply_find_similar(&mut diff)?;
    Ok(collect_file_diff(&diff)?.unwrap_or_else(|| FileDiff {
        path: path.to_string(),
        orig_path: None,
        status: FileStatus::Modified,
        binary: false,
        too_large: false,
        hunks: Vec::new(),
    }))
}

/// Commit details + per-file headers for `oid` vs its FIRST parent
/// (contract §2.2). Root commit -> vs empty tree. Merge -> first parent only.
/// Bad/unknown/non-commit oid -> `AppError::Git`.
pub fn commit_diff(workdir: &Path, oid: &str) -> Result<CommitDiff, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let commit = repo.find_commit(git2::Oid::from_str(oid)?)?;
    let details = commit_details(&commit);
    let (old_tree, new_tree) = commit_trees(&commit)?;
    let mut opts = build_diff_options(&[]);
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
) -> Result<FileDiff, AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    let repo = open_workdir_repo(workdir)?;
    let commit = repo.find_commit(git2::Oid::from_str(oid)?)?;
    let (old_tree, new_tree) = commit_trees(&commit)?;
    let paths = pathspecs(path, orig_path);
    let mut opts = build_diff_options(&paths);
    let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    collect_file_diff(&diff)?
        .ok_or_else(|| AppError::Git(format!("path not changed in commit: {path}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_content_strips_one_newline_then_one_cr() {
        assert_eq!(normalize_content(b"plain"), "plain");
        assert_eq!(normalize_content(b"lf\n"), "lf");
        assert_eq!(normalize_content(b"crlf\r\n"), "crlf");
        assert_eq!(normalize_content(b"cr-only\r"), "cr-only");
        // Only ONE of each is stripped; interior \r preserved.
        assert_eq!(normalize_content(b"a\r\r\n"), "a\r");
        assert_eq!(normalize_content(b"a\n\n"), "a\n");
        assert_eq!(normalize_content(b"mid\rline\n"), "mid\rline");
        // Lossy UTF-8, never an error.
        assert_eq!(normalize_content(b"\xff\xfe\n"), "\u{fffd}\u{fffd}");
    }

    #[test]
    fn delta_status_map_matches_contract() {
        assert_eq!(map_status(git2::Delta::Added), FileStatus::Added);
        assert_eq!(map_status(git2::Delta::Deleted), FileStatus::Deleted);
        assert_eq!(map_status(git2::Delta::Modified), FileStatus::Modified);
        assert_eq!(map_status(git2::Delta::Renamed), FileStatus::Renamed);
        assert_eq!(map_status(git2::Delta::Copied), FileStatus::Renamed);
        assert_eq!(map_status(git2::Delta::Typechange), FileStatus::Typechange);
        assert_eq!(map_status(git2::Delta::Untracked), FileStatus::Untracked);
        assert_eq!(map_status(git2::Delta::Conflicted), FileStatus::Conflicted);
        assert_eq!(map_status(git2::Delta::Unmodified), FileStatus::Modified);
        assert_eq!(map_status(git2::Delta::Ignored), FileStatus::Modified);
        assert_eq!(map_status(git2::Delta::Unreadable), FileStatus::Modified);
    }

    /// Wire shape: camelCase keys, `noNewline` omitted when false, kinds as
    /// lowercase strings.
    #[test]
    fn wire_serialization_shape() {
        let fd = FileDiff {
            path: "a.txt".to_string(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: false,
            too_large: false,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
                lines: vec![
                    DiffLine {
                        kind: LineKind::Del,
                        old_no: Some(1),
                        new_no: None,
                        content: "old".to_string(),
                        no_newline: false,
                    },
                    DiffLine {
                        kind: LineKind::Add,
                        old_no: None,
                        new_no: Some(1),
                        content: "new".to_string(),
                        no_newline: true,
                    },
                ],
            }],
        };
        let json = serde_json::to_string(&fd).expect("serialize FileDiff");
        assert!(json.contains("\"origPath\":null"), "{json}");
        assert!(json.contains("\"tooLarge\":false"), "{json}");
        assert!(json.contains("\"oldStart\":1"), "{json}");
        assert!(json.contains("\"kind\":\"del\""), "{json}");
        assert!(json.contains("\"kind\":\"add\""), "{json}");
        assert!(json.contains("\"noNewline\":true"), "{json}");
        // no_newline: false is skipped entirely.
        assert_eq!(json.matches("noNewline").count(), 1, "{json}");
    }

    /// The benign-race contract (§2.2): a clean path yields an empty FileDiff,
    /// not an error — for both staged and unstaged modes.
    #[test]
    fn clean_path_returns_empty_filediff() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        crate::git::commit::create_commit(dir.path(), "base").expect("commit");

        for staged in [false, true] {
            let fd = workdir_file_diff(dir.path(), "a.txt", None, staged)
                .expect("clean path must not error");
            assert_eq!(fd.path, "a.txt");
            assert_eq!(fd.status, FileStatus::Modified);
            assert!(!fd.binary && !fd.too_large);
            assert!(fd.hunks.is_empty());
        }
    }

    #[test]
    fn invalid_paths_are_rejected() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");
        for bad in ["", "../escape", "/abs", "a\\b"] {
            let err = workdir_file_diff(dir.path(), bad, None, false)
                .expect_err(&format!("must reject {bad:?}"));
            assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
        }
        // orig_path is validated too.
        let err = workdir_file_diff(dir.path(), "ok.txt", Some("../escape"), false)
            .expect_err("must reject bad orig_path");
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }
}
