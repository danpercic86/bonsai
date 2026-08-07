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

/// `revparse_single(spec)` → commit, or `None` on any miss (the model referenced
/// something unresolvable ⇒ a precondition miss, NOT a git error; §2 L4).
pub(crate) fn revparse_commit<'r>(
    repo: &'r git2::Repository,
    spec: &str,
) -> Option<git2::Commit<'r>> {
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
mod tests {
    //! Plan-spine tests: the two NON-NEGOTIABLE safety guarantees
    //! (`plan_never_mutates`, `out_of_allowlist_is_unsupported`), the wire-shape
    //! and deserialize locks, and the single-line-prompt guard. The per-intent
    //! resolution/preview tests live next to the code they exercise in the
    //! `ai_operation_resolve` module.

    use super::*;
    use crate::git::ai_operation_resolve::resolve_intent;
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

    fn expect_unsupported(o: PlanOutcome) -> String {
        match o {
            PlanOutcome::Unsupported { reason, .. } => reason,
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // ----------------------------------------------- §11.1 plan_never_mutates

    /// §11.1 (NON-NEGOTIABLE): the resolve+preview path (the only repo-touching
    /// code after the read-only grounding + pure CLI text transform) mutates
    /// NOTHING for EVERY intent — the four reset/revert intents, the six P55b
    /// ones (switch/create/delete/stash/discard/merge), the escape hatch, and
    /// unparseable garbage. The full `plan_operation` spawn path is additionally
    /// proven in `tests/ai_operation_cli.rs` (process-isolated from the CLI env).
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

        // The six P55b SafeOp variants round-trip their camelCase tags + fields
        // (the wire contract the frontend mock + dispatch rely on).
        assert_eq!(
            serde_json::to_value(SafeOp::Revert {
                oid: "b".repeat(40),
                short: "bbbbbbb".to_string(),
            })
            .expect("json"),
            serde_json::json!({ "kind": "revert", "oid": "b".repeat(40), "short": "bbbbbbb" })
        );
        assert_eq!(
            serde_json::to_value(SafeOp::SwitchBranch {
                name: "origin/x".to_string(),
                remote: true,
            })
            .expect("json"),
            serde_json::json!({ "kind": "switchBranch", "name": "origin/x", "remote": true })
        );
        assert_eq!(
            serde_json::to_value(SafeOp::CreateBranch {
                name: "feat/x".to_string(),
                at_oid: None,
            })
            .expect("json"),
            serde_json::json!({ "kind": "createBranch", "name": "feat/x", "atOid": null })
        );
        assert_eq!(
            serde_json::to_value(SafeOp::DeleteBranch {
                name: "old".to_string(),
            })
            .expect("json"),
            serde_json::json!({ "kind": "deleteBranch", "name": "old" })
        );
        assert_eq!(
            serde_json::to_value(SafeOp::Stash {
                message: None,
                include_untracked: true,
            })
            .expect("json"),
            serde_json::json!({ "kind": "stash", "message": null, "includeUntracked": true })
        );
        assert_eq!(
            serde_json::to_value(SafeOp::Discard {
                paths: vec!["a.txt".to_string()],
            })
            .expect("json"),
            serde_json::json!({ "kind": "discard", "paths": ["a.txt"] })
        );
        assert_eq!(
            serde_json::to_value(SafeOp::Merge {
                name: "topic".to_string(),
            })
            .expect("json"),
            serde_json::json!({ "kind": "merge", "name": "topic" })
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
