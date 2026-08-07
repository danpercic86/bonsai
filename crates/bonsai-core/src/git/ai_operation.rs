//! Natural-language → SAFE git operation PLANNER (P55a — safety core).
//!
//! Turns a free-text request ("undo my last merge") into a STRUCTURED,
//! previewable, confirm-gated operation — **never a raw shell string**. This
//! module is the read-only planner half: it gathers precomputed repo state,
//! asks the local `claude` CLI to SELECT + PARAMETERIZE one operation from a
//! CLOSED allowlist, then Rust resolves the refs/oids and computes a read-only
//! preview. `plan_operation` **WRITES NOTHING** (a hard, tested guarantee —
//! see `plan_never_mutates`); the mutation runs later through the EXISTING,
//! confirm-gated typed command path (P55c dispatch).
//!
//! ## The safety model (contract §2 — the point of this increment)
//! - **L1 closed allowlist** — [`AiOpIntent`] is the ONLY thing the model can
//!   express. Free-form text / shell strings are NOT a representable output.
//! - **L2 fail-closed parse** — the model's stdout is parsed as [`AiOpIntent`]
//!   via serde_json (first `{…}` block extracted first, since some models wrap
//!   JSON in prose/fences). UNPARSEABLE / unknown-tag / off-schema ⇒
//!   `Ok(PlanOutcome::Unsupported{..})` — never a guessed op, never `AiFailed`.
//! - **L3 Rust owns resolution** — Rust does every revparse/oid/precondition;
//!   the model only references items shown in the grounding. It NEVER yields an
//!   oid.
//! - **L4 precondition validation** — any miss (bad ref, HEAD not a merge, op
//!   in progress, …) ⇒ `Ok(Unsupported{reason})`.
//! - **L5 read-only preview** — [`build_preview`] uses only revwalk/revparse;
//!   NO mutation.
//!
//! A badly-behaving model is NEVER an error — it degrades to `Unsupported`.
//! Only a CLI spawn/timeout/empty failure ⇒ `AiFailed`; only a genuine git2
//! infra fault ⇒ `Git`.
//!
//! P55a resolves FOUR intents (`undoLastCommit`, `undoLastMerge`,
//! `resetToCommit`, `revertCommit`); the other six are in the schema (so the
//! model can emit them and the frontend mock covers them) but resolve to
//! "not yet supported" until P55b.

use std::fmt::Write as _;
use std::path::Path;

use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::cap_review_payload;
use crate::git::branches::list_refs;
use crate::git::opstate::{read_op_state, RepoOpState};
use crate::git::reset::ResetMode;
use crate::git::stage::open_workdir_repo;
use crate::git::stash::list_stashes;
use crate::git::status::read_status;
use crate::git::timefmt::epoch_to_ymd;

/// Max commits listed in a preview's `dropped_commits` (rest collapse to a count
/// note in the summary).
pub const MAX_PREVIEW_DROPPED: usize = 20;

/// First-parent HEAD commits sampled into the grounding (mirrors `ai_summary`).
const RECENT_COMMITS: usize = 25;

/// Cap on `CHANGED PATHS` listed in the grounding (rest collapse to a count).
const GROUNDING_MAX_PATHS: usize = 50;

/// System prompt (via `--append-system-prompt`, contract §5.2 — verbatim). SINGLE
/// line: on Windows the `claude` CLI is a `.cmd` shim and Rust's `Command` REFUSES
/// an argv arg containing a newline (asserted by `prompts_are_single_line`). Lists
/// ALL 10 intents so the schema is complete even though P55a only RESOLVES four.
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
#[serde(tag = "intent", rename_all = "camelCase", rename_all_fields = "camelCase")]
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
fn plan_from_reply(
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

/// Maps a parsed intent to a resolved op + preview, or `Unsupported`. Precondition
/// / lookup misses ⇒ `Ok(Unsupported{reason})` (§2 L4); only unexpected git2
/// faults ⇒ `Err`. `cost_usd` from the CLI envelope is threaded onto the outcome.
fn resolve_intent(
    repo: &git2::Repository,
    intent: AiOpIntent,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    match intent {
        AiOpIntent::Unsupported { reason } => Ok(unsupported(reason, cost_usd)),
        AiOpIntent::UndoLastCommit { keep_changes } => {
            resolve_undo_last_commit(repo, keep_changes, cost_usd)
        }
        AiOpIntent::UndoLastMerge => resolve_undo_last_merge(repo, cost_usd),
        AiOpIntent::ResetToCommit {
            commit,
            keep_changes,
        } => resolve_reset_to_commit(repo, &commit, keep_changes, cost_usd),
        AiOpIntent::RevertCommit { commit } => resolve_revert_commit(repo, &commit, cost_usd),
        // The remaining six are valid schema (so the model can select them and the
        // frontend mock exercises them) but are resolved in P55b.
        AiOpIntent::SwitchBranch { .. }
        | AiOpIntent::CreateBranch { .. }
        | AiOpIntent::DeleteBranch { .. }
        | AiOpIntent::StashChanges { .. }
        | AiOpIntent::DiscardChanges { .. }
        | AiOpIntent::MergeBranch { .. } => Ok(unsupported(
            "I can't do that one safely yet — it isn't supported in this version.".to_string(),
            cost_usd,
        )),
    }
}

// ------------------------------------------------------------- resolvers (P55a)

/// undoLastCommit (§4): HEAD must have ≥1 parent. `keepChanges` ⇒ Mixed (Caution),
/// else Hard (Destructive). Target = HEAD's first parent; dropped = [HEAD].
fn resolve_undo_last_commit(
    repo: &git2::Repository,
    keep_changes: bool,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let head = match head_commit(repo)? {
        Some(c) => c,
        None => return Ok(unsupported("there are no commits to undo yet.".to_string(), cost_usd)),
    };
    if head.parent_count() < 1 {
        return Ok(unsupported(
            "there's no commit to undo — HEAD has no parent.".to_string(),
            cost_usd,
        ));
    }
    let parent = head.parent(0)?;
    let mode = if keep_changes {
        ResetMode::Mixed
    } else {
        ResetMode::Hard
    };
    let op = reset_op(&parent, mode);
    let branch = current_branch_name(repo);
    let mut preview = build_preview(repo, &op)?;
    preview.title = "Undo last commit".to_string();
    preview.confirm_label = if keep_changes {
        "Undo commit".to_string()
    } else {
        "Undo & discard".to_string()
    };
    preview.summary = format!(
        "Move `{branch}` back to {} — {} the changes from {}.",
        short7(parent.id()),
        if keep_changes { "keep" } else { "discard" },
        short7(head.id())
    );
    let rationale = format!(
        "Interpreted your request as undoing the most recent commit ({}).",
        short7(head.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// undoLastMerge (§4, OQ2 headline): HEAD must be a merge (≥2 parents). Target =
/// HEAD's FIRST parent, Mixed — but flagged Destructive regardless (it rewrites
/// history), with a shared-history warning when an upstream exists.
fn resolve_undo_last_merge(
    repo: &git2::Repository,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let head = match head_commit(repo)? {
        Some(c) => c,
        None => return Ok(unsupported("there are no commits yet.".to_string(), cost_usd)),
    };
    if head.parent_count() < 2 {
        return Ok(unsupported(
            "your last commit isn't a merge, so there's no merge to undo.".to_string(),
            cost_usd,
        ));
    }
    let parent = head.parent(0)?;
    let op = reset_op(&parent, ResetMode::Mixed);
    let branch = current_branch_name(repo);
    let mut preview = build_preview(repo, &op)?;
    preview.title = "Undo last merge".to_string();
    preview.confirm_label = "Undo merge".to_string();
    // OQ2: always Destructive (rewrites history), even though the reset is Mixed.
    preview.danger = DangerLevel::Destructive;
    preview.summary = format!(
        "Move `{branch}` back to {} (before the merge), keeping your working changes.",
        short7(parent.id())
    );
    if let Some(upstream) = current_upstream_name(repo) {
        preview.worktree_warning = Some(format!(
            "This rewrites history that may be shared with `{upstream}`."
        ));
    }
    let rationale = format!(
        "Interpreted your request as undoing the last merge by resetting to its first parent ({}).",
        short7(parent.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// resetToCommit (§4): `revparse_single(commit)` (fail ⇒ Unsupported, L4).
/// `keepChanges` ⇒ Mixed, else Hard.
fn resolve_reset_to_commit(
    repo: &git2::Repository,
    commit: &str,
    keep_changes: bool,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let target = match revparse_commit(repo, commit) {
        Some(c) => c,
        None => {
            return Ok(unsupported(
                format!("I couldn't find a commit matching '{commit}'."),
                cost_usd,
            ))
        }
    };
    let mode = if keep_changes {
        ResetMode::Mixed
    } else {
        ResetMode::Hard
    };
    let op = reset_op(&target, mode);
    let branch = current_branch_name(repo);
    let mut preview = build_preview(repo, &op)?;
    preview.title = "Reset to commit".to_string();
    preview.confirm_label = if keep_changes {
        "Reset (keep changes)".to_string()
    } else {
        "Reset (discard changes)".to_string()
    };
    preview.summary = format!(
        "Move `{branch}` to {} — {} changes made after it.",
        short7(target.id()),
        if keep_changes { "keep" } else { "discard" }
    );
    let rationale = format!(
        "Interpreted your request as moving the current branch to {}.",
        short7(target.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

/// revertCommit (§4): `revparse` the oid (fail ⇒ Unsupported). Adds ONE commit.
fn resolve_revert_commit(
    repo: &git2::Repository,
    commit: &str,
    cost_usd: Option<f64>,
) -> Result<PlanOutcome, AppError> {
    if let Some(reason) = op_in_progress_reason(repo) {
        return Ok(unsupported(reason, cost_usd));
    }
    let target = match revparse_commit(repo, commit) {
        Some(c) => c,
        None => {
            return Ok(unsupported(
                format!("I couldn't find a commit matching '{commit}'."),
                cost_usd,
            ))
        }
    };
    let op = SafeOp::Revert {
        oid: target.id().to_string(),
        short: short7(target.id()),
    };
    let preview = build_preview(repo, &op)?;
    let rationale = format!(
        "Interpreted your request as reverting {} with a new commit.",
        short7(target.id())
    );
    Ok(proposed(op, preview, rationale, cost_usd))
}

// -------------------------------------------------------------- preview (L5)

/// Read-only preview for a resolved op (revwalk/revparse only — NO mutation).
/// For a `Reset`, `dropped` = commits reachable from the old tip but not the
/// target (`target..oldtip`), capped at [`MAX_PREVIEW_DROPPED`]. Resolvers refine
/// the display fields (title/summary/danger/confirm_label) afterward.
fn build_preview(
    repo: &git2::Repository,
    op: &SafeOp,
) -> Result<OperationPreview, AppError> {
    match op {
        SafeOp::Reset {
            target_oid,
            target_short,
            mode,
        } => {
            let head = repo.head()?.peel_to_commit()?;
            let tip_oid = head.id();
            let branch = current_branch_name(repo);
            let target = git2::Oid::from_str(target_oid)
                .map_err(|_| AppError::Git("invalid target oid".to_string()))?;
            let (dropped, total) = dropped_commits(repo, target, tip_oid)?;
            let hard = matches!(mode, ResetMode::Hard);
            let danger = if hard {
                DangerLevel::Destructive
            } else {
                DangerLevel::Caution
            };
            let worktree_warning = if hard {
                Some("This permanently discards any uncommitted changes in your working tree.".to_string())
            } else {
                None
            };
            let more = (total as usize).saturating_sub(dropped.len());
            let more_note = if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            };
            let summary = format!(
                "Move `{branch}` from {} to {target_short}. {total} commit(s) leave the branch{more_note}.",
                short7(tip_oid)
            );
            Ok(OperationPreview {
                title: "Reset branch".to_string(),
                summary,
                danger,
                ref_changes: vec![RefChange {
                    name: branch,
                    from_short: short7(tip_oid),
                    to_short: target_short.clone(),
                }],
                dropped_commits: dropped,
                added_commits: 0,
                worktree_warning,
                confirm_label: "Reset".to_string(),
            })
        }
        SafeOp::Revert { oid, short } => {
            let branch = current_branch_name(repo);
            let target = revparse_commit(repo, oid)
                .ok_or_else(|| AppError::Git("revert target not found".to_string()))?;
            let subject = summary_of(&target);
            Ok(OperationPreview {
                title: "Revert commit".to_string(),
                summary: format!(
                    "Add a new commit to `{branch}` that undoes {short} (\"{subject}\")."
                ),
                danger: DangerLevel::Caution,
                ref_changes: Vec::new(),
                dropped_commits: Vec::new(),
                added_commits: 1,
                worktree_warning: None,
                confirm_label: "Revert".to_string(),
            })
        }
        // The P55b SafeOps are never constructed by P55a's resolvers; keep
        // build_preview total (no panic) with a neutral placeholder.
        _ => Ok(OperationPreview {
            title: "Operation".to_string(),
            summary: "This operation isn't supported yet.".to_string(),
            danger: DangerLevel::Caution,
            ref_changes: Vec::new(),
            dropped_commits: Vec::new(),
            added_commits: 0,
            worktree_warning: None,
            confirm_label: "Apply".to_string(),
        }),
    }
}

// ------------------------------------------------------------------- helpers

/// A `SafeOp::Reset` targeting `commit` in `mode` (full oid + 7-char display).
fn reset_op(commit: &git2::Commit<'_>, mode: ResetMode) -> SafeOp {
    SafeOp::Reset {
        target_oid: commit.id().to_string(),
        target_short: short7(commit.id()),
        mode,
    }
}

fn unsupported(reason: String, cost_usd: Option<f64>) -> PlanOutcome {
    PlanOutcome::Unsupported { reason, cost_usd }
}

fn proposed(
    op: SafeOp,
    preview: OperationPreview,
    rationale: String,
    cost_usd: Option<f64>,
) -> PlanOutcome {
    PlanOutcome::Proposed {
        operation: Box::new(ProposedOperation {
            op,
            preview,
            rationale,
            cost_usd,
        }),
    }
}

/// 7-char short oid.
fn short7(oid: git2::Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

/// Lossy commit subject (empty for a subject-less commit).
fn summary_of(commit: &git2::Commit<'_>) -> String {
    commit
        .summary_bytes()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default()
}

/// HEAD commit, or `None` when unborn/no-commits (both mapped to a calm
/// `Unsupported` by callers, never an error).
fn head_commit(repo: &git2::Repository) -> Result<Option<git2::Commit<'_>>, AppError> {
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
fn current_branch_name(repo: &git2::Repository) -> String {
    repo.head()
        .ok()
        .and_then(|r| r.shorthand().ok().map(str::to_string))
        .unwrap_or_else(|| "HEAD".to_string())
}

/// Configured upstream shorthand of the current branch, if any (read-only).
fn current_upstream_name(repo: &git2::Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let name = head.shorthand().ok()?;
    let branch = repo.find_branch(name, git2::BranchType::Local).ok()?;
    let upstream = branch.upstream().ok()?;
    upstream.name().ok().flatten().map(str::to_string)
}

/// `revparse_single(spec)` → commit, or `None` on any miss (the model referenced
/// something unresolvable ⇒ a precondition miss, NOT a git error; §2 L4).
fn revparse_commit<'r>(repo: &'r git2::Repository, spec: &str) -> Option<git2::Commit<'r>> {
    repo.revparse_single(spec)
        .ok()
        .and_then(|o| o.peel_to_commit().ok())
}

/// Reason string when a merge/rebase/cherry-pick/revert is mid-flight, else
/// `None` (§4 global precondition). Read from `repo.state()` (matches the guard
/// `reset_branch` enforces at execution time).
fn op_in_progress_reason(repo: &git2::Repository) -> Option<String> {
    use git2::RepositoryState as S;
    let op = match repo.state() {
        S::Clean => return None,
        S::Merge => "merge",
        S::Rebase | S::RebaseInteractive | S::RebaseMerge => "rebase",
        S::CherryPick | S::CherryPickSequence => "cherry-pick",
        S::Revert | S::RevertSequence => "revert",
        _ => "operation",
    };
    Some(format!("finish or abort the in-progress {op} first."))
}

/// Commits reachable from `tip` but not `target` (`target..tip`), newest first,
/// capped at [`MAX_PREVIEW_DROPPED`]. Returns `(listed, total)` — `total` is the
/// pre-cap count for the "(+N more)" note.
fn dropped_commits(
    repo: &git2::Repository,
    target: git2::Oid,
    tip: git2::Oid,
) -> Result<(Vec<CommitRef>, u32), AppError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.push(tip)?;
    walk.hide(target)?;
    let mut listed = Vec::new();
    let mut total = 0u32;
    for oid in walk {
        let oid = oid?;
        total = total.saturating_add(1);
        if listed.len() < MAX_PREVIEW_DROPPED {
            let commit = repo.find_commit(oid)?;
            listed.push(CommitRef {
                short: short7(oid),
                summary: summary_of(&commit),
            });
        }
    }
    Ok((listed, total))
}

// ------------------------------------------------------------ grounding (§7)

/// Assembles the read-only grounding payload (§7) from existing read fns + a
/// first-parent HEAD revwalk. stdin ONLY (multi-line) — never argv.
fn build_grounding(
    repo: &git2::Repository,
    workdir: &Path,
    request: &str,
) -> Result<String, AppError> {
    let mut s = String::new();
    let _ = writeln!(s, "USER REQUEST:\n{}\n", request.trim());
    let _ = writeln!(s, "REPO STATE:");

    // HEAD line.
    match head_commit(repo)? {
        Some(head) => {
            let detached = repo.head_detached().unwrap_or(false);
            let label = if detached {
                "detached".to_string()
            } else {
                current_branch_name(repo)
            };
            let merge = if head.parent_count() >= 2 { "yes" } else { "no" };
            let _ = writeln!(
                s,
                "HEAD: {label} at {} \"{}\"  (merge commit: {merge})",
                short7(head.id()),
                summary_of(&head)
            );
        }
        None => {
            let _ = writeln!(s, "HEAD: (unborn — no commits yet)");
        }
    }

    // Refs snapshot (upstream + branch lists) via the existing read fn.
    let refs = list_refs(workdir)?;
    let upstream = refs.local.iter().find(|b| b.is_head).and_then(|b| {
        b.upstream.as_ref().map(|u| match (b.ahead, b.behind) {
            (Some(a), Some(bh)) => format!("{u}, ahead {a} behind {bh}"),
            _ => u.clone(),
        })
    });
    let _ = writeln!(s, "UPSTREAM: {}", upstream.unwrap_or_else(|| "none".to_string()));

    // Recent commits (first-parent, newest first).
    let _ = writeln!(s, "RECENT COMMITS (first-parent, newest first):");
    if repo.head().is_ok() {
        if let Ok(mut walk) = repo.revwalk() {
            let _ = walk.set_sorting(git2::Sort::TOPOLOGICAL);
            let _ = walk.simplify_first_parent();
            if walk.push_head().is_ok() {
                for oid in walk.take(RECENT_COMMITS) {
                    let oid = match oid {
                        Ok(o) => o,
                        Err(_) => break,
                    };
                    if let Ok(c) = repo.find_commit(oid) {
                        let date = epoch_to_ymd(c.time().seconds());
                        let author = String::from_utf8_lossy(c.author().name_bytes()).into_owned();
                        let merge = if c.parent_count() >= 2 { "  [merge]" } else { "" };
                        let _ = writeln!(
                            s,
                            "- {} {date} {author}  {}{merge}",
                            short7(oid),
                            summary_of(&c)
                        );
                    }
                }
            }
        }
    }

    // Branch lists.
    let locals: Vec<&str> = refs.local.iter().map(|b| b.name.as_str()).collect();
    let _ = writeln!(
        s,
        "LOCAL BRANCHES: {}",
        if locals.is_empty() { "(none)".to_string() } else { locals.join(", ") }
    );
    let remotes: Vec<&str> = refs.remote.iter().map(|b| b.name.as_str()).collect();
    let _ = writeln!(
        s,
        "REMOTE BRANCHES: {}",
        if remotes.is_empty() { "(none)".to_string() } else { remotes.join(", ") }
    );

    // Working tree + changed (tracked-modified) paths.
    let status = read_status(workdir)?;
    if status.staged.is_empty()
        && status.unstaged.is_empty()
        && status.untracked.is_empty()
        && status.conflicted.is_empty()
    {
        let _ = writeln!(s, "WORKING TREE: clean");
    } else {
        let _ = writeln!(
            s,
            "WORKING TREE: {} staged, {} unstaged, {} untracked",
            status.staged.len(),
            status.unstaged.len(),
            status.untracked.len()
        );
    }
    let mut changed: Vec<String> = Vec::new();
    for e in status.staged.iter().chain(status.unstaged.iter()) {
        if !changed.contains(&e.path) {
            changed.push(e.path.clone());
        }
    }
    if !changed.is_empty() {
        let shown: Vec<&str> = changed
            .iter()
            .take(GROUNDING_MAX_PATHS)
            .map(String::as_str)
            .collect();
        let more = changed.len().saturating_sub(shown.len());
        let more_note = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        let _ = writeln!(s, "CHANGED PATHS: {}{}", shown.join(", "), more_note);
    }

    // Stashes.
    let stashes = list_stashes(workdir)?;
    if stashes.is_empty() {
        let _ = writeln!(s, "STASHES: none");
    } else {
        let items: Vec<String> = stashes
            .iter()
            .take(10)
            .map(|e| format!("[{}] \"{}\"", e.index, e.message))
            .collect();
        let _ = writeln!(s, "STASHES: {}", items.join(", "));
    }

    // In-progress op.
    let op = match read_op_state(workdir)? {
        RepoOpState::None => "none",
        RepoOpState::Merge { .. } => "merge",
        RepoOpState::Rebase { .. } => "rebase",
        RepoOpState::CherryPick => "cherryPick",
        RepoOpState::Revert => "revert",
        RepoOpState::Bisect { .. } => "bisect",
    };
    let _ = writeln!(s, "IN-PROGRESS OP: {op}");

    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    // ----------------------------------------------------------- fixtures

    /// git2-init a scratch repo with identity + autocrlf off (mirrors ai_explain).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// Commit `file`=`content` with `msg` on the current branch; returns full oid.
    fn commit(dir: &Path, file: &str, content: &str, msg: &str) -> String {
        std::fs::write(dir.join(file), content).expect("write");
        stage_paths(dir, &[file.to_string()]).expect("stage");
        create_commit(dir, msg).expect("commit").oid
    }

    fn oid(s: &str) -> git2::Oid {
        git2::Oid::from_str(s).expect("oid")
    }

    /// Linear A→B repo (HEAD=B). Returns (dir, a_oid, b_oid).
    fn linear_repo() -> (tempfile::TempDir, String, String) {
        let dir = init_scratch();
        let p = dir.path();
        let a = commit(p, "a.txt", "a\n", "A");
        let b = commit(p, "b.txt", "b\n", "B");
        (dir, a, b)
    }

    /// Repo whose HEAD is a MERGE commit M with parents [A(main), B(feature)].
    /// Uses A's tree for every commit so the worktree stays clean. Returns
    /// (dir, a_oid, m_oid, head_branch_name).
    fn merge_repo() -> (tempfile::TempDir, String, String, String) {
        let dir = init_scratch();
        let p = dir.path();
        let a = commit(p, "a.txt", "a\n", "A");
        let repo = git2::Repository::open(p).expect("open");
        let head_branch = repo
            .head()
            .expect("head")
            .shorthand()
            .expect("shorthand")
            .to_string();
        let a_c = repo.find_commit(oid(&a)).expect("A");
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let tree = a_c.tree().expect("tree");
        let b = repo
            .commit(Some("refs/heads/feature"), &sig, &sig, "B", &tree, &[&a_c])
            .expect("feature commit");
        let b_c = repo.find_commit(b).expect("B");
        let m = repo
            .commit(
                Some(&format!("refs/heads/{head_branch}")),
                &sig,
                &sig,
                "Merge branch 'feature'",
                &tree,
                &[&a_c, &b_c],
            )
            .expect("merge commit");
        (dir, a, m.to_string(), head_branch)
    }

    /// Byte-snapshot of the repo state that a plan MUST NOT touch: HEAD oid, the
    /// raw index file, and a worktree file.
    fn snapshot(p: &Path) -> (Option<String>, Vec<u8>, Vec<u8>) {
        let repo = git2::Repository::open(p).expect("open");
        let head = repo.head().ok().and_then(|r| r.target()).map(|o| o.to_string());
        let index = std::fs::read(repo.path().join("index")).unwrap_or_default();
        let file = std::fs::read(p.join("a.txt")).unwrap_or_default();
        (head, index, file)
    }

    fn expect_proposed(o: PlanOutcome) -> ProposedOperation {
        match o {
            PlanOutcome::Proposed { operation } => *operation,
            other => panic!("expected Proposed, got {other:?}"),
        }
    }

    fn expect_unsupported(o: PlanOutcome) -> String {
        match o {
            PlanOutcome::Unsupported { reason, .. } => reason,
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // ------------------------------------------------- §11.1 plan_never_mutates

    /// §11.1 (NON-NEGOTIABLE): the resolve+preview path (the only repo-touching
    /// code after the read-only grounding + pure CLI text transform) mutates
    /// NOTHING for EVERY intent — including the four resolved intents, the six
    /// deferred ones, the escape hatch, and unparseable garbage. The full
    /// `plan_operation` spawn path is additionally proven in
    /// `tests/ai_operation_cli.rs` (process-isolated from the CLI env).
    #[test]
    fn plan_never_mutates() {
        let (dir, a, _m, _branch) = merge_repo();
        let p = dir.path();
        let short_a: String = a.chars().take(7).collect();
        let repo = git2::Repository::open(p).expect("open");

        let replies: Vec<String> = vec![
            r#"{"intent":"undoLastCommit","keepChanges":true}"#.to_string(),
            r#"{"intent":"undoLastCommit","keepChanges":false}"#.to_string(),
            r#"{"intent":"undoLastMerge"}"#.to_string(),
            format!(r#"{{"intent":"resetToCommit","commit":"{short_a}","keepChanges":true}}"#),
            format!(r#"{{"intent":"revertCommit","commit":"{short_a}"}}"#),
            r#"{"intent":"switchBranch","branch":"feature"}"#.to_string(),
            r#"{"intent":"createBranch","name":"x","atCommit":null}"#.to_string(),
            r#"{"intent":"deleteBranch","branch":"feature"}"#.to_string(),
            r#"{"intent":"stashChanges","message":null,"includeUntracked":true}"#.to_string(),
            r#"{"intent":"discardChanges","paths":["a.txt"]}"#.to_string(),
            r#"{"intent":"mergeBranch","branch":"feature"}"#.to_string(),
            r#"{"intent":"unsupported","reason":"nope"}"#.to_string(),
            "this is not JSON at all".to_string(),
            "git reset --hard HEAD~5".to_string(),
        ];

        let before = snapshot(p);
        for reply in &replies {
            // Ignore the outcome; the guarantee under test is "writes nothing".
            let _ = plan_from_reply(&repo, reply, Some(0.001)).expect("plan_from_reply");
            assert_eq!(
                snapshot(p),
                before,
                "plan resolution mutated the repo for reply: {reply}"
            );
        }
    }

    // --------------------------------------- §11.2 out_of_allowlist_is_unsupported

    /// §11.2 (NON-NEGOTIABLE): every off-allowlist model output — invalid JSON,
    /// an unknown tag, a raw shell string, an unresolvable ref, and
    /// undoLastMerge-when-HEAD-is-not-a-merge — yields `Ok(Unsupported)` (NOT a
    /// guessed op, NOT `Err`), and mutates nothing.
    #[test]
    fn out_of_allowlist_is_unsupported() {
        let (dir, _a, _b) = linear_repo();
        let p = dir.path();
        let repo = git2::Repository::open(p).expect("open");
        let before = snapshot(p);

        // (1) invalid JSON, (2) unknown tag, (3) raw shell string — all fail the
        // CLOSED parse and degrade to Unsupported.
        for reply in [
            "not json",
            r#"{"intent":"rmRf"}"#,
            "git reset --hard HEAD~5",
            r#"{"intent":"deleteEverything","force":true}"#,
        ] {
            let outcome = plan_from_reply(&repo, reply, None).expect("Ok(Unsupported)");
            expect_unsupported(outcome);
        }

        // (4) unresolvable ref (a P55a intent that passes the parse but fails L4).
        let bad_ref = resolve_intent(
            &repo,
            AiOpIntent::ResetToCommit {
                commit: "no-such-ref".to_string(),
                keep_changes: true,
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(bad_ref).contains("couldn't find a commit"));

        // (5) undoLastMerge when HEAD is NOT a merge.
        let not_merge = resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok");
        assert!(expect_unsupported(not_merge).contains("isn't a merge"));

        assert_eq!(snapshot(p), before, "rejecting an intent must mutate nothing");
    }

    // ------------------------------------------------------- §11.3 undoLastCommit

    /// §11.3: undoLastCommit targets HEAD's parent; Mixed (keep) vs Hard
    /// (discard) by `keepChanges`; dropped = [HEAD].
    #[test]
    fn undo_last_commit_targets_head_parent() {
        let (dir, a, b) = linear_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let short_b: String = b.chars().take(7).collect();

        // keepChanges=true → Mixed, Caution, no worktree warning.
        let op = expect_proposed(
            resolve_intent(&repo, AiOpIntent::UndoLastCommit { keep_changes: true }, None)
                .expect("Ok"),
        );
        match &op.op {
            SafeOp::Reset {
                target_oid, mode, ..
            } => {
                assert_eq!(target_oid, &a, "target = HEAD's parent (A)");
                assert_eq!(*mode, ResetMode::Mixed);
            }
            other => panic!("expected Reset, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Caution));
        assert!(op.preview.worktree_warning.is_none());
        assert_eq!(op.preview.dropped_commits.len(), 1, "dropped = [HEAD]");
        assert_eq!(op.preview.dropped_commits[0].short, short_b);

        // keepChanges=false → Hard, Destructive, worktree warning present.
        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::UndoLastCommit { keep_changes: false },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Reset { mode, .. } => assert_eq!(*mode, ResetMode::Hard),
            other => panic!("expected Reset, got {other:?}"),
        }
        assert!(matches!(op.preview.danger, DangerLevel::Destructive));
        assert!(op.preview.worktree_warning.is_some());
    }

    // -------------------------------------------------------- §11.4 undoLastMerge

    /// §11.4: undoLastMerge on a merge HEAD → Reset{first parent, Mixed},
    /// Destructive, with the upstream shared-history warning when an upstream
    /// exists; a non-merge HEAD → Unsupported.
    #[test]
    fn undo_last_merge_requires_merge_head() {
        let (dir, a, m, head_branch) = merge_repo();
        let p = dir.path();
        let repo = git2::Repository::open(p).expect("open");
        let short_m: String = m.chars().take(7).collect();

        let op =
            expect_proposed(resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok"));
        match &op.op {
            SafeOp::Reset {
                target_oid, mode, ..
            } => {
                assert_eq!(target_oid, &a, "target = merge's FIRST parent (A)");
                assert_eq!(*mode, ResetMode::Mixed);
            }
            other => panic!("expected Reset, got {other:?}"),
        }
        assert!(
            matches!(op.preview.danger, DangerLevel::Destructive),
            "undoLastMerge is always Destructive (OQ2)"
        );
        assert!(
            op.preview.dropped_commits.iter().any(|c| c.short == short_m),
            "the merge commit leaves the branch"
        );
        // No upstream yet → no shared-history warning.
        assert!(op.preview.worktree_warning.is_none());

        // Add an upstream → the shared-history warning appears.
        repo.remote("origin", "https://example.invalid/x.git").expect("remote");
        repo.reference(
            &format!("refs/remotes/origin/{head_branch}"),
            oid(&m),
            true,
            "seed upstream",
        )
        .expect("remote-tracking ref");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str(&format!("branch.{head_branch}.remote"), "origin")
                .expect("remote cfg");
            cfg.set_str(
                &format!("branch.{head_branch}.merge"),
                &format!("refs/heads/{head_branch}"),
            )
            .expect("merge cfg");
        }
        let op =
            expect_proposed(resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok"));
        let warn = op.preview.worktree_warning.expect("upstream warning present");
        assert!(warn.contains("rewrites history"), "got: {warn}");
        assert!(warn.contains(&format!("origin/{head_branch}")), "got: {warn}");

        // A non-merge HEAD → Unsupported.
        let (dir2, _a2, _b2) = linear_repo();
        let repo2 = git2::Repository::open(dir2.path()).expect("open");
        let reason =
            expect_unsupported(resolve_intent(&repo2, AiOpIntent::UndoLastMerge, None).expect("Ok"));
        assert!(reason.contains("isn't a merge"), "got: {reason}");
    }

    // ------------------------------------------------------ §11.5 resetToCommit

    /// §11.5: resetToCommit resolves a SHORT hash from the state to a full oid;
    /// a bad ref → Unsupported.
    #[test]
    fn reset_to_commit_resolves_short_hash() {
        let (dir, a, _b) = linear_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let short_a: String = a.chars().take(7).collect();

        let op = expect_proposed(
            resolve_intent(
                &repo,
                AiOpIntent::ResetToCommit {
                    commit: short_a.clone(),
                    keep_changes: true,
                },
                None,
            )
            .expect("Ok"),
        );
        match &op.op {
            SafeOp::Reset {
                target_oid,
                target_short,
                mode,
            } => {
                assert_eq!(target_oid, &a, "short hash resolved to A's FULL oid");
                assert_eq!(target_short, &short_a);
                assert_eq!(*mode, ResetMode::Mixed);
            }
            other => panic!("expected Reset, got {other:?}"),
        }

        let bad = resolve_intent(
            &repo,
            AiOpIntent::ResetToCommit {
                commit: "deadbeefdeadbeef".to_string(),
                keep_changes: false,
            },
            None,
        )
        .expect("Ok");
        assert!(expect_unsupported(bad).contains("couldn't find a commit"));
    }

    // ---------------------------------------------------------- §11.9 deserialize

    /// §11.9: `AiOpIntent` deserializes from the EXACT JSON the TS union / the
    /// system prompt describe, incl. `keepChanges` and `atCommit:null`; an
    /// unknown tag is an Err (⇒ fail-closed at the call site).
    #[test]
    fn ai_op_intent_deserializes_each_variant() {
        let p = |s: &str| serde_json::from_str::<AiOpIntent>(s);

        match p(r#"{"intent":"undoLastCommit","keepChanges":true}"#).expect("undoLastCommit") {
            AiOpIntent::UndoLastCommit { keep_changes } => assert!(keep_changes),
            other => panic!("got {other:?}"),
        }
        // keepChanges omitted → serde default false.
        match p(r#"{"intent":"undoLastCommit"}"#).expect("undoLastCommit default") {
            AiOpIntent::UndoLastCommit { keep_changes } => assert!(!keep_changes),
            other => panic!("got {other:?}"),
        }
        assert!(matches!(
            p(r#"{"intent":"undoLastMerge"}"#).expect("undoLastMerge"),
            AiOpIntent::UndoLastMerge
        ));
        match p(r#"{"intent":"resetToCommit","commit":"a1b2c3d","keepChanges":false}"#)
            .expect("resetToCommit")
        {
            AiOpIntent::ResetToCommit {
                commit,
                keep_changes,
            } => {
                assert_eq!(commit, "a1b2c3d");
                assert!(!keep_changes);
            }
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"revertCommit","commit":"a1b2c3d"}"#).expect("revertCommit") {
            AiOpIntent::RevertCommit { commit } => assert_eq!(commit, "a1b2c3d"),
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"switchBranch","branch":"main"}"#).expect("switchBranch") {
            AiOpIntent::SwitchBranch { branch } => assert_eq!(branch, "main"),
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"createBranch","name":"feat/x","atCommit":null}"#)
            .expect("createBranch")
        {
            AiOpIntent::CreateBranch { name, at_commit } => {
                assert_eq!(name, "feat/x");
                assert_eq!(at_commit, None);
            }
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"deleteBranch","branch":"old"}"#).expect("deleteBranch") {
            AiOpIntent::DeleteBranch { branch } => assert_eq!(branch, "old"),
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"stashChanges","message":null,"includeUntracked":true}"#)
            .expect("stashChanges")
        {
            AiOpIntent::StashChanges {
                message,
                include_untracked,
            } => {
                assert_eq!(message, None);
                assert!(include_untracked);
            }
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"discardChanges","paths":["a.txt","b.txt"]}"#)
            .expect("discardChanges")
        {
            AiOpIntent::DiscardChanges { paths } => assert_eq!(paths, vec!["a.txt", "b.txt"]),
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"mergeBranch","branch":"topic"}"#).expect("mergeBranch") {
            AiOpIntent::MergeBranch { branch } => assert_eq!(branch, "topic"),
            other => panic!("got {other:?}"),
        }
        match p(r#"{"intent":"unsupported","reason":"nope"}"#).expect("unsupported") {
            AiOpIntent::Unsupported { reason } => assert_eq!(reason, "nope"),
            other => panic!("got {other:?}"),
        }

        // Unknown tag ⇒ Err (the fail-closed call site maps it to Unsupported).
        assert!(p(r#"{"intent":"rmRf"}"#).is_err(), "unknown tag must NOT parse");
    }

    // -------------------------------------------------------- §11.10 wire shape

    /// §11.10: `PlanOutcome` / `ProposedOperation` / `SafeOp` / `OperationPreview`
    /// serialize with the EXACT camelCase tags + keys the TS union expects.
    #[test]
    fn plan_outcome_and_safe_op_wire_shape_is_camel_case() {
        let outcome = PlanOutcome::Proposed {
            operation: Box::new(ProposedOperation {
                op: SafeOp::Reset {
                    target_oid: "a".repeat(40),
                    target_short: "aaaaaaa".to_string(),
                    mode: ResetMode::Mixed,
                },
                preview: OperationPreview {
                    title: "Undo last merge".to_string(),
                    summary: "Move `main` back to c3d4e5f.".to_string(),
                    danger: DangerLevel::Destructive,
                    ref_changes: vec![RefChange {
                        name: "main".to_string(),
                        from_short: "c3d4e5f".to_string(),
                        to_short: "aaaaaaa".to_string(),
                    }],
                    dropped_commits: vec![CommitRef {
                        short: "c3d4e5f".to_string(),
                        summary: "Merge branch 'feature/x'".to_string(),
                    }],
                    added_commits: 0,
                    worktree_warning: None,
                    confirm_label: "Undo merge".to_string(),
                },
                rationale: "why".to_string(),
                cost_usd: Some(0.01),
            }),
        };
        let v = serde_json::to_value(&outcome).expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "kind": "proposed",
                "operation": {
                    "op": {
                        "kind": "reset",
                        "targetOid": "a".repeat(40),
                        "targetShort": "aaaaaaa",
                        "mode": "mixed"
                    },
                    "preview": {
                        "title": "Undo last merge",
                        "summary": "Move `main` back to c3d4e5f.",
                        "danger": "destructive",
                        "refChanges": [
                            { "name": "main", "fromShort": "c3d4e5f", "toShort": "aaaaaaa" }
                        ],
                        "droppedCommits": [
                            { "short": "c3d4e5f", "summary": "Merge branch 'feature/x'" }
                        ],
                        "addedCommits": 0,
                        "worktreeWarning": null,
                        "confirmLabel": "Undo merge"
                    },
                    "rationale": "why",
                    "costUsd": 0.01
                }
            })
        );

        let unsupported = PlanOutcome::Unsupported {
            reason: "no".to_string(),
            cost_usd: None,
        };
        assert_eq!(
            serde_json::to_value(&unsupported).expect("json"),
            serde_json::json!({ "kind": "unsupported", "reason": "no", "costUsd": null })
        );

        // A non-reset SafeOp variant round-trips its camelCase tag + fields.
        let revert = serde_json::to_value(SafeOp::Revert {
            oid: "b".repeat(40),
            short: "bbbbbbb".to_string(),
        })
        .expect("json");
        assert_eq!(
            revert,
            serde_json::json!({ "kind": "revert", "oid": "b".repeat(40), "short": "bbbbbbb" })
        );
    }

    // ------------------------------------------------------- §11.11 single-line

    /// §11.11: the prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint — a newline would make `claude.cmd` reject the argument).
    #[test]
    fn prompts_are_single_line() {
        for s in [PLAN_SYSTEM_PROMPT, PLAN_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }
}
