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

/// Context (and interhunk) line count for a full-context "File View" diff
/// (§2.6). A large FINITE value — `u32::MAX` overflows libgit2's xdiff context
/// math. Comfortably larger than [`MAX_FILE_DIFF_LINES`], so any file that is
/// not already `too_large` collapses to one whole-file hunk.
const FULL_CONTEXT_LINES: u32 = 1_000_000;

/// Kind of one diff line. Serialized as `"context" | "add" | "del"`.
/// `Deserialize` is derived so `stage_partial::LineSelection` (P17) can carry a
/// `LineKind` across the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// P61a: CHANGED sub-ranges within `content` as `[start, len]`, measured in
    /// Unicode SCALAR VALUES (code points / `char`s), NOT bytes and NOT UTF-16
    /// units. Present only on `add`/`del` lines PAIRED with a counterpart when
    /// `intraline=true`; empty on context lines, unpaired pure-add/pure-del
    /// blocks, and whenever `intraline=false`. Ascending + non-overlapping.
    /// Default-empty => wire-invisible (byte-identical to pre-P61a when off).
    /// Char offsets (over UTF-16) are natural in Rust (`char_indices`); the
    /// frontend slices via `Array.from(content)`. Guarded by a multibyte test.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub spans: Vec<[u32; 2]>,
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

/// One endpoint of a two-commit comparison (P5 §1.2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEndpoint {
    pub oid: String,     // full 40-char hex; "" when HEAD is unborn (old side)
    pub summary: String, // first line of that commit's message; "" when unborn
}

/// Tree-vs-tree comparison HEAD(old) → `to`(new). Headers only — hunks fetched
/// per file, exactly like `CommitDiff`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareDiff {
    pub from: CompareEndpoint, // OLD = HEAD
    pub to: CompareEndpoint,   // NEW = the right-clicked commit
    /// Sorted path-ascending (byte-wise). Empty when from.oid == to.oid.
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

/// §2.3: fixed diff options (untracked content included — harmless for
/// tree-to-tree) restricted to `paths` when non-empty.
///
/// `full_context` (P17 §2.6): `true` -> `context_lines(FULL_CONTEXT_LINES)`
/// (one whole-file hunk, the File View — `u32::MAX` overflows libgit2's xdiff
/// context math, so a large finite value is used); `false` -> `context_lines(3)`
/// (the M4 default). Context amount never changes add/del line numbers, so a
/// File-View selection maps onto the default-context partial-staging diff
/// unchanged.
pub(crate) fn build_diff_options(paths: &[&str], full_context: bool) -> git2::DiffOptions {
    let mut opts = git2::DiffOptions::new();
    // §2.6: `full_context` collapses the file to ONE whole-file hunk (File
    // View). `u32::MAX` overflows libgit2's xdiff context arithmetic (it wraps
    // to a tiny context and the file stays split); a large FINITE value is the
    // working equivalent. `FULL_CONTEXT_LINES` (> the 5000-line cap) is enough
    // to cover every file that is not already `too_large`. A matching
    // `interhunk_lines` guarantees separated changes merge into one hunk (that
    // merge is decided by the unchanged gap vs `interhunk_lines`, NOT context).
    let ctx = if full_context { FULL_CONTEXT_LINES } else { 3 };
    opts.context_lines(ctx)
        .include_untracked(true)
        .show_untracked_content(true)
        .recurse_untracked_dirs(true);
    if full_context {
        opts.interhunk_lines(FULL_CONTEXT_LINES);
    }
    if !paths.is_empty() {
        // Explicit pathspecs are LITERAL paths from StatusEntry / headers, not
        // globs: without this a file named `a[bc].txt` would fnmatch other
        // deltas and merge them into one corrupted FileDiff.
        opts.disable_pathspec_match(true);
    }
    for p in paths {
        opts.pathspec(p);
    }
    opts
}

/// Rename detection (renames only, no copies), applied AFTER the pathspec
/// restriction so old+new rename sides pair into one delta.
pub(crate) fn apply_find_similar(diff: &mut git2::Diff) -> Result<(), AppError> {
    let mut find = git2::DiffFindOptions::new();
    find.renames(true);
    diff.find_similar(Some(&mut find))?;
    Ok(())
}

/// HEAD tree, or `None` when HEAD is unborn (empty-tree diff side).
pub(crate) fn head_tree(repo: &git2::Repository) -> Result<Option<git2::Tree<'_>>, AppError> {
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
    /// The pathspec matched a SECOND, different-path delta (rename pairing
    /// failed): merging both files' hunks would corrupt the FileDiff → abort
    /// the walk and error out.
    multi: bool,
    hunks: Vec<Hunk>,
    cur: Option<Hunk>,
    emitted: usize,
}

/// Walks a (pathspec-restricted) diff and collects ONE file's hunks.
/// `Ok(None)` when the diff contains no delta at all (pathspec matched
/// nothing) — callers decide whether that is benign (§2.2).
pub(crate) fn collect_file_diff(diff: &git2::Diff) -> Result<Option<FileDiff>, AppError> {
    let state = RefCell::new(Collect {
        seen: false,
        path: String::new(),
        orig_path: None,
        status: FileStatus::Modified,
        binary: false,
        aborted: false,
        multi: false,
        hunks: Vec::new(),
        cur: None,
        emitted: 0,
    });

    let mut file_cb = |delta: git2::DiffDelta, _progress: f32| -> bool {
        let mut s = state.borrow_mut();
        let path = delta
            .new_file()
            .path_bytes()
            .or_else(|| delta.old_file().path_bytes())
            .map(lossy)
            .unwrap_or_default();
        // A second delta with a DIFFERENT path: refuse rather than silently
        // merging two files' hunks into one FileDiff. A same-path re-entry
        // (split hunk batches) keeps accumulating as before.
        if s.seen && path != s.path {
            s.multi = true;
            return false; // aborts foreach with GIT_EUSER
        }
        s.seen = true;
        s.status = map_status(delta.status());
        s.path = path;
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
                        spans: Vec::new(),
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
        Err(e) if e.code() == git2::ErrorCode::User && s.multi => {
            return Err(AppError::Git(
                "pathspec matched multiple diff deltas; refresh the diff".to_string(),
            ));
        }
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

/// Multi-file collection state (P25 §2.1). Holds the finished `FileDiff`s plus
/// the in-progress file's accumulator; each `file_cb` finalizes the previous
/// file and resets the per-file budget/flags (the plural of [`Collect`]).
struct MultiCollect {
    files: Vec<FileDiff>,
    started: bool,
    path: String,
    orig_path: Option<String>,
    status: FileStatus,
    binary: bool,
    /// This file blew the per-file line budget: keep iterating (never abort the
    /// whole diff) but drop its hunks and flag `too_large` at finalize.
    overflow: bool,
    hunks: Vec<Hunk>,
    cur: Option<Hunk>,
    emitted: usize,
}

impl MultiCollect {
    fn new() -> Self {
        MultiCollect {
            files: Vec::new(),
            started: false,
            path: String::new(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: false,
            overflow: false,
            hunks: Vec::new(),
            cur: None,
            emitted: 0,
        }
    }

    /// Finalize the in-progress file (if any) into `files`, then reset the
    /// per-file accumulator for the next delta. All-or-nothing per file
    /// (binary/too-large => empty hunks), exactly like the singular collector.
    fn finish_current(&mut self) {
        if !self.started {
            return;
        }
        let (binary, too_large, hunks) = if self.binary {
            (true, false, Vec::new())
        } else if self.overflow {
            (false, true, Vec::new())
        } else {
            if let Some(cur) = self.cur.take() {
                self.hunks.push(cur);
            }
            (false, false, std::mem::take(&mut self.hunks))
        };
        self.files.push(FileDiff {
            path: std::mem::take(&mut self.path),
            orig_path: self.orig_path.take(),
            status: self.status,
            binary,
            too_large,
            hunks,
        });
        // Reset for the next file.
        self.started = false;
        self.status = FileStatus::Modified;
        self.binary = false;
        self.overflow = false;
        self.hunks = Vec::new();
        self.cur = None;
        self.emitted = 0;
    }
}

/// Walks a MULTI-FILE diff and collects one [`FileDiff`] (with hunks) per delta,
/// in delta order (P25 §2.1). The plural of [`collect_file_diff`]: each new
/// `file_cb` finalizes the previous file, `hunk_cb`/`line_cb` append to the
/// current file, and the per-file [`MAX_FILE_DIFF_LINES`] budget resets per file
/// (an overflowing file is flagged `too_large` with empty hunks, exactly like
/// the singular fn — but iteration CONTINUES so sibling files still collect).
/// Binary files come back `binary:true` with empty hunks. Never fails on a
/// too-large file. Empty diff => empty `Vec`.
pub(crate) fn collect_file_diffs(diff: &git2::Diff) -> Result<Vec<FileDiff>, AppError> {
    let state = RefCell::new(MultiCollect::new());

    let mut file_cb = |delta: git2::DiffDelta, _progress: f32| -> bool {
        let mut s = state.borrow_mut();
        s.finish_current();
        s.started = true;
        s.status = map_status(delta.status());
        s.path = delta
            .new_file()
            .path_bytes()
            .or_else(|| delta.old_file().path_bytes())
            .map(lossy)
            .unwrap_or_default();
        s.orig_path = match delta.status() {
            git2::Delta::Renamed | git2::Delta::Copied => delta.old_file().path_bytes().map(lossy),
            _ => None,
        };
        if delta.flags().is_binary() {
            s.binary = true;
        }
        true
    };
    let mut binary_cb = |_delta: git2::DiffDelta, _binary: git2::DiffBinary| -> bool {
        state.borrow_mut().binary = true;
        true
    };
    let mut hunk_cb = |_delta: git2::DiffDelta, hunk: git2::DiffHunk| -> bool {
        let mut s = state.borrow_mut();
        // Once over budget the file's hunks are discarded at finalize; skip
        // allocating further hunk headers.
        if s.overflow {
            return true;
        }
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
                        // Over budget for THIS file: flag and stop appending, but
                        // keep returning true so the foreach walks sibling files.
                        s.overflow = true;
                        return true;
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
                        spans: Vec::new(),
                    };
                    if let Some(cur) = s.cur.as_mut() {
                        cur.lines.push(dl);
                        s.emitted += 1;
                    }
                    true
                }
                '=' | '>' | '<' => {
                    if let Some(last) = s.cur.as_mut().and_then(|c| c.lines.last_mut()) {
                        last.no_newline = true;
                    }
                    true
                }
                _ => true, // 'F' | 'H' | 'B': ignore
            }
        };

    diff.foreach(
        &mut file_cb,
        Some(&mut binary_cb),
        Some(&mut hunk_cb),
        Some(&mut line_cb),
    )?;

    let mut s = state.into_inner();
    s.finish_current();
    Ok(s.files)
}

/// Header collection over an UNRESTRICTED diff: counts only, no content
/// strings, no line budget (§2.3).
pub(crate) fn collect_headers(diff: &git2::Diff) -> Result<Vec<FileDiffHeader>, AppError> {
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
fn maybe_annotate(mut fd: FileDiff, intraline: bool) -> FileDiff {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    /// git2-init a scratch repo with identity + autocrlf off (mirrors the
    /// other tests in this module).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

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
                        spans: Vec::new(),
                    },
                    DiffLine {
                        kind: LineKind::Add,
                        old_no: None,
                        new_no: Some(1),
                        content: "new".to_string(),
                        no_newline: true,
                        spans: Vec::new(),
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
        // P61a: empty `spans` is wire-invisible (byte-identical to pre-P61a).
        // `intraline=false` never populates spans, so the key must not appear.
        assert!(!json.contains("spans"), "empty spans must be skipped: {json}");
    }

    /// P61a: when a diff is intraline-annotated, changed paired lines serialize
    /// a `spans` array of `[start, len]` code-point ranges; the byte-off case is
    /// the same fixture with `annotate_hunk` NOT run (guarded above).
    #[test]
    fn wire_serialization_spans_present_when_annotated() {
        let mut hunk = Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            lines: vec![
                DiffLine {
                    kind: LineKind::Del,
                    old_no: Some(1),
                    new_no: None,
                    content: "const x = 1;".to_string(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(1),
                    content: "const x = 42;".to_string(),
                    no_newline: false,
                    spans: Vec::new(),
                },
            ],
        };
        crate::git::intraline::annotate_hunk(&mut hunk);
        let json = serde_json::to_string(&hunk).expect("serialize Hunk");
        // Emphasis only on 1 -> 42 (code-point index 10).
        assert!(json.contains("\"spans\":[[10,1]]"), "del spans: {json}");
        assert!(json.contains("\"spans\":[[10,2]]"), "add spans: {json}");
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
        crate::git::commit::create_commit(dir.path(), "base", None, false).expect("commit");

        for staged in [false, true] {
            let fd = workdir_file_diff(dir.path(), "a.txt", None, staged, false, false)
                .expect("clean path must not error");
            assert_eq!(fd.path, "a.txt");
            assert_eq!(fd.status, FileStatus::Modified);
            assert!(!fd.binary && !fd.too_large);
            assert!(fd.hunks.is_empty());
        }
    }

    /// Pathspecs are literal (fixlet): a file whose NAME contains glob
    /// metachars must not fnmatch sibling deltas. `*` is illegal in Windows
    /// filenames, but `[`/`]` are legal AND are fnmatch metachars — the glob
    /// `a[ab].txt` would match `aa.txt` and `ab.txt`, merging three deltas
    /// into one corrupted FileDiff without `disable_pathspec_match`.
    #[test]
    fn glob_metachar_filename_matches_literally() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        for name in ["a[ab].txt", "aa.txt", "ab.txt"] {
            std::fs::write(dir.path().join(name), format!("{name} old\n")).expect("write");
        }
        crate::git::stage::stage_paths(
            dir.path(),
            &["a[ab].txt".into(), "aa.txt".into(), "ab.txt".into()],
        )
        .expect("stage");
        crate::git::commit::create_commit(dir.path(), "base", None, false).expect("commit");
        for name in ["a[ab].txt", "aa.txt", "ab.txt"] {
            std::fs::write(dir.path().join(name), format!("{name} new\n")).expect("rewrite");
        }

        let fd = workdir_file_diff(dir.path(), "a[ab].txt", None, false, false, false).expect("diff");
        assert_eq!(fd.path, "a[ab].txt");
        assert_eq!(fd.status, FileStatus::Modified);
        assert_eq!(fd.hunks.len(), 1, "exactly one delta must match");
        let lines = &fd.hunks[0].lines;
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].content, "a[ab].txt old");
        assert_eq!(lines[1].content, "a[ab].txt new");
    }

    #[test]
    fn invalid_paths_are_rejected() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");
        for bad in ["", "../escape", "/abs", "a\\b"] {
            let err = workdir_file_diff(dir.path(), bad, None, false, false, false)
                .expect_err(&format!("must reject {bad:?}"));
            assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
        }
        // orig_path is validated too.
        let err = workdir_file_diff(dir.path(), "ok.txt", Some("../escape"), false, false, false)
            .expect_err("must reject bad orig_path");
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }

    /// P5 §6.2: `compare_head_diff(HEAD, earlier)` on a LINEAR history. HEAD = B,
    /// `to` = A. Going B -> A: file1 Modified, file2 Deleted (matches
    /// `git diff --name-status HEAD A`). Endpoints carry oids + summaries.
    #[test]
    fn compare_head_diff_linear_history() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("file1.txt"), "one\n").expect("write");
        stage_paths(p, &["file1.txt".into()]).expect("stage");
        let a = create_commit(p, "A", None, false).expect("commit A").oid;

        std::fs::write(p.join("file1.txt"), "one changed\n").expect("write");
        std::fs::write(p.join("file2.txt"), "two\n").expect("write");
        stage_paths(p, &["file1.txt".into(), "file2.txt".into()]).expect("stage");
        let b = create_commit(p, "B", None, false).expect("commit B").oid;

        let cmp = compare_head_diff(p, &a).expect("compare");
        assert_eq!(cmp.to.oid, a);
        assert_eq!(cmp.to.summary, "A");
        assert_eq!(cmp.from.oid, b);
        assert_eq!(cmp.from.summary, "B");

        let got: Vec<(String, FileStatus)> = cmp
            .files
            .iter()
            .map(|f| (f.path.clone(), f.status))
            .collect();
        assert_eq!(
            got,
            vec![
                ("file1.txt".to_string(), FileStatus::Modified),
                ("file2.txt".to_string(), FileStatus::Deleted),
            ]
        );
    }

    /// P5 §6.2: `compare_head_diff(HEAD, branch_tip)` across diverged branches.
    /// main tip B has file_main; feat tip C has file_feat. HEAD = B; `to` = C.
    /// Going B -> C: file_feat Added, file_main Deleted (byte-sorted).
    #[test]
    fn compare_head_diff_branch_tip() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("file1.txt"), "base\n").expect("write");
        stage_paths(p, &["file1.txt".into()]).expect("stage");
        let base = create_commit(p, "A", None, false).expect("commit A");
        // Default branch name is git2's choice (master/main) — resolve it.
        let main_name = base.branch.expect("base commit is on a branch");

        // feat diverges from A.
        crate::git::branches::create_branch(p, "feat").expect("create feat");
        crate::git::branches::checkout_branch(p, "feat").expect("checkout feat");
        std::fs::write(p.join("file_feat.txt"), "feat\n").expect("write");
        stage_paths(p, &["file_feat.txt".into()]).expect("stage");
        let c = create_commit(p, "C", None, false).expect("commit C").oid;

        // Back to the default branch, add a divergent commit B (now HEAD = B).
        crate::git::branches::checkout_branch(p, &main_name).expect("checkout base branch");
        std::fs::write(p.join("file_main.txt"), "main\n").expect("write");
        stage_paths(p, &["file_main.txt".into()]).expect("stage");
        let b = create_commit(p, "B", None, false).expect("commit B").oid;

        let cmp = compare_head_diff(p, &c).expect("compare");
        assert_eq!(cmp.from.oid, b);
        assert_eq!(cmp.to.oid, c);

        let got: Vec<(String, FileStatus)> = cmp
            .files
            .iter()
            .map(|f| (f.path.clone(), f.status))
            .collect();
        assert_eq!(
            got,
            vec![
                ("file_feat.txt".to_string(), FileStatus::Added),
                ("file_main.txt".to_string(), FileStatus::Deleted),
            ]
        );
    }

    /// P5 §1.3 / §6.2: comparing HEAD to itself -> `from.oid == to.oid`, empty
    /// `files`, and NOT an error.
    #[test]
    fn compare_head_to_itself_is_empty() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "one\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let a = create_commit(p, "A", None, false).expect("commit").oid;

        let cmp = compare_head_diff(p, &a).expect("compare HEAD to itself");
        assert_eq!(cmp.from.oid, cmp.to.oid);
        assert_eq!(cmp.from.oid, a);
        assert!(cmp.files.is_empty());
    }

    /// P5 §2.2 / §6.2: unborn HEAD -> `from == {"",""}`, old tree is empty, so
    /// every file of `to` shows as Added (compare-vs-empty-tree).
    #[test]
    fn compare_unborn_head_shows_all_added() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "one\n").expect("write");
        std::fs::write(p.join("b.txt"), "two\n").expect("write");
        stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
        let a = create_commit(p, "A", None, false).expect("commit").oid;

        // Force HEAD unborn: point it at a branch with no commit. Commit A
        // still lives in the object DB (reachable via refs/heads/main).
        {
            let repo = git2::Repository::open(p).expect("open");
            repo.set_head("refs/heads/does-not-exist")
                .expect("set_head unborn");
        }

        let cmp = compare_head_diff(p, &a).expect("compare from unborn HEAD");
        assert_eq!(cmp.from.oid, "");
        assert_eq!(cmp.from.summary, "");
        assert_eq!(cmp.to.oid, a);
        assert!(cmp.files.iter().all(|f| f.status == FileStatus::Added));
        let paths: Vec<String> = cmp.files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(paths, vec!["a.txt".to_string(), "b.txt".to_string()]);
    }

    /// P5 §2.2 / §6.2: malformed, unknown, and non-commit oids all map to
    /// `AppError::Git`.
    #[test]
    fn compare_bad_or_non_commit_oid_errors() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "one\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        create_commit(p, "A", None, false).expect("commit");

        // Malformed hex.
        let err = compare_head_diff(p, "notahexoid").expect_err("malformed oid");
        assert!(matches!(err, AppError::Git(_)));

        // Well-formed but unknown.
        let unknown = "0123456789abcdef0123456789abcdef01234567";
        let err = compare_head_diff(p, unknown).expect_err("unknown oid");
        assert!(matches!(err, AppError::Git(_)));

        // Non-commit oid (a tree): find_commit must reject it.
        let tree_oid = {
            let repo = git2::Repository::open(p).expect("open");
            let head = repo.head().expect("head").peel_to_commit().expect("commit");
            head.tree_id().to_string()
        };
        let err = compare_head_diff(p, &tree_oid).expect_err("tree oid is not a commit");
        assert!(matches!(err, AppError::Git(_)));
    }

    /// Tree of the commit `oid` points at, for direct `collect_file_diffs` tests.
    fn tree_of<'r>(repo: &'r git2::Repository, oid: &str) -> git2::Tree<'r> {
        repo.find_commit(git2::Oid::from_str(oid).expect("oid"))
            .expect("commit")
            .tree()
            .expect("tree")
    }

    /// P25 §2.1 / §9.1(2): a two-file tree-to-tree diff yields two `FileDiff`s
    /// with correct paths/status/hunks, in delta order.
    #[test]
    fn collect_file_diffs_multi_file() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("a.txt"), "a1\n").expect("write");
        std::fs::write(p.join("b.txt"), "b1\n").expect("write");
        stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
        let base = create_commit(p, "base", None, false).expect("commit").oid;

        std::fs::write(p.join("a.txt"), "a1 changed\n").expect("write");
        std::fs::write(p.join("b.txt"), "b1 changed\n").expect("write");
        stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
        let head = create_commit(p, "head", None, false).expect("commit").oid;

        let repo = git2::Repository::open(p).expect("open");
        let old = tree_of(&repo, &base);
        let new = tree_of(&repo, &head);
        let mut opts = build_diff_options(&[], false);
        let mut diff = repo
            .diff_tree_to_tree(Some(&old), Some(&new), Some(&mut opts))
            .expect("diff");
        apply_find_similar(&mut diff).expect("find_similar");

        let files = collect_file_diffs(&diff).expect("collect");
        assert_eq!(files.len(), 2, "two changed files");
        // Delta order is byte-sorted path order for tree-to-tree.
        assert_eq!(files[0].path, "a.txt");
        assert_eq!(files[1].path, "b.txt");
        for fd in &files {
            assert_eq!(fd.status, FileStatus::Modified);
            assert!(!fd.binary && !fd.too_large);
            assert_eq!(fd.hunks.len(), 1, "one hunk per file: {}", fd.path);
        }
        // Empty diff (tree vs itself) => empty Vec.
        let mut same = repo
            .diff_tree_to_tree(Some(&new), Some(&new), None)
            .expect("same diff");
        apply_find_similar(&mut same).expect("find_similar");
        assert!(collect_file_diffs(&same).expect("collect").is_empty());
    }

    /// P25 §2.1 / §9.1(2): a too-large file is flagged `too_large` with empty
    /// hunks while its siblings collect normally, all in one pass.
    #[test]
    fn collect_file_diffs_too_large_sibling() {
        let dir = init_scratch();
        let p = dir.path();

        // A huge new file (> MAX_FILE_DIFF_LINES added lines) + a small sibling.
        let big: String = (0..MAX_FILE_DIFF_LINES + 100)
            .map(|i| format!("line {i}\n"))
            .collect();
        std::fs::write(p.join("big.txt"), &big).expect("write big");
        std::fs::write(p.join("small.txt"), "small\n").expect("write small");
        stage_paths(p, &["big.txt".into(), "small.txt".into()]).expect("stage");
        let head = create_commit(p, "add files", None, false).expect("commit").oid;

        let repo = git2::Repository::open(p).expect("open");
        let new = tree_of(&repo, &head);
        let mut opts = build_diff_options(&[], false);
        // Root commit: diff vs empty (None old tree) => everything Added.
        let mut diff = repo
            .diff_tree_to_tree(None, Some(&new), Some(&mut opts))
            .expect("diff");
        apply_find_similar(&mut diff).expect("find_similar");

        let files = collect_file_diffs(&diff).expect("collect");
        assert_eq!(files.len(), 2);
        let big = files.iter().find(|f| f.path == "big.txt").expect("big");
        assert!(big.too_large, "big file over budget must be too_large");
        assert!(big.hunks.is_empty(), "too_large => empty hunks");
        let small = files.iter().find(|f| f.path == "small.txt").expect("small");
        assert!(!small.too_large, "small sibling collects normally");
        assert_eq!(small.hunks.len(), 1);
    }

    /// P5 §6.2: `compare_head_file_diff` hunks for one changed file. HEAD = B
    /// (f.txt = "line1\nCHANGED"); `to` = A (f.txt = "line1\nline2"). The B -> A
    /// diff deletes "CHANGED" and adds "line2".
    #[test]
    fn compare_head_file_diff_hunks_match() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("f.txt"), "line1\nline2\n").expect("write");
        stage_paths(p, &["f.txt".into()]).expect("stage");
        let a = create_commit(p, "A", None, false).expect("commit A").oid;

        std::fs::write(p.join("f.txt"), "line1\nCHANGED\n").expect("write");
        stage_paths(p, &["f.txt".into()]).expect("stage");
        create_commit(p, "B", None, false).expect("commit B");

        let fd = compare_head_file_diff(p, &a, "f.txt", None, false, false).expect("file diff");
        assert_eq!(fd.path, "f.txt");
        assert_eq!(fd.status, FileStatus::Modified);
        assert!(!fd.binary && !fd.too_large);
        assert_eq!(fd.hunks.len(), 1);

        let lines: Vec<(LineKind, &str)> = fd.hunks[0]
            .lines
            .iter()
            .map(|l| (l.kind, l.content.as_str()))
            .collect();
        assert!(lines.contains(&(LineKind::Del, "CHANGED")), "{lines:?}");
        assert!(lines.contains(&(LineKind::Add, "line2")), "{lines:?}");
        assert!(lines.contains(&(LineKind::Context, "line1")), "{lines:?}");
    }

    /// Audit 2026-08-07 §3.3: `collect_file_diff` must ERROR when the walked
    /// diff contains two deltas with DIFFERENT paths (a pathspec that matched
    /// twice), never merge both files' hunks into one corrupted FileDiff.
    /// Exercised directly with an unrestricted two-file diff.
    #[test]
    fn collect_file_diff_refuses_second_delta() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("a.txt"), "a1\n").expect("write");
        std::fs::write(p.join("b.txt"), "b1\n").expect("write");
        stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
        create_commit(p, "base", None, false).expect("commit");
        std::fs::write(p.join("a.txt"), "a2\n").expect("edit a");
        std::fs::write(p.join("b.txt"), "b2\n").expect("edit b");

        let repo = git2::Repository::open(p).expect("open");
        let diff = repo
            .diff_index_to_workdir(None, None)
            .expect("two-delta diff");
        let err = collect_file_diff(&diff).expect_err("second delta must error");
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("multiple")),
            "got {err:?}"
        );
    }

    /// Same-path re-entry must KEEP working: a single-file diff with two
    /// far-apart edits (two hunks, one delta) still collects normally.
    #[test]
    fn collect_file_diff_same_path_two_hunks_still_collects() {
        let dir = init_scratch();
        let p = dir.path();

        let base: String = (1..=20).map(|i| format!("line{i}\n")).collect();
        std::fs::write(p.join("f.txt"), &base).expect("write");
        stage_paths(p, &["f.txt".into()]).expect("stage");
        create_commit(p, "base", None, false).expect("commit");
        let edited = base.replace("line2\n", "LINE2\n").replace("line19\n", "LINE19\n");
        std::fs::write(p.join("f.txt"), edited).expect("edit");

        let repo = git2::Repository::open(p).expect("open");
        let diff = repo
            .diff_index_to_workdir(None, None)
            .expect("one-delta diff");
        let fd = collect_file_diff(&diff)
            .expect("collect")
            .expect("one delta present");
        assert_eq!(fd.path, "f.txt");
        assert_eq!(fd.hunks.len(), 2, "two far-apart hunks, one file");
    }
}
