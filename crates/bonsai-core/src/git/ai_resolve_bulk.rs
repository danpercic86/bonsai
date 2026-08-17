//! PURE payload/response handling for a BULK AI conflict resolve (P68 §D/§6).
//!
//! One AI run receives EVERY conflict of a merge so the model can reason across
//! them — the common case is one logical change split over several files (the
//! user's i18n JSON). That only works if Rust can (a) render the files with
//! unambiguous delimiters, (b) split the request when the payload would get too
//! big — never truncate it — and (c) attribute the reply back per path, marking a
//! single bad file `failed` without losing the rest of the batch (D11).
//!
//! Nothing here spawns, threads or touches git: every function is total over
//! strings, so the split/attribution rules are unit-testable without a child
//! process (the same reason `ai::stream` is pure — D12). The orchestration lives
//! in [`super::ai_resolve_stream`].

use crate::ai::strip_fence;
use crate::error::AppError;
use crate::git::ai_resolve::{ConflictSides, ABSENT};

use super::ai_resolve_stream::AiResolveFailure;

/// Appended to BOTH streaming system prompts (P68 §4.1, verbatim). Leading space
/// on purpose — these are concatenated onto a prompt that ends with a period.
///
/// This clause is the actual fix for the user's item 6: today's conflict run
/// passes `--tools ""`, so the model is BLIND to the repository and no deadline
/// increase could let it "check the whole application". Read-only by design
/// (D10) — Bonsai still writes nothing, and staging stays the separate explicit
/// `resolve_conflict_text` call after review (D4).
pub(crate) const READ_ONLY_CLAUSE: &str = " You may READ other files in the repository (Read, Grep, Glob) to understand how the conflicting code is used; never modify anything.";

/// Appended to BOTH streaming system prompts (P68 §4.1, verbatim): the mid-run
/// question protocol. A PROMPT-level sentinel, because the CLI's own
/// `SendMessage` tool cannot reach the user in `-p` mode (D9).
pub(crate) const SENTINEL_CLAUSE: &str = " If you cannot resolve without more information, reply with EXACTLY one line beginning BONSAI_NEEDS_INPUT: followed by your question, and nothing else.";

/// Role + output contract for a BULK run. SINGLE line (D13: argv reaching the
/// Windows `.cmd` shim may not contain a newline). The `===== BONSAI RESULT:` +
/// exact-path requirement is what [`parse_bulk_response`] attributes on.
const BULK_SYSTEM_PROMPT: &str = "You are a Git merge-conflict resolver. You are given SEVERAL conflicted files from ONE merge; each is introduced by a `===== BONSAI FILE i/n: <path> =====` header and carries the common ANCESTOR, OURS and THEIRS versions of that file plus the file with Git conflict markers. They are usually one logical change split over several files, so resolve them together and keep them consistent with each other. For EVERY file, output a line `===== BONSAI RESULT: <path> =====` with the path EXACTLY as given, followed by the fully merged contents of that file with NO conflict markers left. Output nothing else — no explanations, no commentary, and no markdown code fences.";

/// The `-p` positional prompt for a bulk run (interactive mode prepends it to the
/// stdin turn instead — D13).
pub(crate) const BULK_PROMPT: &str = "Resolve every merge conflict in the files provided on standard input. For each file, output its `===== BONSAI RESULT: <path> =====` header followed by that file's merged body.";

/// The block header the model must emit per file, and the token this module
/// attributes on.
const RESULT_MARK: &str = "BONSAI RESULT:";
/// Delimiter run that opens and closes both the FILE and RESULT headers.
const RULE: &str = "=====";

/// Slack subtracted from the byte cap before packing (P68 §6.3): the payload's
/// own first line, plus the handful of bytes the `i/n` digits add to each part
/// once the real batch size is known (parts are measured once, as `1/1`).
const HEADER_RESERVE: usize = 512;

/// The bulk system prompt (single line, D13) — role text + the two P68 clauses.
pub(crate) fn bulk_system_prompt() -> String {
    format!("{BULK_SYSTEM_PROMPT}{READ_ONLY_CLAUSE}{SENTINEL_CLAUSE}")
}

/// Render ONE file's section of a bulk payload (P68 §6.1). `index`/`total` are
/// 1-based and describe the position within THIS batch.
pub(crate) fn render_bulk_part(sides: &ConflictSides, index: usize, total: usize) -> String {
    format!(
        "{RULE} BONSAI FILE {index}/{total}: {path} {RULE}\n\
         CONFLICT KIND: {kind:?}\n\
         ----- ANCESTOR (base) -----\n{base}\n\
         ----- OURS -----\n{ours}\n\
         ----- THEIRS -----\n{theirs}\n\
         ----- CONFLICTED (worktree, with markers) -----\n{conflicted}\n",
        path = sides.path,
        kind = sides.kind,
        // Absent stages render exactly as the single-file payload renders them.
        base = absent_if_empty(&sides.base),
        ours = absent_if_empty(&sides.ours),
        theirs = absent_if_empty(&sides.theirs),
        conflicted = sides.conflicted,
    )
}

/// `read_conflict_sides` already substitutes [`ABSENT`] for a missing stage; this
/// only guards the (possible) case of a present-but-empty blob, so a section is
/// never rendered as two blank lines the model might read as "nothing here".
fn absent_if_empty(side: &str) -> &str {
    if side.is_empty() {
        ABSENT
    } else {
        side
    }
}

/// The whole stdin payload for ONE batch (P68 §6.1): the batch header plus every
/// part, in order.
pub(crate) fn build_bulk_payload(parts: &[&ConflictSides]) -> String {
    let total = parts.len();
    let mut out = format!("BONSAI BULK CONFLICT RESOLUTION — {total} files, one merge\n");
    for (i, sides) in parts.iter().enumerate() {
        out.push_str(&render_bulk_part(sides, i + 1, total));
    }
    out
}

/// Approximate rendered size of one part, used for packing only (P68 §6.3).
/// Measured as `1/1` because the real `i/n` is not known until the batches exist;
/// [`HEADER_RESERVE`] covers the few digits of drift.
pub(crate) fn part_bytes(sides: &ConflictSides) -> usize {
    render_bulk_part(sides, 1, 1).len()
}

/// Greedily pack parts into batches that stay within `cap` BYTES (P68 §6.3), in
/// the order given. Returns the batches as index lists into `parts`, plus the
/// per-file failures for parts that cannot fit at all.
///
/// NEVER truncates — that is the whole point of this function. A single part
/// bigger than the budget is reported as an INDIVIDUAL failure and skipped (the
/// rest of the request still runs); everything else is split into batches that
/// the orchestrator then runs sequentially under one run id.
pub(crate) fn pack_batches(
    parts: &[(String, usize)],
    cap: usize,
) -> (Vec<Vec<usize>>, Vec<AiResolveFailure>) {
    let budget = cap.saturating_sub(HEADER_RESERVE);
    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut failed = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_bytes = 0usize;

    for (i, (path, bytes)) in parts.iter().enumerate() {
        if *bytes > budget {
            // Splitting a single file is not an option (the model needs the whole
            // file), and truncating it would silently corrupt the merge.
            failed.push(AiResolveFailure {
                path: path.clone(),
                reason: format!("'{path}' is too large for AI resolution"),
            });
            continue;
        }
        if !current.is_empty() && current_bytes + *bytes > budget {
            batches.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(i);
        current_bytes += *bytes;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    (batches, failed)
}

/// Attribution of one bulk reply (P68 §6.2).
#[derive(Debug, Default, PartialEq)]
pub(crate) struct BulkParse {
    /// `(path, merged body)` for every requested path that came back usable.
    pub proposals: Vec<(String, String)>,
    /// Per-file problems. NEVER fatal to the batch (D11).
    pub failed: Vec<AiResolveFailure>,
    /// Result blocks for paths nobody asked about — logged and ignored.
    pub unknown: Vec<String>,
}

/// The path of a `===== BONSAI RESULT: <path> =====` header line, if this line is
/// one. Hand-rolled rather than a regex: no new dependency, and the rule is short
/// (leading/trailing whitespace and the delimiter runs are tolerated).
fn result_block_path(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix(RULE)?.trim_start();
    let rest = rest.strip_prefix(RESULT_MARK)?;
    let rest = rest.trim().strip_suffix(RULE)?;
    let path = rest.trim();
    (!path.is_empty()).then_some(path)
}

/// True when `text` still contains a conflict-marker line.
///
/// EQUIVALENT to the frontend's `hasUnresolvedMarkers`
/// (`src/utils/conflictRegions.ts:127`, `/^(<{7}|={7}|>{7})/`) on purpose: the two
/// gates must agree, or Rust could hand the UI something the UI would refuse to
/// stage (or worse, the other way round). Nothing markerful may ever be presented
/// as clean.
pub(crate) fn has_conflict_markers(text: &str) -> bool {
    text.lines().any(|line| {
        line.starts_with("<<<<<<<") || line.starts_with("=======") || line.starts_with(">>>>>>>")
    })
}

/// Split a bulk reply into per-path bodies and attribute each (P68 §6.2, PURE).
///
/// - a path matched EXACTLY against `requested` ⇒ proposal (after one leading and
///   one trailing blank line are dropped and a stray ``` fence is stripped);
/// - a path that was not requested ⇒ `unknown` (logged, ignored);
/// - a requested path with no block ⇒ `failed("no result block returned")`;
/// - an empty/whitespace body ⇒ `failed("empty result")`;
/// - a body that still has conflict markers ⇒ `failed(...)`.
///
/// A per-file problem NEVER fails the batch (D11). The only hard error is a reply
/// with NO blocks at all for a multi-file request — there is nothing to attribute
/// then, and silently reporting every file as "no result block" would hide a
/// protocol break.
///
/// Lenient single-file case: a batch may legitimately contain ONE file (a big file
/// packed alone, §6.3), and a model given one file often answers with the bare
/// body. When exactly one path was requested and no block was found, the whole
/// reply is taken as that path's body — still marker- and emptiness-checked below.
pub(crate) fn parse_bulk_response(
    text: &str,
    requested: &[String],
) -> Result<BulkParse, AppError> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in text.lines() {
        match result_block_path(line) {
            Some(path) => {
                if let Some((p, body)) = current.take() {
                    blocks.push((p, body.join("\n")));
                }
                current = Some((path.to_string(), Vec::new()));
            }
            None => {
                if let Some((_, body)) = current.as_mut() {
                    body.push(line);
                }
            }
        }
    }
    if let Some((p, body)) = current.take() {
        blocks.push((p, body.join("\n")));
    }

    if blocks.is_empty() {
        match requested {
            [only] => blocks.push((only.clone(), text.to_string())),
            _ => {
                return Err(AppError::AiFailed(
                    "Claude did not return per-file result blocks".to_string(),
                ))
            }
        }
    }

    let mut out = BulkParse::default();
    for (path, body) in blocks {
        if !requested.contains(&path) {
            out.unknown.push(path);
            continue;
        }
        // A duplicated block for the same path: keep the FIRST (the model
        // sometimes restates); a second entry would make attribution ambiguous.
        if out.proposals.iter().any(|(p, _)| *p == path)
            || out.failed.iter().any(|f| f.path == path)
        {
            continue;
        }
        let body = normalize_body(&strip_fence(trim_one_blank_line(&body)));
        if body.trim().is_empty() {
            out.failed.push(AiResolveFailure {
                path,
                reason: "Claude returned an empty result for this file".to_string(),
            });
        } else if has_conflict_markers(&body) {
            out.failed.push(AiResolveFailure {
                path,
                reason: "AI left unresolved conflict markers".to_string(),
            });
        } else {
            out.proposals.push((path, body));
        }
    }

    for path in requested {
        let seen = out.proposals.iter().any(|(p, _)| p == path)
            || out.failed.iter().any(|f| &f.path == path);
        if !seen {
            out.failed.push(AiResolveFailure {
                path: path.clone(),
                reason: "no result block returned".to_string(),
            });
        }
    }
    Ok(out)
}

/// KNOWN LIMITATION of the block format, made explicit: a body reconstructed from
/// lines cannot say whether the file ended with a newline, because the next
/// `=====` header necessarily starts on its own line. Text files virtually always
/// end with one, and a missing final newline is exactly the kind of diff noise a
/// merge resolution should not introduce, so a non-empty body is terminated with
/// ONE newline. (The single-file payload/response path is untouched by this — it
/// returns the model's bytes verbatim, as it has since P13.)
fn normalize_body(body: &str) -> String {
    if body.is_empty() || body.ends_with('\n') {
        return body.to_string();
    }
    format!("{body}\n")
}

/// Drop ONE leading and ONE trailing blank line (§6.2) — the blank lines that
/// framing a block introduces — while preserving every other byte, including
/// deliberate blank lines inside the file and its final newline structure.
fn trim_one_blank_line(body: &str) -> &str {
    let mut s = body;
    if let Some(rest) = s.strip_prefix('\n') {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("\r\n") {
        s = rest;
    }
    if let Some(rest) = s.strip_suffix('\n') {
        s = rest.strip_suffix('\r').unwrap_or(rest);
    }
    s
}

#[cfg(test)]
#[path = "ai_resolve_bulk_tests.rs"]
mod tests;
