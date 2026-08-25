//! Diff wire types (M4 contract §2.1). Serialized shapes shared by the diff
//! engine and every consumer; no logic lives here.

use crate::git::status::FileStatus;

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
