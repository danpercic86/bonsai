//! Renders precomputed diff data into labeled stdin payloads for `run_claude`.
//! Pure (no git2 / no Tauri): callers gather typed diffs via git/diff.rs, then
//! render here. All payloads are newline-bearing => stdin ONLY, never argv. (P15)

use crate::git::diff::{FileDiff, FileDiffHeader, LineKind};
use crate::git::status::FileStatus;

/// Total emitted diff-content lines (add/del/context) across ALL files in one
/// payload. Past this the render stops adding files and appends a truncation
/// note. Chosen ~= `MAX_FILE_DIFF_LINES` so a payload is at most a few files of
/// max size — comfortably inside the model context and the 90 s call budget.
pub const MAX_PAYLOAD_LINES: usize = 6_000;
/// Hard cap on files rendered in one payload (diffstat/commit-heavy changes).
pub const MAX_PAYLOAD_FILES: usize = 300;

/// One rendered payload plus whether it was clipped (callers may note it).
pub struct RenderedPayload {
    pub text: String,
    pub truncated: bool,
    pub files_shown: usize,
    pub files_total: usize,
}

/// Minimal commit descriptor for `render_commit_list` (assembled by ai_summary
/// from a revwalk — NOT a wire type).
pub struct CommitLine {
    pub short_oid: String, // first 7 hex chars
    pub summary: String,   // first message line
    pub author: String,    // author name
}

/// Lowercase status token for the FILE label (mirrors `FileStatus`'s serde
/// `rename_all = "lowercase"` — kept as a plain match so this module stays
/// serde-free).
fn status_label(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "added",
        FileStatus::Modified => "modified",
        FileStatus::Deleted => "deleted",
        FileStatus::Renamed => "renamed",
        FileStatus::Typechange => "typechange",
        FileStatus::Conflicted => "conflicted",
        FileStatus::Untracked => "untracked",
    }
}

/// `===== FILE: <path> (<status>[, was <origPath>]) =====` header line.
fn file_label(file: &FileDiff) -> String {
    match &file.orig_path {
        Some(orig) => format!(
            "===== FILE: {} ({}, was {}) =====\n",
            file.path,
            status_label(file.status),
            orig
        ),
        None => format!(
            "===== FILE: {} ({}) =====\n",
            file.path,
            status_label(file.status)
        ),
    }
}

/// Render a list of full `FileDiff`s (with hunks) as labeled sections:
///   ===== FILE: <path> (<status>[, was <origPath>]) =====
///   <unified-ish body: " ctx", "+add", "-del" per DiffLine, hunk headers @@ …>
/// binary/too_large files render a one-line placeholder (no body). Stops once
/// `MAX_PAYLOAD_LINES`/`MAX_PAYLOAD_FILES` is hit; on truncation appends
/// "\n... (diff truncated: showed N of M files) ...". Deterministic; input order
/// preserved.
pub fn render_file_diffs(files: &[FileDiff]) -> RenderedPayload {
    let files_total = files.len();
    let mut text = String::new();
    let mut emitted = 0usize;
    let mut files_shown = 0usize;
    let mut truncated = false;

    for file in files {
        // Budget is a soft cap that stops ADDING MORE files: check at the START
        // of each file so every file is rendered whole-or-not-at-all.
        if files_shown >= MAX_PAYLOAD_FILES || emitted >= MAX_PAYLOAD_LINES {
            truncated = true;
            break;
        }

        text.push_str(&file_label(file));

        if file.binary {
            text.push_str("(binary file — diff omitted)\n");
        } else if file.too_large {
            text.push_str("(file too large — diff omitted)\n");
        } else {
            for hunk in &file.hunks {
                text.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                ));
                for line in &hunk.lines {
                    let prefix = match line.kind {
                        LineKind::Context => " ",
                        LineKind::Add => "+",
                        LineKind::Del => "-",
                    };
                    text.push_str(prefix);
                    text.push_str(&line.content);
                    text.push('\n');
                    emitted += 1;
                }
            }
        }

        text.push('\n');
        files_shown += 1;
    }

    if truncated {
        text.push_str(&format!(
            "\n... (diff truncated: showed {files_shown} of {files_total} files) ...\n"
        ));
    }

    RenderedPayload {
        text,
        truncated,
        files_shown,
        files_total,
    }
}

/// Compact diffstat block (no hunks): one line per header
///   <path>  +<additions> -<deletions>[  (binary)][  was <origPath>]
/// Capped at `MAX_PAYLOAD_FILES`. Used by P15c (aggregate range change).
pub fn render_headers(files: &[FileDiffHeader]) -> RenderedPayload {
    let files_total = files.len();
    let mut text = String::new();
    let mut files_shown = 0usize;

    for header in files.iter().take(MAX_PAYLOAD_FILES) {
        text.push_str(&format!(
            "{}  +{} -{}",
            header.path, header.additions, header.deletions
        ));
        if header.binary {
            text.push_str("  (binary)");
        }
        if let Some(orig) = &header.orig_path {
            text.push_str(&format!("  was {orig}"));
        }
        text.push('\n');
        files_shown += 1;
    }

    let truncated = files_total > files_shown;
    if truncated {
        text.push_str(&format!(
            "\n... (diffstat truncated: showed {files_shown} of {files_total} files) ...\n"
        ));
    }

    RenderedPayload {
        text,
        truncated,
        files_shown,
        files_total,
    }
}

/// One commit line per entry for the P15c commit-list section:
///   <short7 oid>  <summary>  (<author>)
/// Caller pre-caps the slice (see AI_SUMMARY_MAX_COMMITS).
pub fn render_commit_list(lines: &[CommitLine]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(&format!(
            "{}  {}  ({})\n",
            line.short_oid, line.summary, line.author
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::diff::{DiffLine, Hunk};

    fn line(kind: LineKind, content: &str) -> DiffLine {
        DiffLine {
            kind,
            old_no: None,
            new_no: None,
            content: content.to_string(),
            no_newline: false,
        }
    }

    fn text_file(path: &str, status: FileStatus, lines: Vec<DiffLine>) -> FileDiff {
        FileDiff {
            path: path.to_string(),
            orig_path: None,
            status,
            binary: false,
            too_large: false,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 3,
                lines,
            }],
        }
    }

    /// §8.1(1): FILE label per file, +/-/space prefixes, and a hunk header.
    #[test]
    fn render_file_diffs_emits_labels_prefixes_and_hunk_headers() {
        let files = vec![text_file(
            "src/main.rs",
            FileStatus::Modified,
            vec![
                line(LineKind::Context, "fn main() {"),
                line(LineKind::Del, "    old();"),
                line(LineKind::Add, "    new();"),
            ],
        )];
        let out = render_file_diffs(&files);
        assert!(!out.truncated);
        assert_eq!(out.files_shown, 1);
        assert_eq!(out.files_total, 1);
        assert!(
            out.text.contains("===== FILE: src/main.rs (modified) ====="),
            "{}",
            out.text
        );
        assert!(out.text.contains("@@ -1,2 +1,3 @@"), "{}", out.text);
        assert!(out.text.contains(" fn main() {"), "{}", out.text);
        assert!(out.text.contains("-    old();"), "{}", out.text);
        assert!(out.text.contains("+    new();"), "{}", out.text);
    }

    /// §8.1(1): a rename shows the `was <origPath>` clause in the label.
    #[test]
    fn render_file_diffs_rename_label() {
        let mut fd = text_file(
            "docs/getting-started.md",
            FileStatus::Renamed,
            vec![line(LineKind::Context, "hello")],
        );
        fd.orig_path = Some("docs/intro.md".to_string());
        let out = render_file_diffs(&[fd]);
        assert!(
            out.text
                .contains("===== FILE: docs/getting-started.md (renamed, was docs/intro.md) ====="),
            "{}",
            out.text
        );
    }

    /// §8.1(1): binary and too_large files render a one-line placeholder and NO
    /// diff body.
    #[test]
    fn render_file_diffs_binary_and_too_large_placeholders() {
        let binary = FileDiff {
            path: "assets/logo.png".to_string(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: true,
            too_large: false,
            hunks: vec![],
        };
        let too_large = FileDiff {
            path: "data/big.csv".to_string(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: false,
            too_large: true,
            hunks: vec![],
        };
        let out = render_file_diffs(&[binary, too_large]);
        assert!(out.text.contains("(binary file — diff omitted)"), "{}", out.text);
        assert!(out.text.contains("(file too large — diff omitted)"), "{}", out.text);
        // No hunk headers for either placeholder file.
        assert!(!out.text.contains("@@"), "{}", out.text);
        assert_eq!(out.files_shown, 2);
    }

    /// §8.1(2): a Vec exceeding `MAX_PAYLOAD_LINES` stops early (truncated=true,
    /// files_shown < files_total) and appends the truncation note.
    #[test]
    fn render_file_diffs_line_budget_truncates() {
        // Each file carries 1000 add lines; 10 files = 10_000 > MAX_PAYLOAD_LINES.
        let mut files = Vec::new();
        for i in 0..10 {
            let lines: Vec<DiffLine> = (0..1000)
                .map(|n| line(LineKind::Add, &format!("f{i} line {n}")))
                .collect();
            files.push(text_file(&format!("f{i}.txt"), FileStatus::Added, lines));
        }
        let out = render_file_diffs(&files);
        assert!(out.truncated);
        assert!(out.files_shown < out.files_total);
        assert_eq!(out.files_total, 10);
        assert!(
            out.text.contains(&format!(
                "... (diff truncated: showed {} of 10 files) ...",
                out.files_shown
            )),
            "{}",
            out.text
        );
    }

    /// §8.1(2): a file-count cap also truncates.
    #[test]
    fn render_file_diffs_file_budget_truncates() {
        let files: Vec<FileDiff> = (0..(MAX_PAYLOAD_FILES + 5))
            .map(|i| {
                text_file(
                    &format!("f{i}.txt"),
                    FileStatus::Added,
                    vec![line(LineKind::Add, "x")],
                )
            })
            .collect();
        let out = render_file_diffs(&files);
        assert!(out.truncated);
        assert_eq!(out.files_shown, MAX_PAYLOAD_FILES);
        assert_eq!(out.files_total, MAX_PAYLOAD_FILES + 5);
    }

    /// §8.1(3): render_headers diffstat lines incl. binary + rename clauses.
    #[test]
    fn render_headers_diffstat_lines() {
        let headers = vec![
            FileDiffHeader {
                path: "src/a.rs".to_string(),
                orig_path: None,
                status: FileStatus::Modified,
                additions: 12,
                deletions: 3,
                binary: false,
            },
            FileDiffHeader {
                path: "assets/logo.png".to_string(),
                orig_path: None,
                status: FileStatus::Modified,
                additions: 0,
                deletions: 0,
                binary: true,
            },
            FileDiffHeader {
                path: "docs/new.md".to_string(),
                orig_path: Some("docs/old.md".to_string()),
                status: FileStatus::Renamed,
                additions: 1,
                deletions: 1,
                binary: false,
            },
        ];
        let out = render_headers(&headers);
        assert!(!out.truncated);
        assert!(out.text.contains("src/a.rs  +12 -3\n"), "{}", out.text);
        assert!(
            out.text.contains("assets/logo.png  +0 -0  (binary)\n"),
            "{}",
            out.text
        );
        assert!(
            out.text.contains("docs/new.md  +1 -1  was docs/old.md\n"),
            "{}",
            out.text
        );
    }

    /// §8.1(3): render_commit_list short-oid / summary / author formatting.
    #[test]
    fn render_commit_list_formats_each_entry() {
        let lines = vec![
            CommitLine {
                short_oid: "abc1234".to_string(),
                summary: "feat: add thing".to_string(),
                author: "Ada".to_string(),
            },
            CommitLine {
                short_oid: "def5678".to_string(),
                summary: "fix: bug".to_string(),
                author: "Linus".to_string(),
            },
        ];
        let out = render_commit_list(&lines);
        assert_eq!(
            out,
            "abc1234  feat: add thing  (Ada)\ndef5678  fix: bug  (Linus)\n"
        );
    }
}
