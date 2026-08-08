//! AI "why does this line exist" (P53a). Blames a SINGLE line to find the
//! commit that introduced it, then grounds a why-focused explanation on that
//! commit's change to the file + its full message + the line text — NOT the
//! whole multi-file commit (contract §0 D1). Read-only prose out; WRITES
//! NOTHING. Pure git2 + crate::ai; the CLI system prompt asks for intent, not a
//! diff restatement ("WHY, not WHAT" — phase2 overview C1).

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::{cap_review_payload, AiAnalysis};
use crate::git::blame::{self, BlameLine};
use crate::git::diff::{commit_file_diff, FileDiff};
use crate::git::stage::{open_workdir_repo, validate_rel_path};
use crate::git::timefmt::epoch_to_ymd;

/// System prompt (via `--append-system-prompt`) for line-why (§3.2, verbatim).
/// SINGLE line — on Windows the `claude` CLI is a `.cmd` shim and Rust's
/// `Command` REFUSES an argv arg containing a newline (same rule as the P15
/// prompts). Multi-line grounding only ever flows through the stdin payload.
const LINE_SYSTEM_PROMPT: &str = "You are explaining WHY a specific line of code exists to a teammate. Standard input gives the line, the commit that introduced it (with its message), and that commit's change to the file. Explain the intent behind the line — what problem it solves and why it was written this way — grounded in the commit's stated purpose. Do not merely restate the diff. Two or three sentences. Output prose only — no markdown code fences.";

/// The `-p` positional prompt for line-why (§3.2, verbatim single line).
const LINE_PROMPT: &str = "Explain why the line described on standard input exists.";

/// Renders the blame-why grounding payload (§3.4, normative template). Pure —
/// unit-tested for the labeled sections. `message` is the introducing commit's
/// full message (trailing whitespace trimmed); `file_diff` is that commit's
/// change to `path` (its `render_file_diffs` body carries the `===== FILE:`
/// label, so it follows the `CHANGE TO …` header directly).
fn render_line_payload(path: &str, bl: &BlameLine, message: &str, file_diff: &FileDiff) -> String {
    let short7: String = bl.oid.chars().take(7).collect();
    let date = epoch_to_ymd(bl.author_ts);
    let rendered = payload::render_file_diffs(std::slice::from_ref(file_diff));
    format!(
        "LINE {line} of {path}:\n    {text}\n\n\
         INTRODUCED BY COMMIT {short7}  {summary}\n\
         AUTHOR {author}  {date}\n\
         MESSAGE:\n{message}\n\n\
         CHANGE TO {path} IN THAT COMMIT:\n{body}",
        line = bl.final_line_no,
        text = bl.line_text,
        summary = bl.summary,
        author = bl.author_name,
        message = message.trim_end(),
        body = rendered.text,
    )
}

/// Blocking. Blames `line_no` (as of `at_oid`, `None` -> HEAD) to find the
/// introducing commit, gathers that commit's change to `path`, reads the commit
/// message, renders the §3.4 grounding, and asks the CLI to explain the line's
/// intent. Read-only; WRITES NOTHING (contract §3.2).
///
/// An empty introducing-commit file diff still proceeds (the line text + message
/// are enough context — no hard-fail here). Errors: bad path => `InvalidName`
/// (mapped up front, mirroring `ai_explain`'s `WorkdirFile` guard); line out of
/// range / bad oid / binary file => `Git` (from `blame_line`); CLI failure =>
/// `AiFailed`; (`AiUnavailable` via the command-layer gate).
pub fn explain_line(
    workdir: &Path,
    path: &str,
    line_no: u32,
    at_oid: Option<&str>,
    opts: RunOpts,
) -> Result<AiAnalysis, AppError> {
    // Bad path => InvalidName BEFORE any repo access (matches the documented IPC
    // error kind, mirroring `ai_explain::build_payload`'s WorkdirFile arm).
    validate_rel_path(path).map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?;

    // 1. Blame the single line -> introducing oid, line text, author/summary.
    let bl = blame::blame_line(workdir, path, line_no, at_oid)?;

    // 2. That commit's change to THIS file (rename origin not tracked — OQ7).
    //    An empty file diff is fine: we do NOT hard-fail (§3.2 step 2).
    let file_diff = commit_file_diff(workdir, &bl.oid, path, None, false, false)?;

    // 3. The introducing commit's full message (lossy UTF-8).
    let repo = open_workdir_repo(workdir)?;
    let oid = git2::Oid::from_str(&bl.oid)
        .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
    let commit = repo.find_commit(oid)?;
    let message = String::from_utf8_lossy(commit.message_bytes()).into_owned();

    // 4. Render + byte-cap the labeled grounding payload (§3.4).
    let payload_text = cap_review_payload(render_line_payload(path, &bl, &message, &file_diff));

    // 5. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    let result = ai::run_claude(
        workdir,
        LINE_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(LINE_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(AiAnalysis {
        text: result.text,
        cost_usd: result.cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::diff::{DiffLine, Hunk, LineKind};
    use crate::git::status::FileStatus;

    fn sample_blame_line() -> BlameLine {
        BlameLine {
            oid: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
            author_name: "Ada".to_string(),
            author_email: "ada@example.com".to_string(),
            author_ts: 1_700_000_000, // 2023-11-14
            summary: "add null guard".to_string(),
            orig_line_no: 3,
            final_line_no: 3,
            line_text: "if x.is_none() { return; }".to_string(),
        }
    }

    fn sample_file_diff() -> FileDiff {
        FileDiff {
            path: "src/a.rs".to_string(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: false,
            too_large: false,
            hunks: vec![Hunk {
                old_start: 2,
                old_lines: 1,
                new_start: 2,
                new_lines: 2,
                lines: vec![DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(3),
                    content: "if x.is_none() { return; }".to_string(),
                    no_newline: false,
                    spans: Vec::new(),
                }],
            }],
        }
    }

    /// §7.2: the grounding payload carries every normative §3.4 section — the
    /// LINE anchor, the introducing COMMIT + MESSAGE, and the per-file diff block
    /// (asserted via the render-only unit; model output is not exercised here,
    /// matching the sibling `ai_explain` test idiom).
    #[test]
    fn explain_line_grounding_shape() {
        let bl = sample_blame_line();
        let fd = sample_file_diff();
        let payload = render_line_payload(
            "src/a.rs",
            &bl,
            "add null guard\n\nGuards against a null deref on the hot path.\n",
            &fd,
        );
        assert!(payload.contains("LINE 3 of src/a.rs:"), "{payload}");
        assert!(payload.contains("    if x.is_none() { return; }"), "{payload}");
        assert!(
            payload.contains("INTRODUCED BY COMMIT abcdef1  add null guard"),
            "{payload}"
        );
        assert!(payload.contains("AUTHOR Ada  2023-11-14"), "{payload}");
        assert!(
            payload.contains("MESSAGE:\nadd null guard\n\nGuards against a null deref on the hot path."),
            "{payload}"
        );
        assert!(
            payload.contains("CHANGE TO src/a.rs IN THAT COMMIT:"),
            "{payload}"
        );
        assert!(
            payload.contains("===== FILE: src/a.rs (modified) ====="),
            "{payload}"
        );
        // The trimmed message must NOT leave a dangling blank line before CHANGE.
        assert!(
            !payload.contains("\n\n\nCHANGE TO"),
            "message trailing newline should be trimmed: {payload}"
        );
    }

    /// §7.3: an empty introducing-commit file diff still yields a well-formed
    /// payload (line text + message are enough context — `explain_line` does not
    /// hard-fail on empty content).
    #[test]
    fn render_line_payload_tolerates_empty_diff() {
        let bl = sample_blame_line();
        let empty = FileDiff {
            path: "src/a.rs".to_string(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: false,
            too_large: false,
            hunks: vec![],
        };
        let payload = render_line_payload("src/a.rs", &bl, "add null guard", &empty);
        assert!(payload.contains("LINE 3 of src/a.rs:"), "{payload}");
        assert!(payload.contains("MESSAGE:\nadd null guard"), "{payload}");
        assert!(
            payload.contains("===== FILE: src/a.rs (modified) ====="),
            "{payload}"
        );
    }

    /// §7.3: a traversing/absolute path is rejected as `InvalidName` (the
    /// documented IPC kind) BEFORE any repo or CLI access.
    #[test]
    fn explain_line_bad_path_is_invalid_name() {
        let dir = std::env::temp_dir();
        let err = explain_line(&dir, "../secret", 1, None, RunOpts::default())
            .expect_err("must reject ..");
        assert!(matches!(err, AppError::InvalidName(_)), "got {err:?}");
    }

    /// §7.9: the prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint) — a newline would make `claude.cmd` reject the argument.
    #[test]
    fn prompts_are_single_line() {
        for s in [LINE_SYSTEM_PROMPT, LINE_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }
}
