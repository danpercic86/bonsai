//! Diff walking + collection (M4 contract §2.3/§2.6). The git2 `foreach`
//! callbacks that turn a `git2::Diff` into `FileDiff`/`FileDiffHeader`, plus the
//! shared diff-option/rename/tree helpers they build on.

use std::cell::RefCell;

use crate::error::AppError;
use crate::git::status::FileStatus;

use super::{
    DiffLine, FileDiff, FileDiffHeader, Hunk, LineKind, FULL_CONTEXT_LINES, MAX_FILE_DIFF_LINES,
};

/// Lossy decode of a byte path.
pub(crate) fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// §2.4: lossy UTF-8, strip exactly one trailing `\n` if present, then
/// exactly one trailing `\r` if present. Mid-line `\r` is preserved.
pub(crate) fn normalize_content(bytes: &[u8]) -> String {
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
pub(crate) fn map_status(delta: git2::Delta) -> FileStatus {
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
