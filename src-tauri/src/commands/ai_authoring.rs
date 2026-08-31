//! `ai` commands (extracted from `commands/ai.rs`, unchanged). Re-exported
//! from the parent so every `commands::ai::*` path resolves as before.

use crate::commands::shared::*;

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

/// Generates grouped Markdown release notes for a tag/ref range or "since the
/// last tag" (P56a §4). Loads settings and REFUSES with `AiUnavailable` unless
/// `ai_enabled && ai_consented` (the authoritative backend gate; the frontend
/// also gates for UX). Read-only prose out — WRITES NOTHING; does NOT emit
/// `repo-changed`. Errors: `aiUnavailable` | `aiFailed` (empty range / no earlier
/// tag / CLI failure) | `git` (bad ref) | `noRepo`.
#[tauri::command]
pub async fn ai_changelog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    range: ChangelogRange,
) -> Result<AiChangelog, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_digest`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_changelog_inner(state.inner(), &file, &repo_id, range).await
}

/// Runtime-free core of `ai_changelog` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`. READ-ONLY ⇒ NO
/// `repo-changed` emit.
pub(crate) async fn ai_changelog_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    range: ChangelogRange,
) -> Result<AiChangelog, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_changelog::generate_changelog(&workdir, range, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Generates a pull-request title + Markdown body grounded in the commits unique
/// to `head` vs `base` + the net diffstat (P64 Part B §4c). Loads settings and
/// REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (the
/// authoritative backend gate; the frontend also gates for UX). Read-only —
/// WRITES NOTHING; never posts to a forge; does NOT emit `repo-changed`. The
/// proposal fills the create-PR form; the user reviews/edits and still clicks
/// Create. Errors: `aiUnavailable` | `aiFailed` (empty range / no usable title /
/// CLI failure) | `git` (bad ref) | `noRepo`.
#[tauri::command]
pub async fn ai_generate_pr_description(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base: String,
    head: String,
) -> Result<PrDescription, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_summarize_range`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_generate_pr_description_inner(state.inner(), &file, &repo_id, base, head).await
}

/// Runtime-free core of `ai_generate_pr_description` (unit-testable without a
/// Tauri app). The consent gate is enforced HERE, BEFORE `repo_path`. READ-ONLY
/// ⇒ NO `repo-changed` emit.
pub(crate) async fn ai_generate_pr_description_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    base: String,
    head: String,
) -> Result<PrDescription, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_pr_description::generate_pr_description(&workdir, &base, &head, RunOpts::default())
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

