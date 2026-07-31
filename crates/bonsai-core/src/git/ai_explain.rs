//! AI explain/review of typed diff data. `analyze_diff` selects a diff source
//! (a commit, a working-dir file, or the whole staged set), renders a payload,
//! and asks the CLI to either EXPLAIN (plain English) or REVIEW (risks/bugs/
//! style) it. Read-only prose out; WRITES NOTHING. Pure git2 + crate::ai. (P15)

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::diff::{commit_diff, commit_file_diff, workdir_file_diff, FileDiff, LineKind};
use crate::git::stage::validate_rel_path;
use crate::git::status::read_status;

/// System prompt (via `--append-system-prompt`) for EXPLAIN mode (contract
/// §4.2, verbatim). SINGLE line — on Windows the `claude` CLI is a `.cmd` shim
/// and Rust's `Command` REFUSES an argv arg containing a newline. Multi-line
/// content only ever flows through the stdin payload. (P15)
const EXPLAIN_SYSTEM_PROMPT: &str = "You are a senior engineer explaining a code change to a teammate. Given a diff on standard input, explain in clear plain English what the change does and, where inferable, why — a one or two sentence high-level summary first, then the key specifics grouped by file. Be concise and concrete. Output prose only — no markdown code fences.";

/// System prompt for REVIEW mode (contract §4.2, verbatim single line). (P15)
const REVIEW_SYSTEM_PROMPT: &str = "You are a meticulous senior code reviewer. Given a diff on standard input, review it for likely bugs, correctness and edge-case risks, security issues, and notable style or maintainability problems. Be concise and specific and cite file names. If you find nothing significant, say so briefly. Output prose only — no markdown code fences.";

/// The `-p` positional prompt for EXPLAIN mode (contract §4.2, verbatim). (P15)
const EXPLAIN_PROMPT: &str = "Explain the change provided on standard input.";

/// The `-p` positional prompt for REVIEW mode (contract §4.2, verbatim). (P15)
const REVIEW_PROMPT: &str = "Review the change provided on standard input.";

/// Which diff to analyze. `#[serde(tag="kind", rename_all="camelCase")]` — this
/// is a COMMAND INPUT (Deserialize); TS mirror is a discriminated union (§5).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiDiffTarget {
    /// Commit vs its first parent (root => vs empty tree). `oid` = 40-hex.
    Commit { oid: String },
    /// One working-dir file. `staged=false` => index vs workdir; `staged=true`
    /// => HEAD vs index. `orig_path` for renames.
    WorkdirFile {
        path: String,
        // The enum's `rename_all = "camelCase"` renames VARIANTS, not
        // struct-variant FIELDS, so name the wire key explicitly to match the
        // TS union (§6.1 sends `origPath`). `default` accepts a missing key too.
        #[serde(default, rename = "origPath")]
        orig_path: Option<String>,
        staged: bool,
    },
    /// The whole staged set (HEAD tree vs index) — the natural Review target.
    Staged,
}

/// Explain (teammate-friendly summary) vs Review (risks/bugs/style).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAnalysisMode {
    Explain,
    Review,
}

/// Prose result. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysis {
    pub text: String,
    pub cost_usd: Option<f64>,
}

/// True when the gathered diffs carry anything worth analyzing: any add/del
/// line, or a binary/too-large placeholder (which still describes a real
/// change). Contract §4.1 says "zero add/del lines => AiFailed"; we extend it so
/// a binary-only change — which produces a non-empty payload placeholder but no
/// textual add/del lines — is NOT misreported as "no changes to analyze".
fn has_analyzable_content(files: &[FileDiff]) -> bool {
    files.iter().any(|f| {
        f.binary
            || f.too_large
            || f.hunks.iter().any(|h| {
                h.lines
                    .iter()
                    .any(|l| matches!(l.kind, LineKind::Add | LineKind::Del))
            })
    })
}

/// Gathers the staged file diffs (HEAD tree vs index), mirroring P15a §3.1
/// steps 1–2 without depending on `ai_commit.rs` internals. An empty staged set
/// (index matches HEAD) => `NothingToCommit` (§7.1). Kept tiny + private.
fn gather_staged(workdir: &Path) -> Result<Vec<FileDiff>, AppError> {
    let staged = read_status(workdir)?.staged;
    if staged.is_empty() {
        return Err(AppError::NothingToCommit);
    }
    let mut file_diffs = Vec::with_capacity(staged.len());
    for entry in &staged {
        let fd = workdir_file_diff(workdir, &entry.path, entry.orig_path.as_deref(), true)?;
        file_diffs.push(fd);
    }
    Ok(file_diffs)
}

/// Gathers `target`'s file diffs and the payload text prefix (empty for
/// non-commit targets). Reuses the existing public diff fns; no new plumbing.
fn build_payload(workdir: &Path, target: &AiDiffTarget) -> Result<(String, Vec<FileDiff>), AppError> {
    match target {
        AiDiffTarget::Commit { oid } => {
            let cd = commit_diff(workdir, oid)?;
            let short7: String = cd.details.oid.chars().take(7).collect();
            let prefix = format!(
                "COMMIT {}  {}\nAUTHOR {}\n\n",
                short7, cd.details.summary, cd.details.author_name
            );
            let mut file_diffs = Vec::with_capacity(cd.files.len());
            for h in &cd.files {
                let fd = commit_file_diff(workdir, oid, &h.path, h.orig_path.as_deref())?;
                file_diffs.push(fd);
            }
            Ok((prefix, file_diffs))
        }
        AiDiffTarget::WorkdirFile {
            path,
            orig_path,
            staged,
        } => {
            // Reject traversal/absolute paths up front and map to `InvalidName`
            // (same guard + mapping as `ai_resolve.rs`), so the wire error kind
            // matches the documented IPC contract rather than the bare `Other`
            // that `validate_rel_path` yields — before any git tree access.
            validate_rel_path(path)
                .map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?;
            let fd = workdir_file_diff(workdir, path, orig_path.as_deref(), *staged)?;
            Ok((String::new(), vec![fd]))
        }
        AiDiffTarget::Staged => Ok((String::new(), gather_staged(workdir)?)),
    }
}

/// Blocking. Gathers `target`'s diff, renders a payload, calls run_claude with
/// the `mode` system prompt. An EMPTY target diff (no changes) => `AiFailed(
/// "no changes to analyze")` before any CLI call (§7.1). Errors: `aiFailed`
/// | `git` (bad oid) | `invalidName` (bad path) | `nothingToCommit` (empty
/// staged set) | (`aiUnavailable` via gate).
pub fn analyze_diff(
    workdir: &Path,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
    opts: RunOpts,
) -> Result<AiAnalysis, AppError> {
    // 1. Gather the typed diffs (+ optional commit header prefix).
    let (prefix, file_diffs) = build_payload(workdir, &target)?;

    // 2. No textual/binary content => nothing to analyze, no CLI call.
    if !has_analyzable_content(&file_diffs) {
        return Err(AppError::AiFailed("no changes to analyze".to_string()));
    }

    // 3. Render the labeled payload (prefix carries commit metadata for Commit).
    let rendered = payload::render_file_diffs(&file_diffs);
    let payload_text = format!("{}{}", prefix, rendered.text);

    // 4. Select the (system prompt, prompt) pair from the mode.
    let (system_prompt, prompt) = match mode {
        AiAnalysisMode::Explain => (EXPLAIN_SYSTEM_PROMPT, EXPLAIN_PROMPT),
        AiAnalysisMode::Review => (REVIEW_SYSTEM_PROMPT, REVIEW_PROMPT),
    };

    // 5. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    let result = ai::run_claude(
        workdir,
        prompt,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(system_prompt.to_string()),
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

    /// The prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint): a newline in any of them would make `claude.cmd` reject the
    /// argument.
    #[test]
    fn prompts_are_single_line() {
        for s in [
            EXPLAIN_SYSTEM_PROMPT,
            REVIEW_SYSTEM_PROMPT,
            EXPLAIN_PROMPT,
            REVIEW_PROMPT,
        ] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }

    /// Serde casing must match the TS `AiAnalysis` type (`text` / `costUsd`);
    /// `None` cost serializes as `null`.
    #[test]
    fn analysis_wire_shape_is_camel_case() {
        let v = serde_json::to_value(AiAnalysis {
            text: "does a thing".to_string(),
            cost_usd: Some(0.006),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "text": "does a thing", "costUsd": 0.006 })
        );

        let v = serde_json::to_value(AiAnalysis {
            text: "no cost".to_string(),
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(v, serde_json::json!({ "text": "no cost", "costUsd": null }));
    }

    /// `AiDiffTarget` deserializes from the EXACT JSON the TS discriminated
    /// union sends for each variant — locking the IPC contract without a CLI.
    #[test]
    fn diff_target_deserializes_each_variant() {
        let commit: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"commit","oid":"deadbeef"}"#).expect("commit");
        match commit {
            AiDiffTarget::Commit { oid } => assert_eq!(oid, "deadbeef"),
            other => panic!("expected Commit, got {other:?}"),
        }

        let wf: AiDiffTarget = serde_json::from_str(
            r#"{"kind":"workdirFile","path":"src/a.rs","origPath":null,"staged":true}"#,
        )
        .expect("workdirFile");
        match wf {
            AiDiffTarget::WorkdirFile {
                path,
                orig_path,
                staged,
            } => {
                assert_eq!(path, "src/a.rs");
                assert_eq!(orig_path, None);
                assert!(staged);
            }
            other => panic!("expected WorkdirFile, got {other:?}"),
        }

        // origPath may also be a string, and (via #[serde(default)]) omitted.
        let wf_renamed: AiDiffTarget = serde_json::from_str(
            r#"{"kind":"workdirFile","path":"src/new.rs","origPath":"src/old.rs","staged":false}"#,
        )
        .expect("workdirFile renamed");
        match wf_renamed {
            AiDiffTarget::WorkdirFile {
                orig_path, staged, ..
            } => {
                assert_eq!(orig_path.as_deref(), Some("src/old.rs"));
                assert!(!staged);
            }
            other => panic!("expected WorkdirFile, got {other:?}"),
        }

        let staged: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"staged"}"#).expect("staged");
        assert!(matches!(staged, AiDiffTarget::Staged));
    }

    /// `AiAnalysisMode` deserializes from the exact `"explain"`/`"review"`
    /// literals the TS `AiAnalysisMode` union sends.
    #[test]
    fn analysis_mode_deserializes_literals() {
        let explain: AiAnalysisMode = serde_json::from_str(r#""explain""#).expect("explain");
        assert!(matches!(explain, AiAnalysisMode::Explain));
        let review: AiAnalysisMode = serde_json::from_str(r#""review""#).expect("review");
        assert!(matches!(review, AiAnalysisMode::Review));
    }
}
