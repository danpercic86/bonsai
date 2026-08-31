//! Natural-language → SAFE git operation PLANNER (P55 — safety core + wire types).
//!
//! Turns a free-text request ("undo my last merge") into a STRUCTURED,
//! previewable, confirm-gated operation — **never a raw shell string**. This
//! module is the read-only planner spine: [`plan_operation`] gathers precomputed
//! repo state, asks the local `claude` CLI to SELECT + PARAMETERIZE one
//! operation from a CLOSED allowlist, then fail-closed-parses the reply and
//! hands it to the resolver. It **WRITES NOTHING** (a hard, tested guarantee —
//! see `plan_never_mutates`); the mutation runs later through the EXISTING,
//! confirm-gated typed command path (P55c dispatch).
//!
//! The feature is split across four focused files (file-size discipline, P55b):
//! - **this file** — wire types ([`AiOpIntent`], [`SafeOp`], [`OperationPreview`],
//!   [`ProposedOperation`], [`PlanOutcome`], …), [`plan_operation`] + the
//!   fail-closed parse, the prompt consts, and the small shared read-only
//!   helpers ([`short7`]/[`head_commit`]/… — `pub(crate)`, reused by the
//!   siblings below);
//! - [`crate::git::ai_operation_grounding`] — the read-only `REPO STATE` payload (§7);
//! - [`crate::git::ai_operation_resolve`] — [`resolve_intent`] + the 10 resolvers (L3/L4);
//! - [`crate::git::ai_operation_preview`] — `build_preview` for every [`SafeOp`] (L5).
//!
//! ## The safety model (contract §2)
//! - **L1 closed allowlist** — [`AiOpIntent`] is the ONLY thing the model can
//!   express. Free-form text / shell strings are NOT a representable output.
//! - **L2 fail-closed parse** — the model's stdout is parsed as [`AiOpIntent`]
//!   via serde_json (first `{…}` block extracted first, since some models wrap
//!   JSON in prose/fences). UNPARSEABLE / unknown-tag / off-schema ⇒
//!   `Ok(PlanOutcome::Unsupported{..})` — never a guessed op, never `AiFailed`.
//! - **L3 Rust owns resolution** / **L4 precondition validation** / **L5
//!   read-only preview** — see the resolve/preview siblings.
//!
//! A badly-behaving model is NEVER an error — it degrades to `Unsupported`.
//! Only a CLI spawn/timeout/empty failure ⇒ `AiFailed`; only a genuine git2
//! infra fault ⇒ `Git`.

use std::path::Path;

use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::cap_review_payload;
use crate::git::ai_operation_grounding::build_grounding;
use crate::git::ai_operation_resolve::resolve_intent;
use crate::git::reset::ResetMode;
use crate::git::stage::open_workdir_repo;

/// Max commits listed in a preview's `dropped_commits` (rest collapse to a count
/// note in the summary).
pub const MAX_PREVIEW_DROPPED: usize = 20;

/// Max chars of a model-derived substring surfaced verbatim in a UI string
/// (F-A2-1). Anything longer is truncated with a `…` marker.
pub(crate) const MAX_MODEL_TEXT: usize = 200;

/// Sanitizes a MODEL-DERIVED substring before it is interpolated into any
/// user-visible string (Unsupported reasons, resolver echoes of branch/commit
/// names — F-A2-1). Three defenses:
/// - `\n`/`\t` become a single space (keep word separation);
/// - every other control char (C0, DEL, C1) is stripped;
/// - Unicode bidi-override/isolate chars (U+202A–U+202E, U+2066–U+2069) are
///   stripped — they can visually reverse surrounding UI text;
/// - the result is capped at [`MAX_MODEL_TEXT`] chars (char-boundary safe)
///   with a trailing `…` when truncated.
pub(crate) fn sanitize_model_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_MODEL_TEXT * 4));
    let mut count = 0usize;
    for c in s.chars() {
        let mapped = match c {
            '\n' | '\t' => Some(' '),
            // `is_control` covers C0 (U+0000–U+001F), DEL (U+007F) and C1
            // (U+0080–U+009F).
            c if c.is_control() => None,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => None,
            c => Some(c),
        };
        if let Some(m) = mapped {
            if count == MAX_MODEL_TEXT {
                out.push('…');
                return out;
            }
            out.push(m);
            count += 1;
        }
    }
    out
}

/// System prompt (via `--append-system-prompt`, contract §5.2 — verbatim). SINGLE
/// line: on Windows the `claude` CLI is a `.cmd` shim and Rust's `Command` REFUSES
/// an argv arg containing a newline (asserted by `prompts_are_single_line`). Lists
/// ALL 10 intents.
pub const PLAN_SYSTEM_PROMPT: &str = "You map a user's natural-language git request to EXACTLY ONE operation from a fixed allowlist. Standard input contains the USER REQUEST and the current REPO STATE. Respond with ONLY one JSON object and nothing else — no prose, no code fences, no shell commands. The object must be one of: {intent:'undoLastCommit',keepChanges:bool} | {intent:'undoLastMerge'} | {intent:'resetToCommit',commit:'<short-hash-from-state>',keepChanges:bool} | {intent:'revertCommit',commit:'<short-hash>'} | {intent:'switchBranch',branch:'<name>'} | {intent:'createBranch',name:'<kebab-name>',atCommit:'<short-hash-or-null>'} | {intent:'deleteBranch',branch:'<name>'} | {intent:'stashChanges',message:'<text-or-null>',includeUntracked:bool} | {intent:'discardChanges',paths:['<path>']} | {intent:'mergeBranch',branch:'<name>'}. Only reference hashes, branch names, and paths that literally appear in the REPO STATE. If the request is ambiguous, references something not in the state, or is not exactly one of these operations, respond {intent:'unsupported',reason:'<short explanation>'}. Never invent a command or a hash; output nothing except the JSON object.";

/// The `-p` positional prompt (contract §5.2, verbatim single line).
pub const PLAN_PROMPT: &str =
    "Map the user request on standard input to one allowlisted operation as JSON.";

/// The CLOSED SET the model may select (P55 allowlist v1) — the ONLY thing it can
/// express (§2 L1). Parsed from the model's JSON stdout; anything off-schema /
/// unknown-tag / unparseable fails CLOSED to [`PlanOutcome::Unsupported`] (§2 L2).
///
/// `rename_all_fields = "camelCase"` maps the struct-variant fields
/// (`keep_changes`↔`keepChanges`, `at_commit`↔`atCommit`,
/// `include_untracked`↔`includeUntracked`) — the enum-level `rename_all` only
/// renames the variant tags. (Same idiom as `opstate::RepoOpState`.)
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(
    tag = "intent",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AiOpIntent {
    UndoLastCommit {
        #[serde(default)]
        keep_changes: bool,
    },
    UndoLastMerge,
    ResetToCommit {
        commit: String,
        #[serde(default)]
        keep_changes: bool,
    },
    RevertCommit {
        commit: String,
    },
    SwitchBranch {
        branch: String,
    },
    CreateBranch {
        name: String,
        #[serde(default)]
        at_commit: Option<String>,
    },
    DeleteBranch {
        branch: String,
    },
    StashChanges {
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        include_untracked: bool,
    },
    DiscardChanges {
        paths: Vec<String>,
    },
    MergeBranch {
        branch: String,
    },
    /// The model's escape hatch (§3 D3). Also the fail-closed target for any
    /// unparseable / off-allowlist model output.
    Unsupported {
        reason: String,
    },
}

/// A fully-RESOLVED typed op. Every variant's fields map 1:1 to an EXISTING typed
/// command's args (dispatch table §6). Rust builds it from an [`AiOpIntent`] after
/// resolving refs/oids; the model never yields an oid.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SafeOp {
    Reset {
        target_oid: String,
        target_short: String,
        mode: ResetMode,
    },
    Revert {
        oid: String,
        short: String,
    },
    SwitchBranch {
        name: String,
        remote: bool,
    },
    CreateBranch {
        name: String,
        at_oid: Option<String>,
    },
    DeleteBranch {
        name: String,
    },
    Stash {
        message: Option<String>,
        include_untracked: bool,
    },
    Discard {
        paths: Vec<String>,
    },
    Merge {
        name: String,
    },
}

/// Danger tier for the preview badge / confirm variant.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DangerLevel {
    Safe,
    Caution,
    Destructive,
}

/// A ref that moves as part of the op (displayed `from → to`).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefChange {
    pub name: String,
    pub from_short: String,
    pub to_short: String,
}

/// One commit line for the preview (dropped / added lists).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRef {
    pub short: String,
    pub summary: String,
}

/// Read-only description of what confirming the op will do (§2 L5). All fields
/// are display-ready; React only renders.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationPreview {
    pub title: String,
    pub summary: String,
    pub danger: DangerLevel,
    pub ref_changes: Vec<RefChange>,
    pub dropped_commits: Vec<CommitRef>,
    pub added_commits: u32,
    pub worktree_warning: Option<String>,
    pub confirm_label: String,
}

/// A resolved, previewable proposal.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedOperation {
    pub op: SafeOp,
    pub preview: OperationPreview,
    /// One-line "why this maps to your ask" (transparency; OQ7). Rust-GENERATED
    /// from the resolved op — NOT free model text (the closed allowlist L1 means
    /// the model never emits prose here), so it is safe by construction.
    pub rationale: String,
    pub cost_usd: Option<f64>,
}

/// Command result. `Unsupported` is a NORMAL `Ok` outcome (renders a calm
/// message), NOT an error.
///
/// `Proposed` boxes its payload so the two variants stay similar in size
/// (`clippy::large_enum_variant`); `Box` is serde-transparent, so the wire shape
/// (§8.2 `OperationPlan`) is unchanged.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PlanOutcome {
    Proposed {
        operation: Box<ProposedOperation>,
    },
    Unsupported {
        reason: String,
        cost_usd: Option<f64>,
    },
}

/// Blocking, READ-ONLY. Gathers repo state (§7), asks the CLI to map `request` to
/// one allowlisted intent, then resolves + previews it. WRITES NOTHING (invariant,
/// tested). Errors: `aiFailed` (CLI empty/timeout/nonzero) | `aiUnavailable` (CLI
/// missing) | `git` (repo unreadable). A bad/garbage/out-of-allowlist model reply
/// is NOT an error — it returns `Ok(PlanOutcome::Unsupported)`.
pub fn plan_operation(
    workdir: &Path,
    request: &str,
    opts: RunOpts,
) -> Result<PlanOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let payload = cap_review_payload(build_grounding(&repo, workdir, request)?);
    let result = ai::run_claude(
        workdir,
        PLAN_PROMPT,
        Some(&payload),
        RunOpts {
            system_prompt: Some(PLAN_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;
    plan_from_reply(&repo, &result.text, result.cost_usd)
}

/// Fail-closed parse (§2 L2) + resolve. Extracts the first `{…}` block, parses it
/// as [`AiOpIntent`]; UNPARSEABLE / no-object ⇒ `Ok(Unsupported)`. Split out of
/// [`plan_operation`] so the fail-closed + resolution logic is unit-testable
/// WITHOUT spawning the CLI (the CLI call is a pure text transform that never
/// touches the repo).
pub(crate) fn plan_from_reply(
    repo: &git2::Repository,
    raw: &str,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    let intent = match extract_json_object(raw)
        .and_then(|j| serde_json::from_str::<AiOpIntent>(&j).ok())
    {
        Some(i) => i,
        None => {
            return Ok(unsupported(
                "I couldn't turn that into a safe operation.".to_string(),
                cost_usd,
            ))
        }
    };
    resolve_intent(repo, intent, cost_usd)
}

/// Extracts a candidate JSON object substring (§2 L2 step): drop ``` code-fence
/// lines, trim, then take the span from the first `{` to the last `}`. Surrounding
/// prose lies outside those braces and is dropped. `None` when no object is
/// present ⇒ the caller fails closed.
fn extract_json_object(raw: &str) -> Option<String> {
    let de_fenced: String = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = de_fenced.trim();
    match (s.find('{'), s.rfind('}')) {
        (Some(i), Some(j)) if i <= j => Some(s[i..=j].to_string()),
        _ => None,
    }
}

// ------------------------------------------- shared read-only helpers (pub(crate))
//
// Tiny, pure, mutation-free helpers reused by the grounding / resolve / preview
// siblings. Kept here (the module spine) so the split modules share ONE copy.

/// 7-char short oid.
pub(crate) fn short7(oid: git2::Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

/// Lossy commit subject (empty for a subject-less commit).
pub(crate) fn summary_of(commit: &git2::Commit<'_>) -> String {
    commit
        .summary_bytes()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default()
}

/// HEAD commit, or `None` when unborn/no-commits (both mapped to a calm
/// `Unsupported` by callers, never an error).
pub(crate) fn head_commit(repo: &git2::Repository) -> Result<Option<git2::Commit<'_>>, AppError> {
    match repo.head() {
        Ok(r) => Ok(Some(r.peel_to_commit()?)),
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

/// Short branch name HEAD points at, or "HEAD" (detached/unborn) — display only.
pub(crate) fn current_branch_name(repo: &git2::Repository) -> String {
    repo.head()
        .ok()
        .and_then(|r| r.shorthand().ok().map(str::to_string))
        .unwrap_or_else(|| "HEAD".to_string())
}

/// Resolves a MODEL-SUPPLIED commit reference to a commit, or `None` on any
/// miss (the model referenced something unresolvable ⇒ a precondition miss,
/// NOT a git error; §2 L4).
///
/// F-A2-2 hardening: the system prompt promises a hash literally taken from
/// the REPO STATE, so anything that is not a plain (possibly short) hex hash —
/// `^[0-9a-fA-F]{4,40}$` — is rejected BEFORE revparse. This closes the gap
/// where arbitrary revspecs (`HEAD~50`, `@{2.days.ago}`, `:/pattern`, ref
/// names) would silently resolve to a commit the grounding never showed.
pub(crate) fn revparse_commit<'r>(
    repo: &'r git2::Repository,
    spec: &str,
) -> Option<git2::Commit<'r>> {
    if !(4..=40).contains(&spec.len()) || !spec.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    repo.revparse_single(spec)
        .ok()
        .and_then(|o| o.peel_to_commit().ok())
}

/// Builds a calm `Unsupported` outcome (a NORMAL `Ok`, not an error). Shared by
/// the fail-closed parse here and every resolver precondition miss.
pub(crate) fn unsupported(reason: String, cost_usd: Option<f64>) -> PlanOutcome {
    PlanOutcome::Unsupported { reason, cost_usd }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_tests;
