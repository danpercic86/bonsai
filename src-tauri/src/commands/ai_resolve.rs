//! `ai` commands (extracted from `commands/ai.rs`, unchanged). Re-exported
//! from the parent so every `commands::ai::*` path resolves as before.

use crate::commands::shared::*;

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

