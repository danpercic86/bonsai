//! AI merge-conflict resolution. `ai_resolve_conflict` builds a prompt from the
//! conflict's three index stages + marker view, calls the local `claude` CLI,
//! and returns the proposed merged body. It WRITES NOTHING — applying is the
//! caller's separate `resolve_conflict_text` step (P12), so ProposeReview holds
//! the bytes before touching disk. Pure git2 + `crate::ai`, no Tauri types. (P13)

use std::path::Path;

use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::conflict::{self, ConflictKind};
use crate::git::stage::{open_workdir_repo, validate_rel_path};

/// System prompt (via `--append-system-prompt`): role + strict output contract
/// (contract §5.1). Words are verbatim; the contract's line-wrapping is
/// collapsed to a SINGLE line on purpose — on Windows the `claude` CLI is a
/// `.cmd` shim and Rust's `Command` REFUSES to pass an argument containing a
/// newline to a batch file (CVE-2024-24576 mitigation → "batch file arguments
/// are invalid"). A multi-line argv here would break the feature against the
/// real CLI, not just the test stub, so the newlines become spaces. (P13)
/// `pub(crate)` since P68 §6.1: the streaming single-file run reuses this PROVEN
/// prompt verbatim and only appends the two P68 clauses (read-only tools +
/// sentinel), so there is exactly one copy of the resolver's role text.
pub(crate) const SYSTEM_PROMPT: &str = "You are a Git merge-conflict resolver. You are given the common ANCESTOR, OURS, and THEIRS versions of a single file, plus the file with Git conflict markers. Produce the fully merged file that integrates the intent of both sides, with NO conflict markers left. Output ONLY the raw merged file contents — no explanations, no commentary, and no markdown code fences.";

/// The `-p` positional prompt (contract §5.1, verbatim). `pub(crate)` since
/// P68 §6.1 — the streaming single-file run sends the same prompt. (P13)
pub(crate) const RESOLVE_PROMPT: &str =
    "Resolve the merge conflict in the file provided on standard input. Output only the merged file body.";

/// Rendered for an index stage that has no entry (contract §5.1). `pub(crate)`
/// since P68 §6.1 — the bulk payload renders absent stages the same way. (P13)
pub(crate) const ABSENT: &str = "(absent)";

/// The model's proposed fully-merged file body for one conflicted path.
/// Serialized camelCase (mirrored in TS §7). (P13)
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResolveProposal {
    pub path: String,
    pub proposed_text: String,
    pub cost_usd: Option<f64>,
}

/// The three index stages plus the marker view of ONE conflicted path, already
/// checked for AI eligibility.
///
/// EXTRACTED in P68 §6.1 (behaviour unchanged) so the single-file runner below
/// and the streaming/bulk runner ([`super::ai_resolve_stream`]) read a conflict
/// exactly ONE way — a second copy of the stage-1/2/3 walk is how the two would
/// silently drift.
#[derive(Debug, Clone)]
pub(crate) struct ConflictSides {
    /// Repo-relative path, as `get_conflict` reports it (forward slashes).
    pub path: String,
    pub kind: ConflictKind,
    /// Stage 1, or [`ABSENT`].
    pub base: String,
    /// Stage 2, or [`ABSENT`].
    pub ours: String,
    /// Stage 3, or [`ABSENT`].
    pub theirs: String,
    /// The worktree body, WITH conflict markers.
    pub conflicted: String,
}

/// Read + guard one conflicted path (P68 §6.1; the P13 body of
/// `ai_resolve_conflict`, moved verbatim):
///
/// - `validate_rel_path` guard first (same as `resolve_conflict`) → traversal /
///   absolute paths map to `InvalidName` (contract §5).
/// - Reuses `conflict::get_conflict` for the marker view + the
///   binary/too_large/missing guards; any of those → `AiFailed` (SKIP AI, manual
///   only — §9). Only text kinds (`BothModified`/`BothAdded`) are eligible; any
///   other kind reaching here → `AiFailed`.
/// - Reads the three sides via the index: base = get_path(rel,1), ours =
///   get_path(rel,2), theirs = get_path(rel,3) + `find_blob` (same pattern as
///   `conflict.rs` `resolve_conflict`). Absent side rendered as "(absent)".
///
/// Errors: `aiFailed` (ineligible) | `git` (path not conflicted) |
/// `invalidName` (traversal).
pub(crate) fn read_conflict_sides(workdir: &Path, path: &str) -> Result<ConflictSides, AppError> {
    // Same guard as `resolve_conflict` — no absolute/.. escapes — BEFORE
    // `get_conflict` (which does not validate), so a traversal path is a clear
    // `invalidName` rather than a confusing "has no conflict".
    validate_rel_path(path).map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?;

    // Marker view + binary/too_large/missing guards + the "has no conflict"
    // (git) error for a non-conflicted path.
    let view = conflict::get_conflict(workdir, path)?;

    if view.binary {
        return Err(AppError::AiFailed(format!(
            "AI resolution is only available for text conflicts; '{path}' is binary"
        )));
    }
    if view.too_large {
        return Err(AppError::AiFailed(format!(
            "'{path}' is too large for AI resolution"
        )));
    }
    if view.missing {
        return Err(AppError::AiFailed(format!(
            "'{path}' has no worktree content to resolve"
        )));
    }

    // Only text-mergeable kinds are eligible; a deletion/add-conflict kind here
    // has no meaningful text merge (§9).
    match view.kind {
        ConflictKind::BothModified | ConflictKind::BothAdded => {}
        other => {
            return Err(AppError::AiFailed(format!(
                "AI resolution is not available for this conflict kind ({other:?})"
            )));
        }
    }

    // Read the three sides from the index stages (same pattern as
    // `resolve_conflict`); an absent stage renders as "(absent)".
    let repo = open_workdir_repo(workdir)?;
    let index = repo.index()?;
    let rel = Path::new(path);
    let read_side = |stage: i32| -> Result<String, AppError> {
        match index.get_path(rel, stage) {
            Some(e) => {
                let blob = repo.find_blob(e.id)?;
                Ok(String::from_utf8_lossy(blob.content()).into_owned())
            }
            None => Ok(ABSENT.to_string()),
        }
    };
    let base = read_side(1)?;
    let ours = read_side(2)?;
    let theirs = read_side(3)?;

    Ok(ConflictSides {
        path: view.path,
        kind: view.kind,
        base,
        ours,
        theirs,
        conflicted: view.text,
    })
}

/// The labeled-section stdin payload for ONE file (contract §5.1). Kept
/// BYTE-IDENTICAL to the P13 original: this exact text is what the proven
/// single-file resolve sends, and P68's streaming single-path run reuses it
/// rather than inventing a second format (§6.1). The bulk delimiter format lives
/// in [`super::ai_resolve_stream`].
pub(crate) fn build_single_payload(sides: &ConflictSides) -> String {
    format!(
        "FILE: {path}\nCONFLICT KIND: {kind:?}\n\n\
         ===== ANCESTOR (base) =====\n{base}\n\n\
         ===== OURS =====\n{ours}\n\n\
         ===== THEIRS =====\n{theirs}\n\n\
         ===== CONFLICTED (worktree, with markers) =====\n{conflicted}\n",
        path = sides.path,
        kind = sides.kind,
        base = sides.base,
        ours = sides.ours,
        theirs = sides.theirs,
        conflicted = sides.conflicted,
    )
}

/// Blocking. Produces a resolution proposal for one CURRENTLY conflicted path.
///
/// Reads + guards the conflict via [`read_conflict_sides`], builds the §5.1
/// payload via [`build_single_payload`], calls `ai::run_claude` with the system
/// prompt, returns the proposal. DOES NOT write or stage.
///
/// UNCHANGED by P68 (D6/§8.1): same signature, same argv, same 90 s
/// `RunOpts::default()` timeout — it remains the non-streaming fallback.
///
/// Errors: `aiUnavailable` | `aiFailed` | `git` (path not conflicted, via
/// `get_conflict`) | `invalidName` (traversal). (P13)
pub fn ai_resolve_conflict(
    workdir: &Path,
    path: &str,
    opts: RunOpts,
) -> Result<AiResolveProposal, AppError> {
    let sides = read_conflict_sides(workdir, path)?;
    let payload = build_single_payload(&sides);

    let result = ai::run_claude(
        workdir,
        RESOLVE_PROMPT,
        Some(&payload),
        RunOpts {
            system_prompt: Some(SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(AiResolveProposal {
        path: sides.path,
        proposed_text: result.text,
        cost_usd: result.cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::conflict::ConflictKind;

    /// The single-file payload is BYTE-IDENTICAL to the P13 original (P68 §6.1):
    /// the streaming single-path run reuses it, so a "tidy-up" here would silently
    /// change the input of the one AI path that is already proven in the field.
    #[test]
    fn single_payload_is_the_p13_labelled_format() {
        let payload = build_single_payload(&ConflictSides {
            path: "src/a.ts".to_string(),
            kind: ConflictKind::BothModified,
            base: "B".to_string(),
            ours: "O".to_string(),
            theirs: "T".to_string(),
            conflicted: "C".to_string(),
        });
        assert_eq!(
            payload,
            "FILE: src/a.ts\nCONFLICT KIND: BothModified\n\n\
             ===== ANCESTOR (base) =====\nB\n\n\
             ===== OURS =====\nO\n\n\
             ===== THEIRS =====\nT\n\n\
             ===== CONFLICTED (worktree, with markers) =====\nC\n"
        );
    }

    /// The serde casing must match the TS `AiResolveProposal` type exactly
    /// (`proposedText` / `costUsd`); `None` cost serializes as `null`.
    #[test]
    fn proposal_wire_shape_is_camel_case() {
        let v = serde_json::to_value(AiResolveProposal {
            path: "src/auth.ts".to_string(),
            proposed_text: "merged body\n".to_string(),
            cost_usd: Some(0.012),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "path": "src/auth.ts",
                "proposedText": "merged body\n",
                "costUsd": 0.012
            })
        );

        let v = serde_json::to_value(AiResolveProposal {
            path: "a.txt".to_string(),
            proposed_text: "x".to_string(),
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "path": "a.txt", "proposedText": "x", "costUsd": null })
        );
    }
}
