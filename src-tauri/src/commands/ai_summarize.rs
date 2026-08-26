//! `ai` commands (extracted from `commands/ai.rs`, unchanged). Re-exported
//! from the parent so every `commands::ai::*` path resolves as before.

use crate::commands::shared::*;

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

