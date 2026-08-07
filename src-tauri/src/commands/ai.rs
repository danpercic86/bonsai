//! `ai` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Cheap Claude Code CLI health probe (P13 §6). No repo, no state; NEVER
/// rejects for CLI state — a missing/broken CLI yields `{ installed:false, .. }`.
/// Only a task-join error can `Err`.
#[tauri::command]
pub async fn check_ai_availability() -> Result<AiAvailability, AppError> {
    tauri::async_runtime::spawn_blocking(ai::check_availability)
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Proposes an AI resolution for one conflicted path (P13 §6). Loads settings
/// and REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (§9.6 —
/// the authoritative backend gate; the frontend also gates for UX). WRITES
/// NOTHING — applying is the separate `resolve_conflict_text` command. Errors:
/// `aiUnavailable` | `aiFailed` | `git` | `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_resolve_conflict(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<AiResolveProposal, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner
    // stays runtime-free and unit-testable (mirrors `settings.rs`'s
    // path-parameterized design), then delegate.
    let file = settings::settings_file(&app)?;
    ai_resolve_conflict_inner(state.inner(), &file, &repo_id, path).await
}

/// Runtime-free core of `ai_resolve_conflict` (unit-testable without a Tauri
/// app). The consent gate is enforced HERE, BEFORE `repo_path`, per §9.6.
pub(crate) async fn ai_resolve_conflict_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    path: String,
) -> Result<AiResolveProposal, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_resolve::ai_resolve_conflict(&workdir, &path, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Generates a commit message from the staged diff (P15a §5). Loads settings and
/// REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (the
/// authoritative backend gate; the frontend also gates for UX). WRITES NOTHING —
/// the user edits the returned text in the commit box and commits separately.
/// Errors: `aiUnavailable` | `aiFailed` | `nothingToCommit` | `git` | `noRepo`.
#[tauri::command]
pub async fn generate_commit_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<CommitMessageProposal, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_resolve_conflict`), then delegate.
    let file = settings::settings_file(&app)?;
    generate_commit_message_inner(state.inner(), &file, &repo_id).await
}

/// Runtime-free core of `generate_commit_message` (unit-testable without a Tauri
/// app). The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn generate_commit_message_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
) -> Result<CommitMessageProposal, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_commit::generate_commit_message(&workdir, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Explains or reviews a diff target (P15b §5). Loads settings and REFUSES with
/// `AiUnavailable` unless `ai_enabled && ai_consented` (the authoritative backend
/// gate; the frontend also gates for UX). Read-only prose out — WRITES NOTHING.
/// Errors: `aiUnavailable` | `aiFailed` | `nothingToCommit` | `git` | `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_analyze_diff(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
) -> Result<AiAnalysis, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `generate_commit_message`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_analyze_diff_inner(state.inner(), &file, &repo_id, target, mode).await
}

/// Runtime-free core of `ai_analyze_diff` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn ai_analyze_diff_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
) -> Result<AiAnalysis, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_explain::analyze_diff(&workdir, target, mode, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Summarizes the commits/diff unique to `target` vs `base` (P15c §5). Loads
/// settings and REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented`
/// (the authoritative backend gate; the frontend also gates for UX). Read-only
/// prose out — WRITES NOTHING. Errors: `aiUnavailable` | `aiFailed` | `git` |
/// `noRepo`.
#[tauri::command]
pub async fn ai_summarize_range(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base: String,
    target: String,
) -> Result<AiSummary, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_summarize_range_inner(state.inner(), &file, &repo_id, base, target).await
}

/// Runtime-free core of `ai_summarize_range` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn ai_summarize_range_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    base: String,
    target: String,
) -> Result<AiSummary, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_summary::summarize_range(&workdir, &base, &target, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// AI "what changed" digest over a selectable range (P28 §5). Loads settings
/// and REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (the
/// authoritative backend gate; the frontend also gates for UX). Read-only prose
/// out — WRITES NOTHING. Errors: `aiUnavailable` | `aiFailed` | `git` |
/// `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_digest(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    range: AiDigestRange,
) -> Result<AiAnalysis, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_digest_inner(state.inner(), &file, &repo_id, range).await
}

/// Runtime-free core of `ai_digest` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn ai_digest_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    range: AiDigestRange,
) -> Result<AiAnalysis, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_explain::digest_changes(&workdir, range, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// AI "why does this line exist" (P53a §4). Blames `line_no` (as of `at_oid`,
/// `None` => HEAD) to find the introducing commit, then explains that commit's
/// change to the file focused on the line. Loads settings and REFUSES with
/// `AiUnavailable` unless `ai_enabled && ai_consented` (the authoritative
/// backend gate; the frontend also gates for UX). Read-only prose out — WRITES
/// NOTHING; does NOT emit `repo-changed`. Errors: `aiUnavailable` | `aiFailed`
/// | `git` | `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_explain_line(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    line_no: u32,
    at_oid: Option<String>,
) -> Result<AiAnalysis, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_explain_line_inner(state.inner(), &file, &repo_id, path, line_no, at_oid).await
}

/// Runtime-free core of `ai_explain_line` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn ai_explain_line_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    path: String,
    line_no: u32,
    at_oid: Option<String>,
) -> Result<AiAnalysis, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_line::explain_line(&workdir, &path, line_no, at_oid.as_deref(), RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// AI branch-name suggestions from `source` (P53c §4). Blames nothing — builds a
/// grounding payload (working-tree change set or a commit range) and asks the CLI
/// for ranked, kebab-case candidates the user picks/edits in the branch-create
/// dialog. Loads settings and REFUSES with `AiUnavailable` unless `ai_enabled &&
/// ai_consented` (the authoritative backend gate; the frontend also gates for
/// UX). Read-only — WRITES NOTHING; does NOT emit `repo-changed`. Errors:
/// `aiUnavailable` | `aiFailed` (empty grounding / no usable name) | `git` (bad
/// ref) | `noRepo`.
#[tauri::command]
pub async fn ai_suggest_branch_name(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    source: BranchNameSource,
) -> Result<BranchNameProposal, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_suggest_branch_name_inner(state.inner(), &file, &repo_id, source).await
}

/// Runtime-free core of `ai_suggest_branch_name` (unit-testable without a Tauri
/// app). The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn ai_suggest_branch_name_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    source: BranchNameSource,
) -> Result<BranchNameProposal, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_branch_name::suggest_branch_name(&workdir, &source, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Maps a natural-language `request` to ONE allowlisted, previewable git
/// operation (P55a §8). Loads settings and REFUSES with `AiUnavailable` unless
/// `ai_enabled && ai_consented` (the authoritative backend gate; the frontend
/// also gates for UX). READ-ONLY — WRITES NOTHING; does NOT emit `repo-changed`.
/// A model reply it can't map to a safe op resolves to `unsupported` (a normal
/// Ok outcome, not an error). The mutation runs later via the EXISTING typed
/// command on the user's explicit confirm (P55c). Errors: `aiUnavailable` |
/// `aiFailed` | `git` | `noRepo`.
#[tauri::command]
pub async fn ai_plan_operation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    request: String,
) -> Result<PlanOutcome, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_plan_operation_inner(state.inner(), &file, &repo_id, request).await
}

/// Runtime-free core of `ai_plan_operation` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`. READ-ONLY ⇒ NO
/// `repo-changed` emit.
pub(crate) async fn ai_plan_operation_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    request: String,
) -> Result<PlanOutcome, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_operation::plan_operation(&workdir, &request, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Proposes grouping the working-tree changes into logical commits (P54a §5).
/// Loads settings and REFUSES with `AiUnavailable` unless `ai_enabled &&
/// ai_consented` (the authoritative backend gate; the frontend also gates for
/// UX). Read-only — WRITES NOTHING; does NOT emit `repo-changed`. The result is
/// ALWAYS an apply-able partition (unknown paths dropped, overlaps first-wins,
/// uncovered files in `unassigned`); unparseable model output is NOT an error.
/// Errors: `aiUnavailable` | `aiFailed` (CLI fail/empty) | `nothingToCommit`
/// (clean tree) | `git` | `noRepo`.
#[tauri::command]
pub async fn ai_compose_commits(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    guidance: Option<String>,
) -> Result<ComposeProposal, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `generate_commit_message`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_compose_commits_inner(state.inner(), &file, &repo_id, guidance).await
}

/// Runtime-free core of `ai_compose_commits` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
pub(crate) async fn ai_compose_commits_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    guidance: Option<String>,
) -> Result<ComposeProposal, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_compose::compose_commits(&workdir, guidance.as_deref(), RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Optional AI helper (P24e §6.8): translate the `source_asset_id` instruction
/// file into `target_agent`'s flavor via the local `claude` CLI. Enforces the
/// consent gate FIRST (before `repo_path`), exactly like `generate_commit_message`.
/// WRITES NOTHING — returns proposed text the user reviews and saves into a
/// profile target. Errors: `aiUnavailable` | `aiFailed` | `other` | `io` | `noRepo`.
#[tauri::command]
pub async fn ai_generate_asset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    source_asset_id: String,
    target_agent: String,
    guidance: Option<String>,
) -> Result<AiGeneratedAsset, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free (mirrors `generate_commit_message`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_generate_asset_inner(state.inner(), &file, &repo_id, source_asset_id, target_agent, guidance)
        .await
}

/// Runtime-free core of `ai_generate_asset`. The consent gate is enforced HERE,
/// BEFORE `repo_path` (§6.8).
pub(crate) async fn ai_generate_asset_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    source_asset_id: String,
    target_agent: String,
    guidance: Option<String>,
) -> Result<AiGeneratedAsset, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled — enable them in Settings".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    // Resolve the source asset id to its mapped file, then read its content. A
    // missing/empty source is an error (nothing to translate) → `Other`.
    let descriptor = assets::descriptor(&source_asset_id)
        .ok_or_else(|| AppError::Other(format!("unknown asset id: '{source_asset_id}'")))?;
    let src_path = descriptor.path.to_string();
    let source_content = {
        let workdir = workdir.clone();
        tauri::async_runtime::spawn_blocking(move || assets::read_asset(&workdir, &src_path))
            .await
            .map_err(|e| AppError::Other(format!("task join error: {e}")))??
    };
    let content = match source_content.content {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Err(AppError::Other(format!(
                "source asset '{source_asset_id}' has no content to translate"
            )))
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        assets::generate_asset(&workdir, &content, &target_agent, guidance.as_deref(), RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
